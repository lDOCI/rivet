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
    ComponentHandle, PhysicalSize, PlatformError, Rgb8Pixel, Rgba8Pixel,
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
    "files",
    "files-empty",
    "file",
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
            ui.set_last_thumb(fake_preview());
            ui.set_has_last_thumb(true);
            ui.set_last_name("fan_duct_2.gcode".into());
            ui.set_last_minutes(83);
            ui.set_last_days_ago(1);
        }
        "printing" => {
            ui.set_print_thumb(fake_preview());
            ui.set_has_print_thumb(true);
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
        "files" | "files-empty" | "file" => {
            let names = [
                ("fan_duct_2", 1, 4410),
                ("benchy", 3, 2250),
                ("spool_holder_v3", 8, 12800),
                ("electronics_cover", 12, 980),
                ("calibration_cube", 30, 310),
                ("screwdriver_organizer", 31, 8800),
                ("vase_spiral", 40, 1500),
            ];
            let items: Vec<rivet::FileItem> = if name == "files-empty" {
                Vec::new()
            } else {
                names
                    .iter()
                    .map(|(n, d, kb)| rivet::FileItem {
                        path: format!("{n}.gcode").as_str().into(),
                        name: (*n).into(),
                        days_ago: *d,
                        size_kb: *kb,
                        thumb: fake_preview(),
                        has_thumb: true,
                        ..Default::default()
                    })
                    .collect()
            };
            ui.set_files(slint::ModelRc::new(slint::VecModel::from(items)));
            if name == "file" {
                ui.set_screen("file".into());
                ui.set_open_path("fan_duct_2.gcode".into());
                ui.set_open_name("fan_duct_2.gcode".into());
                ui.set_file_info(rivet::FileInfo {
                    path: "fan_duct_2.gcode".into(),
                    minutes: 83,
                    filament: "PETG".into(),
                    grams: 18,
                    layers: 180,
                    nozzle: 0.4,
                    nozzle_check: 1,
                    thumb: fake_preview(),
                    has_thumb: true,
                });
            } else {
                ui.set_screen("files".into());
            }
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

/// A stand-in preview so snapshots show how a real one is clipped and fitted.
fn fake_preview() -> slint::Image {
    let (w, h) = (200u32, 120u32);
    let mut buf = slint::SharedPixelBuffer::<Rgba8Pixel>::new(w, h);
    for (i, px) in buf.make_mut_slice().iter_mut().enumerate() {
        let (x, y) = ((i as u32 % w) as f32, (i as u32 / w) as f32);
        let inside = ((x - 100.0).powi(2) + (y - 60.0).powi(2)).sqrt() < 45.0;
        *px = if inside {
            Rgba8Pixel::new(93, 125, 224, 255)
        } else {
            Rgba8Pixel::new(40 + (x / 4.0) as u8, 50, 70 + (y / 2.0) as u8, 255)
        };
    }
    slint::Image::from_rgba8(buf)
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
