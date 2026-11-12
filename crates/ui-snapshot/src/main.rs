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
    "exclude",
    "messages",
    "heat-presets",
    "filament",
    "calibrate",
    "bed-mesh",
    "screws",
    "probe",
    "shaper",
    "shaper-result",
    "belts",
    "pid",
    "pa",
    "tmc",
    "set-screen",
    "set-lock",
    "set-sound",
    "set-language",
    "set-safety",
    "set-system",
    "set-network",
    "set-theme",
    "theme-picker",
    "touch-test",
    "macro-params",
    "wifi-join",
    "set-printers",
    "set-updates",
    "lock",
    "limits",
    "sensors",
    "history",
    "service",
    "camera",
    "temp-graph",
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
    {
        let weak = ui.as_weak();
        ui.on_draw_resonance(move |aspect| {
            rivet::resonance::set_aspect(f64::from(aspect));
            if let Some(ui) = weak.upgrade() {
                rivet::resonance::show_now(&ui);
            }
        });
    }
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
    // The default theme unless the scene picks another.
    rivet::theme::apply(ui, 0, 0);
    ui.set_link_state("ready".into());
    ui.set_overlay("".into());
    ui.set_runout(false);
    ui.set_prompt_shown(false);
    ui.set_locked(false);
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
        "macro-params" => {
            // KlipperMod's LOAD_MATERIAL-like template: two with defaults, one typed.
            let p = |n: &str, v: &str, d: &str| rivet::MacroParam {
                name: n.into(),
                value: v.into(),
                default: d.into(),
            };
            ui.set_macro_pending("LOAD_MATERIAL".into());
            ui.set_macro_params(slint::ModelRc::new(slint::VecModel::from(vec![
                p("TEMP", "250", "220"),
                p("LENGTH", "", "100"),
                p("MATERIAL", "", ""),
            ])));
            ui.set_screen("macro-params".into());
        }
        "touch-test" => ui.set_overlay("touch-test".into()),
        "heat-presets" => ui.set_screen("heat-presets".into()),
        "calibrate" | "bed-mesh" | "screws" | "probe" | "shaper" | "pid" | "pa" | "tmc"
        | "shaper-result" | "belts" => {
            // The AD5M Pro's own config and the prototype pages' mesh and screws.
            let cfg: serde_json::Value = serde_json::from_str(include_str!(
                "../../model/tests/fixtures/ad5m-pro/configfile.json"
            ))
            .unwrap();
            let cfg = cfg.get("result").unwrap_or(&cfg)["status"]["configfile"]["settings"].clone();
            let objects: Vec<String> = [
                "probe",
                "bed_mesh",
                "screws_tilt_adjust",
                "tmc2209 stepper_x",
                "tmc2209 stepper_y",
                "tmc2209 stepper_z",
                "tmc2209 extruder",
                "gcode_macro LOAD_CELL_TARE",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect();
            let mesh: Vec<Vec<f64>> = (0..5)
                .map(|r| {
                    (0..5)
                        .map(|c| -0.09 + 0.02 * (c as f64) + 0.02 * (r as f64))
                        .collect()
                })
                .collect();
            let status = serde_json::json!({
                "bed_mesh": { "profile_name": "default", "profiles": { "default": {}, "PETG-80": {} }, "probed_matrix": mesh },
                "screws_tilt_adjust": { "results": {
                    "screw1": { "sign": "CCW", "adjust": "00:20", "is_base": false },
                    "screw2": { "sign": "CW", "adjust": "00:00", "is_base": false },
                    "screw3": { "sign": "CW", "adjust": "00:15", "is_base": false },
                    "screw4": { "sign": "CW", "adjust": "00:00", "is_base": true }
                } }
            });
            let info = model::CalibInfo::read(&cfg, &status, &objects);
            rivet::calib::show(ui, info);
            ui.set_pressure_advance(0.04);
            ui.set_pa_smooth(0.04);
            ui.set_probe_range(0.004);
            if name == "shaper-result" || name == "belts" {
                resonance_fixture();
            }
            ui.set_screen(name.into());
        }
        "set-screen" | "set-lock" | "set-sound" | "set-language" | "set-safety" | "set-system"
        | "set-network" | "set-printers" | "set-updates" | "wifi-join" | "set-theme"
        | "theme-picker" => {
            // The AD5M Pro as it answered: T113, 110 MB, KlipperMod's versions.
            ui.set_sys_cpu("T113-S3 · 2 × · 18%".into());
            ui.set_sys_memory("54 / 110 MB".into());
            ui.set_sys_disk("1.2 / 7.1 GB".into());
            ui.set_net_wifi("wlan0".into());
            ui.set_net_ip("192.168.0.20".into());
            ui.set_net_mac("8c:aa:b5:12:34:56".into());
            ui.set_host_name("AD5M".into());
            ui.set_ver_klipper("v0.13.0-285".into());
            ui.set_ver_moonraker("v0.9.3".into());
            ui.set_ver_rivet(env!("CARGO_PKG_VERSION").into());
            ui.set_ver_mainsail("v2.19.0".into());
            ui.set_ver_mainsail_sub("актуально".into());
            ui.set_ver_fluidd("v1.37.7".into());
            ui.set_ver_fluidd_sub("актуально".into());
            ui.set_has_stock_macro(true);
            ui.set_time_zone("Europe/Moscow".into());
            // Networks as the prototype page lists them.
            let row = |key: &str, title: &str, sub: &str| rivet::FormRow {
                kind: "link".into(),
                key: key.into(),
                title: title.into(),
                sub: sub.into(),
                shown: true,
                ..Default::default()
            };
            ui.set_wifi_available(true);
            ui.set_wifi_rows(slint::ModelRc::new(slint::VecModel::from(vec![
                row("0", "HomeNet_5G", "5 ГГц · WPA2 · сильный сигнал"),
                row("1", "Office", "2.4 ГГц · WPA2 · средний сигнал"),
            ])));
            ui.set_join_ssid("HomeNet".into());
            ui.set_join_auth("psk".into());
            ui.set_join_password("hunter22".into());
            ui.on_mask(|n| "•".repeat(n.max(0) as usize).into());
            if name == "set-theme" || name == "theme-picker" {
                // Orange on black: every accent and surface follows.
                rivet::theme::show_choices(ui);
                rivet::theme::apply(ui, 1, 1);
            }
            ui.set_screen(name.into());
        }
        "lock" => {
            print("printing", 0.42);
            ui.set_remaining_minutes(48);
            ui.set_set_pin("1234".into());
            ui.set_locked(true);
        }
        "limits" => {
            ui.set_screen("limits".into());
            ui.set_limit_velocity(600.0);
            ui.set_limit_accel(20000.0);
            ui.set_limit_corner(25.0);
            ui.set_limit_cruise(0.5);
        }
        "sensors" => {
            ui.set_screen("sensors".into());
            ui.set_runout_name("runout_sensor".into());
            ui.set_runout_enabled(true);
            ui.set_runout_sees(true);
            ui.set_has_air(true);
            ui.set_has_load_cell(true);
            ui.set_load_g(-40);
        }
        "history" => {
            ui.set_screen("history".into());
            ui.set_history_total(770);
            ui.set_history_minutes(999 * 60);
            ui.set_history_km(8.1);
            let r = |t: &str, sub: &str, v: &str| rivet::FormRow {
                kind: "link".into(),
                key: t.into(),
                title: t.into(),
                sub: sub.into(),
                value: v.into(),
                shown: true,
                ..Default::default()
            };
            let rows = vec![
                r("Корпус вентилятора №2", "вчера · 1 ч 23 мин · PETG", "✓"),
                r("Benchy", "вчера · 31 мин · PLA", "■"),
                r("Держатель катушки v3", "3 дня назад · 3 ч 05 мин", "✓"),
            ];
            ui.set_history_rows(slint::ModelRc::new(slint::VecModel::from(rows)));
        }
        "service" => {
            ui.set_screen("service".into());
            let s = |key: &str, used: f32, every: f32| rivet::ServiceItem {
                key: key.into(),
                title: "".into(),
                used,
                every,
            };
            let items = vec![
                s("filter", 82.0, 300.0),
                s("nozzle", 311.0, 500.0),
                s("rails", 96.0, 100.0),
            ];
            ui.set_service_items(slint::ModelRc::new(slint::VecModel::from(items)));
        }
        "camera" => ui.set_screen("camera".into()),
        "filament" => {
            ui.set_screen("filament".into());
            ui.set_filament_step(1);
        }
        "temp-graph" => {
            ui.set_screen("temp-graph".into());
            // The prototype's curves: the nozzle to 220°, the bed to 80°, from 25°.
            let top = 250.0_f32;
            let aspect = 2.17_f32;
            let line = |target: f32, rise: f32| {
                let mut d = String::new();
                for k in 0..=100 {
                    let t = k as f32 / 100.0;
                    let v = 25.0 + (target - 25.0) * (t / rise).min(1.0);
                    // The plot is about 2.2 times wider than high here.
                    let (x, y) = (t * 1000.0 * aspect, (1.0 - v / top) * 1000.0);
                    d.push_str(&format!("{}{x:.1} {y:.1} ", if k == 0 { "M" } else { "L" }));
                }
                d
            };
            let lines = vec![
                rivet::GraphLine {
                    path: line(220.0, 0.45).into(),
                    color: slint::Color::from_rgb_u8(0xf9, 0x73, 0x16),
                    legend: "Сопло 220° / 220°".into(),
                },
                rivet::GraphLine {
                    path: line(80.0, 0.42).into(),
                    color: slint::Color::from_rgb_u8(0x60, 0xa5, 0xfa),
                    legend: "Стол 80° / 80°".into(),
                },
            ];
            let grid: Vec<(slint::SharedString, f32)> = [60.0_f32, 120.0, 180.0, 240.0]
                .iter()
                .map(|v| (format!("{v:.0}°").into(), 1.0 - v / top))
                .collect();
            ui.set_graph_lines(slint::ModelRc::new(slint::VecModel::from(lines)));
            ui.set_graph_grid(slint::ModelRc::new(slint::VecModel::from(grid)));
        }
        "messages" => {
            ui.set_screen("messages".into());
            let m = |kind: &str, title: &str, sub: &str, days: i32, clock: &str, unread: bool| {
                rivet::MessageRow {
                    kind: kind.into(),
                    title: title.into(),
                    sub: sub.into(),
                    days,
                    clock: clock.into(),
                    unread,
                }
            };
            let rows = vec![
                m("done", "Корпус вентилятора №2", "", 0, "14:02", true),
                m(
                    "klipper",
                    "",
                    "Option 'max_accel_to_decel' in section 'printer' is deprecated",
                    -1,
                    "",
                    false,
                ),
                m("stopped", "Benchy", "", 1, "", false),
                m("save", "", "", -1, "", false),
            ];
            ui.set_messages(slint::ModelRc::new(slint::VecModel::from(rows)));
            ui.set_unread(true);
        }
        "exclude" => {
            print("printing", 0.42);
            ui.set_screen("exclude".into());
            // Four parts on a 224 mm bed as a slicer would label them: an L-shaped
            // housing, a round lid, a long bar, and a small clip already skipped.
            let bed = 112.0_f32;
            let o = |name: &str, pts: &[(f32, f32)], excluded: bool, current: bool| {
                let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                let mut shape = String::new();
                for (i, (x, y)) in pts.iter().enumerate() {
                    (x0, y0, x1, y1) = (x0.min(*x), y0.min(*y), x1.max(*x), y1.max(*y));
                    let (px, py) = (
                        (x + bed) / (2.0 * bed) * 1000.0,
                        (bed - y) / (2.0 * bed) * 1000.0,
                    );
                    shape.push_str(&format!(
                        "{}{px:.1} {py:.1} ",
                        if i == 0 { "M" } else { "L" }
                    ));
                }
                shape.push('Z');
                let f = |v: f32| (v + bed) / (2.0 * bed);
                rivet::PlateObject {
                    name: name.into(),
                    title: name.into(),
                    shape: shape.into(),
                    cx: f((x0 + x1) / 2.0),
                    cy: 1.0 - f((y0 + y1) / 2.0),
                    x: f(x0),
                    y: 1.0 - f(y1),
                    w: (x1 - x0) / (2.0 * bed),
                    h: (y1 - y0) / (2.0 * bed),
                    excluded,
                    current,
                }
            };
            let lid: Vec<(f32, f32)> = (0..16)
                .map(|k| {
                    let a = k as f32 / 16.0 * std::f32::consts::TAU;
                    (30.0 + 26.0 * a.cos(), 40.0 + 26.0 * a.sin())
                })
                .collect();
            let objects = vec![
                o(
                    "Корпус",
                    &[
                        (-80.0, 10.0),
                        (-20.0, 10.0),
                        (-20.0, 35.0),
                        (-50.0, 35.0),
                        (-50.0, 75.0),
                        (-80.0, 75.0),
                    ],
                    false,
                    true,
                ),
                o("Крышка", &lid, false, false),
                o(
                    "Планка",
                    &[(-70.0, -60.0), (40.0, -60.0), (40.0, -45.0), (-70.0, -45.0)],
                    false,
                    false,
                ),
                o(
                    "Клипса",
                    &[(62.0, -30.0), (78.0, -30.0), (78.0, -14.0), (62.0, -14.0)],
                    true,
                    false,
                ),
            ];
            ui.set_plate_objects(slint::ModelRc::new(slint::VecModel::from(objects)));
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

/// The vibration screens from the AD5M Pro's own SHAPER_CALIBRATE CSVs, read
/// the way the printer does (newest files in Klipper's /tmp). There is no real
/// belt check yet, so the belts get made-up responses (78 and 71 Hz, as the
/// prototype page).
fn resonance_fixture() {
    let dir = std::env::temp_dir().join("rivet-snapshot");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    for (name, text) in [
        (
            "calibration_data_x_20260726_132933.csv",
            include_str!("../../model/tests/fixtures/ad5m-pro/resonance/calibration_data_x_20260726_132933.csv"),
        ),
        (
            "calibration_data_y_20260726_133151.csv",
            include_str!("../../model/tests/fixtures/ad5m-pro/resonance/calibration_data_y_20260726_133151.csv"),
        ),
    ] {
        let _ = std::fs::write(dir.join(name), text);
    }
    let peak = |f: f64, at: f64, width: f64| 1.0 / (1.0 + ((f - at) / width).powi(2));
    for (name, main, second) in [("belt_a", 78.0, 40.0), ("belt_b", 71.0, 38.0)] {
        let mut csv = String::from("freq,psd_x,psd_y,psd_z,psd_xyz\n");
        let mut f = 0.0;
        while f < 150.0 {
            let p = 1.0e4 * (peak(f, main, 4.0) + 0.25 * peak(f, second, 6.0)) + 300.0;
            csv.push_str(&format!("{f:.1},0,0,0,{p:.3e}\n"));
            f += 1.6;
        }
        let _ = std::fs::write(
            dir.join(format!("resonances_axis_20260726_120000_{name}.csv")),
            csv,
        );
    }
    rivet::resonance::load_latest(&dir);
}
