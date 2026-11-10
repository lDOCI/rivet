//! Network side: owns the Moonraker client and printer state, pushes
//! throttled snapshots to the UI thread and executes UI commands.

use std::time::Duration;

use model::{Connection, Klippy, PrinterState, Snapshot};
use moonraker::{Client, Event};
use serde_json::{json, Value};
use tokio::{sync::mpsc, time::MissedTickBehavior};

use crate::AppWindow;

/// UI refresh cap. Moonraker sends ~4 updates/s while printing.
const UI_TICK: Duration = Duration::from_millis(100);
/// Setup calls run inline in the event loop, so a hung Moonraker must not
/// stall UI updates for the default 30 s.
const SETUP_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub enum Command {
    Preheat { extruder: f32, bed: f32 },
    Cooldown,
    HomeAll,
    Pause,
    Resume,
    Cancel,
    EmergencyStop,
    FirmwareRestart,
}

pub async fn run(
    url: String,
    ui: slint::Weak<AppWindow>,
    mut cmds: mpsc::UnboundedReceiver<Command>,
) {
    let (client, mut events) = Client::spawn(url);
    let mut state = PrinterState::default();
    let mut last_pushed: Option<Snapshot> = None;

    let mut tick = tokio::time::interval(UI_TICK);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            ev = events.recv() => {
                let Some(ev) = ev else { break };
                handle_event(ev, &client, &mut state).await;
            }
            cmd = cmds.recv() => {
                let Some(cmd) = cmd else { break };
                let client = client.clone();
                let ui = ui.clone();
                tokio::spawn(async move {
                    if let Err(err) = execute(&client, &cmd).await {
                        tracing::warn!(?cmd, %err, "command failed");
                        let (kind, detail) = toast_for(&err);
                        let _ = ui.upgrade_in_event_loop(move |ui| {
                            ui.set_toast_detail(detail.into());
                            ui.set_toast_kind(kind.into());
                        });
                    }
                });
            }
            _ = tick.tick() => {
                let snap = state.snapshot();
                if last_pushed.as_ref() != Some(&snap) {
                    push(&ui, snap.clone());
                    last_pushed = Some(snap);
                }
            }
        }
    }
}

async fn handle_event(ev: Event, client: &Client, state: &mut PrinterState) {
    match ev {
        Event::Connected => {
            state.connection = Connection::Connected;
            if let Err(err) = client
                .identify("Rivet", env!("CARGO_PKG_VERSION"), SETUP_TIMEOUT)
                .await
            {
                tracing::warn!(%err, "identify failed");
            }
            refresh_klippy(client, state).await;
        }
        Event::Disconnected => {
            state.connection = Connection::Disconnected;
            state.klippy = Klippy::Unknown;
            state.clear_status();
        }
        Event::Notification { method, params } => match method.as_str() {
            "notify_status_update" => {
                tracing::trace!(%params, "status update");
                if let Value::Array(mut args) = params {
                    if !args.is_empty() {
                        state.apply_status(args.swap_remove(0));
                    }
                }
            }
            "notify_klippy_ready" => {
                state.klippy = Klippy::Ready;
                subscribe(client, state).await;
            }
            "notify_klippy_shutdown" => {
                // Ask for the reason; webhooks.state_message carries it.
                refresh_klippy(client, state).await;
            }
            "notify_klippy_disconnected" => {
                state.klippy = Klippy::Disconnected;
                state.clear_status();
            }
            _ => {}
        },
    }
}

async fn refresh_klippy(client: &Client, state: &mut PrinterState) {
    match client
        .call_timeout("server.info", Value::Null, SETUP_TIMEOUT)
        .await
    {
        Ok(info) => {
            state.klippy = Klippy::parse(info["klippy_state"].as_str().unwrap_or_default());
            // When Klippy isn't ready, Moonraker sends notify_klippy_ready later.
            if state.klippy == Klippy::Ready {
                subscribe(client, state).await;
            } else {
                // Klipper stops reporting objects outside "ready", so the
                // last values are stale: drop them. The reason still comes
                // from printer.info.
                state.clear_status();
                if let Ok(mut info) = client
                    .call_timeout("printer.info", Value::Null, SETUP_TIMEOUT)
                    .await
                {
                    state.apply_status(json!({
                        "webhooks": { "state_message": info["state_message"].take() }
                    }));
                }
            }
        }
        Err(err) => tracing::warn!(%err, "server.info failed"),
    }
}

async fn subscribe(client: &Client, state: &mut PrinterState) {
    match client
        .call_timeout(
            "printer.objects.subscribe",
            model::subscription(),
            SETUP_TIMEOUT,
        )
        .await
    {
        Ok(mut result) => state.replace_status(result["status"].take()),
        Err(err) => tracing::warn!(%err, "subscribe failed"),
    }
}

async fn execute(client: &Client, cmd: &Command) -> Result<(), moonraker::Error> {
    match cmd {
        Command::Preheat { extruder, bed } => {
            client
                .gcode(&format!("M104 S{extruder:.0}\nM140 S{bed:.0}"))
                .await
        }
        Command::Cooldown => client.gcode("TURN_OFF_HEATERS").await,
        Command::HomeAll => client.gcode("G28").await,
        Command::Pause => client
            .call("printer.print.pause", Value::Null)
            .await
            .map(drop),
        Command::Resume => client
            .call("printer.print.resume", Value::Null)
            .await
            .map(drop),
        Command::Cancel => client
            .call("printer.print.cancel", Value::Null)
            .await
            .map(drop),
        Command::EmergencyStop => client
            .call("printer.emergency_stop", Value::Null)
            .await
            .map(drop),
        Command::FirmwareRestart => client
            .call("printer.firmware_restart", Value::Null)
            .await
            .map(drop),
    }
}

/// What the user sees for a failed command: a kind the UI words itself (so the
/// text is translated) and, for errors from Klipper, the message it gave.
fn toast_for(err: &moonraker::Error) -> (&'static str, String) {
    match err {
        moonraker::Error::NotConnected | moonraker::Error::Closed => {
            ("not-connected", String::new())
        }
        moonraker::Error::Timeout => ("timeout", String::new()),
        moonraker::Error::Rpc { message, .. } => ("rpc", message.clone()),
    }
}

fn push(ui: &slint::Weak<AppWindow>, s: Snapshot) {
    let _ = ui.upgrade_in_event_loop(move |ui| {
        let link = match (s.connection, s.klippy) {
            (Connection::Connected, Klippy::Ready) => "ready",
            (Connection::Connected, Klippy::Startup) => "startup",
            (Connection::Connected, Klippy::Shutdown | Klippy::Error) => "error",
            (Connection::Connected, _) => "klippy-offline",
            _ => "offline",
        };
        ui.set_link_state(link.into());
        ui.set_klippy_message(s.klippy_message.as_str().into());
        ui.set_extruder_temp(s.extruder.temperature as f32);
        ui.set_extruder_target(s.extruder.target as f32);
        ui.set_bed_temp(s.bed.temperature as f32);
        ui.set_bed_target(s.bed.target as f32);
        ui.set_print_state(s.print_state.as_str().into());
        ui.set_filename(s.filename.as_str().into());
        ui.set_progress(s.progress as f32);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_commands_map_to_toasts() {
        assert_eq!(
            toast_for(&moonraker::Error::NotConnected).0,
            "not-connected"
        );
        assert_eq!(toast_for(&moonraker::Error::Closed).0, "not-connected");
        assert_eq!(toast_for(&moonraker::Error::Timeout).0, "timeout");
        let rpc = moonraker::Error::Rpc {
            code: 400,
            message: "Must home axis first".into(),
        };
        assert_eq!(toast_for(&rpc), ("rpc", "Must home axis first".to_owned()));
    }
}
