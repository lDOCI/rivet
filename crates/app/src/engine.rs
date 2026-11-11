//! Network side: owns the Moonraker client and printer state, pushes
//! throttled snapshots to the UI thread and executes UI commands.

use std::time::Duration;

use model::{Connection, Klippy, PrinterState, Snapshot};
use moonraker::{Client, Event};
use serde_json::{json, Value};
use tokio::{sync::mpsc, time::MissedTickBehavior};

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

use slint::{Rgba8Pixel, SharedPixelBuffer};

use crate::{thumbs, AppWindow};

/// UI refresh cap. Moonraker sends ~4 updates/s while printing.
const UI_TICK: Duration = Duration::from_millis(100);
/// Setup calls run inline in the event loop, so a hung Moonraker must not
/// stall UI updates for the default 30 s.
const SETUP_TIMEOUT: Duration = Duration::from_secs(5);

/// Decoded preview, shared between the cache and the UI without copying.
type Preview = SharedPixelBuffer<Rgba8Pixel>;

/// Cache limit for decoded previews. The printer has ~38 MB free in total.
const THUMB_CACHE_BYTES: usize = 3 * 1024 * 1024;
/// Files whose preview is fetched after a list loads (newest first).
const THUMB_PREFETCH: usize = 24;

/// Where previews come from and what is already decoded.
pub struct Thumbs {
    host: String,
    cache: Mutex<thumbs::Cache<Preview>>,
    /// Bumped on every list reload so a stale prefetch stops early.
    generation: AtomicU64,
}

impl Thumbs {
    fn new(url: &str) -> Arc<Self> {
        // ws://host:port/websocket -> host:port
        let host = url
            .split_once("://")
            .map_or(url, |(_, rest)| rest)
            .split('/')
            .next()
            .unwrap_or_default()
            .to_owned();
        Arc::new(Self {
            host,
            cache: Mutex::new(thumbs::Cache::new(THUMB_CACHE_BYTES)),
            generation: AtomicU64::new(0),
        })
    }
}

/// Preview for a G-code file, from cache or the printer. `None` when the file
/// has no PNG preview or anything on the way fails: the UI keeps its icon.
async fn preview(
    client: &Client,
    thumbs: &Thumbs,
    file: &str,
    meta: Option<model::FileMeta>,
) -> Option<Preview> {
    if let Some(hit) = thumbs.cache.lock().ok()?.get(file) {
        return Some(hit);
    }
    let meta = match meta {
        Some(m) => m,
        None => model::parse_file_meta(
            &client
                .call_timeout(
                    "server.files.metadata",
                    json!({ "filename": file }),
                    SETUP_TIMEOUT,
                )
                .await
                .ok()?,
        ),
    };
    let wanted = model::best_thumbnail(&meta.thumbnails, thumbs::MAX_WIDTH)?;
    let path = format!(
        "/server/files/gcodes/{}",
        thumbs::encode_path(&model::thumbnail_path(file, &wanted.relative_path))
    );
    let body = thumbs::http_get(&thumbs.host, &path)
        .await
        .inspect_err(|err| tracing::debug!(%err, file, "thumbnail fetch failed"))
        .ok()?;
    let img = thumbs::downscale(thumbs::decode_png(&body)?, thumbs::MAX_WIDTH);
    let size = img.data.len();
    let buf = Preview::clone_from_slice(&img.data, img.width, img.height);
    thumbs.cache.lock().ok()?.insert(file, buf.clone(), size);
    Some(buf)
}

fn image_of(preview: Option<Preview>) -> (slint::Image, bool) {
    match preview {
        Some(buf) => (slint::Image::from_rgba8(buf), true),
        None => (slint::Image::default(), false),
    }
}

#[derive(Debug, Clone)]
pub enum Command {
    HomeAll,
    Pause,
    Resume,
    Cancel,
    /// Speed / flow factor in percent (M220 / M221).
    SetSpeed(f32),
    SetFlow(f32),
    /// Heater targets in °C (M104 / M140); 0 turns the heater off.
    SetNozzle(f32),
    SetBed(f32),
    /// Part fan, percent.
    SetFan(f32),
    /// Move one axis (0 X, 1 Y, 2 Z) by `delta` mm from where it is.
    Move {
        axis: usize,
        delta: f32,
    },
    MotorsOff,
    /// Push (positive) or pull (negative) filament: mm and mm/s.
    Extrude {
        length: f32,
        speed: f32,
    },
    /// Move the Z offset by this many mm, live (`SET_GCODE_OFFSET`).
    AdjustZ(f32),
    SetPressureAdvance(f32),
    /// Light by Klipper name; `white` picks the channel set.
    SetLight {
        name: String,
        on: bool,
        white: bool,
    },
    /// Reload the list of G-code files.
    ListFiles,
    /// Slicer data and checks for one file, by its path in `gcodes`.
    FileDetails(String),
    DeleteFiles(Vec<String>),
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
    let thumbs = Thumbs::new(&url);
    let mut print_thumb_for = String::new();
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
                let cmd = match cmd {
                    Command::Move { axis, delta } => {
                        match clamp_move(&state.snapshot(), axis, delta) {
                            Some(delta) => Command::Move { axis, delta },
                            None => continue,
                        }
                    }
                    other => other,
                };
                predict(&mut state, &cmd);
                let client = client.clone();
                let ui = ui.clone();
                let thumbs = thumbs.clone();
                let resync = resync_tx.clone();
                tokio::spawn(async move {
                    if let Err(err) = execute(&client, &cmd, &ui, &thumbs).await {
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
                    // The preview of whatever is printing.
                    if ready && snap.filename != print_thumb_for {
                        print_thumb_for.clone_from(&snap.filename);
                        spawn_print_thumb(&client, &ui, &thumbs, &print_thumb_for);
                    }
                    push(&ui, snap.clone());
                    last_pushed = Some(snap);
                    if ready && changed {
                        refresh_history(&client, &ui, &thumbs).await;
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
async fn refresh_history(client: &Client, ui: &slint::Weak<AppWindow>, thumbs: &Arc<Thumbs>) {
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
    if let Some(l) = &last {
        let (client, ui, thumbs, name) = (
            client.clone(),
            ui.clone(),
            thumbs.clone(),
            l.filename.clone(),
        );
        tokio::spawn(async move {
            let image = preview(&client, &thumbs, &name, None).await;
            let _ = ui.upgrade_in_event_loop(move |ui| {
                let (img, has) = image_of(image);
                ui.set_last_thumb(img);
                ui.set_has_last_thumb(has);
            });
        });
    }
    let _ = ui.upgrade_in_event_loop(move |ui| match last {
        Some(l) => {
            let now = unix_now();
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
        Ok(mut result) => {
            state.replace_status(result["status"].take());
            if let Some(limits) = printer_limits(client).await {
                state.limits = limits;
            }
        }
        Err(err) => tracing::warn!(%err, "subscribe failed"),
    }
}

/// Heater limits and nozzle size from the printer's config, read once (a
/// failed read is not remembered, the next caller tries again).
async fn printer_limits(client: &Client) -> Option<model::Limits> {
    static LIMITS: tokio::sync::OnceCell<model::Limits> = tokio::sync::OnceCell::const_new();
    LIMITS
        .get_or_try_init(|| async {
            let query = json!({ "objects": { "configfile": ["settings"] } });
            let result = client
                .call_timeout("printer.objects.query", query, SETUP_TIMEOUT)
                .await
                .map_err(drop)?;
            Ok::<_, ()>(model::parse_limits(
                &result["status"]["configfile"]["settings"],
            ))
        })
        .await
        .ok()
        .copied()
}

async fn list_files(
    client: &Client,
    ui: &slint::Weak<AppWindow>,
    thumbs: &Arc<Thumbs>,
) -> Result<(), moonraker::Error> {
    let result = client
        .call_timeout(
            "server.files.list",
            json!({ "root": "gcodes" }),
            Duration::from_secs(20),
        )
        .await?;
    let now = unix_now();
    let files = model::parse_file_list(&result);
    // Plain data here; the UI thread builds the rows (images are not Send).
    let rows: Vec<(String, String, i32, i32)> = files
        .iter()
        .map(|f| {
            let name = f.path.rsplit('/').next().unwrap_or(&f.path);
            let name = name
                .strip_suffix(".gcode")
                .or_else(|| name.strip_suffix(".GCODE"))
                .unwrap_or(name);
            (
                f.path.clone(),
                name.to_owned(),
                ((now - f.modified).max(0.0) / 86_400.0) as i32,
                (f.size / 1024) as i32,
            )
        })
        .collect();
    let paths: Vec<String> = files.into_iter().map(|f| f.path).collect();
    let _ = ui.upgrade_in_event_loop(move |ui| {
        let items: Vec<crate::FileItem> = rows
            .into_iter()
            .map(|(path, name, days_ago, size_kb)| crate::FileItem {
                path: path.into(),
                name: name.into(),
                days_ago,
                size_kb,
                ..Default::default()
            })
            .collect();
        ui.set_files(slint::ModelRc::new(slint::VecModel::from(items)));
    });

    // Previews arrive one by one, newest first, and appear as they land.
    let generation = thumbs.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let (client, ui, thumbs) = (client.clone(), ui.clone(), thumbs.clone());
    tokio::spawn(async move {
        for path in paths.into_iter().take(THUMB_PREFETCH) {
            if thumbs.generation.load(Ordering::SeqCst) != generation {
                return;
            }
            let Some(buf) = preview(&client, &thumbs, &path, None).await else {
                continue;
            };
            let _ = ui.upgrade_in_event_loop(move |ui| {
                use slint::Model;
                let files = ui.get_files();
                let row = (0..files.row_count())
                    .find(|&i| files.row_data(i).is_some_and(|f| f.path == path.as_str()));
                if let Some((i, mut item)) = row.and_then(|i| Some((i, files.row_data(i)?))) {
                    item.thumb = slint::Image::from_rgba8(buf);
                    item.has_thumb = true;
                    files.set_row_data(i, item);
                }
            });
        }
    });
    Ok(())
}

async fn file_details(
    client: &Client,
    ui: &slint::Weak<AppWindow>,
    thumbs: &Arc<Thumbs>,
    path: &str,
) -> Result<(), moonraker::Error> {
    let result = client
        .call_timeout(
            "server.files.metadata",
            json!({ "filename": path }),
            SETUP_TIMEOUT,
        )
        .await?;
    let meta = model::parse_file_meta(&result);
    let printer_nozzle = printer_limits(client)
        .await
        .map(|l| l.nozzle_diameter)
        .filter(|d| *d > 0.0);
    // 1 = same as the printer, 0 = differs, -1 = one of them is unknown.
    let nozzle_check = match (meta.nozzle_diameter, printer_nozzle) {
        (Some(file), Some(printer)) => i32::from((file - printer).abs() < 0.001),
        _ => -1,
    };
    let image = preview(client, thumbs, path, Some(meta.clone())).await;
    let path = path.to_owned();
    let _ = ui.upgrade_in_event_loop(move |ui| {
        let (thumb, has_thumb) = image_of(image);
        ui.set_file_info(crate::FileInfo {
            path: path.into(),
            minutes: meta.estimated_secs.map_or(-1, |s| s.div_ceil(60) as i32),
            filament: meta.filament_type.unwrap_or_default().into(),
            grams: meta.filament_grams.map_or(-1, |g| g.round() as i32),
            layers: meta.layers.map_or(-1, |l| l as i32),
            nozzle: meta.nozzle_diameter.map_or(-1.0, |n| n as f32),
            nozzle_check,
            thumb,
            has_thumb,
        });
    });
    Ok(())
}

/// Fetch the preview of the file now printing and show it.
fn spawn_print_thumb(
    client: &Client,
    ui: &slint::Weak<AppWindow>,
    thumbs: &Arc<Thumbs>,
    file: &str,
) {
    let (client, ui, thumbs, file) = (client.clone(), ui.clone(), thumbs.clone(), file.to_owned());
    tokio::spawn(async move {
        let image = if file.is_empty() {
            None
        } else {
            preview(&client, &thumbs, &file, None).await
        };
        let _ = ui.upgrade_in_event_loop(move |ui| {
            let (img, has) = image_of(image);
            ui.set_print_thumb(img);
            ui.set_has_print_thumb(has);
        });
    });
}

fn unix_now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

/// A move that stays inside the axis range; `None` when it would not move.
fn clamp_move(snap: &Snapshot, axis: usize, delta: f32) -> Option<f32> {
    let axis = axis.min(2);
    let at = snap.position[axis];
    let target = (at + f64::from(delta)).clamp(snap.axis_min[axis], snap.axis_max[axis]);
    let delta = (target - at) as f32;
    (delta.abs() >= 0.001).then_some(delta)
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
        Command::SetFan(p) => {
            state.apply_status(json!({ "fan": { "speed": f64::from(*p) / 100.0 } }));
        }
        Command::SetNozzle(t) => {
            state.apply_status(json!({ "extruder": { "target": f64::from(*t) } }));
        }
        Command::SetBed(t) => {
            state.apply_status(json!({ "heater_bed": { "target": f64::from(*t) } }));
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
        Command::SetNozzle(t) => format!("M104 S{t:.0}"),
        Command::Move { axis, delta } => {
            let (letter, feed) = [("X", 6000), ("Y", 6000), ("Z", 600)][(*axis).min(2)];
            // Relative for this move only; the printer's modes come back after.
            format!(
                "SAVE_GCODE_STATE NAME=RIVET\nG91\nG1 {letter}{delta:.3} F{feed}\nRESTORE_GCODE_STATE NAME=RIVET"
            )
        }
        Command::MotorsOff => "M84".to_owned(),
        Command::Extrude { length, speed } => format!(
            "SAVE_GCODE_STATE NAME=RIVET\nM83\nG1 E{length:.1} F{:.0}\nRESTORE_GCODE_STATE NAME=RIVET",
            speed * 60.0
        ),
        Command::SetFan(p) => format!("M106 S{:.0}", p * 2.55),
        Command::SetBed(t) => format!("M140 S{t:.0}"),
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

async fn execute(
    client: &Client,
    cmd: &Command,
    ui: &slint::Weak<AppWindow>,
    thumbs: &Arc<Thumbs>,
) -> Result<(), moonraker::Error> {
    if let Some(code) = gcode_for(cmd) {
        return client.gcode(&code).await;
    }
    match cmd {
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
        Command::ListFiles => list_files(client, ui, thumbs).await,
        Command::FileDetails(path) => file_details(client, ui, thumbs, path).await,
        Command::DeleteFiles(paths) => {
            for path in paths {
                client
                    .call(
                        "server.files.delete_file",
                        json!({ "path": format!("gcodes/{path}") }),
                    )
                    .await?;
            }
            list_files(client, ui, thumbs).await
        }
        Command::SetSpeed(_)
        | Command::SetFlow(_)
        | Command::SetFan(_)
        | Command::Move { .. }
        | Command::MotorsOff
        | Command::Extrude { .. }
        | Command::SetNozzle(_)
        | Command::SetBed(_)
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
        ui.set_position_x(s.position[0] as f32);
        ui.set_position_y(s.position[1] as f32);
        ui.set_position_z(s.position[2] as f32);
        ui.set_homed_x(s.homed_axes.contains('x'));
        ui.set_homed_y(s.homed_axes.contains('y'));
        ui.set_homed_z(s.homed_axes.contains('z'));
        ui.set_can_extrude(s.can_extrude);
        ui.set_min_extrude_temp(s.limits.min_extrude_temp as f32);
        ui.set_max_nozzle(s.limits.extruder_max as f32);
        ui.set_max_bed(s.limits.bed_max as f32);
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
        assert_eq!(gcode_for(&Command::SetNozzle(240.0)).unwrap(), "M104 S240");
        assert_eq!(gcode_for(&Command::SetBed(0.0)).unwrap(), "M140 S0");
        assert_eq!(gcode_for(&Command::SetFan(60.0)).unwrap(), "M106 S153");
        assert_eq!(gcode_for(&Command::SetFan(100.0)).unwrap(), "M106 S255");
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
    fn moves_are_clamped_to_the_axis_range() {
        let snap = Snapshot {
            position: [100.0, 0.0, 5.0],
            axis_min: [-112.0, -112.0, -10.0],
            axis_max: [112.0, 112.0, 230.0],
            ..Default::default()
        };
        assert_eq!(clamp_move(&snap, 0, 10.0), Some(10.0));
        // Only 12 mm of travel left on X.
        assert_eq!(clamp_move(&snap, 0, 50.0), Some(12.0));
        assert_eq!(clamp_move(&snap, 2, -50.0), Some(-15.0));
        // Already at the end: no move at all.
        let at_end = Snapshot {
            position: [112.0, 0.0, 5.0],
            ..snap
        };
        assert_eq!(clamp_move(&at_end, 0, 10.0), None);
    }

    #[test]
    fn motion_and_extrusion_gcode_restore_the_modes() {
        let code = gcode_for(&Command::Move {
            axis: 1,
            delta: -10.0,
        })
        .unwrap();
        assert_eq!(
            code,
            "SAVE_GCODE_STATE NAME=RIVET\nG91\nG1 Y-10.000 F6000\nRESTORE_GCODE_STATE NAME=RIVET"
        );
        assert!(gcode_for(&Command::Move {
            axis: 2,
            delta: 1.0
        })
        .unwrap()
        .contains("Z1.000 F600"));
        let code = gcode_for(&Command::Extrude {
            length: -25.0,
            speed: 5.0,
        })
        .unwrap();
        assert!(code.contains("M83") && code.contains("G1 E-25.0 F300"));
        assert_eq!(gcode_for(&Command::MotorsOff).unwrap(), "M84");
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
