#[cfg(all(feature = "fbdev", not(target_os = "linux")))]
compile_error!("the `fbdev` backend is Linux-only: build it for the printer with `cargo printer`");

use rivet::{
    engine::{self, Command},
    AppWindow,
};
use slint::ComponentHandle;
use tokio::sync::mpsc;

const DEFAULT_URL: &str = "ws://127.0.0.1:7125/websocket";

fn main() -> anyhow::Result<()> {
    let config = rivet::config::Config::load(&rivet::config::config_path(
        flag_value("--config").as_deref(),
    ));
    init_logging(&config);
    let url = moonraker_url(&config);
    tracing::info!(%url, "starting Rivet");

    #[cfg(feature = "fbdev")]
    fbdev_platform::init(fbdev_platform::Config::from_env())?;
    #[cfg(feature = "fbdev")]
    fbdev_platform::touch_state::set_correction(rivet::touchcal::load());

    let ui = AppWindow::new()?;
    // --lang, RIVET_LANG, rivet.toml, then the system's LANG.
    let lang_arg = flag_value("--lang");
    let lang_env = std::env::var("RIVET_LANG").ok();
    let lang = rivet::language(
        lang_arg.as_deref(),
        lang_env.as_deref().or(config.language.as_deref()),
        std::env::var("LANG").ok().as_deref(),
    );
    rivet::apply_language(&lang);
    ui.set_russian(lang == "ru");
    // --screen <name>: open on that screen (proof shots on the printer); the rail
    // and back buttons work as usual from there.
    let start_screen = flag_value("--screen");
    if let Some(screen) = &start_screen {
        ui.set_screen(screen.as_str().into());
    }
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    wire_callbacks(&ui, &cmd_tx);
    // Load what the tile that leads there would load, once Moonraker answers.
    let start_refresh = slint::Timer::default();
    let weak = ui.as_weak();
    let mut pending = start_screen.is_some();
    start_refresh.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(500),
        move || {
            let Some(ui) = weak.upgrade() else { return };
            // Only once: later screens load through their tiles.
            if pending && ui.get_link_state() == "ready" {
                pending = false;
                refresh_for(&ui, start_screen.as_deref());
            }
        },
    );

    // While printing, the screensaver takes the place of screen sleep.
    #[cfg(feature = "fbdev")]
    let _ambient = {
        let weak = ui.as_weak();
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_secs(1),
            move || {
                let Some(ui) = weak.upgrade() else { return };
                let printing = matches!(ui.get_print_state().as_str(), "printing" | "paused");
                let idle = fbdev_platform::screen::sleep_after()
                    .is_some_and(|after| fbdev_platform::screen::idle() >= after);
                // Lock by itself after the chosen idle minutes.
                let lock_after = [0u64, 5, 15, 30][ui.get_set_autolock().clamp(0, 3) as usize];
                if lock_after > 0
                    && !ui.get_locked()
                    && fbdev_platform::screen::idle()
                        >= std::time::Duration::from_secs(lock_after * 60)
                {
                    ui.set_locked(true);
                }
                if printing && idle && !ui.get_ambient() {
                    ui.set_ambient(true);
                } else if !printing && ui.get_ambient() {
                    ui.set_ambient(false);
                }
            },
        );
        timer
    };
    let weak = ui.as_weak();
    std::thread::Builder::new()
        .name("net".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            rt.block_on(engine::run(url, weak, cmd_rx));
        })?;

    ui.run()?;
    Ok(())
}

/// Log to `--log-file`/`log_file` (rotated, 512 KB x 3) or the terminal.
/// `RUST_LOG` wins over `log_level`; the default is `info`.
fn init_logging(config: &rivet::config::Config) {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new(config.log_level.as_deref().unwrap_or("info"))
    });
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    let path = flag_value("--log-file").or_else(|| config.log_file.clone());
    match path
        .as_deref()
        .map(|p| rivet::logfile::RotatingFile::open(std::path::Path::new(p), 512 * 1024, 3))
    {
        Some(Ok(file)) => builder.with_ansi(false).with_writer(file).init(),
        Some(Err(err)) => {
            builder.init();
            tracing::warn!(%err, "cannot open the log file, logging to the terminal");
        }
        None => builder.init(),
    }
}

/// Value after `--name` on the command line.
fn flag_value(name: &str) -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == name {
            return args.next();
        }
    }
    None
}

/// `--moonraker <url>`, `RIVET_MOONRAKER`, rivet.toml, else localhost.
fn moonraker_url(config: &rivet::config::Config) -> String {
    flag_value("--moonraker")
        .or_else(|| std::env::var("RIVET_MOONRAKER").ok())
        .or_else(|| config.moonraker.clone())
        .unwrap_or_else(|| DEFAULT_URL.to_owned())
}

fn wire_callbacks(ui: &AppWindow, tx: &mpsc::UnboundedSender<Command>) {
    let send = |cmd: Command| {
        let tx = tx.clone();
        move || {
            let _ = tx.send(cmd.clone());
        }
    };

    ui.on_home_all(send(Command::HomeAll));
    ui.on_pause_print(send(Command::Pause));
    ui.on_resume_print(send(Command::Resume));
    ui.on_cancel_print(send(Command::Cancel));
    {
        let tx = tx.clone();
        ui.on_start_print(move |file| {
            let _ = tx.send(Command::StartPrint(file.to_string()));
        });
    }
    {
        let tx = tx.clone();
        ui.on_set_fan(move |object, percent| {
            let _ = tx.send(Command::SetFan {
                object: object.to_string(),
                percent,
            });
        });
    }
    {
        let tx = tx.clone();
        ui.on_set_air(move |mode| {
            let _ = tx.send(Command::SetAir(mode.clamp(0, 2) as u8));
        });
    }
    {
        let tx = tx.clone();
        let weak = ui.as_weak();
        // Jog speed from the saved settings (Motion · more), mm/s.
        ui.on_move_axis(move |axis, delta| {
            let Some(ui) = weak.upgrade() else { return };
            let speed = if axis == 2 {
                ui.get_jog_z_speed()
            } else {
                ui.get_jog_xy_speed()
            };
            let _ = tx.send(Command::Move {
                axis: axis as usize,
                delta,
                speed,
            });
        });
    }
    ui.on_motors_off(send(Command::MotorsOff));
    {
        let tx = tx.clone();
        ui.on_extrude(move |length, speed, heat| {
            let _ = tx.send(Command::Extrude {
                length,
                speed,
                heat,
            });
        });
    }
    {
        let tx = tx.clone();
        ui.on_set_nozzle(move |t| {
            let _ = tx.send(Command::SetNozzle(t));
        });
    }
    {
        let tx = tx.clone();
        ui.on_set_bed(move |t| {
            let _ = tx.send(Command::SetBed(t));
        });
    }
    {
        let tx = tx.clone();
        ui.on_set_speed(move |p| {
            let _ = tx.send(Command::SetSpeed(p));
        });
    }
    {
        let tx = tx.clone();
        ui.on_set_flow(move |p| {
            let _ = tx.send(Command::SetFlow(p));
        });
    }
    {
        let tx = tx.clone();
        ui.on_adjust_z(move |d| {
            let _ = tx.send(Command::AdjustZ(d));
        });
    }
    {
        let tx = tx.clone();
        ui.on_set_pressure_advance(move |v| {
            let _ = tx.send(Command::SetPressureAdvance(v));
        });
    }
    {
        let tx = tx.clone();
        let weak = ui.as_weak();
        // Percent from the screen; a switch sends 0 or 100.
        ui.on_set_light_level(move |percent| {
            if let Some(ui) = weak.upgrade() {
                let _ = tx.send(Command::SetLight {
                    name: ui.get_light_name().to_string(),
                    level: percent / 100.0,
                    white: ui.get_light_white(),
                });
            }
        });
    }
    {
        let tx = tx.clone();
        ui.on_save_flag(move |key, on| {
            let _ = tx.send(Command::SaveSetting {
                key: key.to_string(),
                value: on.into(),
            });
        });
    }
    {
        let tx = tx.clone();
        ui.on_save_number(move |key, v| {
            let _ = tx.send(Command::SaveSetting {
                key: key.to_string(),
                value: f64::from(v).into(),
            });
        });
    }
    {
        let tx = tx.clone();
        ui.on_pause_at(move |layer| {
            let _ = tx.send(Command::PauseAtLayer(layer.max(0) as u32));
        });
    }
    {
        let tx = tx.clone();
        ui.on_pause_next_layer(move |on| {
            let _ = tx.send(Command::PauseNextLayer(on));
        });
    }
    {
        let tx = tx.clone();
        ui.on_set_retraction(move |key, value| {
            let _ = tx.send(Command::SetRetraction {
                key: key.to_string(),
                value,
            });
        });
    }
    {
        let tx = tx.clone();
        ui.on_off_after(move |on| {
            let _ = tx.send(Command::OffAfterPrint(on));
        });
    }
    {
        let tx = tx.clone();
        ui.on_set_timelapse(move |on| {
            let _ = tx.send(Command::SetTimelapse(on));
        });
    }
    ui.on_save_z_offset(send(Command::SaveZOffset));
    ui.on_retry_now(send(Command::RetryNow));
    ui.on_list_spools(send(Command::ListSpools));
    {
        let tx = tx.clone();
        // "spool:12" picks that spool; "spool-clear" clears the active one.
        ui.on_pick_spool(move |key| {
            let id = key
                .strip_prefix("spool:")
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
            let _ = tx.send(Command::SetActiveSpool(id));
        });
    }
    {
        let tx = tx.clone();
        // Row keys carry what they switch: "power:<device>" or "pin:<name>".
        ui.on_device_toggled(move |key, on| {
            let cmd = if let Some(device) = key.strip_prefix("power:") {
                Command::SetPower {
                    device: device.to_owned(),
                    on,
                }
            } else if let Some(name) = key.strip_prefix("pin:") {
                Command::SetPin {
                    name: name.to_owned(),
                    on,
                }
            } else {
                return;
            };
            let _ = tx.send(cmd);
        });
    }
    {
        let tx = tx.clone();
        ui.on_dry(move |temp, minutes| {
            let _ = tx.send(Command::Dry {
                temp,
                minutes: minutes.max(0) as u32,
            });
        });
    }
    {
        let tx = tx.clone();
        let weak = ui.as_weak();
        ui.on_go_to(move |x, y, z| {
            let Some(ui) = weak.upgrade() else { return };
            let _ = tx.send(Command::GoTo {
                x,
                y,
                z,
                speed: ui.get_jog_xy_speed(),
            });
        });
    }
    {
        let tx = tx.clone();
        ui.on_run_gcode(move |script| {
            let script = script.trim();
            if !script.is_empty() {
                let _ = tx.send(Command::Gcode(script.to_owned()));
            }
        });
    }
    ui.on_clear_console(send(Command::ClearConsole));
    // Typing is plain string editing, done here: Slint has no string slicing.
    ui.on_apply_key(|text, key| rivet::apply_key(&text, &key).into());
    {
        let tx = tx.clone();
        let weak = ui.as_weak();
        ui.on_refresh_files(move || {
            let usb = weak.upgrade().is_some_and(|ui| ui.get_files_on_usb());
            let _ = tx.send(Command::ListFiles { usb });
        });
    }
    {
        let tx = tx.clone();
        ui.on_open_file(move |path| {
            let _ = tx.send(Command::FileDetails(path.to_string()));
        });
    }
    {
        let tx = tx.clone();
        ui.on_delete_file(move |path| {
            let _ = tx.send(Command::DeleteFiles(vec![path.to_string()]));
        });
    }
    {
        let tx = tx.clone();
        let weak = ui.as_weak();
        ui.on_delete_selected(move || {
            use slint::Model;
            let Some(ui) = weak.upgrade() else { return };
            let paths: Vec<String> = ui
                .get_files()
                .iter()
                .filter(|f| f.selected)
                .map(|f| f.path.to_string())
                .collect();
            if !paths.is_empty() {
                let _ = tx.send(Command::DeleteFiles(paths));
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_files_view_changed(move || {
            if let Some(ui) = weak.upgrade() {
                rivet::files::show(&ui);
            }
        });
    }
    ui.on_file_title(|path| rivet::files::title(&path).into());
    ui.on_folder_of(|path| model::folder_of(&path).into());
    ui.on_name_ok(|path, name| model::renamed(&path, &name).is_some());
    {
        let tx = tx.clone();
        ui.on_rename_file(move |path, name| {
            if let Some(to) = model::renamed(&path, &name) {
                let _ = tx.send(Command::MoveFile {
                    from: path.to_string(),
                    to,
                });
            }
        });
    }
    {
        let tx = tx.clone();
        ui.on_move_file(move |path, folder| {
            if let Some(to) = model::moved(&path, &folder) {
                let _ = tx.send(Command::MoveFile {
                    from: path.to_string(),
                    to,
                });
            }
        });
    }
    {
        let tx = tx.clone();
        ui.on_copy_to_printer(move |path| {
            if let Some(to) = model::moved(&path, "") {
                let _ = tx.send(Command::CopyFile {
                    from: path.to_string(),
                    to,
                });
            }
        });
    }
    {
        let tx = tx.clone();
        ui.on_queue_files(move |paths| {
            use slint::Model;
            let paths: Vec<String> = paths.iter().map(|p| p.to_string()).collect();
            let _ = tx.send(Command::QueueAdd(paths));
        });
    }
    {
        let tx = tx.clone();
        let weak = ui.as_weak();
        ui.on_queue_selected(move || {
            use slint::Model;
            let Some(ui) = weak.upgrade() else { return };
            let paths: Vec<String> = ui
                .get_files()
                .iter()
                .filter(|f| f.selected)
                .map(|f| f.path.to_string())
                .collect();
            ui.invoke_select_all(false);
            if !paths.is_empty() {
                let _ = tx.send(Command::QueueAdd(paths));
            }
        });
    }
    {
        let tx = tx.clone();
        ui.on_queue_run(move |start| {
            let _ = tx.send(Command::QueueRun(start));
        });
    }
    ui.on_queue_clear(send(Command::QueueClear));
    ui.on_refresh_queue(send(Command::ListQueue));
    ui.on_refresh_messages(send(Command::ListMessages));
    ui.on_refresh_history(send(Command::ListHistory));
    ui.on_refresh_system(send(Command::SystemInfo));
    {
        let tx = tx.clone();
        ui.on_system_action(move |what| {
            let _ = tx.send(Command::SystemAction(what.to_string()));
        });
    }
    {
        let tx = tx.clone();
        ui.on_setting_changed(move |key, value| {
            use rivet::engine::prefs;
            let on = value == "1";
            match key.as_str() {
                "awake_print" => prefs::set(&prefs::AWAKE_PRINT, on),
                "clock_12h" => prefs::set(&prefs::CLOCK_12, on),
                "end_tune" => prefs::set(&prefs::END_TUNE, on),
                "sounds" if !on => prefs::set(&prefs::END_TUNE, false),
                #[cfg(feature = "fbdev")]
                "brightness" => {
                    fbdev_platform::screen::set_brightness(value.parse().unwrap_or(75));
                }
                #[cfg(feature = "fbdev")]
                "screen_sleep_min" => {
                    let minutes: f64 = value.parse().unwrap_or(10.0);
                    fbdev_platform::screen::set_sleep_after(
                        (minutes > 0.0).then(|| std::time::Duration::from_secs_f64(minutes * 60.0)),
                    );
                }
                _ => {}
            }
            // Flags as booleans, minutes as numbers, the PIN as text.
            let value = match key.as_str() {
                "pin" => serde_json::Value::String(value.to_string()),
                "screen_sleep_min" | "autolock_min" | "brightness" => value
                    .parse::<f64>()
                    .map_or(serde_json::Value::Null, Into::into),
                _ => serde_json::Value::Bool(on),
            };
            let _ = tx.send(Command::SaveSetting {
                key: key.to_string(),
                value,
            });
        });
    }
    {
        let tx = tx.clone();
        let weak = ui.as_weak();
        ui.on_set_language(move |i| {
            let lang = if i == 0 { "ru" } else { "en" };
            rivet::apply_language(lang);
            if let Some(ui) = weak.upgrade() {
                ui.set_russian(lang == "ru");
            }
            let _ = tx.send(Command::SaveSetting {
                key: "language".into(),
                value: lang.into(),
            });
        });
    }
    ui.on_refresh_calib(send(Command::CalibInfo));
    {
        let tx = tx.clone();
        ui.on_calibrate(move |script| {
            let _ = tx.send(Command::Calibrate(script.to_string()));
        });
    }
    ui.on_clear_history(send(Command::ClearHistory));
    ui.on_refresh_service(send(Command::ListService));
    ui.on_refresh_cameras(send(Command::ListCameras));
    {
        let tx = tx.clone();
        ui.on_service_done(move |key| {
            let _ = tx.send(Command::ServiceDone(key.to_string()));
        });
    }
    {
        let tx = tx.clone();
        ui.on_set_limit(move |key, value| {
            let _ = tx.send(Command::SetLimit {
                key: key.to_string(),
                value,
            });
        });
    }
    {
        let tx = tx.clone();
        ui.on_set_runout(move |on| {
            let _ = tx.send(Command::SetRunout(on));
        });
    }
    {
        let tx = tx.clone();
        let weak = ui.as_weak();
        ui.on_set_loaded(move |name| {
            if let Some(ui) = weak.upgrade() {
                ui.set_loaded_material(name.clone());
            }
            let _ = tx.send(Command::SaveSetting {
                key: "loaded_material".into(),
                value: serde_json::Value::String(name.to_string()),
            });
            // When: a print started later shows its own filament instead.
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            if let Some(ui) = weak.upgrade() {
                ui.set_loaded_at(now as i32);
            }
            let _ = tx.send(Command::SaveSetting {
                key: "loaded_at".into(),
                value: serde_json::json!(now),
            });
        });
    }
    {
        let tx = tx.clone();
        ui.on_refresh_graph(move |aspect| {
            let _ = tx.send(Command::TempGraph(aspect));
        });
    }
    ui.on_mask(|n| "•".repeat(n.clamp(0, 64) as usize).into());
    // Before a cold extrusion: the last print's filament profile, then the presets.
    {
        let weak = ui.as_weak();
        ui.on_build_heat_choices(move || {
            let Some(ui) = weak.upgrade() else { return };
            let mut choices = Vec::new();
            let name = ui.get_last_filament();
            let kind = ui.get_last_material();
            let temp = ui.get_last_nozzle_temp().round() as i32;
            if temp > 0 && !kind.is_empty() {
                let words = slint::ComponentHandle::global::<rivet::Strings>(&ui);
                choices.push(rivet::HeatChoice {
                    label: if name.is_empty() { kind.clone() } else { name },
                    sub: words.get_last_print_filament(),
                    temp,
                });
            }
            for p in slint::Model::iter(&ui.get_heat_presets()) {
                if p.nozzle > 0 {
                    choices.push(rivet::HeatChoice {
                        label: p.name,
                        sub: "".into(),
                        temp: p.nozzle,
                    });
                }
            }
            ui.set_heat_choices(slint::ModelRc::new(slint::VecModel::from(choices)));
        });
    }
    // The nozzle temperature of a material's heat preset (0 when it has none).
    {
        let weak = ui.as_weak();
        ui.on_material_temp(move |name| {
            let Some(ui) = weak.upgrade() else { return 0 };
            slint::Model::iter(&ui.get_heat_presets())
                .find(|p| p.name.eq_ignore_ascii_case(&name))
                .map_or(0, |p| p.nozzle)
        });
    }
    ui.set_wifi_available(rivet::wifi::available());
    #[cfg(feature = "fbdev")]
    ui.set_backlight_available(fbdev_platform::screen::backlight_available());
    // A press stops a list only while it really glides: lists report each move.
    #[cfg(feature = "fbdev")]
    {
        use std::{cell::Cell, rc::Rc, time::Instant};
        let moved: Rc<Cell<Option<Instant>>> = Rc::default();
        let m = moved.clone();
        slint::ComponentHandle::global::<rivet::Glide>(ui)
            .on_moved(move || m.set(Some(Instant::now())));
        fbdev_platform::touch_state::set_glide_probe(move || {
            moved
                .get()
                .is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(120))
        });
    }
    // Macro parameters.
    {
        let tx = tx.clone();
        ui.on_macro_open(move |name| {
            let _ = tx.send(Command::MacroParams(name.into()));
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_macro_param_set(move |i, value| {
            let Some(ui) = weak.upgrade() else { return };
            let rows = ui.get_macro_params();
            if let Some(mut row) = slint::Model::row_data(&rows, i.max(0) as usize) {
                row.value = value;
                slint::Model::set_row_data(&rows, i.max(0) as usize, row);
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_macro_script(move || {
            let Some(ui) = weak.upgrade() else {
                return "".into();
            };
            let mut script = ui.get_macro_pending().to_string();
            for p in slint::Model::iter(&ui.get_macro_params()) {
                let v = p.value.trim();
                if v.is_empty() {
                    continue;
                }
                // Klipper takes a value with spaces in quotes.
                if v.contains(' ') {
                    script.push_str(&format!(" {}=\"{}\"", p.name, v.replace('"', "")));
                } else {
                    script.push_str(&format!(" {}={v}", p.name));
                }
            }
            script.into()
        });
    }
    // Appearance: the swatches, and a pick applied and saved.
    {
        let weak = ui.as_weak();
        ui.on_theme_open(move || {
            if let Some(ui) = weak.upgrade() {
                rivet::theme::show_choices(&ui);
            }
        });
    }
    {
        let weak = ui.as_weak();
        let tx = tx.clone();
        ui.on_theme_pick(move |accent, base| {
            let Some(ui) = weak.upgrade() else { return };
            rivet::theme::apply(&ui, accent.max(0) as usize, base.max(0) as usize);
            for (key, v) in [("theme_accent", accent), ("theme_base", base)] {
                let _ = tx.send(Command::SaveSetting {
                    key: key.into(),
                    value: serde_json::json!(v),
                });
            }
        });
    }
    {
        let weak = ui.as_weak();
        let tx = tx.clone();
        ui.on_theme_custom(move |kind, color| {
            let Some(ui) = weak.upgrade() else { return };
            let c = rivet::theme::color_hex(color);
            let (mut accent, mut base) =
                (ui.get_theme_accent() as usize, ui.get_theme_base() as usize);
            if kind == "accent" {
                rivet::theme::set_own(Some(c), None);
                accent = rivet::theme::OWN_ACCENT_INDEX;
            } else {
                rivet::theme::set_own(None, Some(c));
                base = rivet::theme::OWN_BASE_INDEX;
            }
            rivet::theme::apply(&ui, accent, base);
            let (own_accent, own_base) = rivet::theme::own_hex();
            for (key, v) in [
                ("theme_accent", serde_json::json!(accent)),
                ("theme_base", serde_json::json!(base)),
                ("theme_accent_rgb", serde_json::json!(own_accent)),
                ("theme_base_rgb", serde_json::json!(own_base)),
            ] {
                let _ = tx.send(Command::SaveSetting {
                    key: key.into(),
                    value: v,
                });
            }
        });
    }
    ui.on_touch_test_tap(|round, point, tx, ty, x, y| {
        let unix_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        tracing::info!(target: "touchtest", round, point, tx, ty, x, y, unix_ms, "tap");
    });
    // Touch calibration.
    ui.on_cal_begin(rivet::touchcal::begin);
    ui.on_cal_point(|i, x, y| rivet::touchcal::tap(i.max(0) as usize, x, y));
    {
        let weak = ui.as_weak();
        ui.on_cal_finish(move || {
            let Some(ui) = weak.upgrade() else { return };
            let (was, now) = rivet::touchcal::finish().unwrap_or((-1.0, -1.0));
            ui.set_cal_miss_before(was);
            ui.set_cal_miss_after(now);
        });
    }
    ui.on_cal_keep(|| {
        if let Err(err) = rivet::touchcal::keep() {
            tracing::warn!(%err, "touch calibration not saved");
        }
    });
    ui.on_cal_undo(rivet::touchcal::undo);
    ui.on_cal_reset(|| {
        if let Err(err) = rivet::touchcal::reset() {
            tracing::warn!(%err, "touch calibration not reset");
        }
    });
    ui.on_wifi_scan(send(Command::WifiScan));
    {
        let tx = tx.clone();
        ui.on_wifi_join(move |ssid, auth, password| {
            let _ = tx.send(Command::WifiJoin(ssid.into(), auth.into(), password.into()));
        });
    }
    {
        let tx = tx.clone();
        let weak = ui.as_weak();
        ui.on_files_window(move |first| {
            let Some(ui) = weak.upgrade() else { return };
            let paths = rivet::files::window(&ui, first.max(0) as usize);
            let _ = tx.send(Command::Thumbs(paths));
        });
    }
    {
        let tx = tx.clone();
        ui.on_draw_resonance(move |aspect| {
            let _ = tx.send(Command::DrawResonance(aspect));
        });
    }
    {
        let tx = tx.clone();
        let weak = ui.as_weak();
        ui.on_preset_changed(move |i, nozzle, bed| {
            use slint::Model;
            let Some(ui) = weak.upgrade() else { return };
            let mut presets: Vec<rivet::HeatPreset> = ui.get_heat_presets().iter().collect();
            if let Some(p) = presets.get_mut(i as usize) {
                p.nozzle = nozzle;
                p.bed = bed;
            }
            let value = serde_json::Value::Array(
                presets
                    .iter()
                    .map(|p| serde_json::json!({ "name": p.name.as_str(), "nozzle": p.nozzle, "bed": p.bed }))
                    .collect(),
            );
            ui.set_heat_presets(slint::ModelRc::new(slint::VecModel::from(presets)));
            let _ = tx.send(Command::SaveSetting { key: "heat_presets".into(), value });
        });
    }
    ui.on_read_messages(send(Command::ReadMessages));
    {
        let weak = ui.as_weak();
        ui.on_select_all(move |on| {
            use slint::Model;
            let Some(ui) = weak.upgrade() else { return };
            let files = ui.get_files();
            for i in 0..files.row_count() {
                if let Some(mut f) = files.row_data(i) {
                    f.selected = on;
                    files.set_row_data(i, f);
                }
            }
            ui.set_selected_count(if on { files.row_count() as i32 } else { 0 });
        });
    }
    ui.on_emergency_stop(send(Command::EmergencyStop));
    ui.on_firmware_restart(send(Command::FirmwareRestart));
}

/// What opening a screen from its tile loads, for `--screen`.
fn refresh_for(ui: &AppWindow, screen: Option<&str>) {
    match screen {
        Some("files") => ui.invoke_refresh_files(),
        Some("history") => ui.invoke_refresh_history(),
        Some("service") => ui.invoke_refresh_service(),
        Some("camera") => ui.invoke_refresh_cameras(),
        Some("messages") => ui.invoke_refresh_messages(),
        Some("queue") => ui.invoke_refresh_queue(),
        Some(s) if s.starts_with("set-") => ui.invoke_refresh_system(),
        Some(
            "calibrate" | "bed-mesh" | "mesh-profiles" | "screws" | "probe" | "shaper"
            | "shaper-result" | "belts" | "pid" | "pa" | "tmc",
        ) => ui.invoke_refresh_calib(),
        _ => {}
    }
}
