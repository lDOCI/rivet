slint::include_modules!();

mod engine;

use engine::Command;
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

/// `--moonraker <url>` or `RIVET_MOONRAKER`, else localhost.
fn moonraker_url() -> String {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--moonraker" {
            if let Some(url) = args.next() {
                return url;
            }
        }
    }
    std::env::var("RIVET_MOONRAKER").unwrap_or_else(|_| DEFAULT_URL.to_owned())
}

fn wire_callbacks(ui: &AppWindow, tx: &mpsc::UnboundedSender<Command>) {
    let send = |cmd: Command| {
        let tx = tx.clone();
        move || {
            let _ = tx.send(cmd.clone());
        }
    };

    {
        let tx = tx.clone();
        ui.on_preheat(move |extruder, bed| {
            let _ = tx.send(Command::Preheat { extruder, bed });
        });
    }
    ui.on_cooldown(send(Command::Cooldown));
    ui.on_home_all(send(Command::HomeAll));
    ui.on_pause_print(send(Command::Pause));
    ui.on_resume_print(send(Command::Resume));
    ui.on_cancel_print(send(Command::Cancel));
    ui.on_emergency_stop(send(Command::EmergencyStop));
    ui.on_firmware_restart(send(Command::FirmwareRestart));
}
