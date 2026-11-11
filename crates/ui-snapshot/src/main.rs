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
    "offline",
    "startup",
    "klippy-offline",
    "error",
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
    "heating",
    "tools",
    "console",
    "console-keyboard",
    "macros",
    "motion",
    "light",
    "speed",
    "zoffset",
    "extruder",
    "motion-more",
    "file-actions",
    "layer-pause",
    "ambient",
    "retraction",
    "dryer",
    "spools",
    "devices",
    "heating-bed",
    "files",
    "files-empty",
    "files-select",
    "files-find",
    "file",
    "file-rename",
    "file-move",
    "queue",
    "queue-empty",
    "dialog",
    "estop",
    "runout",
    "prompt",
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
    // `RIVET_LANG=ru` renders the Russian interface to check string lengths.
    rivet::apply_language(&std::env::var("RIVET_LANG").unwrap_or_default());
    ui.set_russian(std::env::var("RIVET_LANG").is_ok_and(|l| l == "ru"));
    // The app's helpers that screens call (no engine here).
    ui.on_file_title(|path| rivet::files::title(&path).into());
    ui.on_folder_of(|path| model::folder_of(&path).into());
    ui.on_name_ok(|path, name| model::renamed(&path, &name).is_some());
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
    ui.set_link_state("ready".into());
    ui.set_overlay("".into());
    ui.set_runout(false);
    ui.set_prompt_shown(false);
    ui.set_filament_out(false);
    ui.set_ambient(false);
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
            // The prototype page's values.
            ui.set_extruder_temp(24.0);
            ui.set_extruder_target(0.0);
            ui.set_bed_temp(23.0);
            ui.set_bed_target(0.0);
            ui.set_last_material("PETG".into());
            ui.set_files_local(24);
            ui.set_files_usb(3);
            ui.set_loaded_material("PETG".into());
            ui.set_loaded_color(slint::Color::from_rgb_u8(0x44, 0x66, 0xcc));
            ui.set_last_thumb(fake_preview());
            ui.set_has_last_thumb(true);
            ui.set_last_name("Корпус вентилятора №2".into());
            ui.set_last_minutes(83);
            ui.set_last_days_ago(1);
        }
        "printing" => {
            ui.set_print_thumb(fake_preview());
            ui.set_has_print_thumb(true);
            print("printing", 0.42);
            ui.set_layer(76);
            ui.set_total_layers(180);
            // The prototype page's values.
            ui.set_filename("Корпус вентилятора №2".into());
            ui.set_extruder_temp(240.0);
            ui.set_eta_clock("15:20".into());
            ui.set_has_light(true);
        }
        "tune" => {
            // The prototype page's values.
            ui.set_screen("tune".into());
            ui.set_speed_percent(100.0);
            ui.set_flow_percent(100.0);
            ui.set_z_offset(-0.12);
            ui.set_tune_z_start(-0.12);
            ui.set_pressure_advance(0.04);
            ui.set_fan_percent(60);
            ui.set_extruder_temp(240.0);
            ui.set_bed_temp(80.0);
        }
        "more" => {
            // The prototype page's state: every row this printer can have.
            print("printing", 0.42);
            ui.set_screen("more".into());
            ui.set_has_light(true);
            ui.set_light_on(true);
            ui.set_has_pause_layer(true);
            ui.set_has_timelapse(true);
            ui.set_timelapse_on(true);
            ui.set_has_auto_shutdown(true);
        }
        "files" | "files-empty" | "files-select" | "files-find" | "file" | "file-rename"
        | "file-move" => {
            // The prototype page's files: names, print time and filament from metadata.
            let names = [
                ("Корпус вентилятора №2", 83, "PETG"),
                ("Benchy", 52, "PLA"),
                ("Держатель катушки v3", 185, "PETG"),
                ("Крышка электроники", 41, "PLA"),
                ("Калибровочный куб", 18, "PLA"),
                ("Органайзер для отвёрток", 130, "PLA"),
                ("Ваза", 95, "PETG"),
            ];
            let items: Vec<rivet::FileItem> = if name == "files-empty" {
                Vec::new()
            } else {
                names
                    .iter()
                    .enumerate()
                    .map(|(i, (n, minutes, material))| rivet::FileItem {
                        path: format!("{n}.gcode").as_str().into(),
                        name: (*n).into(),
                        rank: i as i32,
                        has_meta: true,
                        minutes: *minutes,
                        material: (*material).into(),
                        thumb: fake_preview(),
                        has_thumb: true,
                        ..Default::default()
                    })
                    .collect()
            };
            ui.set_files_local(24);
            ui.set_files_usb(3);
            ui.set_loaded_material("PETG".into());
            ui.set_queue_count(2);
            let folders: Vec<slint::SharedString> =
                vec!["".into(), "Детали".into(), "Подарки".into()];
            ui.set_move_folders(slint::ModelRc::new(slint::VecModel::from(folders)));
            if name == "files-find" {
                ui.set_files_query("кор".into());
                ui.set_files_sort("name".into());
            }
            rivet::files::set_all(ui, items);
            if name == "files-select" {
                // Two files picked, as the prototype's select mode.
                use slint::Model;
                let files = ui.get_files();
                for i in [0, 3] {
                    if let Some(mut f) = files.row_data(i) {
                        f.selected = true;
                        files.set_row_data(i, f);
                    }
                }
                ui.set_selected_count(2);
                ui.set_select_mode(true);
            }
            if name == "files-find" || name == "file-rename" || name == "file-move" {
                ui.set_screen(name.into());
                ui.set_open_path("Корпус вентилятора №2.gcode".into());
                ui.set_open_name("Корпус вентилятора №2".into());
                ui.set_rename_text("Корпус вентилятора №3".into());
            } else if name == "file" {
                ui.set_screen("file".into());
                ui.set_open_path("Корпус вентилятора №2.gcode".into());
                ui.set_open_name("Корпус вентилятора №2".into());
                ui.set_file_info(rivet::FileInfo {
                    path: "Корпус вентилятора №2.gcode".into(),
                    color: slint::Color::from_rgb_u8(0x44, 0x66, 0xcc),
                    has_color: true,
                    minutes: 83,
                    filament: "PETG".into(),
                    grams: 18,
                    layers: 180,
                    nozzle: 0.4,
                    nozzle_check: 1,
                    height: 64.0,
                    thumb: fake_preview(),
                    has_thumb: true,
                });
            } else {
                ui.set_screen("files".into());
            }
        }
        "light" => {
            ui.set_screen("light".into());
            // The prototype page's state: on, 80 %, on at startup.
            ui.set_has_light(true);
            ui.set_light_on(true);
            ui.set_light_level(80.0);
            ui.set_light_on_start(true);
        }
        "speed" => {
            // The prototype page's values: 100 % / 100 %, the head at 86 mm/s.
            ui.set_screen("speed".into());
            ui.set_speed_percent(100.0);
            ui.set_flow_percent(100.0);
            ui.set_live_velocity(86.0);
        }
        "motion-more" => {
            ui.set_screen("motion-more".into());
            ui.set_homed_x(true);
            ui.set_homed_y(true);
            ui.set_homed_z(true);
            ui.set_axis_min_x(-112.0);
            ui.set_axis_max_x(112.0);
            ui.set_axis_min_y(-112.0);
            ui.set_axis_max_y(112.0);
            ui.set_axis_min_z(-10.0);
            ui.set_axis_max_z(230.0);
            ui.set_goto_x(0.0);
            ui.set_goto_y(0.0);
            ui.set_goto_z(10.0);
        }
        "ambient" => {
            // The prototype page's values.
            print("printing", 0.42);
            ui.set_layer(76);
            ui.set_total_layers(180);
            ui.set_eta_clock("15:20".into());
            ui.set_ambient(true);
        }
        "layer-pause" => {
            print("printing", 0.42);
            ui.set_screen("layer-pause".into());
            ui.set_layer(76);
            ui.set_total_layers(180);
            ui.set_has_pause_layer(true);
            ui.set_pause_layer_pick(120.0);
        }
        "retraction" => {
            ui.set_screen("retraction".into());
            ui.set_has_retraction(true);
            ui.set_retract_length(0.8);
            ui.set_retract_speed(40.0);
            ui.set_unretract_extra(0.0);
            ui.set_unretract_speed(30.0);
        }
        "dryer" => {
            ui.set_screen("dryer".into());
            ui.set_dryer_minutes_left(-1);
        }
        "spools" | "devices" => {
            ui.set_screen(name.into());
            let row = |kind: &str, key: &str, title: &str, sub: &str, value: &str, on: bool| {
                rivet::FormRow {
                    kind: kind.into(),
                    key: key.into(),
                    title: title.into(),
                    sub: sub.into(),
                    value: value.into(),
                    on,
                    shown: true,
                    ..Default::default()
                }
            };
            let spools = vec![
                row(
                    "link",
                    "spool:12",
                    "PETG · синий",
                    "Polymaker · 640 г · #12",
                    "активна",
                    false,
                ),
                row(
                    "link",
                    "spool:9",
                    "PLA · белый",
                    "eSun · 910 г · #9",
                    "",
                    false,
                ),
                row(
                    "link",
                    "spool:4",
                    "PLA · чёрный",
                    "eSun · 210 г · #4",
                    "",
                    false,
                ),
                row(
                    "link",
                    "spool:15",
                    "TPU · красный",
                    "Overture · 450 г · #15",
                    "",
                    false,
                ),
            ];
            ui.set_spool_rows(slint::ModelRc::new(slint::VecModel::from(spools)));
            ui.set_has_active_spool(true);
            let power = vec![row(
                "toggle",
                "power:camera",
                "Розетка камеры",
                "",
                "",
                false,
            )];
            ui.set_power_rows(slint::ModelRc::new(slint::VecModel::from(power)));
            let pins = vec![row(
                "toggle",
                "pin:light_relay",
                "Реле подсветки",
                "",
                "",
                true,
            )];
            ui.set_pin_rows(slint::ModelRc::new(slint::VecModel::from(pins)));
        }
        "file-actions" => {
            ui.set_screen("file-actions".into());
            ui.set_open_name("Корпус вентилятора №2".into());
            ui.set_queue_count(2);
            let folders: Vec<slint::SharedString> = vec!["".into(), "Детали".into()];
            ui.set_move_folders(slint::ModelRc::new(slint::VecModel::from(folders)));
        }
        "queue" | "queue-empty" => {
            ui.set_screen("queue".into());
            ui.set_queue_state("paused".into());
            let jobs: &[(&str, &str)] = if name == "queue" {
                &[
                    ("1 · Benchy", "52 мин · PLA"),
                    ("2 · Крышка электроники", "41 мин · PLA"),
                ]
            } else {
                &[]
            };
            let rows: Vec<rivet::FormRow> = jobs
                .iter()
                .map(|(t, sub)| rivet::FormRow {
                    kind: "link".into(),
                    key: (*t).into(),
                    title: (*t).into(),
                    sub: (*sub).into(),
                    shown: true,
                    ..Default::default()
                })
                .collect();
            ui.set_queue_count(rows.len() as i32);
            ui.set_queue_minutes(93);
            ui.set_queue_rows(slint::ModelRc::new(slint::VecModel::from(rows)));
        }
        "zoffset" => {
            // The prototype page's values: −0.12, unchanged since the screen opened.
            ui.set_screen("zoffset".into());
            ui.set_z_offset(-0.12);
            ui.set_tune_z_start(-0.12);
        }
        "tools" | "macros" | "console" | "console-keyboard" => {
            ui.set_screen(name.trim_end_matches("-keyboard").into());
            let macros: Vec<slint::SharedString> = ["BELTS_TEST", "PAUSE", "RESUME", "START_PRINT"]
                .iter()
                .map(|m| (*m).into())
                .collect();
            ui.set_macros(slint::ModelRc::new(slint::VecModel::from(macros)));
            let lines = [
                ("> G28", 0),
                ("ok", 1),
                ("> QUERY_ENDSTOPS", 0),
                ("x:open y:open z:TRIGGERED", 1),
                ("// Notice: z-offset should not be saved to endstop", 2),
                ("!! Move out of range: 0.000 0.000 -12.000", 3),
            ]
            .map(|(text, kind)| rivet::ConsoleLine {
                text: text.into(),
                kind,
            });
            ui.set_console_lines(slint::ModelRc::new(slint::VecModel::from(lines.to_vec())));
            ui.set_console_keyboard(name == "console-keyboard");
            ui.set_console_text(if name == "console-keyboard" {
                "G28 X0".into()
            } else {
                "".into()
            });
        }
        "motion" => {
            ui.set_screen("motion".into());
            ui.set_homed_x(true);
            ui.set_homed_y(true);
            ui.set_homed_z(true);
            ui.set_position_x(10.0);
            ui.set_position_y(-20.5);
            ui.set_position_z(3.0);
        }
        "extruder" => {
            ui.set_screen("extruder".into());
            ui.set_can_extrude(true);
            ui.set_min_extrude_temp(170.0);
            ui.set_extruder_temp(240.0);
            ui.set_extruder_target(240.0);
            ui.set_has_spoolman(true);
            ui.set_has_active_spool(true);
            ui.set_active_spool("PETG · синий · 640 г".into());
            ui.set_has_retraction(true);
            ui.set_retract_length(0.8);
        }
        "heating" | "heating-bed" => {
            ui.set_screen("heating".into());
            ui.set_heating_heater(i32::from(name == "heating-bed"));
            ui.set_max_nozzle(350.0);
            ui.set_max_bed(130.0);
            // The prototype's values, whatever an earlier scene left behind.
            ui.set_extruder_temp(187.0);
            ui.set_extruder_target(240.0);
            ui.set_bed_temp(23.0);
            ui.set_bed_target(0.0);
            // As in the prototype: rising, about 38 s to go; the links row shown.
            ui.set_nozzle_eta(38);
            ui.set_has_heat_links(true);
        }
        "offline" | "startup" | "klippy-offline" | "error" => {
            ui.set_link_state(name.into());
            ui.set_klippy_message("MCU 'mcu' shutdown: Timer too close".into());
            // The prototype pages' state: a print cut at 42 %, attempt 7 in 3 s.
            ui.set_error_kind("timer".into());
            ui.set_print_state("error".into());
            ui.set_progress(0.42);
            ui.set_retry_attempt(7);
            ui.set_retry_in(3);
        }
        "home-empty" => {}
        "paused" => print("paused", 0.42),
        "done" => {
            print("complete", 1.0);
            ui.set_elapsed_minutes(83);
            ui.set_bed_temp(61.0);
            ui.set_bed_target(0.0);
            // The prototype page's values, as the history entry gives them.
            ui.set_filename("Корпус вентилятора №2".into());
            ui.set_last_name("Корпус вентилятора №2".into());
            ui.set_last_clock("14:02".into());
            ui.set_last_material("PETG".into());
            ui.set_last_grams(18);
        }
        "dialog" => {
            ui.set_screen("home".into());
            ui.set_print_state("printing".into());
            ui.set_filename("Корпус вентилятора №2.gcode".into());
            ui.set_progress(0.42);
            ui.set_overlay("cancel-print".into());
        }
        "prompt" => {
            ui.set_prompt_title("Смена филамента".into());
            ui.set_prompt_text("Вставьте новый пруток и нажмите «Загрузить». Когда пойдёт чистый цвет — «Продолжить».".into());
            let b = |label: &str, style: &str| rivet::PromptButton {
                label: label.into(),
                gcode: label.into(),
                style: style.into(),
            };
            let buttons = vec![
                b("Выгрузить", ""),
                b("Загрузить", ""),
                b("Продолжить", "primary"),
            ];
            ui.set_prompt_buttons(slint::ModelRc::new(slint::VecModel::from(buttons)));
            ui.set_prompt_shown(true);
        }
        "runout" => {
            print("paused", 0.42);
            ui.set_layer(76);
            ui.set_total_layers(180);
            ui.set_has_load_macros(true);
            ui.set_can_extrude(true);
            ui.set_filament_out(true);
            ui.set_runout(true);
        }
        "estop" => {
            ui.set_link_state("error".into());
            ui.set_print_state("error".into());
            ui.set_progress(0.42);
            ui.set_error_kind("estop".into());
            ui.set_klippy_message("Shutdown due to webhooks request".into());
        }
        "keypad" => {
            ui.set_screen("control".into());
            ui.set_keypad_title("Nozzle, °C".into());
            ui.set_keypad_hint("0 … 350".into());
            ui.set_keypad_current("240".into());
            ui.set_keypad_max(350.0);
            ui.set_overlay("keypad".into());
        }
        "fans" => {
            let row = |object: &str, key: &str, title: &str, kind, percent, trigger, value| {
                rivet::FanRow {
                    object: object.into(),
                    key: key.into(),
                    title: title.into(),
                    kind,
                    percent,
                    trigger,
                    trigger_value: value,
                }
            };
            let model = |rows: Vec<rivet::FanRow>| slint::ModelRc::new(slint::VecModel::from(rows));
            ui.set_fan_bars(model(vec![
                row("fan", "fan", "Fan", 0, 60.0, 0, 0.0),
                row(
                    "fan_generic chamber_fan",
                    "chamber_fan",
                    "Chamber fan",
                    1,
                    0.0,
                    0,
                    0.0,
                ),
            ]));
            // With air modes the engine shows no inside/outside rows (set_fans).
            ui.set_fan_extras(model(vec![]));
            ui.set_fan_autos(model(vec![
                row(
                    "heater_fan hotend_fan",
                    "hotend_fan",
                    "Hotend fan",
                    2,
                    100.0,
                    1,
                    50.0,
                ),
                row(
                    "controller_fan stepper_driver_fan",
                    "stepper_driver_fan",
                    "Stepper driver fan",
                    2,
                    0.0,
                    2,
                    0.0,
                ),
            ]));
            // As in the prototype: air off; inside/outside fans belong to the air modes.
            ui.set_air_mode(0);
            ui.set_print_state("standby".into());
            ui.set_screen("fans".into());
        }
        "toast" => {
            ui.set_screen("control".into());
            ui.set_toast_detail("Must home axis first".into());
            ui.set_toast_kind("rpc".into());
        }
        "control" => {
            ui.set_screen("control".into());
            // The prototype page's values.
            ui.set_extruder_temp(187.0);
            ui.set_extruder_target(240.0);
            ui.set_fan_percent(60);
            ui.set_has_light(true);
            ui.set_light_on(true);
            ui.set_light_level(80.0);
            ui.set_z_offset(-0.12);
            // The prototype shows the Devices tile: one output pin is enough.
            let pin = rivet::FormRow {
                kind: "toggle".into(),
                key: "pin:light_relay".into(),
                title: "Light relay".into(),
                shown: true,
                ..Default::default()
            };
            ui.set_pin_rows(slint::ModelRc::new(slint::VecModel::from(vec![pin])));
        }
        screen => {
            ui.set_print_state("standby".into());
            ui.set_screen(screen.into());
        }
    }
}

/// A stand-in preview, shaped like a slicer's (a model on a transparent background,
/// 140×110) and put through the same steps as a fetched one.
fn fake_preview() -> slint::Image {
    let (w, h) = (140u32, 110u32);
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for i in 0..w * h {
        let (x, y) = ((i % w) as f32, (i / w) as f32);
        let inside = ((x - 70.0).powi(2) / 2.0 + (y - 55.0).powi(2)).sqrt() < 30.0;
        let shade = (y / 2.0) as u8;
        data.extend(if inside {
            [120 - shade, 140 - shade, 230, 255]
        } else {
            [0; 4]
        });
    }
    let img = rivet::thumbs::prepare(rivet::thumbs::Rgba {
        width: w,
        height: h,
        data,
    });
    let buf =
        slint::SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&img.data, img.width, img.height);
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
