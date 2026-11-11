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
    Preheat {
        extruder: f32,
        bed: f32,
    },
    Cooldown,
    HomeAll,
    Pause,
    Resume,
    Cancel,
    /// Speed / flow factor in percent (M220 / M221).
    SetSpeed(f32),
    SetFlow(f32),
    /// Move the Z offset by this many mm, live (`SET_GCODE_OFFSET`).
    AdjustZ(f32),
    SetPressureAdvance(f32),
    /// Light by Klipper name; `white` picks the channel set.
    SetLight {
        name: String,
        on: bool,
        white: bool,
    },
    /// Start a file again, by its path in `gcodes`.
    StartPrint(String),
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

    // A failed command leaves our optimistic value wrong: ask the printer.
    let (resync_tx, mut resync_rx) = mpsc::unbounded_channel::<()>();

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
                predict(&mut state, &cmd);
                let client = client.clone();
                let ui = ui.clone();
                let resync = resync_tx.clone();
                tokio::spawn(async move {
                    if let Err(err) = execute(&client, &cmd).await {
                        tracing::warn!(?cmd, %err, "command failed");
                        let _ = resync.send(());
                        let (kind, detail) = toast_for(&err);
                        let _ = ui.upgrade_in_event_loop(move |ui| {
                            ui.set_toast_detail(detail.into());
                            ui.set_toast_kind(kind.into());
                        });
                    }
                });
            }
            Some(()) = resync_rx.recv() => {
                resync_status(&client, &mut state).await;
            }
            _ = tick.tick() => {
                let snap = state.snapshot();
                if last_pushed.as_ref() != Some(&snap) {
                    // A finished job lands in history; so does the first look
                    // after the printer becomes ready.
                    let ready = snap.klippy == Klippy::Ready;
                    let changed = last_pushed.as_ref().is_none_or(|p| {
                        p.print_state != snap.print_state || p.klippy != snap.klippy
                    });
                    push(&ui, snap.clone());
                    last_pushed = Some(snap);
                    if ready && changed {
                        refresh_history(&client, &ui).await;
                    }
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

/// Newest job in history, for «print again» on Home. A printer without
/// history (component off) simply has none.
async fn refresh_history(client: &Client, ui: &slint::Weak<AppWindow>) {
    let params = json!({ "limit": 1, "order": "desc" });
    let last = match client
        .call_timeout("server.history.list", params, SETUP_TIMEOUT)
        .await
    {
        Ok(result) => model::parse_last_print(&result),
        Err(err) => {
            tracing::debug!(%err, "history unavailable");
            None
        }
    };
    let _ = ui.upgrade_in_event_loop(move |ui| match last {
        Some(l) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0.0, |d| d.as_secs_f64());
            ui.set_last_name(l.filename.as_str().into());
            ui.set_last_minutes((l.duration_secs / 60) as i32);
            ui.set_last_days_ago(if l.ended > 0.0 {
                ((now - l.ended).max(0.0) / 86_400.0) as i32
            } else {
                -1
            });
            ui.set_last_completed(l.completed);
        }
        None => ui.set_last_name("".into()),
    });
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
    // What this printer has decides what we subscribe to (lights, for now).
    let available: Vec<String> = match client
        .call_timeout("printer.objects.list", Value::Null, SETUP_TIMEOUT)
        .await
    {
        Ok(list) => list["objects"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        Err(err) => {
            tracing::warn!(%err, "objects.list failed");
            Vec::new()
        }
    };
    state.leds = available
        .iter()
        .filter(|n| n.starts_with("led "))
        .cloned()
        .collect();
    match client
        .call_timeout(
            "printer.objects.subscribe",
            model::subscription_for(&available),
            SETUP_TIMEOUT,
        )
        .await
    {
        Ok(mut result) => state.replace_status(result["status"].take()),
        Err(err) => tracing::warn!(%err, "subscribe failed"),
    }
}

/// Apply a tuning command to our copy of the state right away, so the screen
/// answers the tap instead of waiting for Klipper's next status update.
fn predict(state: &mut PrinterState, cmd: &Command) {
    match cmd {
        Command::SetSpeed(p) => {
            state.apply_status(json!({ "gcode_move": { "speed_factor": f64::from(*p) / 100.0 } }));
        }
        Command::SetFlow(p) => {
            state
                .apply_status(json!({ "gcode_move": { "extrude_factor": f64::from(*p) / 100.0 } }));
        }
        Command::SetPressureAdvance(v) => {
            state.apply_status(json!({ "extruder": { "pressure_advance": f64::from(*v) } }));
        }
        Command::AdjustZ(d) => state.adjust_z_offset(f64::from(*d)),
        Command::SetLight { name, on, white } => {
            let level = f64::from(u8::from(*on));
            let channels = if *white {
                json!([0.0, 0.0, 0.0, level])
            } else {
                json!([level, level, level])
            };
            state.apply_status(json!({ format!("led {name}"): { "color_data": [channels] } }));
        }
        _ => {}
    }
}

/// Read the current values back after a command failed.
async fn resync_status(client: &Client, state: &mut PrinterState) {
    let objects: Vec<String> = state.leds.clone();
    let query = model::subscription_for(&objects);
    if let Ok(mut result) = client
        .call_timeout("printer.objects.query", query, SETUP_TIMEOUT)
        .await
    {
        state.apply_status(result["status"].take());
    }
}

/// G-code for the commands that are plain G-code.
fn gcode_for(cmd: &Command) -> Option<String> {
    Some(match cmd {
        Command::SetSpeed(p) => format!("M220 S{p:.0}"),
        Command::SetFlow(p) => format!("M221 S{p:.0}"),
        Command::AdjustZ(d) => format!("SET_GCODE_OFFSET Z_ADJUST={d:.3} MOVE=1"),
        Command::SetPressureAdvance(v) => format!("SET_PRESSURE_ADVANCE ADVANCE={v:.3}"),
        Command::SetLight { name, on, white } => {
            let v = u8::from(*on);
            if *white {
                format!("SET_LED LED={name} WHITE={v} TRANSMIT=1")
            } else {
                format!("SET_LED LED={name} RED={v} GREEN={v} BLUE={v} TRANSMIT=1")
            }
        }
        _ => return None,
    })
}

async fn execute(client: &Client, cmd: &Command) -> Result<(), moonraker::Error> {
    if let Some(code) = gcode_for(cmd) {
        return client.gcode(&code).await;
    }
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
        Command::StartPrint(file) => client
            .call("printer.print.start", json!({ "filename": file }))
            .await
            .map(drop),
        Command::SetSpeed(_)
        | Command::SetFlow(_)
        | Command::AdjustZ(_)
        | Command::SetPressureAdvance(_)
        | Command::SetLight { .. } => unreachable!("sent as G-code above"),
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
        ui.set_remaining_minutes(s.remaining_secs.map_or(-1, |t| t.div_ceil(60) as i32));
        ui.set_elapsed_minutes((s.print_duration / 60.0) as i32);
        ui.set_speed_percent(s.speed_percent as f32);
        ui.set_flow_percent(s.flow_percent as f32);
        ui.set_z_offset(s.z_offset as f32);
        ui.set_pressure_advance(s.pressure_advance as f32);
        ui.set_fan_percent(s.fan_percent.round() as i32);
        ui.set_has_light(s.light.is_some());
        if let Some(l) = &s.light {
            ui.set_light_on(l.on);
            ui.set_light_name(l.name.as_str().into());
            ui.set_light_white(l.white);
        }
        ui.set_layer(s.layer.map_or(-1, |n| n as i32));
        ui.set_total_layers(s.total_layers.map_or(-1, |n| n as i32));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tuning_commands_are_plain_gcode() {
        assert_eq!(gcode_for(&Command::SetSpeed(110.0)).unwrap(), "M220 S110");
        assert_eq!(gcode_for(&Command::SetFlow(95.0)).unwrap(), "M221 S95");
        assert_eq!(
            gcode_for(&Command::AdjustZ(-0.05)).unwrap(),
            "SET_GCODE_OFFSET Z_ADJUST=-0.050 MOVE=1"
        );
        assert_eq!(
            gcode_for(&Command::SetPressureAdvance(0.035)).unwrap(),
            "SET_PRESSURE_ADVANCE ADVANCE=0.035"
        );
        let led = |on, white| Command::SetLight {
            name: "chamber_led".into(),
            on,
            white,
        };
        assert_eq!(
            gcode_for(&led(true, true)).unwrap(),
            "SET_LED LED=chamber_led WHITE=1 TRANSMIT=1"
        );
        assert_eq!(
            gcode_for(&led(false, false)).unwrap(),
            "SET_LED LED=chamber_led RED=0 GREEN=0 BLUE=0 TRANSMIT=1"
        );
        assert!(gcode_for(&Command::Pause).is_none());
    }

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
