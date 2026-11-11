//! `ui-snapshot <out-dir> [screen…]` renders each screen of the app at 800x480
//! with the software renderer (the one the printer uses) and writes PNGs.

use std::{fs::File, io::BufWriter, path::PathBuf, rc::Rc};

use anyhow::Context;
use rivet::AppWindow;
use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        Platform, WindowAdapter,
    },
    ComponentHandle, PhysicalSize, PlatformError, Rgb8Pixel,
};

const WIDTH: u32 = 800;
const HEIGHT: u32 = 480;
const ALL: &[&str] = &[
    "home",
    "home-empty",
    "printing",
    "paused",
    "done",
    "control",
    "settings",
    "fans",
    "tune",
    "more",
    "dialog",
    "keypad",
    "toast",
];

struct Headless(Rc<MinimalSoftwareWindow>);

impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.0.clone())
    }
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().unwrap_or_else(|| "snapshots".into()));
    let screens: Vec<String> = args.collect();
    let screens: Vec<&str> = if screens.is_empty() {
        ALL.to_vec()
    } else {
        screens.iter().map(String::as_str).collect()
    };
    std::fs::create_dir_all(&out)?;

    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    window.set_size(PhysicalSize::new(WIDTH, HEIGHT));
    slint::platform::set_platform(Box::new(Headless(window.clone())))
        .map_err(|e| anyhow::anyhow!("set platform: {e}"))?;

    let ui = AppWindow::new()?;
    ui.set_link_state("ready".into());
    ui.set_extruder_temp(187.0);
    ui.set_extruder_target(240.0);
    ui.set_bed_temp(23.0);
    ui.show()?;

    let mut buf = vec![Rgb8Pixel::default(); (WIDTH * HEIGHT) as usize];
    for screen in screens {
        show(&ui, screen);
        slint::platform::update_timers_and_animations();
        window.request_redraw();
        window.draw_if_needed(|renderer| {
            renderer.render(&mut buf, WIDTH as usize);
        });
        let path = out.join(format!("{screen}.png"));
        write_png(&path, &buf).with_context(|| format!("write {}", path.display()))?;
        println!("{}", path.display());
    }
    Ok(())
}

/// Put the window into the state a snapshot name stands for. The overlay ones
/// sit on top of a real screen.
fn show(ui: &AppWindow, name: &str) {
    ui.set_overlay("".into());
    ui.set_toast_kind("".into());
    ui.set_screen("home".into());
    ui.set_print_state("standby".into());
    ui.set_last_name("".into());
    let print = |state: &str, progress: f32| {
        ui.set_print_state(state.into());
        ui.set_filename("fan_duct_2.gcode".into());
        ui.set_progress(progress);
        ui.set_elapsed_minutes(35);
        ui.set_remaining_minutes(48);
        ui.set_extruder_target(240.0);
        ui.set_extruder_temp(239.0);
        ui.set_bed_target(80.0);
        ui.set_bed_temp(80.0);
    };
    match name {
        "home" => {
            ui.set_last_name("fan_duct_2.gcode".into());
            ui.set_last_minutes(83);
            ui.set_last_days_ago(1);
        }
        "printing" => {
            print("printing", 0.42);
            ui.set_layer(76);
            ui.set_total_layers(180);
        }
        "tune" => {
            ui.set_screen("tune".into());
            ui.set_speed_percent(110.0);
            ui.set_z_offset(-0.07);
            ui.set_pressure_advance(0.035);
            ui.set_fan_percent(60);
        }
        "more" => {
            ui.set_screen("more".into());
            ui.set_has_light(true);
            ui.set_light_on(true);
        }
        "home-empty" => {}
        "paused" => print("paused", 0.42),
        "done" => {
            print("complete", 1.0);
            ui.set_elapsed_minutes(83);
            ui.set_bed_temp(61.0);
            ui.set_bed_target(0.0);
        }
        "dialog" => {
            ui.set_screen("home".into());
            ui.set_print_state("printing".into());
            ui.set_filename("benchy.gcode".into());
            ui.set_progress(0.42);
            ui.set_overlay("cancel-print".into());
        }
        "keypad" => {
            ui.set_screen("control".into());
            ui.set_keypad_title("Nozzle, °C".into());
            ui.set_keypad_hint("0 … 350".into());
            ui.set_keypad_current("240".into());
            ui.set_keypad_max(350.0);
            ui.set_overlay("keypad".into());
        }
        "toast" => {
            ui.set_screen("control".into());
            ui.set_toast_detail("Must home axis first".into());
            ui.set_toast_kind("rpc".into());
        }
        screen => {
            ui.set_print_state("standby".into());
            ui.set_screen(screen.into());
        }
    }
}

fn write_png(path: &PathBuf, buf: &[Rgb8Pixel]) -> anyhow::Result<()> {
    let mut encoder = png::Encoder::new(BufWriter::new(File::create(path)?), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    let bytes: Vec<u8> = buf.iter().flat_map(|p| [p.r, p.g, p.b]).collect();
    writer.write_image_data(&bytes)?;
    Ok(())
}
