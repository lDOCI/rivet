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
    if let Some(screen) = flag_value("--screen") {
        ui.set_screen(screen.into());
    }
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    wire_callbacks(&ui, &cmd_tx);

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
        ui.on_extrude(move |length, speed| {
            let _ = tx.send(Command::Extrude { length, speed });
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
