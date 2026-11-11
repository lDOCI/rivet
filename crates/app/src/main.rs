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
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let url = moonraker_url();
    tracing::info!(%url, "starting Rivet");

    #[cfg(feature = "fbdev")]
    fbdev_platform::init(fbdev_platform::Config::from_env())?;

    let ui = AppWindow::new()?;
    let lang_arg = flag_value("--lang");
    rivet::apply_language(&rivet::language(
        lang_arg.as_deref(),
        std::env::var("RIVET_LANG").ok().as_deref(),
        std::env::var("LANG").ok().as_deref(),
    ));
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    wire_callbacks(&ui, &cmd_tx);

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

/// `--moonraker <url>` or `RIVET_MOONRAKER`, else localhost.
fn moonraker_url() -> String {
    flag_value("--moonraker")
        .or_else(|| std::env::var("RIVET_MOONRAKER").ok())
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
        ui.on_set_fan(move |p| {
            let _ = tx.send(Command::SetFan(p));
        });
    }
    {
        let tx = tx.clone();
        ui.on_move_axis(move |axis, delta| {
            let _ = tx.send(Command::Move {
                axis: axis as usize,
                delta,
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
        ui.on_set_light(move |on| {
            if let Some(ui) = weak.upgrade() {
                let _ = tx.send(Command::SetLight {
                    name: ui.get_light_name().to_string(),
                    on,
                    white: ui.get_light_white(),
                });
            }
        });
    }
    ui.on_refresh_files(send(Command::ListFiles));
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
