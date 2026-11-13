//! Network side: owns the Moonraker client and printer state, pushes
//! throttled snapshots to the UI thread and executes UI commands.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
/// Where Klipper writes its resonance CSVs (Rivet runs on the same host).
const KLIPPER_TMP: &str = "/tmp";
const SETUP_TIMEOUT: Duration = Duration::from_secs(5);

/// Decoded preview, shared between the cache and the UI without copying.
type Preview = SharedPixelBuffer<Rgba8Pixel>;

/// Cache limit for decoded previews. The printer has ~38 MB free in total.
const THUMB_CACHE_BYTES: usize = 3 * 1024 * 1024;
/// The Home card's preview area, px (the card less its padding, by the thumbnail's height).
const HOME_THUMB: (u32, u32) = (372, 232);

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

/// The smallest PNG preview at least `want` wide (else the largest), decoded.
async fn fetch_png(
    thumbs: &Thumbs,
    file: &str,
    meta: &model::FileMeta,
    want: u32,
) -> Option<thumbs::Rgba> {
    let wanted = model::best_thumbnail(&meta.thumbnails, want)?;
    let path = format!(
        "/server/files/gcodes/{}",
        thumbs::encode_path(&model::thumbnail_path(file, &wanted.relative_path))
    );
    let body = thumbs::http_get(&thumbs.host, &path)
        .await
        .inspect_err(|err| tracing::debug!(%err, file, "thumbnail fetch failed"))
        .ok()?;
    thumbs::decode_png(&body)
}

/// The Home card's preview: as large as its place (HOME_THUMB), not the grid's.
async fn home_preview(client: &Client, thumbs: &Thumbs, file: &str) -> Option<Preview> {
    let key = format!("{file}\u{0}home");
    if let Some(hit) = thumbs.cache.lock().ok()?.get(&key) {
        return Some(hit);
    }
    let meta = model::parse_file_meta(
        &client
            .call_timeout(
                "server.files.metadata",
                json!({ "filename": file }),
                SETUP_TIMEOUT,
            )
            .await
            .ok()?,
    );
    let img = thumbs::prepare_for(
        fetch_png(thumbs, file, &meta, HOME_THUMB.0).await?,
        HOME_THUMB.0,
        HOME_THUMB.1,
    );
    let size = img.data.len();
    let buf = Preview::clone_from_slice(&img.data, img.width, img.height);
    thumbs.cache.lock().ok()?.insert(&key, buf.clone(), size);
    Some(buf)
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
    let img = thumbs::prepare(fetch_png(thumbs, file, &meta, thumbs::MAX_WIDTH).await?);
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
    /// A fan by Klipper object name (`fan` is the part fan), percent.
    SetFan {
        object: String,
        percent: f32,
    },
    /// Chamber air: 0 off, 1 filter, 2 exhaust (the printer's own macros).
    SetAir(u8),
    /// Move one axis (0 X, 1 Y, 2 Z) by `delta` mm from where it is, at `speed` mm/s.
    Move {
        axis: usize,
        delta: f32,
        speed: f32,
    },
    /// Go to a position (absolute, mm) at `speed` mm/s.
    GoTo {
        x: f32,
        y: f32,
        z: f32,
        speed: f32,
    },
    MotorsOff,
    /// Push (positive) or pull (negative) filament: mm and mm/s. A cold
    /// nozzle is heated to `heat` degrees first (0: it is hot enough).
    Extrude {
        length: f32,
        speed: f32,
        heat: f32,
    },
    /// Move the Z offset by this many mm, live (`SET_GCODE_OFFSET`).
    AdjustZ(f32),
    SetPressureAdvance(f32),
    /// Light by Klipper name, brightness 0.0 ..= 1.0; `white` picks the channel set.
    SetLight {
        name: String,
        level: f32,
        white: bool,
    },
    /// Try to reach Moonraker now instead of waiting for the next attempt.
    RetryNow,
    /// One of Rivet's own settings, kept in Moonraker's database (namespace `rivet`).
    SaveSetting {
        key: String,
        value: Value,
    },
    /// Pause when this layer starts (0 cancels); the printer's SET_PAUSE_AT_LAYER.
    PauseAtLayer(u32),
    /// Pause when the next layer starts, or cancel that.
    PauseNextLayer(bool),
    /// Firmware retraction: one value by its row key, mm or mm/s.
    SetRetraction {
        key: String,
        value: f32,
    },
    SetTimelapse(bool),
    /// Power the printer off once the print is over (the printer's AUTO_SHUTDOWN).
    OffAfterPrint(bool),
    /// Keep the live Z offset: applied now, written by SAVE_CONFIG once no print runs.
    SaveZOffset,
    /// The bed as a filament dryer: °C for this many minutes; 0 minutes stops it.
    Dry {
        temp: f32,
        minutes: u32,
    },
    /// A Moonraker power device on or off.
    SetPower {
        device: String,
        on: bool,
    },
    /// A user output pin on or off.
    SetPin {
        name: String,
        on: bool,
    },
    /// Read the Spoolman spools and the active one.
    ListSpools,
    /// Make this spool the active one; 0: none.
    SetActiveSpool(i32),
    /// G-code typed in the console or a macro to run.
    Gcode(String),
    ClearConsole,
    /// Reload the list of G-code files.
    /// The files of one storage: the printer's, or the USB stick (`usb/`).
    ListFiles {
        usb: bool,
    },
    /// Slicer data and checks for one file, by its path in `gcodes`.
    FileDetails(String),
    DeleteFiles(Vec<String>),
    /// Rename or move a file inside `gcodes`.
    MoveFile {
        from: String,
        to: String,
    },
    /// Copy a file inside `gcodes` (from the USB stick to the printer).
    CopyFile {
        from: String,
        to: String,
    },
    /// System, network and versions for Settings.
    SystemInfo,
    /// A Settings › System action: "klipper", "firmware", "moonraker", "reboot",
    /// "shutdown", "stock", "reset", "check-updates".
    SystemAction(String),
    /// A calibration's G-code; the screens show it running, then read the result.
    Calibrate(String),
    /// Read what the calibration screens show (config, bed mesh, screws, drivers).
    CalibInfo,
    /// SET_VELOCITY_LIMIT: "velocity", "accel", "corner" or "cruise" (0..1).
    SetLimit {
        key: String,
        value: f32,
    },
    /// Switch the runout sensor on or off.
    SetRunout(bool),
    ListHistory,
    ClearHistory,
    /// Service intervals from print hours; a part replaced (or greased) now.
    ListService,
    ServiceDone(String),
    ListCameras,
    /// Temperatures of the last minutes for the graph, drawn for the plot's
    /// width / height.
    TempGraph(f32),
    /// Vibration and belt curves, drawn for the plot's width / height.
    DrawResonance(f32),
    /// A macro was picked: its parameters, or straight on to confirm or run it.
    MacroParams(String),
    /// Look for Wi-Fi networks around (wpa_supplicant on this machine).
    WifiScan,
    /// Join a network: name, "open"/"psk"/"sae", password ("" for a saved one).
    WifiJoin(String, String, String),
    /// Previews for these Files grid entries (the ones near the glass).
    Thumbs(Vec<String>),
    /// Read the messages again; mark all of them read.
    ListMessages,
    ReadMessages,
    /// Moonraker's print queue: read it, add files, start (true) or pause it, empty it.
    ListQueue,
    QueueAdd(Vec<String>),
    QueueRun(bool),
    QueueClear,
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
    let (resync_tx, mut resync_rx) = mpsc::unbounded_channel::<Internal>();
    let mut shown_console = u64::MAX;
    let mut shown_prompt = 0u64;
    let mut shown_macros: Vec<String> = Vec::new();
    // "Turn on at startup" is applied once per start of Rivet, when the light is known.
    let mut light_start_done = false;
    // Things Rivet does itself when a print ends, and the bed dryer's timer.
    let mut off_after_print = false;
    // Layers for a print whose slicer does not report them: the file it is
    // for, and the tracker once the file's layer plan came in.
    let mut layers_for = String::new();
    let mut layers: Option<model::LayerTracker> = None;
    // When the layer count was last sent: Klipper may drop it (one sent as the
    // file loads is cleared by the load), so it is sent again until it holds.
    let mut layers_sent: Option<Instant> = None;
    let started = Instant::now();
    let mut save_config_pending = false;
    let mut dryer: Option<(SystemTime, f32)> = None;
    let mut dryer_shown_minutes = -1;
    let mut last_print_state = String::new();
    // Reconnecting: which attempt failed last and when the next one comes.
    let mut retry_at: Option<(u32, Instant)> = None;
    let mut retry_shown = (0u32, -1i32);
    let mut nozzle_rate = HeatRate::default();
    let mut bed_rate = HeatRate::default();

    let mut tick = tokio::time::interval(UI_TICK);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            ev = events.recv() => {
                let Some(ev) = ev else { break };
                match &ev {
                    Event::Retrying { attempt, next_in } => {
                        retry_at = Some((*attempt, Instant::now() + *next_in));
                        continue;
                    }
                    Event::Connected => retry_at = None,
                    _ => {}
                }
                handle_event(ev, &client, &mut state).await;
            }
            cmd = cmds.recv() => {
                let Some(cmd) = cmd else { break };
                tracing::debug!(?cmd, "command from the UI");
                match &cmd {
                    Command::ClearConsole => {
                        state.clear_console();
                        continue;
                    }
                    Command::Gcode(script) => {
                        state.push_console(format!("> {script}"), model::LineKind::Command);
                    }
                    _ => {}
                }
                // Commands Rivet keeps state for itself.
                let printing = matches!(state.snapshot().print_state.as_str(), "printing" | "paused");
                match &cmd {
                    Command::RetryNow => {
                        client.retry_now();
                        continue;
                    }
                    Command::OffAfterPrint(on) => {
                        off_after_print = *on;
                        continue;
                    }
                    Command::SaveZOffset => {
                        let apply = state.caps.z_offset_apply.clone();
                        let code = if printing {
                            save_config_pending = true;
                            apply
                        } else {
                            format!("{apply}\nSAVE_CONFIG")
                        };
                        spawn_gcode(&client, code);
                        continue;
                    }
                    Command::Dry { temp, minutes } => {
                        dryer = (*minutes > 0).then(|| {
                            (SystemTime::now() + Duration::from_secs(u64::from(*minutes) * 60), *temp)
                        });
                        dryer_shown_minutes = -2;
                        let target = if *minutes > 0 { *temp } else { 0.0 };
                        spawn_gcode(&client, format!("M140 S{target:.0}"));
                        save_dryer(&client, dryer);
                        continue;
                    }
                    _ => {}
                }
                let cmd = match cmd {
                    Command::Move { axis, delta, speed } => {
                        match clamp_move(&state.snapshot(), axis, delta) {
                            Some(delta) => Command::Move { axis, delta, speed },
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
                        let _ = resync.send(Internal::Resync);
                        if let moonraker::Error::Rpc { message, .. } = &err {
                            let _ = resync.send(Internal::ConsoleError(message.clone()));
                        }
                        let (kind, detail) = toast_for(&err);
                        let _ = ui.upgrade_in_event_loop(move |ui| {
                            ui.set_toast_detail(detail.into());
                            ui.set_toast_kind(kind.into());
                        });
                    }
                });
            }
            Some(internal) = resync_rx.recv() => match internal {
                Internal::Resync => resync_status(&client, &mut state).await,
                Internal::ResumeDryer(end, temp) => {
                    dryer = Some((end, temp));
                    dryer_shown_minutes = -2;
                }
                Internal::ConsoleError(message) => {
                    state.push_console(format!("!! {message}"), model::LineKind::Error);
                }
                Internal::LayerPlan(file, plan) => {
                    let snap = state.snapshot();
                    if file == layers_for && snap.total_layers.is_none() {
                        layers = plan.and_then(|(first, height, count)| {
                            model::LayerTracker::new(first, height, count)
                        });
                        // Klipper learns the count now: layer pauses become possible.
                        if let Some(t) = &layers {
                            spawn_gcode(&client, format!("SET_PRINT_STATS_INFO TOTAL_LAYER={}", t.count()));
                            layers_sent = Some(Instant::now());
                        }
                    }
                }
            },
            _ = tick.tick() => {
                if shown_console != state.console_revision {
                    shown_console = state.console_revision;
                    push_console(&ui, &state.console);
                }
                if shown_prompt != state.prompt_revision {
                    shown_prompt = state.prompt_revision;
                    push_prompt(&ui, state.prompt.clone());
                }
                if shown_macros != state.macros {
                    shown_macros.clone_from(&state.macros);
                    let names = state.macros.clone();
                    let _ = ui.upgrade_in_event_loop(move |ui| {
                        let rows: Vec<slint::SharedString> =
                            names.into_iter().map(Into::into).collect();
                        ui.set_macros(slint::ModelRc::new(slint::VecModel::from(rows)));
                    });
                }
                // "attempt 7 · next in 3 s" while there is no link.
                let shown = retry_at.map_or((0, -1), |(n, at)| {
                    (n, at.saturating_duration_since(Instant::now()).as_secs_f32().ceil() as i32)
                });
                if shown != retry_shown {
                    retry_shown = shown;
                    let _ = ui.upgrade_in_event_loop(move |ui| {
                        ui.set_retry_attempt(shown.0 as i32);
                        ui.set_retry_in(shown.1);
                    });
                }
                let snap = state.snapshot();
                let now = Instant::now();
                let etas = (
                    nozzle_rate.eta(now, &snap.extruder),
                    bed_rate.eta(now, &snap.bed),
                );
                if etas != (nozzle_rate.shown, bed_rate.shown) {
                    nozzle_rate.shown = etas.0;
                    bed_rate.shown = etas.1;
                    let _ = ui.upgrade_in_event_loop(move |ui| {
                        ui.set_nozzle_eta(etas.0);
                        ui.set_bed_eta(etas.1);
                    });
                }
                if !light_start_done && snap.klippy == Klippy::Ready {
                    light_start_done = true;
                    tokio::spawn(load_settings(
                        client.clone(),
                        ui.clone(),
                        snap.light.clone(),
                        snap.caps.clone(),
                        resync_tx.clone(),
                    ));
                }
                // The screen stays on while printing, and wakes when a print ends or
                // Klipper stops, so the result or the error is there to see.
                #[cfg(feature = "fbdev")]
                {
                    let running = matches!(snap.print_state.as_str(), "printing" | "paused");
                    fbdev_platform::screen::set_keep_awake(running && prefs::get(&prefs::AWAKE_PRINT));
                    if last_pushed.as_ref().is_some_and(|p| {
                        p.print_state != snap.print_state || (p.klippy != snap.klippy && snap.klippy != Klippy::Ready)
                    }) {
                        fbdev_platform::screen::wake();
                    }
                }
                // A print whose slicer reports no layers: Rivet tells Klipper.
                let printing = snap.print_state == "printing";
                if matches!(snap.print_state.as_str(), "printing" | "paused") {
                    if snap.filename != layers_for {
                        layers_for.clone_from(&snap.filename);
                        layers = None;
                        if snap.total_layers.is_none() && !snap.filename.is_empty() {
                            let (client, tx, file) = (client.clone(), resync_tx.clone(), snap.filename.clone());
                            tokio::spawn(async move {
                                let plan = client
                                    .call_timeout("server.files.metadata", json!({ "filename": file }), SETUP_TIMEOUT)
                                    .await
                                    .ok()
                                    .and_then(|m| model::layer_plan(&m));
                                let _ = tx.send(Internal::LayerPlan(file, plan));
                            });
                        }
                    }
                    // Sent, but Klipper still has no count: send it again.
                    if let Some(t) = &layers {
                        if snap.total_layers.is_none()
                            && layers_sent.is_some_and(|at| at.elapsed() >= Duration::from_secs(5))
                        {
                            tracing::info!(count = t.count(), "Klipper lost the layer count: sending it again");
                            spawn_gcode(&client, format!("SET_PRINT_STATS_INFO TOTAL_LAYER={}", t.count()));
                            layers_sent = Some(Instant::now());
                        }
                    }
                    if printing {
                        if let Some(t) = layers.as_mut() {
                            let new = t.update(started.elapsed().as_secs_f64(), snap.gcode_z);
                            if !new.is_empty() {
                                let code = new
                                    .iter()
                                    .map(|n| format!("SET_PRINT_STATS_INFO CURRENT_LAYER={n}"))
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                spawn_gcode(&client, code);
                            }
                        }
                    }
                } else {
                    layers_for.clear();
                    layers = None;
                    layers_sent = None;
                }
                // A print just ended: what was waiting for it.
                if snap.print_state != last_print_state {
                    let running = |s: &str| matches!(s, "printing" | "paused");
                    if running(&last_print_state) && !running(&snap.print_state) {
                        if snap.print_state == "complete" && prefs::get(&prefs::END_TUNE) {
                            // A short rising tune on the printer's beeper (KlipperMod's M300).
                            spawn_gcode(
                                &client,
                                "M300 S1047 P150\nM300 S1319 P150\nM300 S1568 P300".to_owned(),
                            );
                        }
                        if off_after_print && snap.print_state == "complete" {
                            spawn_gcode(&client, "AUTO_SHUTDOWN".to_owned());
                        }
                        off_after_print = false;
                        if save_config_pending {
                            save_config_pending = false;
                            spawn_gcode(&client, "SAVE_CONFIG".to_owned());
                        }
                        let _ = ui.upgrade_in_event_loop(|ui| ui.set_off_after_print(false));
                    }
                    last_print_state.clone_from(&snap.print_state);
                }
                // The dryer: switch the bed off on time; show the minutes left.
                if let Some((end, _)) = dryer {
                    if SystemTime::now() >= end {
                        dryer = None;
                        spawn_gcode(&client, "M140 S0".to_owned());
                        save_dryer(&client, None);
                    }
                }
                let minutes = dryer.map_or(-1, |(end, _)| {
                    end.duration_since(SystemTime::now())
                        .map_or(0, |d| d.as_secs().div_ceil(60) as i32)
                });
                if minutes != dryer_shown_minutes {
                    dryer_shown_minutes = minutes;
                    let _ = ui.upgrade_in_event_loop(move |ui| ui.set_dryer_minutes_left(minutes));
                }
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
                        list_messages(&client, &ui).await;
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
        Event::Retrying { .. } => {}
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
            "notify_gcode_response" => {
                if let Some(line) = params.get(0).and_then(Value::as_str) {
                    state.prompt_line(line);
                    crate::resonance::feed(line);
                    if let Some(range) = model::probe_range(line) {
                        state.probe_range = Some(range);
                    }
                    state.push_console(line.to_owned(), model::line_kind(line));
                }
            }
            // An update from anywhere (Mainsail too): when Rivet's is done, start
            // the new binary.
            "notify_update_response" => {
                let r = &params[0];
                if r["application"].as_str() == Some("rivet")
                    && r["complete"].as_bool() == Some(true)
                    && !r["message"].as_str().unwrap_or_default().contains("Error")
                {
                    restart_for_update();
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

/// Moonraker has put a new Rivet next to this one: exit, and S80rivet starts it
/// (and rolls back if it does not come up). Only when S80rivet runs this
/// binary (it sets RIVET_SUPERVISED): started by hand, Rivet would just be gone.
fn restart_for_update() {
    if std::env::var_os("RIVET_SUPERVISED").is_none() {
        tracing::info!(
            "a new Rivet is installed; this one is not run by S80rivet, so it keeps running"
        );
        return;
    }
    tracing::info!("a new Rivet is installed: exiting so S80rivet starts it");
    std::process::exit(0);
}

/// Things the engine tells itself from spawned tasks.
enum Internal {
    /// A drying session that was running when Rivet stopped: end time, °C.
    ResumeDryer(SystemTime, f32),
    /// A command failed: read the real values back.
    Resync,
    /// Klipper's reason a command failed, for the console.
    ConsoleError(String),
    /// A printing file's first-layer height, layer height and layer count.
    LayerPlan(String, Option<(f64, f64, u32)>),
}

/// A macro's prompt to the dialog, or close it.
fn push_prompt(ui: &slint::Weak<AppWindow>, prompt: Option<model::Prompt>) {
    let _ = ui.upgrade_in_event_loop(move |ui| {
        let prompt = prompt.unwrap_or_default();
        let shown = !prompt.title.is_empty() || !prompt.buttons.is_empty();
        let buttons: Vec<crate::PromptButton> = prompt
            .buttons
            .into_iter()
            .map(|b| crate::PromptButton {
                label: b.label.into(),
                gcode: b.gcode.into(),
                style: b.style.into(),
            })
            .collect();
        ui.set_prompt_title(prompt.title.into());
        ui.set_prompt_text(prompt.text.join("\n").into());
        ui.set_prompt_buttons(slint::ModelRc::new(slint::VecModel::from(buttons)));
        ui.set_prompt_shown(shown);
    });
}

fn push_console(ui: &slint::Weak<AppWindow>, lines: &[(String, model::LineKind)]) {
    let rows: Vec<(String, i32)> = lines.iter().map(|(t, k)| (t.clone(), *k as i32)).collect();
    let _ = ui.upgrade_in_event_loop(move |ui| {
        let items: Vec<crate::ConsoleLine> = rows
            .into_iter()
            .map(|(text, kind)| crate::ConsoleLine {
                text: text.into(),
                kind,
            })
            .collect();
        ui.set_console_lines(slint::ModelRc::new(slint::VecModel::from(items)));
    });
}

/// Messages: finished prints, what Klipper and Moonraker warn about, Moonraker's
/// news. Read up to the time saved in Rivet's database (`messages_read`).
async fn list_messages(client: &Client, ui: &slint::Weak<AppWindow>) {
    let call = |method: &'static str, params: Value| async move {
        client
            .call_timeout(method, params, SETUP_TIMEOUT)
            .await
            .unwrap_or(Value::Null)
    };
    let jobs = call(
        "server.history.list",
        json!({ "limit": 10, "order": "desc" }),
    )
    .await;
    let config = call(
        "printer.objects.query",
        json!({ "objects": { "configfile": ["save_config_pending", "warnings"] } }),
    )
    .await;
    let info = call("server.info", json!({})).await;
    let news = call(
        "server.announcements.list",
        json!({ "include_dismissed": false }),
    )
    .await;
    let read = call(
        "server.database.get_item",
        json!({ "namespace": DB_NAMESPACE, "key": "messages_read" }),
    )
    .await["value"]
        .as_f64()
        .unwrap_or(0.0);
    let mut messages = model::standing_messages(&config["status"]["configfile"], &info);
    let mut dated = model::job_messages(&jobs);
    dated.extend(model::news_messages(&news));
    dated.sort_by(|a, b| b.time.total_cmp(&a.time));
    messages.extend(dated);
    let now = unix_now();
    let rows: Vec<(model::Message, i32, String, bool)> = messages
        .into_iter()
        .map(|m| {
            let days = if m.time > 0.0 {
                days_between(m.time, now)
            } else {
                -1
            };
            let clock = if days == 0 {
                local_clock(m.time).unwrap_or_default()
            } else {
                String::new()
            };
            let unread = m.time > read;
            (m, days, clock, unread)
        })
        .collect();
    let _ = ui.upgrade_in_event_loop(move |ui| {
        let unread = rows.iter().any(|r| r.3);
        let items: Vec<crate::MessageRow> = rows
            .into_iter()
            .map(|(m, days, clock, unread)| crate::MessageRow {
                kind: m.kind.into(),
                title: crate::files::title(&m.title).into(),
                sub: m.sub.into(),
                days,
                clock: clock.into(),
                unread,
            })
            .collect();
        ui.set_messages(slint::ModelRc::new(slint::VecModel::from(items)));
        ui.set_unread(unread);
    });
}

/// CPU, memory, disk, network, versions and the time zone for Settings.
async fn system_info(client: &Client, ui: &slint::Weak<AppWindow>) {
    let call = |method: &'static str, params: Value| async move {
        client
            .call_timeout(method, params, SETUP_TIMEOUT)
            .await
            .unwrap_or(Value::Null)
    };
    let sys = call("machine.system_info", json!({})).await;
    let stats = call("machine.proc_stats", json!({})).await;
    let dir = call("server.files.get_directory", json!({ "path": "gcodes" })).await;
    let printer = call("printer.info", json!({})).await;
    let server = call("server.info", json!({})).await;
    let updates = call("machine.update.status", json!({ "refresh": false })).await;
    let objects = call("printer.objects.list", json!({})).await;
    let sys = &sys["system_info"];
    let cpu = format!(
        "{} · {}{}",
        sys["cpu_info"]["processor"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("CPU"),
        sys["cpu_info"]["cpu_count"]
            .as_i64()
            .map(|n| format!("{n} × "))
            .unwrap_or_default(),
        stats["system_cpu_usage"]["cpu"]
            .as_f64()
            .map(|p| format!("{p:.0}%"))
            .unwrap_or_default()
    );
    let mem = &stats["system_memory"];
    let memory = match (
        mem["total"].as_f64(),
        mem["used"]
            .as_f64()
            .or_else(|| Some(mem["total"].as_f64()? - mem["available"].as_f64()?)),
    ) {
        (Some(total), Some(used)) => format!("{:.0} / {:.0} MB", used / 1024.0, total / 1024.0),
        _ => String::new(),
    };
    let du = &dir["disk_usage"];
    let disk = match (du["used"].as_f64(), du["total"].as_f64()) {
        (Some(u), Some(t)) => format!("{:.1} / {:.1} GB", u / 1e9, t / 1e9),
        _ => String::new(),
    };
    // Addresses by interface: Wi-Fi is wlan*, the cable eth*/end*.
    let mut wifi = (String::new(), String::new(), String::new());
    let mut eth = String::new();
    if let Some(net) = sys["network"].as_object() {
        for (name, iface) in net {
            let ip = iface["ip_addresses"]
                .as_array()
                .and_then(|a| a.iter().find(|x| x["family"].as_str() == Some("ipv4")))
                .and_then(|x| x["address"].as_str())
                .unwrap_or_default()
                .to_owned();
            if ip.is_empty() {
                continue;
            }
            if name.starts_with("wl") {
                wifi = (
                    name.clone(),
                    ip,
                    iface["mac_address"].as_str().unwrap_or_default().to_owned(),
                );
            } else if name.starts_with("eth") || name.starts_with("en") {
                eth = ip;
            }
        }
    }
    let web = |name: &str| {
        let v = &updates["version_info"][name];
        let version = v["version"].as_str().unwrap_or_default().to_owned();
        let remote = v["remote_version"].as_str().unwrap_or_default();
        let fresh = remote.is_empty() || remote == version;
        (version, fresh, remote.to_owned())
    };
    let (mainsail, fluidd) = (web("mainsail"), web("fluidd"));
    // Rivet itself, when the installer gave Moonraker an [update_manager rivet].
    let rivet = updates["version_info"]["rivet"]
        .is_object()
        .then(|| web("rivet"));
    // S80rivet leaves this next to the binary when a new one did not come up.
    let rolled_back = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("rolled-back")))
        .is_some_and(|p| p.exists());
    let zone = std::fs::read_link("/etc/localtime")
        .ok()
        .and_then(|p| {
            let p = p.to_string_lossy().into_owned();
            p.split_once("zoneinfo/").map(|(_, z)| z.to_owned())
        })
        .unwrap_or_default();
    let stock = objects["objects"].as_array().is_some_and(|o| {
        o.iter()
            .any(|n| n.as_str() == Some("gcode_macro REBOOT_STOCK_SYSTEM"))
    });
    let host = printer["hostname"].as_str().unwrap_or_default().to_owned();
    let klipper = printer["software_version"]
        .as_str()
        .unwrap_or_default()
        .split("-g")
        .next()
        .unwrap_or_default()
        .to_owned();
    let moonraker = server["moonraker_version"]
        .as_str()
        .unwrap_or_default()
        .split("-g")
        .next()
        .unwrap_or_default()
        .to_owned();
    let _ = ui.upgrade_in_event_loop(move |ui| {
        let words = slint::ComponentHandle::global::<crate::Strings>(&ui);
        let state = |(_, fresh, remote): &(String, bool, String)| {
            if *fresh {
                words.get_up_to_date().to_string()
            } else {
                words.get_update_to().replace("{}", remote)
            }
        };
        ui.set_sys_cpu(cpu.into());
        ui.set_sys_memory(memory.into());
        ui.set_sys_disk(disk.into());
        ui.set_net_wifi(
            if wifi.0.is_empty() {
                String::new()
            } else {
                wifi.0.clone()
            }
            .into(),
        );
        ui.set_net_ip(wifi.1.clone().into());
        ui.set_net_mac(wifi.2.clone().into());
        ui.set_net_eth(eth.into());
        ui.set_host_name(host.into());
        ui.set_ver_klipper(klipper.into());
        ui.set_ver_moonraker(moonraker.into());
        ui.set_ver_rivet(env!("CARGO_PKG_VERSION").into());
        let (rivet_sub, update) = match &rivet {
            _ if rolled_back => (words.get_rivet_rolled_back().to_string(), None),
            Some(r) if !r.1 => (state(r), Some(r.2.clone())),
            Some(r) => (state(r), None),
            None => (String::new(), None),
        };
        ui.set_ver_rivet_sub(rivet_sub.into());
        ui.set_rivet_update(update.is_some());
        ui.set_rivet_update_sub(
            update
                .map(|v| words.get_rivet_to().replace("{}", &v))
                .unwrap_or_default()
                .into(),
        );
        ui.set_ver_mainsail_sub(state(&mainsail).into());
        ui.set_ver_mainsail(mainsail.0.into());
        ui.set_ver_fluidd_sub(state(&fluidd).into());
        ui.set_ver_fluidd(fluidd.0.into());
        ui.set_has_stock_macro(stock);
        ui.set_time_zone(zone.into());
    });
}

/// Everything the calibration screens show, read fresh: the saved config, the
/// bed mesh, the screws' last result, the stepper drivers.
async fn calib_info(client: &Client, ui: &slint::Weak<AppWindow>) {
    let objects: Vec<String> = client
        .call_timeout("printer.objects.list", json!({}), SETUP_TIMEOUT)
        .await
        .ok()
        .and_then(|l| l["objects"].as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    let has = |name: &str| objects.iter().any(|o| o == name);
    let tmcs: Vec<String> = objects
        .iter()
        .filter(|o| o.starts_with("tmc"))
        .cloned()
        .collect();
    let mut query = serde_json::Map::new();
    query.insert("configfile".into(), json!(["settings"]));
    for name in ["bed_mesh", "screws_tilt_adjust"] {
        if has(name) {
            query.insert(name.into(), Value::Null);
        }
    }
    for t in &tmcs {
        query.insert(t.clone(), Value::Null);
    }
    let status = client
        .call_timeout(
            "printer.objects.query",
            json!({ "objects": query }),
            SETUP_TIMEOUT,
        )
        .await
        .map(|v| v["status"].clone())
        .unwrap_or(Value::Null);
    let cfg = &status["configfile"]["settings"];
    let info = model::CalibInfo::read(cfg, &status, &objects);
    let _ = ui.upgrade_in_event_loop(move |ui| crate::calib::show(&ui, info));
}

/// Run a G-code line and wait for Klipper's answer.
async fn gcode(client: &Client, script: &str) -> Result<(), moonraker::Error> {
    client.gcode(script).await
}

/// The runout sensor's name, from the printer's objects.
async fn runout_name(client: &Client) -> Option<String> {
    let list = client
        .call_timeout("printer.objects.list", json!({}), SETUP_TIMEOUT)
        .await
        .ok()?;
    list["objects"]
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .find_map(|n| {
            n.strip_prefix("filament_switch_sensor ")
                .or_else(|| n.strip_prefix("filament_motion_sensor "))
                .map(str::to_owned)
        })
}

/// Hours printed in all, from Moonraker's history totals.
async fn print_hours(client: &Client) -> f64 {
    client
        .call_timeout("server.history.totals", json!({}), SETUP_TIMEOUT)
        .await
        .ok()
        .and_then(|t| t["job_totals"]["total_print_time"].as_f64())
        .unwrap_or(0.0)
        / 3600.0
}

/// Totals and the latest jobs for the History screen.
async fn list_history(client: &Client, ui: &slint::Weak<AppWindow>) {
    let totals = client
        .call_timeout("server.history.totals", json!({}), SETUP_TIMEOUT)
        .await
        .unwrap_or(Value::Null);
    let list = client
        .call_timeout(
            "server.history.list",
            json!({ "limit": 20, "order": "desc" }),
            SETUP_TIMEOUT,
        )
        .await
        .unwrap_or(Value::Null);
    let t = &totals["job_totals"];
    let total = t["total_jobs"].as_i64().map_or(-1, |n| n as i32);
    let minutes = (t["total_print_time"].as_f64().unwrap_or(0.0) / 60.0) as i32;
    let km = (t["total_filament_used"].as_f64().unwrap_or(0.0) / 1_000_000.0) as f32;
    let now = unix_now();
    let jobs: Vec<(String, String, i32, i32, String)> = list["jobs"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|j| {
            let status = j["status"].as_str()?;
            if status == "in_progress" {
                return None;
            }
            Some((
                j["filename"].as_str()?.to_owned(),
                status.to_owned(),
                days_between(j["end_time"].as_f64().unwrap_or(now), now),
                (j["print_duration"].as_f64().unwrap_or(0.0) / 60.0).round() as i32,
                j.pointer("/metadata/filament_type")
                    .and_then(Value::as_str)
                    .map(|t| t.split(';').next().unwrap_or(t).trim().to_owned())
                    .unwrap_or_default(),
            ))
        })
        .collect();
    let _ = ui.upgrade_in_event_loop(move |ui| {
        let format = slint::ComponentHandle::global::<crate::Format>(&ui);
        let rows: Vec<crate::FormRow> = jobs
            .into_iter()
            .map(|(path, status, days, minutes, material)| {
                let mut sub = vec![format.invoke_age(days).to_string()];
                if minutes > 0 {
                    sub.push(format.invoke_duration(minutes).to_string());
                }
                if !material.is_empty() {
                    sub.push(material);
                }
                crate::FormRow {
                    kind: "link".into(),
                    key: path.as_str().into(),
                    title: crate::files::title(&path).into(),
                    sub: sub.join(" · ").into(),
                    value: match status.as_str() {
                        "completed" => "✓",
                        "cancelled" => "■",
                        _ => "✗",
                    }
                    .into(),
                    shown: true,
                    ..Default::default()
                }
            })
            .collect();
        ui.set_history_total(total);
        ui.set_history_minutes(minutes);
        ui.set_history_km(km);
        ui.set_history_rows(slint::ModelRc::new(slint::VecModel::from(rows)));
    });
}

/// Wear of the parts from print hours since each was last replaced.
async fn list_service(client: &Client, ui: &slint::Weak<AppWindow>) {
    let hours = print_hours(client).await;
    let saved = client
        .call_timeout(
            "server.database.get_item",
            json!({ "namespace": DB_NAMESPACE }),
            SETUP_TIMEOUT,
        )
        .await
        .map(|v| v["value"].clone())
        .unwrap_or(Value::Null);
    // The HEPA filter only on printers with the chamber air macros (AD5M Pro).
    let has_filter = client
        .call_timeout("printer.objects.list", json!({}), SETUP_TIMEOUT)
        .await
        .ok()
        .and_then(|l| l["objects"].as_array().cloned())
        .is_some_and(|o| {
            o.iter()
                .any(|n| n.as_str() == Some("gcode_macro AIR_CIRCULATION_INTERNAL"))
        });
    let mut items: Vec<(&'static str, f64)> = Vec::new();
    if has_filter {
        items.push(("filter", 300.0));
    }
    items.push(("nozzle", 500.0));
    items.push(("rails", 100.0));
    let rows: Vec<(String, f32, f32)> = items
        .into_iter()
        .map(|(key, every)| {
            let since = saved[format!("service_{key}")].as_f64().unwrap_or(0.0);
            (
                key.to_owned(),
                (hours - since).max(0.0) as f32,
                every as f32,
            )
        })
        .collect();
    let _ = ui.upgrade_in_event_loop(move |ui| {
        let items: Vec<crate::ServiceItem> = rows
            .into_iter()
            .map(|(key, used, every)| crate::ServiceItem {
                key: key.into(),
                title: "".into(),
                used,
                every,
            })
            .collect();
        ui.set_service_items(slint::ModelRc::new(slint::VecModel::from(items)));
    });
}

/// Samples the graph shows: Moonraker keeps one a second.
const GRAPH_SECONDS: usize = 600;
const GRAPH_POINTS: usize = 200;

/// Nozzle, bed (and a chamber, MCU or host sensor when there is one) as lines
/// over the last ten minutes, with a scale that fits them.
// Plain tuples between the reading and the UI thread (images and colours are built there).
#[allow(clippy::type_complexity)]
fn show_graph(ui: &slint::Weak<AppWindow>, store: &Value, aspect: f64) {
    // The Path's viewbox is 1000 high and as wide as the plot's proportions.
    let width = 1000.0 * aspect.clamp(0.5, 5.0);
    let series = |key: &str, field: &str| -> Vec<f64> {
        let all: Vec<f64> = store[key][field]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_f64).collect())
            .unwrap_or_default();
        all[all.len().saturating_sub(GRAPH_SECONDS)..].to_vec()
    };
    let mut picked: Vec<(String, &'static str, [u8; 3])> = vec![
        ("extruder".into(), "nozzle", [0xf9, 0x73, 0x16]),
        ("heater_bed".into(), "bed", [0x60, 0xa5, 0xfa]),
    ];
    if let Some(obj) = store.as_object() {
        if let Some(extra) = obj.keys().find(|k| {
            let k = k.to_ascii_lowercase();
            k.starts_with("temperature_sensor ")
                && ["chamber", "mcu", "host"].iter().any(|w| k.contains(w))
        }) {
            picked.push((extra.clone(), "", [0x98, 0xa1, 0xac]));
        }
    }
    let lines: Vec<(Vec<f64>, f64, String, &'static str, [u8; 3])> = picked
        .into_iter()
        .filter_map(|(key, label, rgb)| {
            let temps = series(&key, "temperatures");
            let target = series(&key, "targets").last().copied().unwrap_or(0.0);
            (!temps.is_empty()).then(|| {
                let name = key
                    .strip_prefix("temperature_sensor ")
                    .unwrap_or(&key)
                    .replace('_', " ");
                (temps, target, name, label, rgb)
            })
        })
        .collect();
    let top = lines
        .iter()
        .flat_map(|l| l.0.iter().copied().chain(std::iter::once(l.1)))
        .fold(50.0_f64, f64::max);
    // Round the top up to a step that gives three or four gridlines.
    let step = if top > 200.0 {
        60.0
    } else if top > 100.0 {
        40.0
    } else {
        20.0
    };
    let top = (top / step).ceil() * step + step / 2.0;
    let paths: Vec<(String, [u8; 3], f64, f64, String, &'static str)> = lines
        .into_iter()
        .map(|(temps, target, name, label, rgb)| {
            let n = temps.len();
            let stride = (n / GRAPH_POINTS).max(1);
            let mut d = String::new();
            for (k, i) in (0..n)
                .step_by(stride)
                .chain(std::iter::once(n - 1))
                .enumerate()
            {
                let x = if n > 1 {
                    i as f64 / (n - 1) as f64 * width
                } else {
                    0.0
                };
                let y = (1.0 - temps[i] / top) * 1000.0;
                d.push_str(&format!("{}{x:.1} {y:.1} ", if k == 0 { "M" } else { "L" }));
            }
            (d, rgb, temps[n - 1], target, name, label)
        })
        .collect();
    let grid: Vec<f64> = (1..)
        .map(|k| k as f64 * step)
        .take_while(|v| *v < top)
        .collect();
    let _ = ui.upgrade_in_event_loop(move |ui| {
        let words = slint::ComponentHandle::global::<crate::Strings>(&ui);
        let rows: Vec<crate::GraphLine> = paths
            .into_iter()
            .map(|(path, rgb, now, target, name, label)| {
                let label = match label {
                    "nozzle" => words.get_nozzle().to_string(),
                    "bed" => words.get_bed().to_string(),
                    _ => name,
                };
                let legend = if target > 0.0 {
                    format!("{label} {now:.0}° / {target:.0}°")
                } else {
                    format!("{label} {now:.0}°")
                };
                crate::GraphLine {
                    path: path.into(),
                    color: slint::Color::from_rgb_u8(rgb[0], rgb[1], rgb[2]),
                    legend: legend.into(),
                }
            })
            .collect();
        let grid: Vec<_> = grid
            .into_iter()
            .map(|v| {
                (
                    slint::SharedString::from(format!("{v:.0}°")),
                    (1.0 - v / top) as f32,
                )
            })
            .collect();
        ui.set_graph_lines(slint::ModelRc::new(slint::VecModel::from(rows)));
        ui.set_graph_grid(slint::ModelRc::new(slint::VecModel::from(grid)));
    });
}

/// Calendar days from `then` to `now` in local time (UTC without a timezone).
#[allow(deprecated)]
fn days_between(then: f64, now: f64) -> i32 {
    // Local day number: seconds shifted by the zone's offset, in whole days.
    let day = |t: f64| {
        let secs = t as libc::time_t;
        // SAFETY: localtime_r only writes into the struct we pass.
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        let offset = if unsafe { libc::localtime_r(&secs, &mut tm).is_null() } {
            0
        } else {
            tm.tm_gmtoff as i64
        };
        (t as i64 + offset).div_euclid(86_400)
    };
    (day(now) - day(then)).max(0) as i32
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
            let image = home_preview(&client, &thumbs, &name).await;
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
            ui.set_last_material(l.material.as_str().into());
            ui.set_last_filament(l.filament_name.as_str().into());
            ui.set_last_color_hex(l.color.as_str().into());
            ui.set_last_color(hex_color(&l.color));
            ui.set_last_nozzle_temp(l.nozzle_temp as f32);
            ui.set_last_started(l.started as i32);
            ui.set_last_grams(l.grams.round() as i32);
            ui.set_last_clock(local_clock(l.ended).unwrap_or_default().into());
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
    state.macros = model::macro_names(&available);
    if let Ok(store) = client
        .call_timeout("server.gcode_store", json!({ "count": 100 }), SETUP_TIMEOUT)
        .await
    {
        state.clear_console();
        for (text, kind) in model::parse_gcode_store(&store) {
            state.push_console(text, kind);
        }
    }
    state.leds = available
        .iter()
        .filter(|n| n.starts_with("led "))
        .cloned()
        .collect();
    state.fan_objects = model::fan_object_names(&available);
    state.pins = model::user_pins(&available);
    // Moonraker's components (timelapse, spoolman, power, ...) decide rows too.
    let components: Vec<String> = match client
        .call_timeout("server.info", Value::Null, SETUP_TIMEOUT)
        .await
    {
        Ok(info) => info["components"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    let config = match client
        .call_timeout(
            "printer.objects.query",
            json!({ "objects": { "configfile": ["settings", "config"] } }),
            SETUP_TIMEOUT,
        )
        .await
    {
        Ok(mut r) => r["status"]["configfile"].take(),
        Err(_) => Value::Null,
    };
    let z_on_probe = config["settings"]["stepper_z"]["endstop_pin"]
        .as_str()
        .is_some_and(|pin| pin.contains("probe:"));
    // A pin the printer's own macros switch (AD5M: extruder_heater, by EXTRUDER_ON/OFF)
    // is not the user's to toggle.
    let managed = model::macro_managed_pins(&config["config"]);
    state.pins.retain(|p| {
        !managed
            .iter()
            .any(|m| p.trim_start_matches("output_pin ").eq_ignore_ascii_case(m))
    });
    state.caps = model::Caps::new(&available, &components, z_on_probe);
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
            state.fan_heater_temps = fan_heater_temps(client).await;
        }
        Err(err) => tracing::warn!(%err, "subscribe failed"),
    }
}

/// The temperatures `heater_fan`s switch on at, from the printer's config.
async fn fan_heater_temps(client: &Client) -> std::collections::HashMap<String, f64> {
    let query = json!({ "objects": { "configfile": ["settings"] } });
    match client
        .call_timeout("printer.objects.query", query, SETUP_TIMEOUT)
        .await
    {
        Ok(result) => model::parse_fan_heater_temps(&result["status"]["configfile"]["settings"]),
        Err(_) => Default::default(),
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

/// "24 on the printer · 3 on USB" for the Files card: USB sticks show up as a
/// folder named usb (KlipperMod, Moonraker's usual mount).
fn set_file_counts(ui: &slint::Weak<AppWindow>, files: &[model::FileEntry]) {
    let usb = files
        .iter()
        .filter(|f| f.path.to_ascii_lowercase().starts_with("usb/"))
        .count() as i32;
    let local = files.len() as i32 - usb;
    let _ = ui.upgrade_in_event_loop(move |ui| {
        ui.set_files_local(local);
        ui.set_files_usb(usb);
    });
}

/// Count the files once Klipper is ready, for the Home screen.
async fn list_files(
    client: &Client,
    ui: &slint::Weak<AppWindow>,
    usb: bool,
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
    set_file_counts(ui, &files);
    // Where a file of this storage can be moved: the top and its folders (a USB
    // stick's top is its usb/ folder).
    let mut folders = model::folders(&files, usb);
    if !usb {
        folders.insert(0, String::new());
    }
    let files: Vec<model::FileEntry> = files
        .into_iter()
        .filter(|f| f.path.to_ascii_lowercase().starts_with("usb/") == usb)
        .collect();
    // Plain data here; the UI thread builds the rows (images are not Send).
    let rows: Vec<(String, String, i32, i32)> = files
        .iter()
        .map(|f| {
            let name = crate::files::title(&f.path);
            (
                f.path.clone(),
                name.to_owned(),
                ((now - f.modified).max(0.0) / 86_400.0) as i32,
                (f.size / 1024) as i32,
            )
        })
        .collect();
    let _ = ui.upgrade_in_event_loop(move |ui| {
        ui.set_files_loaded(true);
        let items: Vec<crate::FileItem> = rows
            .into_iter()
            .enumerate()
            .map(|(rank, (path, name, days_ago, size_kb))| crate::FileItem {
                path: path.into(),
                name: name.into(),
                rank: rank as i32,
                days_ago,
                size_kb,
                ..Default::default()
            })
            .collect();
        crate::files::set_all(&ui, items);
        let folders: Vec<slint::SharedString> = folders.into_iter().map(Into::into).collect();
        ui.set_move_folders(slint::ModelRc::new(slint::VecModel::from(folders)));
    });

    // Previews: the grid asks for the ones near the glass (Command::Thumbs).
    Ok(())
}

/// Previews (and print time, material) for these grid files, one by one, as
/// they land. A newer request (the grid scrolled) stops this one.
fn load_thumbs(
    client: &Client,
    ui: &slint::Weak<AppWindow>,
    thumbs: &Arc<Thumbs>,
    paths: Vec<String>,
) {
    let generation = thumbs.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let (client, ui, thumbs) = (client.clone(), ui.clone(), thumbs.clone());
    tokio::spawn(async move {
        for path in paths {
            if thumbs.generation.load(Ordering::SeqCst) != generation {
                return;
            }
            // Metadata first: print time and filament for the card, and where the
            // preview is.
            let meta = client
                .call_timeout(
                    "server.files.metadata",
                    json!({ "filename": path }),
                    SETUP_TIMEOUT,
                )
                .await
                .ok()
                .map(|m| model::parse_file_meta(&m));
            let buf = preview(&client, &thumbs, &path, meta.clone()).await;
            let _ = ui.upgrade_in_event_loop(move |ui| {
                let thumb = buf.map(slint::Image::from_rgba8);
                crate::files::update(&ui, &path, |item| {
                    if let Some(thumb) = &thumb {
                        item.thumb = thumb.clone();
                        item.has_thumb = true;
                    }
                    if let Some(meta) = &meta {
                        item.has_meta = true;
                        item.minutes = meta.estimated_secs.map_or(0, |s| s.div_ceil(60) as i32);
                        item.material = meta.filament_type.clone().unwrap_or_default().into();
                    }
                });
            });
        }
    });
}

/// Networks around, for the Network screen: the connected one is on its own
/// row, so the list has the others, strongest first.
async fn wifi_scan(ui: &slint::Weak<AppWindow>) {
    if !crate::wifi::available() {
        let _ = ui.upgrade_in_event_loop(|ui| ui.set_wifi_available(false));
        return;
    }
    let _ = ui.upgrade_in_event_loop(|ui| {
        ui.set_wifi_available(true);
        ui.set_wifi_scanning(true);
    });
    let view = tokio::task::spawn_blocking(crate::wifi::scan)
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
    let view = match view {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!(%err, "Wi-Fi scan failed");
            let _ = ui.upgrade_in_event_loop(|ui| ui.set_wifi_scanning(false));
            return;
        }
    };
    let nets: Vec<model::WifiNet> = view
        .nets
        .into_iter()
        .filter(|n| Some(&n.ssid) != view.current.as_ref() && n.auth != model::WifiAuth::Other)
        .collect();
    let saved = view.saved;
    let current = view.current.unwrap_or_default();
    let _ = ui.upgrade_in_event_loop(move |ui| {
        ui.set_wifi_current(current.into());
        let words = slint::ComponentHandle::global::<crate::Strings>(&ui);
        let rows: Vec<crate::FormRow> = nets
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let band = if n.freq >= 4900 { "5" } else { "2.4" };
                let security = match n.auth {
                    model::WifiAuth::Open => words.get_open_net().to_string(),
                    model::WifiAuth::Sae => "WPA3".into(),
                    _ => "WPA2".into(),
                };
                let signal = if n.signal >= -60 {
                    words.get_signal_strong()
                } else if n.signal >= -72 {
                    words.get_signal_fair()
                } else {
                    words.get_signal_weak()
                };
                let mut sub = format!(
                    "{} · {security} · {signal}",
                    words.get_ghz().replace("{}", band)
                );
                if saved.contains(&n.ssid) {
                    sub = format!("{sub} · {}", words.get_saved_net());
                }
                crate::FormRow {
                    kind: "link".into(),
                    key: i.to_string().into(),
                    title: n.ssid.clone().into(),
                    sub: sub.into(),
                    shown: true,
                    ..Default::default()
                }
            })
            .collect();
        let model_of = |v: Vec<slint::SharedString>| slint::ModelRc::new(slint::VecModel::from(v));
        ui.set_wifi_ssids(model_of(
            nets.iter().map(|n| n.ssid.clone().into()).collect(),
        ));
        ui.set_wifi_auths(model_of(
            nets.iter()
                .map(|n| match n.auth {
                    model::WifiAuth::Open => "open",
                    model::WifiAuth::Sae => "sae",
                    _ => "psk",
                })
                .map(Into::into)
                .collect(),
        ));
        ui.set_wifi_saved(slint::ModelRc::new(slint::VecModel::from(
            nets.iter()
                .map(|n| saved.contains(&n.ssid))
                .collect::<Vec<bool>>(),
        )));
        ui.set_wifi_rows(slint::ModelRc::new(slint::VecModel::from(rows)));
        ui.set_wifi_scanning(false);
    });
}

/// Moonraker's print queue for the Files button and the Queue screen. A printer
/// without the job_queue component gets no button (count -1).
async fn list_queue(client: &Client, ui: &slint::Weak<AppWindow>) {
    let Ok(result) = client
        .call_timeout("server.job_queue.status", json!({}), SETUP_TIMEOUT)
        .await
    else {
        let _ = ui.upgrade_in_event_loop(|ui| ui.set_queue_count(-1));
        return;
    };
    let queue = model::parse_job_queue(&result);
    // Print time and filament of each job, from the file's metadata.
    let mut jobs = Vec::with_capacity(queue.jobs.len());
    for job in queue.jobs {
        let meta = client
            .call_timeout(
                "server.files.metadata",
                json!({ "filename": job.path }),
                SETUP_TIMEOUT,
            )
            .await
            .ok()
            .map(|m| model::parse_file_meta(&m));
        let minutes = meta
            .as_ref()
            .and_then(|m| m.estimated_secs)
            .map(|s| s.div_ceil(60) as i32);
        let material = meta.and_then(|m| m.filament_type).unwrap_or_default();
        jobs.push((job.path, minutes, material));
    }
    let state = queue.state;
    let _ = ui.upgrade_in_event_loop(move |ui| {
        let format = slint::ComponentHandle::global::<crate::Format>(&ui);
        let total: i32 = jobs.iter().filter_map(|j| j.1).sum();
        let rows: Vec<crate::FormRow> = jobs
            .iter()
            .enumerate()
            .map(|(i, (path, minutes, material))| {
                let sub: Vec<String> = minutes
                    .map(|m| format.invoke_duration(m).to_string())
                    .into_iter()
                    .chain((!material.is_empty()).then(|| material.clone()))
                    .collect();
                crate::FormRow {
                    kind: "link".into(),
                    key: path.as_str().into(),
                    title: format!("{} · {}", i + 1, crate::files::title(path)).into(),
                    sub: sub.join(" · ").into(),
                    shown: true,
                    ..Default::default()
                }
            })
            .collect();
        ui.set_queue_count(rows.len() as i32);
        ui.set_queue_rows(slint::ModelRc::new(slint::VecModel::from(rows)));
        ui.set_queue_state(state.into());
        ui.set_queue_minutes(total);
    });
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
            height: meta.height.map_or(-1.0, |h| h as f32),
            color: meta
                .filament_color
                .as_deref()
                .map(hex_color)
                .unwrap_or_default(),
            has_color: meta.filament_color.is_some(),
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

/// "15:20" in the printer's timezone, or nothing when no timezone is set: a clock
/// that silently shows UTC would be wrong (KlipperMod points /etc/localtime at
/// printer_data/config/timezone, which exists once a timezone is chosen).
// libc::time_t is deprecated in name only (it widens in a later musl): it is the
// type localtime_r takes here.
#[allow(deprecated)]
fn local_clock(unix: f64) -> Option<String> {
    if std::fs::metadata("/etc/localtime").is_err() {
        return None;
    }
    let t = unix as libc::time_t;
    // SAFETY: localtime_r only writes into the struct we pass.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let ok = unsafe { !libc::localtime_r(&t, &mut tm).is_null() };
    ok.then(|| {
        if prefs::get(&prefs::CLOCK_12) {
            let h = (tm.tm_hour + 11) % 12 + 1;
            format!(
                "{h}:{:02} {}",
                tm.tm_min,
                if tm.tm_hour < 12 { "AM" } else { "PM" }
            )
        } else {
            format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)
        }
    })
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
        Command::SetFan { object, percent } => {
            state
                .apply_status(json!({ object.as_str(): { "speed": f64::from(*percent) / 100.0 } }));
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
        Command::SetLight { name, level, white } => {
            let level = f64::from(*level);
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

/// How fast a heater is rising, to say how long until its target.
#[derive(Default)]
struct HeatRate {
    /// Last sample, taken at most once a second.
    last: Option<(Instant, f64)>,
    /// Smoothed rise, °C per second.
    rate: f64,
    /// What the screen shows now, seconds; -1 = nothing.
    shown: i32,
}

impl HeatRate {
    /// Seconds until `heater` reaches its target, or -1 when it is not heating
    /// or the rise is too small or too new to say.
    fn eta(&mut self, now: Instant, heater: &model::Heater) -> i32 {
        let t = heater.temperature;
        match self.last {
            Some((at, prev)) => {
                let dt = now.duration_since(at).as_secs_f64();
                if dt < 1.0 {
                    return self.shown;
                }
                let r = (t - prev) / dt;
                // Smooth over a few seconds; a fresh start takes the first rate.
                self.rate = if self.rate == 0.0 {
                    r
                } else {
                    self.rate * 0.7 + r * 0.3
                };
                self.last = Some((now, t));
            }
            None => {
                self.last = Some((now, t));
                self.shown = -1;
                return -1;
            }
        }
        let left = heater.target - t;
        if heater.target <= 0.0 || left <= 2.0 || self.rate < 0.2 {
            return -1;
        }
        // Nearer seconds than the noise suggests are not worth showing.
        ((left / self.rate).round() as i32).clamp(1, 3600)
    }
}

/// Rivet's settings live in Moonraker's database under this namespace.
const DB_NAMESPACE: &str = "rivet";

/// Settings the engine itself follows (set from the database and the screens).
pub mod prefs {
    use std::sync::atomic::{AtomicBool, Ordering};
    /// Keep the screen on while a print runs.
    pub static AWAKE_PRINT: AtomicBool = AtomicBool::new(true);
    /// Clocks as 12-hour "3:20 PM".
    pub static CLOCK_12: AtomicBool = AtomicBool::new(false);
    /// A tune on the beeper when a print completes.
    pub static END_TUNE: AtomicBool = AtomicBool::new(true);
    pub fn set(flag: &AtomicBool, on: bool) {
        flag.store(on, Ordering::Relaxed);
    }
    pub fn get(flag: &AtomicBool) -> bool {
        flag.load(Ordering::Relaxed)
    }
}

/// Read Rivet's saved settings once Klipper is ready, show them, and act on
/// "turn the light on at startup".
async fn load_settings(
    client: Client,
    ui: slint::Weak<AppWindow>,
    light: Option<model::Light>,
    caps: model::Caps,
    internal: mpsc::UnboundedSender<Internal>,
) {
    let saved = match client
        .call_timeout(
            "server.database.get_item",
            json!({ "namespace": DB_NAMESPACE }),
            SETUP_TIMEOUT,
        )
        .await
    {
        Ok(mut item) => item["value"].take(),
        // Nothing saved yet: the namespace does not exist.
        Err(err) => {
            tracing::debug!(%err, "no saved settings");
            Value::Null
        }
    };
    let flag = |key: &str| saved[key].as_bool().unwrap_or(false);
    let number = |key: &str, default: f64| saved[key].as_f64().unwrap_or(default) as f32;
    let light_on = flag("light_on_start");
    // Minutes without a touch before the screen sleeps; 0 = never.
    #[cfg(feature = "fbdev")]
    {
        let minutes = saved["screen_sleep_min"].as_f64().unwrap_or(10.0);
        fbdev_platform::screen::set_sleep_after(
            (minutes > 0.0).then(|| Duration::from_secs_f64(minutes * 60.0)),
        );
        // The backlight as last set from the Screen settings.
        if let Some(percent) = saved["brightness"].as_f64() {
            fbdev_platform::screen::set_brightness(percent as u32);
        }
    }
    // The theme picked in Settings › Screen › Appearance.
    let (accent, base) = (
        saved["theme_accent"].as_u64().unwrap_or(0) as usize,
        saved["theme_base"].as_u64().unwrap_or(0) as usize,
    );
    let own = |key: &str| {
        saved[key]
            .as_str()
            .and_then(|h| u32::from_str_radix(h, 16).ok())
    };
    let (own_accent, own_base) = (own("theme_accent_rgb"), own("theme_base_rgb"));
    let _ = ui.upgrade_in_event_loop(move |ui| {
        crate::theme::set_own(own_accent, own_base);
        crate::theme::apply(&ui, accent, base);
    });
    let brightness = saved["brightness"].as_f64().unwrap_or(75.0) as f32;
    let _ = ui.upgrade_in_event_loop(move |ui| ui.set_set_brightness(brightness));
    let (invert, jog_xy, jog_z) = (
        [flag("invert_x"), flag("invert_y"), flag("invert_z")],
        number("jog_xy_speed", 100.0),
        number("jog_z_speed", 10.0),
    );
    // Heat presets the user changed; the built-in ones otherwise.
    let presets: Vec<(String, i32, i32)> = saved["heat_presets"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|p| {
            Some((
                p["name"].as_str()?.to_owned(),
                p["nozzle"].as_i64()? as i32,
                p["bed"].as_i64()? as i32,
            ))
        })
        .collect();
    let text = |key: &str| saved[key].as_str().unwrap_or_default().to_owned();
    let pin = text("pin");
    let sleep = saved["screen_sleep_min"].as_f64().unwrap_or(10.0) as i32;
    let autolock = saved["autolock_min"].as_f64().unwrap_or(0.0) as i32;
    let on = |key: &str, default: bool| saved[key].as_bool().unwrap_or(default);
    let (awake, lock_print, sounds, tune, clock12, hold, confirm) = (
        on("awake_print", true),
        on("lock_print", false),
        on("sounds", true),
        on("end_tune", true),
        on("clock_12h", false),
        on("hold_stop", false),
        on("confirm_macros", true),
    );
    prefs::set(&prefs::AWAKE_PRINT, awake);
    prefs::set(&prefs::CLOCK_12, clock12);
    prefs::set(&prefs::END_TUNE, sounds && tune);
    let language = text("language");
    let _ = ui.upgrade_in_event_loop(move |ui| {
        ui.set_set_sleep(
            [1, 5, 10, 30]
                .iter()
                .position(|m| *m == sleep)
                .map_or(4, |i| i as i32),
        );
        ui.set_set_autolock(
            [0, 5, 15, 30]
                .iter()
                .position(|m| *m == autolock)
                .unwrap_or(0) as i32,
        );
        ui.set_set_awake(awake);
        ui.set_set_lock_print(lock_print);
        ui.set_set_pin(pin.into());
        ui.set_set_sounds(sounds);
        ui.set_set_tune(tune);
        ui.set_set_clock(i32::from(clock12));
        ui.set_set_hold_stop(hold);
        ui.set_set_confirm_macros(confirm);
        // A language picked on the screen wins over the start-up one.
        if language == "ru" || language == "en" {
            crate::apply_language(&language);
            ui.set_russian(language == "ru");
        }
        if !presets.is_empty() {
            let rows: Vec<crate::HeatPreset> = presets
                .into_iter()
                .map(|(name, nozzle, bed)| crate::HeatPreset {
                    name: name.into(),
                    nozzle,
                    bed,
                })
                .collect();
            ui.set_heat_presets(slint::ModelRc::new(slint::VecModel::from(rows)));
        }
        ui.set_light_on_start(light_on);
        ui.set_invert_x(invert[0]);
        ui.set_invert_y(invert[1]);
        ui.set_invert_z(invert[2]);
        ui.set_jog_xy_speed(jog_xy);
        ui.set_jog_z_speed(jog_z);
    });
    // A drying session survives a restart of Rivet; a finished one switches the bed off.
    if let (Some(until), Some(temp)) = (saved["dryer_until"].as_f64(), saved["dryer_temp"].as_f64())
    {
        let end = UNIX_EPOCH + Duration::from_secs_f64(until.max(0.0));
        if end > SystemTime::now() {
            let _ = internal.send(Internal::ResumeDryer(end, temp as f32));
        } else {
            let _ = client.gcode("M140 S0").await;
            save_dryer(&client, None);
        }
    }
    // The whole list now, not just the counts: Moonraker takes about a second to
    // list files on this printer, and Files then opens with them already there.
    if let Err(err) = list_files(&client, &ui, false).await {
        tracing::debug!(%err, "file list at start");
    }
    if caps.spoolman {
        if let Err(err) = list_spools(&client, &ui).await {
            tracing::warn!(%err, "spoolman");
        }
    }
    // What the filament wizard last loaded (when there is no Spoolman).
    let loaded = saved["loaded_material"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let loaded_color = saved["loaded_color"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let loaded_at = saved["loaded_at"].as_f64().unwrap_or(0.0);
    let _ = ui.upgrade_in_event_loop(move |ui| {
        ui.set_loaded_material(loaded.into());
        ui.set_loaded_color(hex_color(&loaded_color));
        ui.set_loaded_at(loaded_at as i32);
    });
    if caps.timelapse {
        if let Ok(t) = client
            .call_timeout("machine.timelapse.get_settings", Value::Null, SETUP_TIMEOUT)
            .await
        {
            let on = t["enabled"].as_bool().unwrap_or(false);
            let _ = ui.upgrade_in_event_loop(move |ui| ui.set_timelapse_on(on));
        }
    }
    if caps.power_devices {
        if let Err(err) = list_power(&client, &ui).await {
            tracing::warn!(%err, "power devices");
        }
    }
    if let Some(light) = light.filter(|l| light_on && !l.on) {
        let cmd = Command::SetLight {
            name: light.name.clone(),
            level: 1.0,
            white: light.white,
        };
        if let Some(code) = gcode_for(&cmd) {
            if let Err(err) = client.gcode(&code).await {
                tracing::warn!(%err, "light on at startup failed");
            }
        }
    }
}

/// "4466cc" or "#4466CC" as a colour; grey when there is none.
fn hex_color(hex: &str) -> slint::Color {
    let hex = hex.trim_start_matches('#');
    match u32::from_str_radix(hex.get(..6).unwrap_or(""), 16) {
        Ok(v) => slint::Color::from_rgb_u8((v >> 16) as u8, (v >> 8) as u8, v as u8),
        Err(_) => slint::Color::from_rgb_u8(0x4a, 0x52, 0x5c),
    }
}

/// Send G-code without waiting; a failure only goes to the log.
fn spawn_gcode(client: &Client, code: String) {
    let client = client.clone();
    tokio::spawn(async move {
        if let Err(err) = client.gcode(&code).await {
            tracing::warn!(%err, %code, "G-code failed");
        }
    });
}

/// Keep the dryer's end in Moonraker, so a restart of Rivet still ends it.
fn save_dryer(client: &Client, dryer: Option<(SystemTime, f32)>) {
    let client = client.clone();
    tokio::spawn(async move {
        let result = match dryer {
            Some((end, temp)) => {
                let until = end
                    .duration_since(UNIX_EPOCH)
                    .map_or(0.0, |d| d.as_secs_f64());
                let item = |key: &str, value: Value| json!({ "namespace": DB_NAMESPACE, "key": key, "value": value });
                let a = client
                    .call(
                        "server.database.post_item",
                        item("dryer_until", json!(until)),
                    )
                    .await;
                let b = client
                    .call("server.database.post_item", item("dryer_temp", json!(temp)))
                    .await;
                a.and(b).map(drop)
            }
            None => client
                .call(
                    "server.database.delete_item",
                    json!({ "namespace": DB_NAMESPACE, "key": "dryer_until" }),
                )
                .await
                .map(drop),
        };
        if let Err(err) = result {
            tracing::debug!(%err, "dryer state not saved");
        }
    });
}

/// Moonraker's power devices as toggle rows.
async fn list_power(client: &Client, ui: &slint::Weak<AppWindow>) -> Result<(), moonraker::Error> {
    let list = client
        .call("machine.device_power.devices", Value::Null)
        .await?;
    let devices: Vec<(String, bool)> = list["devices"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|d| {
                    Some((
                        d["device"].as_str()?.to_owned(),
                        d["status"].as_str() == Some("on"),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let _ = ui.upgrade_in_event_loop(move |ui| {
        let rows: Vec<crate::FormRow> = devices
            .into_iter()
            .map(|(name, on)| crate::FormRow {
                kind: "toggle".into(),
                key: format!("power:{name}").into(),
                title: pretty_name(&name).into(),
                on,
                shown: true,
                ..Default::default()
            })
            .collect();
        ui.set_power_rows(slint::ModelRc::new(slint::VecModel::from(rows)));
    });
    Ok(())
}

/// Spoolman's spools as rows, the active one marked; its name for the Extruder row.
async fn list_spools(client: &Client, ui: &slint::Weak<AppWindow>) -> Result<(), moonraker::Error> {
    let active = client
        .call("server.spoolman.get_spool_id", Value::Null)
        .await?["spool_id"]
        .as_i64()
        .unwrap_or(0);
    let spools = client
        .call(
            "server.spoolman.proxy",
            json!({ "request_method": "GET", "path": "/v1/spool", "use_v2_response": true }),
        )
        .await?;
    let active_color = spools["response"]
        .as_array()
        .and_then(|a| a.iter().find(|s| s["id"].as_i64() == Some(active)))
        .and_then(|s| s["filament"]["color_hex"].as_str())
        .unwrap_or_default()
        .to_owned();
    let active_material = spools["response"]
        .as_array()
        .and_then(|a| a.iter().find(|s| s["id"].as_i64() == Some(active)))
        .and_then(|s| s["filament"]["material"].as_str())
        .unwrap_or_default()
        .to_owned();
    let items: Vec<(i64, String, String, f64)> = spools["response"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|s| !s["archived"].as_bool().unwrap_or(false))
                .map(|s| {
                    let f = &s["filament"];
                    let title = [f["material"].as_str(), f["name"].as_str()]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join(" · ");
                    let vendor = f["vendor"]["name"].as_str().unwrap_or_default().to_owned();
                    let grams = s["remaining_weight"].as_f64().unwrap_or(0.0);
                    (s["id"].as_i64().unwrap_or(0), title, vendor, grams)
                })
                .collect()
        })
        .unwrap_or_default();
    let _ = ui.upgrade_in_event_loop(move |ui| {
        use slint::ComponentHandle;
        let text = ui.global::<crate::Strings>();
        let grams = |g: f64| text.get_grams().replace("{}", &format!("{g:.0}"));
        let mut active_name = String::new();
        let rows: Vec<crate::FormRow> = items
            .into_iter()
            .map(|(id, title, vendor, left)| {
                let sub = [vendor, grams(left), format!("#{id}")]
                    .into_iter()
                    .filter(|p| !p.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · ");
                if id == active {
                    active_name = format!("{title} · {}", grams(left));
                }
                crate::FormRow {
                    kind: "link".into(),
                    key: format!("spool:{id}").into(),
                    title: title.into(),
                    sub: sub.into(),
                    value: if id == active {
                        text.get_active()
                    } else {
                        "".into()
                    },
                    shown: true,
                    ..Default::default()
                }
            })
            .collect();
        ui.set_spool_rows(slint::ModelRc::new(slint::VecModel::from(rows)));
        ui.set_active_spool(active_name.into());
        ui.set_has_active_spool(active > 0);
        ui.set_spool_material(active_material.into());
        ui.set_spool_color(hex_color(&active_color));
    });
    Ok(())
}

/// Read the current values back after a command failed.
async fn resync_status(client: &Client, state: &mut PrinterState) {
    let mut objects: Vec<String> = state.leds.clone();
    objects.extend(state.fan_objects.iter().cloned());
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
        Command::Move { axis, delta, speed } => {
            let letter = ["X", "Y", "Z"][(*axis).min(2)];
            let feed = speed * 60.0;
            // Relative for this move only; the printer's modes come back after.
            format!(
                "SAVE_GCODE_STATE NAME=RIVET\nG91\nG1 {letter}{delta:.3} F{feed:.0}\nRESTORE_GCODE_STATE NAME=RIVET"
            )
        }
        Command::GoTo { x, y, z, speed } => format!(
            "SAVE_GCODE_STATE NAME=RIVET\nG90\nG1 X{x:.2} Y{y:.2} Z{z:.2} F{:.0}\nRESTORE_GCODE_STATE NAME=RIVET",
            speed * 60.0
        ),
        Command::MotorsOff => "M84".to_owned(),
        Command::PauseAtLayer(n) if *n > 0 => format!("SET_PAUSE_AT_LAYER LAYER={n}"),
        Command::PauseAtLayer(_) => "SET_PAUSE_AT_LAYER ENABLE=0".to_owned(),
        Command::PauseNextLayer(on) => format!("SET_PAUSE_NEXT_LAYER ENABLE={}", u8::from(*on)),
        Command::SetRetraction { key, value } => {
            let param = match key.as_str() {
                "retract-length" => "RETRACT_LENGTH",
                "retract-speed" => "RETRACT_SPEED",
                "unretract-extra" => "UNRETRACT_EXTRA_LENGTH",
                "unretract-speed" => "UNRETRACT_SPEED",
                _ => return None,
            };
            format!("SET_RETRACTION {param}={value:.2}")
        }
        Command::SetPin { name, on } => format!("SET_PIN PIN={name} VALUE={}", u8::from(*on)),
        Command::Extrude {
            length,
            speed,
            heat,
        } => format!(
            "{}SAVE_GCODE_STATE NAME=RIVET\nM83\nG1 E{length:.1} F{:.0}\nRESTORE_GCODE_STATE NAME=RIVET",
            if *heat > 0.0 {
                // Heat and wait, then move the filament.
                format!("M109 S{heat:.0}\n")
            } else {
                String::new()
            },
            speed * 60.0
        ),
        Command::SetFan { object, percent } => match object.split_once(' ') {
            // `fan_generic NAME` is set by name; the part fan takes M106.
            Some((_, name)) => format!(
                "SET_FAN_SPEED FAN={name} SPEED={:.2}",
                (percent / 100.0).clamp(0.0, 1.0)
            ),
            None => format!("M106 S{:.0}", percent * 2.55),
        },
        Command::SetAir(mode) => match mode {
            1 => "AIR_CIRCULATION_INTERNAL",
            2 => "AIR_CIRCULATION_EXTERNAL",
            _ => "AIR_CIRCULATION_STOP",
        }
        .to_owned(),
        Command::SetBed(t) => format!("M140 S{t:.0}"),
        Command::AdjustZ(d) => format!("SET_GCODE_OFFSET Z_ADJUST={d:.3} MOVE=1"),
        Command::SetPressureAdvance(v) => format!("SET_PRESSURE_ADVANCE ADVANCE={v:.3}"),
        Command::SetLight { name, level, white } => {
            let v = level.clamp(0.0, 1.0);
            if *white {
                format!("SET_LED LED={name} WHITE={v:.2} TRANSMIT=1")
            } else {
                format!("SET_LED LED={name} RED={v:.2} GREEN={v:.2} BLUE={v:.2} TRANSMIT=1")
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
    // Heating first takes a while: the button shows it and takes no more taps.
    if let Command::Extrude { heat, .. } = cmd {
        if *heat > 0.0 {
            let code = gcode_for(cmd).unwrap_or_default();
            let _ = ui.upgrade_in_event_loop(|ui| ui.set_extrude_busy(true));
            let result = client.gcode(&code).await;
            let _ = ui.upgrade_in_event_loop(|ui| ui.set_extrude_busy(false));
            return result;
        }
    }
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
        Command::Gcode(script) => client.gcode(script).await,
        Command::SetTimelapse(on) => {
            client
                .call("machine.timelapse.post_settings", json!({ "enabled": on }))
                .await?;
            let on = *on;
            let _ = ui.upgrade_in_event_loop(move |ui| ui.set_timelapse_on(on));
            Ok(())
        }
        Command::SetPower { device, on } => {
            client
                .call(
                    "machine.device_power.post_device",
                    json!({ "device": device, "action": if *on { "on" } else { "off" } }),
                )
                .await?;
            list_power(client, ui).await
        }
        Command::ListSpools => list_spools(client, ui).await,
        Command::SetActiveSpool(id) => {
            let id = if *id > 0 { json!(id) } else { Value::Null };
            client
                .call("server.spoolman.post_spool_id", json!({ "spool_id": id }))
                .await?;
            list_spools(client, ui).await
        }
        Command::OffAfterPrint(_)
        | Command::SaveZOffset
        | Command::Dry { .. }
        | Command::RetryNow => {
            unreachable!("handled in the loop")
        }
        Command::SaveSetting { key, value } => client
            .call(
                "server.database.post_item",
                json!({ "namespace": DB_NAMESPACE, "key": key, "value": value }),
            )
            .await
            .map(drop),
        Command::ClearConsole => Ok(()),
        Command::ListFiles { usb } => {
            list_queue(client, ui).await;
            list_files(client, ui, *usb).await
        }
        Command::MoveFile { from, to } => {
            client
                .call(
                    "server.files.move",
                    json!({ "source": format!("gcodes/{from}"), "dest": format!("gcodes/{to}") }),
                )
                .await?;
            let usb = from.to_ascii_lowercase().starts_with("usb/");
            list_files(client, ui, usb).await
        }
        Command::CopyFile { from, to } => {
            client
                .call(
                    "server.files.copy",
                    json!({ "source": format!("gcodes/{from}"), "dest": format!("gcodes/{to}") }),
                )
                .await?;
            let usb = from.to_ascii_lowercase().starts_with("usb/");
            list_files(client, ui, usb).await
        }
        Command::Calibrate(script) => {
            let shaper = script.contains("SHAPER_CALIBRATE");
            let resonance = shaper || script.contains("TEST_RESONANCES");
            let _ = ui.upgrade_in_event_loop(|ui| ui.set_calib_busy(true));
            let result = gcode(client, script).await;
            if resonance {
                crate::resonance::load();
                crate::resonance::load_latest(std::path::Path::new(KLIPPER_TMP));
                crate::resonance::show(ui);
            }
            let fitted = shaper && result.is_ok() && crate::resonance::has_shapers();
            let _ = ui.upgrade_in_event_loop(move |ui| {
                ui.set_calib_busy(false);
                if fitted {
                    // A new measurement: start from the recommended shapers.
                    ui.set_shaper_pick_x(0);
                    ui.set_shaper_pick_y(0);
                    ui.set_shaper_axis(0);
                    if ui.get_screen() == "shaper" {
                        ui.set_screen("shaper-result".into());
                    }
                }
            });
            calib_info(client, ui).await;
            result
        }
        Command::MacroParams(name) => {
            let gcode = client
                .call_timeout(
                    "printer.objects.query",
                    json!({ "objects": { "configfile": ["settings"] } }),
                    SETUP_TIMEOUT,
                )
                .await
                .ok()
                .and_then(|r| {
                    r["status"]["configfile"]["settings"]
                        [format!("gcode_macro {}", name.to_lowercase())]["gcode"]
                        .as_str()
                        .map(str::to_owned)
                })
                .unwrap_or_default();
            let params = model::macro_params(&gcode);
            let name = name.clone();
            let _ = ui.upgrade_in_event_loop(move |ui| {
                if params.is_empty() {
                    if ui.get_set_confirm_macros() {
                        ui.set_overlay("run-macro".into());
                    } else {
                        ui.invoke_run_gcode(name.into());
                    }
                    return;
                }
                let rows: Vec<crate::MacroParam> = params
                    .into_iter()
                    .map(|(n, d)| crate::MacroParam {
                        name: n.into(),
                        value: "".into(),
                        default: d.into(),
                    })
                    .collect();
                ui.set_macro_params(slint::ModelRc::new(slint::VecModel::from(rows)));
                ui.set_screen("macro-params".into());
            });
            Ok(())
        }
        Command::WifiScan => {
            wifi_scan(ui).await;
            Ok(())
        }
        Command::WifiJoin(ssid, auth, password) => {
            let _ = ui.upgrade_in_event_loop(|ui| {
                ui.set_join_busy(true);
                ui.set_join_error("".into());
            });
            let (s, p) = (ssid.clone(), password.clone());
            let kind = match auth.as_str() {
                "open" => model::WifiAuth::Open,
                "sae" => model::WifiAuth::Sae,
                _ => model::WifiAuth::Psk,
            };
            let result = tokio::task::spawn_blocking(move || crate::wifi::join(&s, kind, &p))
                .await
                .unwrap_or_else(|e| Err(e.to_string()));
            let ok = result.is_ok();
            if let Err(err) = &result {
                tracing::warn!(%err, ssid, "Wi-Fi join failed");
            }
            let name = ssid.clone();
            let _ = ui.upgrade_in_event_loop(move |ui| {
                ui.set_join_busy(false);
                if ok {
                    ui.set_join_password("".into());
                    if ui.get_screen() == "wifi-join" {
                        ui.set_screen("set-network".into());
                    }
                } else {
                    let words = slint::ComponentHandle::global::<crate::Strings>(&ui);
                    ui.set_join_error(words.get_join_failed().replace("{}", &name).into());
                    // A tap on a saved network failed: let the password be typed.
                    ui.set_screen("wifi-join".into());
                }
            });
            if ok {
                // The new address comes with the lease; give DHCP a moment.
                tokio::time::sleep(Duration::from_secs(4)).await;
                system_info(client, ui).await;
            }
            wifi_scan(ui).await;
            Ok(())
        }
        Command::Thumbs(paths) => {
            load_thumbs(client, ui, thumbs, paths.clone());
            Ok(())
        }
        Command::DrawResonance(aspect) => {
            crate::resonance::set_aspect(f64::from(*aspect));
            crate::resonance::show(ui);
            Ok(())
        }
        Command::SystemInfo => {
            system_info(client, ui).await;
            Ok(())
        }
        Command::SystemAction(what) => {
            match what.as_str() {
                "klipper" => client.call("printer.restart", json!({})).await.map(drop)?,
                "firmware" => client
                    .call("printer.firmware_restart", json!({}))
                    .await
                    .map(drop)?,
                "moonraker" => client.call("server.restart", json!({})).await.map(drop)?,
                "reboot" => client.call("machine.reboot", json!({})).await.map(drop)?,
                "shutdown" => client.call("machine.shutdown", json!({})).await.map(drop)?,
                "stock" => gcode(client, "REBOOT_STOCK_SYSTEM").await?,
                "update-rivet" => {
                    let _ = ui.upgrade_in_event_loop(|ui| {
                        let words = slint::ComponentHandle::global::<crate::Strings>(&ui);
                        ui.set_rivet_update_sub(words.get_rivet_updating());
                    });
                    // Moonraker downloads the release into /opt/rivet; it answers
                    // when done, and refuses while printing.
                    let done = client
                        .call_timeout(
                            "machine.update.client",
                            json!({ "name": "rivet" }),
                            Duration::from_secs(600),
                        )
                        .await;
                    if let Err(err) = done {
                        system_info(client, ui).await;
                        return Err(err);
                    }
                    restart_for_update();
                }
                "check-updates" => {
                    client
                        .call_timeout(
                            "machine.update.refresh",
                            json!({}),
                            Duration::from_secs(120),
                        )
                        .await
                        .map(drop)?;
                    system_info(client, ui).await;
                }
                "reset" => {
                    let keys = client
                        .call_timeout(
                            "server.database.get_item",
                            json!({ "namespace": DB_NAMESPACE }),
                            SETUP_TIMEOUT,
                        )
                        .await
                        .ok()
                        .and_then(|v| {
                            v["value"]
                                .as_object()
                                .map(|o| o.keys().cloned().collect::<Vec<_>>())
                        })
                        .unwrap_or_default();
                    for key in keys {
                        client
                            .call(
                                "server.database.delete_item",
                                json!({ "namespace": DB_NAMESPACE, "key": key }),
                            )
                            .await?;
                    }
                }
                _ => {}
            }
            Ok(())
        }
        Command::CalibInfo => {
            calib_info(client, ui).await;
            crate::resonance::load_latest(std::path::Path::new(KLIPPER_TMP));
            crate::resonance::show(ui);
            Ok(())
        }
        Command::SetLimit { key, value } => {
            let arg = match key.as_str() {
                "velocity" => format!("VELOCITY={value:.0}"),
                "accel" => format!("ACCEL={value:.0}"),
                "corner" => format!("SQUARE_CORNER_VELOCITY={value:.1}"),
                _ => format!("MINIMUM_CRUISE_RATIO={:.2}", value.clamp(0.0, 0.99)),
            };
            gcode(client, &format!("SET_VELOCITY_LIMIT {arg}")).await
        }
        Command::SetRunout(on) => {
            let name = runout_name(client)
                .await
                .unwrap_or_else(|| "runout_sensor".into());
            gcode(
                client,
                &format!("SET_FILAMENT_SENSOR SENSOR={name} ENABLE={}", u8::from(*on)),
            )
            .await
        }
        Command::ListHistory => {
            list_history(client, ui).await;
            Ok(())
        }
        Command::ClearHistory => {
            client
                .call("server.history.delete_job", json!({ "all": true }))
                .await?;
            list_history(client, ui).await;
            Ok(())
        }
        Command::ListService => {
            list_service(client, ui).await;
            Ok(())
        }
        Command::ServiceDone(key) => {
            let hours = print_hours(client).await;
            client
                .call(
                    "server.database.post_item",
                    json!({ "namespace": DB_NAMESPACE, "key": format!("service_{key}"), "value": hours }),
                )
                .await?;
            list_service(client, ui).await;
            Ok(())
        }
        Command::ListCameras => {
            let cams = client
                .call_timeout("server.webcams.list", json!({}), SETUP_TIMEOUT)
                .await
                .unwrap_or(Value::Null);
            let names: Vec<String> = cams["webcams"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default()
                .iter()
                .filter(|w| w["enabled"].as_bool() != Some(false))
                .filter_map(|w| w["name"].as_str().map(str::to_owned))
                .collect();
            let _ = ui.upgrade_in_event_loop(move |ui| {
                let rows: Vec<slint::SharedString> = names.into_iter().map(Into::into).collect();
                ui.set_cameras(slint::ModelRc::new(slint::VecModel::from(rows)));
            });
            Ok(())
        }
        Command::TempGraph(aspect) => {
            let store = client
                .call_timeout(
                    "server.temperature_store",
                    json!({ "include_monitors": false }),
                    SETUP_TIMEOUT,
                )
                .await?;
            show_graph(ui, &store, f64::from(*aspect));
            Ok(())
        }
        Command::ListMessages => {
            list_messages(client, ui).await;
            Ok(())
        }
        Command::ReadMessages => {
            client
                .call(
                    "server.database.post_item",
                    json!({ "namespace": DB_NAMESPACE, "key": "messages_read", "value": unix_now() }),
                )
                .await?;
            list_messages(client, ui).await;
            Ok(())
        }
        Command::ListQueue => {
            list_queue(client, ui).await;
            Ok(())
        }
        Command::QueueAdd(paths) => {
            client
                .call("server.job_queue.post_job", json!({ "filenames": paths }))
                .await?;
            list_queue(client, ui).await;
            Ok(())
        }
        Command::QueueRun(start) => {
            let method = if *start {
                "server.job_queue.start"
            } else {
                "server.job_queue.pause"
            };
            client.call(method, json!({})).await?;
            list_queue(client, ui).await;
            Ok(())
        }
        Command::QueueClear => {
            client
                .call("server.job_queue.delete_job", json!({ "all": true }))
                .await?;
            list_queue(client, ui).await;
            Ok(())
        }
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
            // Back to the storage the files were deleted from.
            let usb = paths
                .first()
                .is_some_and(|p| p.to_ascii_lowercase().starts_with("usb/"));
            list_files(client, ui, usb).await
        }
        Command::SetSpeed(_)
        | Command::SetFlow(_)
        | Command::SetFan { .. }
        | Command::SetAir(_)
        | Command::Move { .. }
        | Command::GoTo { .. }
        | Command::PauseAtLayer(_)
        | Command::PauseNextLayer(_)
        | Command::SetRetraction { .. }
        | Command::SetPin { .. }
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

/// Put the fans on the screen: the part fan and the first settable one get a
/// bar of their own, further settable fans are rows (how the prototype lays
/// the screen out), automatic ones are listed read-only. With chamber air modes
/// (AD5M Pro), the inside and outside fans belong to those modes, as in the
/// prototype: no rows of their own.
fn set_fans(ui: &AppWindow, fans: &[model::Fan], air_modes: bool) {
    let first_generic = fans.iter().position(|f| f.kind == model::FanKind::Generic);
    let row = |f: &model::Fan| {
        let (trigger, value) = match f.trigger {
            model::FanTrigger::None => (0, 0.0),
            model::FanTrigger::HeaterAbove(t) => (1, t),
            model::FanTrigger::Motors => (2, 0.0),
            model::FanTrigger::Holds(t) => (3, t),
        };
        crate::FanRow {
            object: f.object.as_str().into(),
            key: f.name.as_str().into(),
            title: pretty_name(&f.name).into(),
            kind: match f.kind {
                model::FanKind::Part => 0,
                model::FanKind::Generic => 1,
                model::FanKind::Auto => 2,
            },
            percent: f.percent.round() as f32,
            trigger,
            trigger_value: value as f32,
        }
    };
    let pick = |keep: &dyn Fn(usize, &model::Fan) -> bool| -> Vec<crate::FanRow> {
        fans.iter()
            .enumerate()
            .filter(|(i, f)| keep(*i, f))
            .map(|(_, f)| row(f))
            .collect()
    };
    let bars = pick(&|i, f| f.kind == model::FanKind::Part || first_generic == Some(i));
    let extras = pick(&|i, f| {
        f.kind == model::FanKind::Generic
            && first_generic != Some(i)
            && !(air_modes && matches!(f.name.as_str(), "internal_fan" | "external_fan"))
    });
    let autos = pick(&|_, f| f.kind == model::FanKind::Auto);
    update_rows(&ui.get_fan_bars(), bars, |m| ui.set_fan_bars(m));
    update_rows(&ui.get_fan_extras(), extras, |m| ui.set_fan_extras(m));
    update_rows(&ui.get_fan_autos(), autos, |m| ui.set_fan_autos(m));
}

/// Replace a list on the screen only when something in it changed.
/// An object's outline as path commands in a 1000×1000 box over the whole bed
/// (X right, Y up: the back of the bed at the top), as Mainsail draws it.
fn plate_path(points: &[[f64; 2]], min: [f64; 3], max: [f64; 3]) -> String {
    let (w, h) = ((max[0] - min[0]).max(1.0), (max[1] - min[1]).max(1.0));
    let mut out = String::new();
    for (i, p) in points.iter().enumerate() {
        let x = (p[0] - min[0]) / w * 1000.0;
        let y = (max[1] - p[1]) / h * 1000.0;
        out.push_str(&format!("{}{x:.1} {y:.1} ", if i == 0 { "M" } else { "L" }));
    }
    if !out.is_empty() {
        out.push('Z');
    }
    out
}

fn replace_if_changed<T: Clone + PartialEq + 'static>(
    shown: &slint::ModelRc<T>,
    rows: Vec<T>,
    replace: impl FnOnce(slint::ModelRc<T>),
) {
    use slint::Model;
    if shown.row_count() == rows.len()
        && rows
            .iter()
            .enumerate()
            .all(|(i, r)| shown.row_data(i).as_ref() == Some(r))
    {
        return;
    }
    replace(slint::ModelRc::new(slint::VecModel::from(rows)));
}

/// `stepper_driver_fan` → `Stepper driver fan`, for fans we have no name for.
fn pretty_name(name: &str) -> String {
    let spaced = name.replace('_', " ");
    let mut chars = spaced.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// Change the rows in place when the fans are the same ones, so a bar being
/// dragged is not rebuilt under the finger; replace the model otherwise.
fn update_rows(
    shown: &slint::ModelRc<crate::FanRow>,
    rows: Vec<crate::FanRow>,
    replace: impl FnOnce(slint::ModelRc<crate::FanRow>),
) {
    use slint::Model;
    if let Some(vec) = shown
        .as_any()
        .downcast_ref::<slint::VecModel<crate::FanRow>>()
    {
        let same = vec.row_count() == rows.len()
            && rows
                .iter()
                .enumerate()
                .all(|(i, r)| vec.row_data(i).is_some_and(|o| o.object == r.object));
        if same {
            for (i, r) in rows.into_iter().enumerate() {
                if vec.row_data(i).as_ref() != Some(&r) {
                    vec.set_row_data(i, r);
                }
            }
            return;
        }
    }
    replace(slint::ModelRc::new(slint::VecModel::from(rows)));
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
        ui.set_error_kind(model::error_kind(&s.klippy_message).into());
        // Whole degrees, as every screen shows them, and only when they change:
        // a reading moves by hundredths several times a second, and each change
        // would redraw the dials (most of Rivet's idle CPU while heating).
        let whole = |t: f64| t.round() as f32;
        if ui.get_extruder_temp() != whole(s.extruder.temperature) {
            ui.set_extruder_temp(whole(s.extruder.temperature));
        }
        if ui.get_bed_temp() != whole(s.bed.temperature) {
            ui.set_bed_temp(whole(s.bed.temperature));
        }
        ui.set_extruder_target(s.extruder.target as f32);
        ui.set_bed_target(s.bed.target as f32);
        ui.set_print_state(s.print_state.as_str().into());
        ui.set_filename(s.filename.as_str().into());
        ui.set_progress(s.progress as f32);
        ui.set_remaining_minutes(s.remaining_secs.map_or(-1, |t| t.div_ceil(60) as i32));
        let eta = s
            .remaining_secs
            .and_then(|r| local_clock(unix_now() + r as f64))
            .unwrap_or_default();
        ui.set_eta_clock(eta.into());
        ui.set_elapsed_minutes((s.print_duration / 60.0) as i32);
        ui.set_position_x(s.position[0] as f32);
        ui.set_position_y(s.position[1] as f32);
        ui.set_position_z(s.position[2] as f32);
        ui.set_homed_x(s.homed_axes.contains('x'));
        ui.set_homed_y(s.homed_axes.contains('y'));
        ui.set_homed_z(s.homed_axes.contains('z'));
        ui.set_axis_min_x(s.axis_min[0] as f32);
        ui.set_axis_min_y(s.axis_min[1] as f32);
        ui.set_axis_min_z(s.axis_min[2] as f32);
        ui.set_axis_max_x(s.axis_max[0] as f32);
        ui.set_axis_max_y(s.axis_max[1] as f32);
        ui.set_axis_max_z(s.axis_max[2] as f32);
        ui.set_can_extrude(s.can_extrude);
        ui.set_filament_out(s.filament_out);
        ui.set_probe_range(s.probe_range.map_or(-1.0, |r| r as f32));
        ui.set_limit_velocity(s.max_velocity as f32);
        ui.set_limit_accel(s.max_accel as f32);
        ui.set_limit_corner(s.corner_velocity as f32);
        ui.set_limit_cruise(s.cruise_ratio.map_or(-1.0, |r| r as f32));
        let (name, enabled, sees) = s.runout.clone().unwrap_or_default();
        ui.set_runout_name(name.into());
        ui.set_runout_enabled(enabled);
        ui.set_runout_sees(sees);
        ui.set_has_air(s.caps.air_sensor);
        ui.set_air_text(
            s.air
                .map(|[tvoc, co2, hcho]| format!("{tvoc:.0} · {co2:.0} · {hcho:.2}"))
                .unwrap_or_default()
                .into(),
        );
        ui.set_has_load_cell(s.load_g.is_some());
        ui.set_load_g(s.load_g.unwrap_or(0.0).round() as i32);
        ui.set_board_temps(
            s.board_temps
                .iter()
                .map(|(_, t)| format!("{t:.0}°"))
                .collect::<Vec<_>>()
                .join(" · ")
                .into(),
        );
        // Objects on the plate as fractions of the bed, Y up (top of the map = back).
        let (w, h) = (
            (s.axis_max[0] - s.axis_min[0]).max(1.0),
            (s.axis_max[1] - s.axis_min[1]).max(1.0),
        );
        let objects: Vec<crate::PlateObject> = s
            .objects
            .iter()
            .map(|o| crate::PlateObject {
                name: o.name.as_str().into(),
                title: model::object_title(&o.name).into(),
                shape: plate_path(&o.polygon, s.axis_min, s.axis_max).into(),
                cx: ((o.center[0] - s.axis_min[0]) / w) as f32,
                cy: ((s.axis_max[1] - o.center[1]) / h) as f32,
                x: ((o.min[0] - s.axis_min[0]) / w) as f32,
                y: ((s.axis_max[1] - o.max[1]) / h) as f32,
                w: ((o.max[0] - o.min[0]) / w) as f32,
                h: ((o.max[1] - o.min[1]) / h) as f32,
                excluded: o.excluded,
                current: o.current,
            })
            .collect();
        replace_if_changed(&ui.get_plate_objects(), objects, |m| {
            ui.set_plate_objects(m)
        });
        ui.set_has_load_macros(s.caps.load_filament);
        ui.set_min_extrude_temp(s.limits.min_extrude_temp as f32);
        ui.set_max_nozzle(s.limits.extruder_max as f32);
        ui.set_max_bed(s.limits.bed_max as f32);
        ui.set_speed_percent(s.speed_percent as f32);
        ui.set_flow_percent(s.flow_percent as f32);
        ui.set_z_offset(s.z_offset as f32);
        ui.set_pressure_advance(s.pressure_advance as f32);
        ui.set_pa_smooth(s.pa_smooth as f32);
        ui.set_live_velocity(s.live_velocity as f32);
        let caps = &s.caps;
        // Pause at layer needs the slicer's layer numbers, or it never fires.
        ui.set_has_pause_layer(caps.pause_at_layer && s.total_layers.is_some());
        ui.set_pause_at_layer(s.pause_at_layer.map_or(0, |n| n as i32));
        ui.set_pause_next(s.pause_next_layer);
        ui.set_has_retraction(caps.firmware_retraction);
        ui.set_retract_length(s.retraction.length as f32);
        ui.set_retract_speed(s.retraction.speed as f32);
        ui.set_unretract_extra(s.retraction.extra as f32);
        ui.set_unretract_speed(s.retraction.unretract_speed as f32);
        ui.set_has_auto_shutdown(caps.auto_shutdown);
        ui.set_has_timelapse(caps.timelapse);
        ui.set_has_spoolman(caps.spoolman);
        let pins: Vec<crate::FormRow> = s
            .pins
            .iter()
            .map(|(name, value)| crate::FormRow {
                kind: "toggle".into(),
                key: format!("pin:{name}").into(),
                title: pretty_name(name).into(),
                on: *value > 0.0,
                shown: true,
                ..Default::default()
            })
            .collect();
        replace_if_changed(&ui.get_pin_rows(), pins, |m| ui.set_pin_rows(m));
        ui.set_fan_percent(s.fan_percent.round() as i32);
        set_fans(&ui, &s.fans, s.air_mode.is_some());
        ui.set_air_mode(s.air_mode.map_or(-1, i32::from));
        ui.set_has_light(s.light.is_some());
        if let Some(l) = &s.light {
            ui.set_light_on(l.on);
            ui.set_light_level((l.level * 100.0).round() as f32);
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
        let fan = |object: &str, percent| Command::SetFan {
            object: object.into(),
            percent,
        };
        assert_eq!(gcode_for(&fan("fan", 60.0)).unwrap(), "M106 S153");
        assert_eq!(gcode_for(&fan("fan", 100.0)).unwrap(), "M106 S255");
        assert_eq!(
            gcode_for(&fan("fan_generic chamber_fan", 45.0)).unwrap(),
            "SET_FAN_SPEED FAN=chamber_fan SPEED=0.45"
        );
        assert_eq!(
            gcode_for(&Command::SetAir(2)).unwrap(),
            "AIR_CIRCULATION_EXTERNAL"
        );
        assert_eq!(
            gcode_for(&Command::SetAir(0)).unwrap(),
            "AIR_CIRCULATION_STOP"
        );
        assert_eq!(
            gcode_for(&Command::AdjustZ(-0.05)).unwrap(),
            "SET_GCODE_OFFSET Z_ADJUST=-0.050 MOVE=1"
        );
        assert_eq!(
            gcode_for(&Command::SetPressureAdvance(0.035)).unwrap(),
            "SET_PRESSURE_ADVANCE ADVANCE=0.035"
        );
        let led = |level, white| Command::SetLight {
            name: "chamber_led".into(),
            level,
            white,
        };
        assert_eq!(
            gcode_for(&led(1.0, true)).unwrap(),
            "SET_LED LED=chamber_led WHITE=1.00 TRANSMIT=1"
        );
        assert_eq!(
            gcode_for(&led(0.0, false)).unwrap(),
            "SET_LED LED=chamber_led RED=0.00 GREEN=0.00 BLUE=0.00 TRANSMIT=1"
        );
        assert_eq!(
            gcode_for(&led(0.55, true)).unwrap(),
            "SET_LED LED=chamber_led WHITE=0.55 TRANSMIT=1"
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
            speed: 100.0,
        })
        .unwrap();
        assert_eq!(
            code,
            "SAVE_GCODE_STATE NAME=RIVET\nG91\nG1 Y-10.000 F6000\nRESTORE_GCODE_STATE NAME=RIVET"
        );
        assert!(gcode_for(&Command::Move {
            axis: 2,
            delta: 1.0,
            speed: 10.0,
        })
        .unwrap()
        .contains("Z1.000 F600"));
        let code = gcode_for(&Command::Extrude {
            length: -25.0,
            speed: 5.0,
            heat: 0.0,
        })
        .unwrap();
        assert!(code.contains("M83") && code.contains("G1 E-25.0 F300"));
        assert!(!code.contains("M109"));
        // A cold nozzle: heat and wait first.
        let code = gcode_for(&Command::Extrude {
            length: 10.0,
            speed: 5.0,
            heat: 215.0,
        })
        .unwrap();
        assert!(code.starts_with("M109 S215\n") && code.contains("G1 E10.0 F300"));
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
