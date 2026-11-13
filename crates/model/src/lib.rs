//! Printer state built from Moonraker's `printer.objects.subscribe` result
//! and the partial updates that follow in `notify_status_update`.

use std::collections::HashMap;

use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Connection {
    #[default]
    Connecting,
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Klippy {
    #[default]
    Unknown,
    Startup,
    Ready,
    Shutdown,
    Error,
    Disconnected,
}

impl Klippy {
    pub fn parse(s: &str) -> Self {
        match s {
            "startup" => Self::Startup,
            "ready" => Self::Ready,
            "shutdown" => Self::Shutdown,
            "error" => Self::Error,
            "disconnected" => Self::Disconnected,
            _ => Self::Unknown,
        }
    }
}

/// Objects the UI subscribes to. `null` means "all fields".
pub fn subscription() -> Value {
    json!({
        "objects": {
            "webhooks": null,
            "print_stats": null,
            "virtual_sdcard": null,
            "display_status": null,
            "extruder": null,
            "heater_bed": null,
            "toolhead": null,
            "gcode_move": null,
            "fan": null,
            "motion_report": ["live_velocity"],
        }
    })
}

const PAUSE_MACRO: &str = "gcode_macro SET_PRINT_STATS_INFO";

/// A runout sensor: `filament_switch_sensor …` or `filament_motion_sensor …`.
fn is_filament_sensor(name: &str) -> bool {
    name.starts_with("filament_switch_sensor ") || name.starts_with("filament_motion_sensor ")
}

/// A temperature sensor of a board: the MCU or the host.
fn is_board_sensor(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("temperature_sensor ")
        && ["mcu", "host", "raspberry"]
            .iter()
            .any(|w| lower.contains(w))
}

/// An `output_pin` meant for the user (names starting with `_` are internal).
fn is_user_pin(name: &str) -> bool {
    name.strip_prefix("output_pin ")
        .is_some_and(|pin| !pin.starts_with('_'))
}

/// What this printer can do, from its Klipper objects and Moonraker components.
/// A screen row shows only when its source is here.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Caps {
    /// `SET_PAUSE_AT_LAYER` / `SET_PAUSE_NEXT_LAYER` macros.
    pub pause_at_layer: bool,
    pub exclude_object: bool,
    pub firmware_retraction: bool,
    /// KlipperMod's `AUTO_SHUTDOWN`: powers off once the printer is idle and cool.
    pub auto_shutdown: bool,
    /// The command that keeps a live Z offset: `Z_OFFSET_APPLY_PROBE` when Z homes
    /// on the probe, else `Z_OFFSET_APPLY_ENDSTOP`.
    pub z_offset_apply: String,
    pub timelapse: bool,
    pub spoolman: bool,
    pub power_devices: bool,
    pub job_queue: bool,
    pub load_filament: bool,
    /// The AD5M Pro's air sensor (`flashforge_tvoc`).
    pub air_sensor: bool,
}

impl Caps {
    pub fn new(objects: &[String], components: &[String], z_homes_on_probe: bool) -> Self {
        let has = |name: &str| objects.iter().any(|n| n == name);
        let macro_ = |name: &str| has(&format!("gcode_macro {name}"));
        let component = |name: &str| components.iter().any(|c| c == name);
        Self {
            pause_at_layer: macro_("SET_PAUSE_AT_LAYER") && macro_("SET_PAUSE_NEXT_LAYER"),
            exclude_object: has("exclude_object"),
            firmware_retraction: has("firmware_retraction"),
            auto_shutdown: macro_("AUTO_SHUTDOWN"),
            z_offset_apply: if z_homes_on_probe {
                "Z_OFFSET_APPLY_PROBE"
            } else {
                "Z_OFFSET_APPLY_ENDSTOP"
            }
            .to_owned(),
            timelapse: component("timelapse"),
            spoolman: component("spoolman"),
            power_devices: component("power"),
            job_queue: component("job_queue"),
            load_filament: macro_("LOAD_FILAMENT") && macro_("UNLOAD_FILAMENT"),
            air_sensor: has("flashforge_tvoc"),
        }
    }
}

/// Firmware retraction, as Klipper has it now.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Retraction {
    pub length: f64,
    pub speed: f64,
    pub extra: f64,
    pub unretract_speed: f64,
}

/// The base subscription plus what this printer actually has, from
/// `printer.objects.list`: every `led …` and fan object, so Rivet shows only
/// the lights and fans that exist.
pub fn subscription_for(available: &[String]) -> Value {
    let mut sub = subscription();
    if let Some(objects) = sub["objects"].as_object_mut() {
        for name in available.iter().filter(|n| {
            n.starts_with("led ")
                || is_fan_object(n)
                || is_user_pin(n)
                || is_filament_sensor(n)
                || is_board_sensor(n)
        }) {
            objects.insert(name.clone(), Value::Null);
        }
        for name in [
            "firmware_retraction",
            "exclude_object",
            "flashforge_tvoc",
            "flashforge_loadcell",
        ] {
            if available.iter().any(|n| n == name) {
                objects.insert(name.to_owned(), Value::Null);
            }
        }
        // Pause at layer: KlipperMod / Mainsail keep it in this macro's variables.
        if available.iter().any(|n| n == PAUSE_MACRO) {
            objects.insert(
                PAUSE_MACRO.to_owned(),
                json!(["pause_next_layer", "pause_at_layer"]),
            );
        }
    }
    sub
}

#[derive(Debug, Default)]
pub struct PrinterState {
    pub connection: Connection,
    pub klippy: Klippy,
    /// `led …` objects of this printer, set before subscribing.
    pub leds: Vec<String>,
    /// From the printer's config, read once after subscribing.
    pub limits: Limits,
    /// Macros of this printer, from `printer.objects.list`.
    pub macros: Vec<String>,
    /// What the printer can do; set after subscribing.
    pub caps: Caps,
    /// User output pins, set before subscribing.
    pub pins: Vec<String>,
    /// Fan objects besides the part fan (`fan_generic …`, `heater_fan …`, …).
    pub fan_objects: Vec<String>,
    /// `heater_fan` object → the temperature it switches on at, from the config.
    pub fan_heater_temps: HashMap<String, f64>,
    /// Console log, oldest first; `console_revision` changes with it.
    pub console: Vec<(String, LineKind)>,
    pub console_revision: u64,
    /// A macro's question to the user (`action:prompt_*`); `None` when closed.
    pub prompt: Option<Prompt>,
    /// The prompt being built between `prompt_begin` and `prompt_show`.
    prompt_draft: Option<Prompt>,
    pub prompt_revision: u64,
    /// Range of the last PROBE_ACCURACY, mm.
    pub probe_range: Option<f64>,
    status: Value,
}

/// A dialog a macro opens with `RESPOND TYPE=command MSG="action:prompt_…"`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Prompt {
    pub title: String,
    pub text: Vec<String>,
    /// Buttons in the body and in the footer, in order.
    pub buttons: Vec<PromptButton>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PromptButton {
    pub label: String,
    /// G-code to run; the label itself when the macro gave none.
    pub gcode: String,
    /// "primary", "secondary", "info", "warning", "error" or "".
    pub style: String,
}

impl PromptButton {
    fn parse(args: &str) -> Self {
        let mut parts = args.split('|').map(str::trim);
        let label = parts.next().unwrap_or_default().to_owned();
        let gcode = parts
            .next()
            .filter(|g| !g.is_empty())
            .unwrap_or(&label)
            .to_owned();
        let style = parts.next().unwrap_or_default().to_owned();
        Self {
            label,
            gcode,
            style,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Heater {
    pub temperature: f64,
    pub target: f64,
}

/// Plain values the UI needs for one frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub connection: Connection,
    pub klippy: Klippy,
    pub klippy_message: String,
    pub extruder: Heater,
    pub bed: Heater,
    /// `standby`, `printing`, `paused`, `complete`, `cancelled`, `error`.
    pub print_state: String,
    pub filename: String,
    /// 0.0 ..= 1.0
    pub progress: f64,
    /// Seconds spent printing (pauses excluded), as Klipper counts it.
    pub print_duration: f64,
    /// Estimated seconds left; `None` until there is enough progress to say.
    pub remaining_secs: Option<u64>,
    /// Only when the slicer sends them (`SET_PRINT_STATS_INFO`); else `None`.
    pub layer: Option<u32>,
    pub total_layers: Option<u32>,
    /// Speed and flow factors in percent (100 = as sliced).
    pub speed_percent: f64,
    pub flow_percent: f64,
    /// Live Z offset, mm (`gcode_move.homing_origin[2]`).
    pub z_offset: f64,
    pub pressure_advance: f64,
    /// Pressure advance smooth time, s.
    pub pa_smooth: f64,
    /// Real toolhead speed, mm/s (`motion_report.live_velocity`).
    pub live_velocity: f64,
    /// Part fan, percent.
    pub fan_percent: f64,
    /// Every fan of the printer: the part fan first, then the ones that can
    /// be set, then the ones that run by themselves.
    pub fans: Vec<Fan>,
    /// Chamber air mode (0 off, 1 filter, 2 exhaust) when the printer has the
    /// `AIR_CIRCULATION_*` macros; `None` otherwise.
    pub air_mode: Option<u8>,
    pub light: Option<Light>,
    pub caps: Caps,
    pub retraction: Retraction,
    /// Pause armed for a layer (`SET_PAUSE_AT_LAYER`), and for the next one.
    pub pause_at_layer: Option<u32>,
    pub pause_next_layer: bool,
    /// User output pins: name and value 0.0 ..= 1.0.
    pub pins: Vec<(String, f64)>,
    pub limits: Limits,
    /// Toolhead X, Y, Z in mm.
    pub position: [f64; 3],
    /// Z as the G-code sets it (`gcode_move.gcode_position`), without the Z offset.
    pub gcode_z: f64,
    /// Axes Klipper knows the position of, e.g. `"xy"`; empty after motors off.
    pub homed_axes: String,
    pub axis_min: [f64; 3],
    pub axis_max: [f64; 3],
    /// Klipper's own answer: nozzle hot enough to extrude.
    pub can_extrude: bool,
    /// A switched-on runout sensor sees no filament.
    pub filament_out: bool,
    /// Objects of the print (`exclude_object`), when the slicer labelled them.
    pub objects: Vec<PrintObject>,
    /// The first runout sensor: its name, switched on, sees filament.
    pub runout: Option<(String, bool, bool)>,
    /// Air quality (AD5M Pro): TVOC, CO₂, formaldehyde; `None` until it reports.
    pub air: Option<[f64; 3]>,
    /// The load cell under the bed (AD5M), grams.
    pub load_g: Option<f64>,
    /// MCU / host temperatures, by sensor name.
    pub board_temps: Vec<(String, f64)>,
    /// Range of the last PROBE_ACCURACY, mm.
    pub probe_range: Option<f64>,
    /// Motion limits: velocity, acceleration, square corner velocity, minimum
    /// cruise ratio (0..1; `None` on Klipper versions without it).
    pub max_velocity: f64,
    pub max_accel: f64,
    pub corner_velocity: f64,
    pub cruise_ratio: Option<f64>,
}

/// One labelled object of the print, by its outline's bounding box (mm).
#[derive(Debug, Clone, PartialEq)]
pub struct PrintObject {
    pub name: String,
    /// The slicer's outline, mm (a small square at the centre when it sent none).
    pub polygon: Vec<[f64; 2]>,
    /// Where Klipper says the object's centre is, mm.
    pub center: [f64; 2],
    pub min: [f64; 2],
    pub max: [f64; 2],
    pub excluded: bool,
    pub current: bool,
}

/// An object name as a person reads it: slicers add `.stl`, `_id_0_copy_1`
/// and the like ("CASE.STL_ID_0_COPY_0" → "CASE"; second copies keep "2").
pub fn object_title(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    let mut title = name.to_owned();
    if let Some(i) = lower
        .find(".stl")
        .or_else(|| lower.find(".step"))
        .or_else(|| lower.find(".3mf"))
        .or_else(|| lower.find(".obj"))
    {
        title.truncate(i);
    } else if let Some(i) = lower.find("_id_") {
        title.truncate(i);
    }
    let copy = lower
        .rsplit_once("copy_")
        .and_then(|(_, n)| n.parse::<u32>().ok())
        .filter(|n| *n > 0);
    let title = title.replace('_', " ").trim().to_owned();
    match copy {
        Some(n) => format!("{title} {}", n + 1),
        None => title,
    }
}

/// Whether a person can set the fan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FanKind {
    /// The part fan: `fan`, set with M106.
    Part,
    /// `fan_generic …`: set with SET_FAN_SPEED.
    Generic,
    /// `heater_fan`, `controller_fan`, `temperature_fan`: Klipper runs it.
    Auto,
}

/// Why an automatic fan runs, said from what Klipper knows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FanTrigger {
    None,
    /// `heater_fan`: switches on above this temperature, °C (0 = not known).
    HeaterAbove(f64),
    /// `controller_fan`: runs while the motors are on.
    Motors,
    /// `temperature_fan`: holds this temperature, °C.
    Holds(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fan {
    /// Klipper object name: `fan`, `fan_generic chamber_fan`, …
    pub object: String,
    /// The part after the type, `chamber_fan`; `fan` for the part fan.
    pub name: String,
    pub kind: FanKind,
    pub percent: f64,
    pub trigger: FanTrigger,
}

const FAN_TYPES: [&str; 4] = [
    "fan_generic ",
    "heater_fan ",
    "controller_fan ",
    "temperature_fan ",
];

fn is_fan_object(name: &str) -> bool {
    FAN_TYPES.iter().any(|t| name.starts_with(t))
}

/// Fan objects besides the part fan, settable ones first, each group sorted.
pub fn fan_object_names(available: &[String]) -> Vec<String> {
    let mut names: Vec<String> = available
        .iter()
        .filter(|n| is_fan_object(n))
        .cloned()
        .collect();
    names.sort_by_key(|n| (!n.starts_with("fan_generic "), n.clone()));
    names
}

/// `heater_fan` temperatures from `configfile.settings`.
pub fn parse_fan_heater_temps(settings: &Value) -> HashMap<String, f64> {
    settings
        .as_object()
        .map(|o| {
            o.iter()
                .filter(|(name, _)| name.starts_with("heater_fan "))
                .filter_map(|(name, s)| Some((name.clone(), s.get("heater_temp")?.as_f64()?)))
                .collect()
        })
        .unwrap_or_default()
}

/// The first light of the printer.
#[derive(Debug, Clone, PartialEq)]
pub struct Light {
    /// Klipper name without the `led ` prefix.
    pub name: String,
    pub on: bool,
    /// Brightest channel, 0.0 ..= 1.0.
    pub level: f64,
    /// Has a white channel (RGBW). Otherwise it is RGB.
    pub white: bool,
}

/// How a console line is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// What the user sent.
    Command,
    Response,
    /// `// …` from Klipper.
    Notice,
    /// `!! …` from Klipper.
    Error,
}

pub fn line_kind(text: &str) -> LineKind {
    if text.starts_with("!!") {
        LineKind::Error
    } else if text.starts_with("//") {
        LineKind::Notice
    } else {
        LineKind::Response
    }
}

/// Lines from `server.gcode_store`, oldest first.
pub fn parse_gcode_store(result: &Value) -> Vec<(String, LineKind)> {
    result
        .get("gcode_store")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|l| {
            let text = l.get("message")?.as_str()?;
            Some(match l.get("type").and_then(Value::as_str) {
                Some("command") => (format!("> {text}"), LineKind::Command),
                _ => (text.to_owned(), line_kind(text)),
            })
        })
        .collect()
}

/// Macros a person would run: `gcode_macro NAME`, not the `_helpers`.
/// Pins that the printer's own macros switch (`SET_PIN PIN=name` in any macro):
/// from the raw config (`configfile.config`).
pub fn macro_managed_pins(config: &Value) -> Vec<String> {
    let mut pins = Vec::new();
    for (section, body) in config.as_object().into_iter().flatten() {
        if !section.starts_with("gcode_macro ") && !section.starts_with("delayed_gcode ") {
            continue;
        }
        let Some(gcode) = body["gcode"].as_str() else {
            continue;
        };
        for line in gcode.lines() {
            let upper = line.to_ascii_uppercase();
            if let Some(rest) = upper.trim_start().strip_prefix("SET_PIN") {
                if let Some(pos) = rest.find("PIN=") {
                    let name: String = rest[pos + 4..]
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() && !pins.contains(&name) {
                        pins.push(name);
                    }
                }
            }
        }
    }
    pins
}

/// What a Klipper shutdown message is about, for a plain-words explanation on the
/// screen; "" when Rivet does not know it (the message is still shown as is).
pub fn error_kind(message: &str) -> &'static str {
    let m = message.to_ascii_lowercase();
    // Klipper's own words for M112 and for Moonraker's emergency_stop.
    if m.contains("m112") || m.contains("webhooks request") || m.contains("emergency stop") {
        "estop"
    } else if m.contains("timer too close") || m.contains("rescheduled timer in the past") {
        "timer"
    } else if m.contains("lost communication with mcu") || m.contains("unable to connect") {
        "lost"
    } else if m.contains("adc out of range") {
        "adc"
    } else if m.contains("not heating at expected rate") {
        "heating"
    } else if m.contains("move out of range") {
        "range"
    } else {
        ""
    }
}

/// User output pins of this printer (`output_pin` not starting with `_`).
pub fn user_pins(available: &[String]) -> Vec<String> {
    available
        .iter()
        .filter(|n| is_user_pin(n))
        .cloned()
        .collect()
}

pub fn macro_names(available: &[String]) -> Vec<String> {
    let mut names: Vec<String> = available
        .iter()
        .filter_map(|n| n.strip_prefix("gcode_macro "))
        .filter(|n| !n.starts_with('_'))
        .map(str::to_owned)
        .collect();
    names.sort();
    names
}

/// Heater limits from the printer's own config; 0 = not known.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Limits {
    pub extruder_max: f64,
    pub bed_max: f64,
    pub nozzle_diameter: f64,
    /// Coldest nozzle that may extrude.
    pub min_extrude_temp: f64,
}

/// Read limits from `configfile.settings`.
pub fn parse_limits(settings: &Value) -> Limits {
    let num = |section: &str, key: &str| {
        settings
            .get(section)
            .and_then(|s| s.get(key))
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
    };
    Limits {
        extruder_max: num("extruder", "max_temp"),
        bed_max: num("heater_bed", "max_temp"),
        nozzle_diameter: num("extruder", "nozzle_diameter"),
        min_extrude_temp: num("extruder", "min_extrude_temp"),
    }
}

/// One G-code file in the `gcodes` root.
#[derive(Debug, Clone, PartialEq)]
pub struct FileEntry {
    /// Path relative to `gcodes`, as Moonraker takes it back.
    pub path: String,
    /// Unix time of the last change.
    pub modified: f64,
    pub size: u64,
}

fn is_gcode(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    [".gcode", ".gco", ".g"].iter().any(|e| lower.ends_with(e))
}

/// G-code files from `server.files.list`, newest first. Thumbnails and other
/// helper folders (`.thumbs`) are not files to print.
pub fn parse_file_list(result: &Value) -> Vec<FileEntry> {
    let mut files: Vec<FileEntry> = result
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|f| {
            let path = f.get("path")?.as_str()?;
            (is_gcode(path) && !path.split('/').any(|part| part.starts_with('.'))).then(|| {
                FileEntry {
                    path: path.to_owned(),
                    modified: f.get("modified").and_then(Value::as_f64).unwrap_or(0.0),
                    size: f.get("size").and_then(Value::as_u64).unwrap_or(0),
                }
            })
        })
        .collect();
    files.sort_by(|a, b| b.modified.total_cmp(&a.modified));
    files
}

/// The folder part of a path in `gcodes` ("" for the top).
pub fn folder_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// Where `path` goes when the user renames it to `name`: same folder, same
/// extension (added unless typed). None when the name cannot be a file name.
pub fn renamed(path: &str, name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() || name.contains('/') || name.starts_with('.') {
        return None;
    }
    let file = path.rsplit('/').next().unwrap_or(path);
    let ext = file.rfind('.').map_or("", |i| &file[i..]);
    let name = if !ext.is_empty()
        && name
            .to_ascii_lowercase()
            .ends_with(&ext.to_ascii_lowercase())
    {
        name.to_owned()
    } else {
        format!("{name}{ext}")
    };
    let dir = folder_of(path);
    let to = if dir.is_empty() {
        name
    } else {
        format!("{dir}/{name}")
    };
    (to != path).then_some(to)
}

/// `path` moved into `folder` ("" = the top), keeping its file name.
pub fn moved(path: &str, folder: &str) -> Option<String> {
    let file = path.rsplit('/').next().unwrap_or(path);
    let to = if folder.is_empty() {
        file.to_owned()
    } else {
        format!("{folder}/{file}")
    };
    (to != path).then_some(to)
}

/// The folders that hold files, sorted, without the top level; for "Move to
/// folder". Folders under `usb/` belong to the stick and stay with it.
pub fn folders(files: &[FileEntry], usb: bool) -> Vec<String> {
    let mut out: Vec<String> = files
        .iter()
        .map(|f| folder_of(&f.path))
        .filter(|d| !d.is_empty())
        .filter(|d| {
            let lower = d.to_ascii_lowercase();
            (lower == "usb" || lower.starts_with("usb/")) == usb
        })
        .map(str::to_owned)
        .collect();
    out.sort_by_key(|d| d.to_lowercase());
    out.dedup();
    out
}

/// One job of Moonraker's print queue.
#[derive(Debug, Clone, PartialEq)]
pub struct QueuedJob {
    pub id: String,
    /// Path in `gcodes`.
    pub path: String,
}

/// `server.job_queue.status`: "ready" (prints the next job when the printer is
/// free), "paused" (waits for a start), "loading".
#[derive(Debug, Clone, Default, PartialEq)]
pub struct JobQueue {
    pub state: String,
    pub jobs: Vec<QueuedJob>,
}

pub fn parse_job_queue(result: &Value) -> JobQueue {
    JobQueue {
        state: result
            .get("queue_state")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        jobs: result
            .get("queued_jobs")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .filter_map(|j| {
                Some(QueuedJob {
                    id: j.get("job_id")?.as_str()?.to_owned(),
                    path: j.get("filename")?.as_str()?.to_owned(),
                })
            })
            .collect(),
    }
}

/// A preview image the slicer embedded; the file lives next to the G-code.
#[derive(Debug, Clone, PartialEq)]
pub struct ThumbRef {
    pub width: u32,
    pub height: u32,
    /// Relative to the G-code file's folder, e.g. `.thumbs/x-300x300.png`.
    pub relative_path: String,
}

/// The smallest image at least `want` pixels wide, else the largest one.
/// Only PNG: that is what the decoder reads.
pub fn best_thumbnail(thumbs: &[ThumbRef], want: u32) -> Option<&ThumbRef> {
    let png = || {
        thumbs
            .iter()
            .filter(|t| t.relative_path.to_ascii_lowercase().ends_with(".png"))
    };
    png()
        .filter(|t| t.width >= want)
        .min_by_key(|t| t.width)
        .or_else(|| png().max_by_key(|t| t.width))
}

/// Path under the server's file root for a thumbnail of `file`, both
/// relative to `gcodes`.
pub fn thumbnail_path(file: &str, relative: &str) -> String {
    match file.rsplit_once('/') {
        Some((dir, _)) => format!("{dir}/{relative}"),
        None => relative.to_owned(),
    }
}

/// What the slicer wrote into a file, from `server.files.metadata`. Every
/// field is optional: slicers differ and some files carry none of it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileMeta {
    pub estimated_secs: Option<u64>,
    pub filament_type: Option<String>,
    pub filament_grams: Option<f64>,
    pub layers: Option<u32>,
    pub nozzle_diameter: Option<f64>,
    /// Height of the model, mm (`object_height`).
    pub height: Option<f64>,
    /// First filament colour the slicer wrote ("#4466CC"), if any.
    pub filament_color: Option<String>,
    pub thumbnails: Vec<ThumbRef>,
}

/// A per-filament slicer value as a list: OrcaSlicer may write a JSON list in
/// the string (`["PETG", "PETG"]`, names with `\u` escapes), PrusaSlicer and
/// others separate with `;`.
pub fn slicer_values(raw: &str) -> Vec<String> {
    let raw = raw.trim();
    let items: Vec<String> = if raw.starts_with('[') {
        serde_json::from_str::<Vec<Value>>(raw)
            .map(|v| {
                v.iter()
                    .map(|x| x.as_str().map_or_else(|| x.to_string(), str::to_owned))
                    .collect()
            })
            .unwrap_or_else(|_| vec![raw.trim_matches(|c| c == '[' || c == ']').to_owned()])
    } else {
        raw.split(';').map(str::to_owned).collect()
    };
    items
        .into_iter()
        .map(|s| s.trim().trim_matches('"').trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

/// The filament types of a file, each once: "PETG", "PLA, PETG".
pub fn slicer_types(raw: &str) -> String {
    let mut seen: Vec<String> = Vec::new();
    for t in slicer_values(raw) {
        if !seen.contains(&t) {
            seen.push(t);
        }
    }
    seen.join(", ")
}

/// The first filament's value (its name, its colour).
pub fn slicer_first(raw: &str) -> String {
    slicer_values(raw).into_iter().next().unwrap_or_default()
}

pub fn parse_file_meta(result: &Value) -> FileMeta {
    let num = |k: &str| result.get(k).and_then(Value::as_f64).filter(|v| *v > 0.0);
    FileMeta {
        estimated_secs: num("estimated_time").map(|v| v as u64),
        filament_color: result
            .get("filament_colors")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .filter(|c| !c.is_empty())
            .map(str::to_owned),
        filament_type: result
            .get("filament_type")
            .and_then(Value::as_str)
            .map(slicer_types)
            .filter(|s| !s.is_empty()),
        filament_grams: num("filament_weight_total"),
        layers: num("layer_count").map(|v| v as u32),
        nozzle_diameter: num("nozzle_diameter"),
        height: num("object_height"),
        thumbnails: result
            .get("thumbnails")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .filter_map(|t| {
                Some(ThumbRef {
                    width: t.get("width")?.as_u64()? as u32,
                    height: t.get("height")?.as_u64()? as u32,
                    relative_path: t.get("relative_path")?.as_str()?.to_owned(),
                })
            })
            .collect(),
    }
}

/// What the calibration screens show, from the saved config and live status.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CalibInfo {
    pub has_probe: bool,
    /// Z offset saved in the config (`probe.z_offset`), mm.
    pub z_offset: Option<f64>,
    pub probe_offset: [f64; 2],
    pub probe_samples: u32,
    pub has_mesh: bool,
    /// Loaded mesh profile ("" when none) and saved profile names.
    pub mesh_profile: String,
    pub mesh_profiles: Vec<String>,
    /// Probed heights, rows from the front (min Y) to the back.
    pub mesh: Vec<Vec<f64>>,
    pub has_screws: bool,
    /// Screws: name, X, Y, and the last result (turn "CW"/"CCW", "HH:MM", base).
    pub screws: Vec<Screw>,
    /// PID of the nozzle and the bed: Kp, Ki, Kd.
    pub pid: [Option<[f64; 3]>; 2],
    /// Input shaper per axis: type and frequency.
    pub shaper: [Option<(String, f64)>; 2],
    pub has_accel: bool,
    /// CoreXY: the two belts can be compared along the diagonals.
    pub corexy: bool,
    /// One resonance sweep (min_freq..max_freq at hz_per_sec), seconds.
    pub sweep_seconds: f64,
    /// The measured range, Hz (resonance_tester min_freq..max_freq).
    pub freq_range: (f64, f64),
    /// Stepper drivers: section ("tmc2209 stepper_x"), run current, quiet mode.
    pub drivers: Vec<(String, f64, bool)>,
    pub has_tare: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Screw {
    pub name: String,
    pub x: f64,
    pub y: f64,
    /// "CW" / "CCW" with "HH:MM" (turns:minutes of a turn); None before a check.
    pub turn: Option<(String, String)>,
    pub base: bool,
}

impl CalibInfo {
    pub fn read(cfg: &Value, status: &Value, objects: &[String]) -> Self {
        let has = |name: &str| objects.iter().any(|o| o == name);
        let num = |sec: &str, key: &str| cfg[sec][key].as_f64();
        let pid = |sec: &str| {
            Some([
                num(sec, "pid_kp")?,
                num(sec, "pid_ki")?,
                num(sec, "pid_kd")?,
            ])
        };
        let shaper = |axis: &str| {
            let t = cfg["input_shaper"][format!("shaper_type_{axis}")]
                .as_str()
                .or_else(|| cfg["input_shaper"]["shaper_type"].as_str())?;
            Some((
                t.to_ascii_uppercase(),
                num("input_shaper", &format!("shaper_freq_{axis}"))?,
            ))
        };
        let mesh_status = &status["bed_mesh"];
        let mesh: Vec<Vec<f64>> = mesh_status["probed_matrix"]
            .as_array()
            .map(|rows| {
                rows.iter()
                    .filter_map(|r| {
                        r.as_array()
                            .map(|r| r.iter().filter_map(Value::as_f64).collect::<Vec<_>>())
                    })
                    .filter(|r| !r.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let results = &status["screws_tilt_adjust"]["results"];
        let screws = (1..=8)
            .filter_map(|i| {
                let key = format!("screw{i}");
                let pos = cfg["screws_tilt_adjust"][&key].as_array()?;
                let r = &results[&key];
                Some(Screw {
                    name: cfg["screws_tilt_adjust"][format!("{key}_name")]
                        .as_str()
                        .map_or_else(|| format!("{i}"), str::to_owned),
                    x: pos.first()?.as_f64()?,
                    y: pos.get(1)?.as_f64()?,
                    turn: r["sign"]
                        .as_str()
                        .zip(r["adjust"].as_str())
                        .map(|(s, a)| (s.to_owned(), a.to_owned())),
                    base: r["is_base"].as_bool().unwrap_or(false),
                })
            })
            .collect();
        let drivers = objects
            .iter()
            .filter(|o| o.starts_with("tmc"))
            .map(|o| {
                let current = status[o]["run_current"]
                    .as_f64()
                    .or_else(|| num(o, "run_current"))
                    .unwrap_or(0.0);
                // StealthChop below this speed: "quiet" when it is set high.
                let quiet = num(o, "stealthchop_threshold").is_some_and(|t| t > 0.0);
                (o.clone(), current, quiet)
            })
            .collect();
        let mut profiles: Vec<String> = mesh_status["profiles"]
            .as_object()
            .map(|p| p.keys().cloned().collect())
            .unwrap_or_default();
        profiles.sort();
        Self {
            has_probe: has("probe")
                || objects
                    .iter()
                    .any(|o| o == "bltouch" || o.starts_with("probe_eddy") || o == "beacon"),
            z_offset: num("probe", "z_offset"),
            probe_offset: [
                num("probe", "x_offset").unwrap_or(0.0),
                num("probe", "y_offset").unwrap_or(0.0),
            ],
            probe_samples: num("probe", "samples").unwrap_or(1.0) as u32,
            has_mesh: has("bed_mesh"),
            mesh_profile: mesh_status["profile_name"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            mesh_profiles: profiles,
            mesh,
            has_screws: has("screws_tilt_adjust"),
            screws,
            pid: [pid("extruder"), pid("heater_bed")],
            shaper: [shaper("x"), shaper("y")],
            // Accelerometers are not listed among Klipper's objects; their
            // config sections are.
            has_accel: cfg.as_object().is_some_and(|c| {
                c.keys().any(|k| {
                    [
                        "adxl345", "lis2dw", "lis3dh", "mpu9250", "icm20948", "bmi160",
                    ]
                    .iter()
                    .any(|chip| k.starts_with(chip))
                })
            }) && cfg.get("resonance_tester").is_some(),
            corexy: cfg["printer"]["kinematics"].as_str() == Some("corexy"),
            freq_range: (
                cfg["resonance_tester"]["min_freq"].as_f64().unwrap_or(5.0),
                cfg["resonance_tester"]["max_freq"]
                    .as_f64()
                    .unwrap_or(133.33),
            ),
            sweep_seconds: {
                let rt = &cfg["resonance_tester"];
                let lo = rt["min_freq"].as_f64().unwrap_or(5.0);
                let hi = rt["max_freq"].as_f64().unwrap_or(133.33);
                let rate = rt["hz_per_sec"].as_f64().unwrap_or(1.0).max(0.1);
                (hi - lo).max(0.0) / rate
            },
            drivers,
            has_tare: has("gcode_macro LOAD_CELL_TARE"),
        }
    }
}

/// One input shaper Klipper fitted to a measurement.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShaperFit {
    pub name: String,
    pub freq: f64,
    /// Remaining vibrations, %.
    pub vibrations: f64,
    pub smoothing: f64,
    /// The highest acceleration that keeps smoothing acceptable, mm/s².
    pub max_accel: Option<f64>,
}

/// SHAPER_CALIBRATE's findings for one axis.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AxisShapers {
    pub fits: Vec<ShaperFit>,
    pub recommended: Option<(String, f64)>,
    /// The CSV Klipper wrote (frequency response and every shaper's curve).
    pub csv: Option<String>,
}

/// What resonance measurements printed to the console, gathered line by line.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Resonance {
    /// X and Y.
    pub axes: [AxisShapers; 2],
    /// TEST_RESONANCES CSVs of belt A and belt B (NAME=belt_a / belt_b).
    pub belts: [Option<String>; 2],
    current: Option<usize>,
}

impl Resonance {
    /// Feed one console line (as Moonraker sends it, with or without "// ").
    pub fn feed(&mut self, line: &str) {
        let line = line.trim_start_matches("//").trim();
        let axis_of = |a: &str| match a.trim() {
            "x" | "X" => Some(0),
            "y" | "Y" => Some(1),
            _ => None,
        };
        if let Some(rest) = line.strip_prefix("Calculating the best input shaper parameters for") {
            self.current = axis_of(rest.trim().trim_end_matches("axis"));
            if let Some(a) = self.current {
                self.axes[a] = AxisShapers::default();
            }
        } else if let Some(rest) = line.strip_prefix("Fitted shaper '") {
            // zv' frequency = 56.2 Hz (vibrations = 4.2%, smoothing ~= 0.075)
            let Some(a) = self.current else { return };
            let Some((name, rest)) = rest.split_once('\'') else {
                return;
            };
            let num = |key: &str| -> Option<f64> {
                let after = rest.split(key).nth(1)?;
                let t: String = after
                    .trim_start_matches([' ', '=', '~'])
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                t.parse().ok()
            };
            if let (Some(freq), Some(vibrations), Some(smoothing)) =
                (num("frequency"), num("vibrations"), num("smoothing"))
            {
                self.axes[a].fits.push(ShaperFit {
                    name: name.to_owned(),
                    freq,
                    vibrations,
                    smoothing,
                    max_accel: None,
                });
            }
        } else if let Some(rest) = line.strip_prefix("To avoid too much smoothing with '") {
            // mzv', suggested max_accel <= 13000 mm/sec^2
            let Some(a) = self.current else { return };
            let Some((name, rest)) = rest.split_once('\'') else {
                return;
            };
            let accel = rest
                .split("<=")
                .nth(1)
                .and_then(|t| t.split_whitespace().next())
                .and_then(|t| t.parse().ok());
            if let Some(fit) = self.axes[a].fits.iter_mut().rev().find(|f| f.name == name) {
                fit.max_accel = accel;
            }
        } else if let Some(rest) = line.strip_prefix("Recommended shaper_type_") {
            // x = mzv, shaper_freq_x = 56.4 Hz
            let Some((axis, rest)) = rest.split_once('=') else {
                return;
            };
            let Some(a) = axis_of(axis) else { return };
            let mut parts = rest.split(',');
            let name = parts.next().unwrap_or_default().trim().to_owned();
            let freq = parts
                .next()
                .and_then(|p| p.split('=').nth(1))
                .and_then(|f| f.split_whitespace().next())
                .and_then(|f| f.parse().ok());
            if let Some(freq) = freq {
                self.axes[a].recommended = Some((name, freq));
            }
        } else if let Some(rest) = line.strip_prefix("Shaper calibration data written to") {
            let path = rest.trim().trim_end_matches("file").trim();
            let a = self.current.or_else(|| {
                let file = path.rsplit('/').next().unwrap_or(path);
                if file.contains("_x_") {
                    Some(0)
                } else if file.contains("_y_") {
                    Some(1)
                } else {
                    None
                }
            });
            if let Some(a) = a {
                self.axes[a].csv = Some(path.to_owned());
            }
        } else if let Some(rest) = line.strip_prefix("Resonances data written to") {
            let path = rest.trim().trim_end_matches("file").trim();
            let file = path.rsplit('/').next().unwrap_or(path);
            if file.contains("belt_a") {
                self.belts[0] = Some(path.to_owned());
            } else if file.contains("belt_b") {
                self.belts[1] = Some(path.to_owned());
            }
        }
    }
}

/// A frequency response from one of Klipper's resonance CSVs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Psd {
    pub freq: Vec<f64>,
    /// psd_xyz: the sum over the three axes, what Klipper's graphs plot.
    pub psd: Vec<f64>,
    /// Each shaper's response (0..1 per frequency), keyed by name ("mzv").
    pub shapers: Vec<(String, Vec<f64>)>,
    /// Each shaper's frequency, from the header ("mzv(56.4)"), same order.
    pub shaper_freqs: Vec<f64>,
}

impl Psd {
    /// Parse "freq,psd_x,psd_y,psd_z,psd_xyz[,zv(56.2),...]".
    pub fn parse(text: &str) -> Option<Self> {
        let mut lines = text.lines();
        let head: Vec<&str> = lines.next()?.split(',').map(str::trim).collect();
        let col = |name: &str| head.iter().position(|h| *h == name);
        let (f, p) = (col("freq")?, col("psd_xyz")?);
        let shaper_cols: Vec<(usize, String)> = head
            .iter()
            .enumerate()
            .skip(p + 1)
            .map(|(i, h)| (i, h.split('(').next().unwrap_or(h).to_owned()))
            .collect();
        let mut out = Self {
            shapers: shaper_cols
                .iter()
                .map(|(_, n)| (n.clone(), Vec::new()))
                .collect(),
            shaper_freqs: head
                .iter()
                .skip(p + 1)
                .map(|h| {
                    h.split('(')
                        .nth(1)
                        .and_then(|f| f.trim_end_matches(')').parse().ok())
                        .unwrap_or(0.0)
                })
                .collect(),
            ..Self::default()
        };
        for line in lines {
            let cells: Vec<f64> = line
                .split(',')
                .map(|c| c.trim().parse().unwrap_or(f64::NAN))
                .collect();
            let (Some(&fr), Some(&ps)) = (cells.get(f), cells.get(p)) else {
                continue;
            };
            if !fr.is_finite() || !ps.is_finite() {
                continue;
            }
            out.freq.push(fr);
            out.psd.push(ps);
            for (k, (i, _)) in shaper_cols.iter().enumerate() {
                out.shapers[k].1.push(cells.get(*i).copied().unwrap_or(1.0));
            }
        }
        (!out.freq.is_empty()).then_some(out)
    }

    /// The frequency of the strongest response between lo and hi Hz.
    pub fn peak(&self, lo: f64, hi: f64) -> Option<f64> {
        self.freq
            .iter()
            .zip(&self.psd)
            .filter(|(f, _)| **f >= lo && **f <= hi)
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(f, _)| *f)
    }
}

/// The square corner velocity SHAPER_CALIBRATE uses unless given SCV=.
pub const SHAPER_SCV: f64 = 5.0;

/// Klipper's input shapers (klippy/extras/shaper_defs.py): impulse amplitudes
/// and times for a frequency, at the default damping ratio 0.1.
fn shaper_impulses(name: &str, freq: f64) -> Option<(Vec<f64>, Vec<f64>)> {
    const DAMPING: f64 = 0.1;
    // 1 / SHAPER_VIBRATION_REDUCTION
    const V_TOL: f64 = 1.0 / 20.0;
    let pi = std::f64::consts::PI;
    let df = (1.0 - DAMPING * DAMPING).sqrt();
    let t_d = 1.0 / (freq * df);
    let k = (-DAMPING * pi / df).exp();
    let shaper = match name {
        "zv" => (vec![1.0, k], vec![0.0, 0.5 * t_d]),
        "mzv" => {
            let k = (-0.75 * DAMPING * pi / df).exp();
            let a1 = 1.0 - 1.0 / 2f64.sqrt();
            let a2 = (2f64.sqrt() - 1.0) * k;
            let a3 = a1 * k * k;
            (vec![a1, a2, a3], vec![0.0, 0.375 * t_d, 0.75 * t_d])
        }
        "ei" => {
            let a1 = 0.25 * (1.0 + V_TOL);
            let a2 = 0.5 * (1.0 - V_TOL) * k;
            let a3 = a1 * k * k;
            (vec![a1, a2, a3], vec![0.0, 0.5 * t_d, t_d])
        }
        "2hump_ei" => {
            let v2 = V_TOL * V_TOL;
            let x = (v2 * ((1.0 - v2).sqrt() + 1.0)).powf(1.0 / 3.0);
            let a1 = (3.0 * x * x + 2.0 * x + 3.0 * v2) / (16.0 * x);
            let a2 = (0.5 - a1) * k;
            let a3 = a2 * k;
            let a4 = a1 * k * k * k;
            (vec![a1, a2, a3, a4], vec![0.0, 0.5 * t_d, t_d, 1.5 * t_d])
        }
        "3hump_ei" => {
            let k2 = k * k;
            let a1 = 0.0625 * (1.0 + 3.0 * V_TOL + 2.0 * (2.0 * (V_TOL + 1.0) * V_TOL).sqrt());
            let a2 = 0.25 * (1.0 - V_TOL) * k;
            let a3 = (0.5 * (1.0 + V_TOL) - 2.0 * a1) * k2;
            let a4 = a2 * k2;
            let a5 = a1 * k2 * k2;
            (
                vec![a1, a2, a3, a4, a5],
                vec![0.0, 0.5 * t_d, t_d, 1.5 * t_d, 2.0 * t_d],
            )
        }
        _ => return None,
    };
    Some(shaper)
}

/// Klipper's `_get_shaper_smoothing`: how far a corner gets rounded, mm.
fn shaper_smoothing(a: &[f64], t: &[f64], accel: f64, scv: f64) -> f64 {
    let half_accel = accel * 0.5;
    let inv_d = 1.0 / a.iter().sum::<f64>();
    let ts = a.iter().zip(t).map(|(a, t)| a * t).sum::<f64>() * inv_d;
    let (mut off_90, mut off_180) = (0.0, 0.0);
    for (a, t) in a.iter().zip(t) {
        if *t >= ts {
            off_90 += a * (scv + half_accel * (t - ts)) * (t - ts);
        }
        off_180 += a * half_accel * (t - ts) * (t - ts);
    }
    (off_90 * inv_d * 2f64.sqrt()).max(off_180 * inv_d)
}

/// Klipper's `find_shaper_max_accel`: the highest acceleration that keeps the
/// smoothing at or under 0.12 mm (its `_bisect`).
fn shaper_max_accel(a: &[f64], t: &[f64], scv: f64) -> f64 {
    let ok = |accel: f64| shaper_smoothing(a, t, accel, scv) <= 0.12;
    if !ok(1e-9) {
        return 0.0;
    }
    let (mut left, mut right) = (1.0_f64, 1.0_f64);
    while !ok(left) {
        right = left;
        left *= 0.5;
    }
    if right == left {
        while ok(right) {
            right *= 2.0;
        }
    }
    while right - left > 1e-8 {
        let mid = (left + right) * 0.5;
        if ok(mid) {
            left = mid;
        } else {
            right = mid;
        }
    }
    left
}

impl AxisShapers {
    /// What SHAPER_CALIBRATE printed, worked out again from the CSV it wrote:
    /// remaining vibrations, smoothing and max accel per shaper, and the pick
    /// by Klipper's rule (`find_best_shaper`). `scv`: [`SHAPER_SCV`] as Klipper.
    ///
    /// The CSV keeps each shaper's response at its fitted frequency, already
    /// the worst over Klipper's test damping ratios, so the figures come out
    /// as Klipper's up to rounding.
    pub fn from_csv(psd: &Psd, scv: f64) -> Self {
        let top = psd.psd.iter().fold(0.0_f64, |m, v| m.max(*v));
        // Vibrations under 1/20 of the peak do not count (SHAPER_VIBRATION_REDUCTION).
        let thr = top / 20.0;
        let all: f64 = psd.psd.iter().map(|p| (p - thr).max(0.0)).sum();
        let mut fits = Vec::new();
        // Index, score and smoothing of the best so far.
        let mut best: Option<(usize, f64, f64)> = None;
        for ((name, vals), freq) in psd.shapers.iter().zip(&psd.shaper_freqs) {
            let Some((a, t)) = shaper_impulses(name, *freq) else {
                continue;
            };
            let left: f64 = psd
                .psd
                .iter()
                .zip(vals)
                .map(|(p, v)| (v * p - thr).max(0.0))
                .sum();
            let vib = if all > 0.0 { left / all } else { 0.0 };
            let smoothing = shaper_smoothing(&a, &t, 5000.0, scv);
            let score = smoothing * (vib.powf(1.5) + vib * 0.2 + 0.01);
            // A later shaper wins only when clearly better.
            let better = match best {
                None => true,
                Some((_, s, sm)) => score * 1.2 < s || (score * 1.05 < s && smoothing * 1.1 < sm),
            };
            if better {
                best = Some((fits.len(), score, smoothing));
            }
            fits.push(ShaperFit {
                name: name.clone(),
                freq: *freq,
                vibrations: vib * 100.0,
                smoothing,
                max_accel: Some(shaper_max_accel(&a, &t, scv)),
            });
        }
        let recommended = best.map(|(i, _, _)| (fits[i].name.clone(), fits[i].freq));
        Self {
            fits,
            recommended,
            csv: None,
        }
    }
}

/// How a Wi-Fi network wants to be joined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiAuth {
    Open,
    /// WPA/WPA2 passphrase (WPA3 networks that also allow WPA2 count here).
    Psk,
    /// WPA3 only.
    Sae,
    /// WEP or enterprise: not offered.
    Other,
}

/// A network from wpa_supplicant's SCAN_RESULTS.
#[derive(Debug, Clone, PartialEq)]
pub struct WifiNet {
    pub ssid: String,
    /// MHz.
    pub freq: u32,
    /// dBm.
    pub signal: i32,
    pub auth: WifiAuth,
}

/// wpa_supplicant writes SSIDs with printf escapes: \xNN, \\, \", \e, \n…
pub fn wpa_unescape(s: &str) -> String {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && i + 1 < b.len() {
            match b[i + 1] {
                b'x' if i + 4 <= b.len() => {
                    if let Some(v) = s
                        .get(i + 2..i + 4)
                        .and_then(|h| u8::from_str_radix(h, 16).ok())
                    {
                        out.push(v);
                        i += 4;
                        continue;
                    }
                    out.push(b'\\');
                    i += 1;
                    continue;
                }
                b'n' => out.push(b'\n'),
                b'r' => out.push(b'\r'),
                b't' => out.push(b'\t'),
                b'e' => out.push(0x1b),
                c => out.push(c),
            }
            i += 2;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// SCAN_RESULTS → networks with a name, one per name (the strongest), the
/// strongest first.
pub fn parse_wifi_scan(text: &str) -> Vec<WifiNet> {
    let mut nets: Vec<WifiNet> = Vec::new();
    for line in text.lines().skip(1) {
        let cols: Vec<&str> = line.splitn(5, '\t').collect();
        let [_, freq, signal, flags, ssid] = cols[..] else {
            continue;
        };
        let ssid = wpa_unescape(ssid);
        if ssid.is_empty() || ssid.chars().all(|c| c == '\0') {
            continue;
        }
        let auth = if flags.contains("PSK") {
            WifiAuth::Psk
        } else if flags.contains("SAE") {
            WifiAuth::Sae
        } else if flags.contains("WEP") || flags.contains("EAP") {
            WifiAuth::Other
        } else {
            WifiAuth::Open
        };
        let net = WifiNet {
            ssid,
            freq: freq.parse().unwrap_or(0),
            signal: signal.parse().unwrap_or(-100),
            auth,
        };
        match nets.iter_mut().find(|n| n.ssid == net.ssid) {
            Some(n) if n.signal < net.signal => *n = net,
            Some(_) => {}
            None => nets.push(net),
        }
    }
    nets.sort_by_key(|n| -n.signal);
    nets
}

/// LIST_NETWORKS → (id, ssid, is it the current one).
pub fn parse_wifi_saved(text: &str) -> Vec<(u32, String, bool)> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let cols: Vec<&str> = line.split('\t').collect();
            Some((
                cols.first()?.parse().ok()?,
                wpa_unescape(cols.get(1)?),
                cols.get(3).is_some_and(|f| f.contains("[CURRENT]")),
            ))
        })
        .collect()
}

/// An SSID as SET_NETWORK takes it without quoting trouble: hex bytes.
pub fn wpa_hex(s: &str) -> String {
    s.bytes().map(|b| format!("{b:02x}")).collect()
}

/// "key=value" lines of STATUS.
pub fn parse_wpa_status(text: &str) -> std::collections::HashMap<String, String> {
    text.lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

/// The most layers a reading may move the count on (two seconds of printing).
const MAX_LAYER_STEP: u32 = 3;

/// Layers of a print whose slicer does not tell Klipper about them (no
/// `SET_PRINT_STATS_INFO` at layer changes): worked out from the height the
/// G-code sets and the file's first-layer and layer heights.
///
/// Travel moves hop Z for a moment, so a layer counts once the lowest Z over
/// the last two seconds has reached it. Counting starts when the nozzle is
/// down at the first layer (not while it waits up high for the bed).
#[derive(Debug, Clone)]
pub struct LayerTracker {
    first: f64,
    height: f64,
    count: u32,
    window: std::collections::VecDeque<(f64, f64)>,
    /// Since when Z has stayed at the first layer, before counting starts.
    low_since: Option<f64>,
    armed: bool,
    sent: u32,
}

impl LayerTracker {
    pub fn new(first: f64, height: f64, count: u32) -> Option<Self> {
        (first > 0.0 && height > 0.0 && count > 0).then(|| Self {
            first,
            height,
            count,
            window: std::collections::VecDeque::new(),
            low_since: None,
            armed: false,
            sent: 0,
        })
    }

    pub fn count(&self) -> u32 {
        self.count
    }

    /// A Z reading at `t` seconds. Returns the layers just reached, in order
    /// and none skipped (a pause at layer N waits for exactly N).
    pub fn update(&mut self, t: f64, z: f64) -> Vec<u32> {
        self.window.push_back((t, z));
        while self.window.front().is_some_and(|(t0, _)| t - t0 > 2.0) {
            self.window.pop_front();
        }
        let low = self
            .window
            .iter()
            .map(|w| w.1)
            .fold(f64::INFINITY, f64::min);
        if !self.armed {
            // At the first layer, and has stayed there for a moment.
            if z > 0.0 && z <= self.first + self.height / 2.0 {
                let since = *self.low_since.get_or_insert(t);
                if t - since < 1.0 {
                    return Vec::new();
                }
                self.armed = true;
            } else {
                self.low_since = None;
                return Vec::new();
            }
        }
        let layer = (((low - self.first) / self.height).round() as i64 + 1)
            .clamp(1, i64::from(self.count)) as u32;
        // Further than a few layers at once is the head going up (end of the
        // print, a park), not printing: it must not count, nor fire a pause.
        if layer > self.sent + MAX_LAYER_STEP {
            return Vec::new();
        }
        let new: Vec<u32> = (self.sent + 1..=layer).collect();
        self.sent = self.sent.max(layer);
        new
    }
}

/// First-layer height, layer height and layer count from a file's metadata.
pub fn layer_plan(meta: &Value) -> Option<(f64, f64, u32)> {
    let num = |k: &str| meta.get(k).and_then(Value::as_f64).filter(|v| *v > 0.0);
    let height = num("layer_height")?;
    let first = num("first_layer_height").unwrap_or(height);
    let count = num("layer_count")
        .or_else(|| num("object_height").map(|h| ((h - first) / height).round() + 1.0))?;
    Some((first, height, count as u32))
}

/// The parameters a `gcode_macro` reads (`params.NAME`, or `params["NAME"]`),
/// with the default its template gives (`|default(…)`), in order of first use.
pub fn macro_params(gcode: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut rest = gcode;
    while let Some(i) = rest.find("params") {
        rest = &rest[i + "params".len()..];
        let name: String = if let Some(r) = rest.strip_prefix('.') {
            r.chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect()
        } else if let Some(r) = rest.strip_prefix("[\"").or_else(|| rest.strip_prefix("['")) {
            r.chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect()
        } else {
            continue;
        };
        // `params` itself, `params.get(...)`, lower case helpers: not a parameter.
        if name.is_empty() || name.chars().any(|c| c.is_ascii_lowercase()) {
            continue;
        }
        let after = &rest[rest.find(&name).map_or(0, |k| k + name.len())..];
        let after = after.trim_start_matches(['"', '\'', ']']);
        // Only a literal default means something to the user: a number or a
        // quoted text (`default(pause_at_layer.layer)` is the template's own state).
        let default = after
            .trim_start()
            .strip_prefix("|default(")
            .or_else(|| after.trim_start().strip_prefix("| default("))
            .and_then(|d| d.split(')').next())
            .map(|d| d.split(',').next().unwrap_or(d).trim())
            .filter(|d| d.parse::<f64>().is_ok() || d.starts_with(['"', '\'']))
            .map(|d| d.trim_matches(['"', '\'']).to_owned())
            .unwrap_or_default();
        match out.iter_mut().find(|(n, _)| *n == name) {
            Some((_, d)) if d.is_empty() => *d = default,
            Some(_) => {}
            None => out.push((name, default)),
        }
    }
    out
}

/// How alike two belts' responses are, 0..100 %: the cosine of the two curves
/// between lo and hi Hz (1 when they have the same shape).
pub fn belt_similarity(a: &Psd, b: &Psd, lo: f64, hi: f64) -> Option<f64> {
    let (mut ab, mut aa, mut bb) = (0.0, 0.0, 0.0);
    for (f, pa) in a.freq.iter().zip(&a.psd) {
        if *f < lo || *f > hi {
            continue;
        }
        // The same bins in both files; take the nearest one to be safe.
        let k = b
            .freq
            .iter()
            .enumerate()
            .min_by(|x, y| (x.1 - f).abs().total_cmp(&(y.1 - f).abs()))?
            .0;
        let pb = b.psd[k];
        ab += pa * pb;
        aa += pa * pa;
        bb += pb * pb;
    }
    (aa > 0.0 && bb > 0.0).then(|| (ab / (aa.sqrt() * bb.sqrt()) * 100.0).clamp(0.0, 100.0))
}

/// "probe accuracy results: maximum …, range 0.004000, …" → the range, mm.
pub fn probe_range(line: &str) -> Option<f64> {
    let rest = line.split("probe accuracy results:").nth(1)?;
    let after = rest.split("range").nth(1)?;
    after
        .trim()
        .trim_start_matches(':')
        .split(',')
        .next()?
        .trim()
        .parse()
        .ok()
}

/// A message for the Messages screen. `time` 0: a standing condition (no date).
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    /// "done", "stopped", "failed", "save", "klipper", "moonraker", "news"
    pub kind: String,
    /// Words from the source (file name, warning text, announcement); the
    /// screen adds its own title for the kinds it knows.
    pub title: String,
    pub sub: String,
    pub time: f64,
}

/// Finished jobs from `server.history.list` (newest first) as messages.
pub fn job_messages(result: &Value) -> Vec<Message> {
    result
        .get("jobs")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|job| {
            let kind = match job.get("status")?.as_str()? {
                "completed" => "done",
                "cancelled" => "stopped",
                "in_progress" => return None,
                _ => "failed",
            };
            let file = job.get("filename")?.as_str()?;
            Some(Message {
                kind: kind.to_owned(),
                title: file.to_owned(),
                sub: String::new(),
                time: job.get("end_time").and_then(Value::as_f64).unwrap_or(0.0),
            })
        })
        .collect()
}

/// Standing conditions from Klipper's `configfile` and Moonraker's `server.info`.
pub fn standing_messages(configfile: &Value, server_info: &Value) -> Vec<Message> {
    let mut out = Vec::new();
    if configfile["save_config_pending"].as_bool() == Some(true) {
        out.push(Message {
            kind: "save".into(),
            title: String::new(),
            sub: String::new(),
            time: 0.0,
        });
    }
    let text = |v: &Value| {
        v.get("message")
            .and_then(Value::as_str)
            .or_else(|| v.as_str())
            .map(str::to_owned)
    };
    for w in configfile["warnings"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        if let Some(m) = text(w) {
            out.push(Message {
                kind: "klipper".into(),
                title: String::new(),
                sub: m,
                time: 0.0,
            });
        }
    }
    for w in server_info["warnings"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        if let Some(m) = text(w) {
            out.push(Message {
                kind: "moonraker".into(),
                title: String::new(),
                sub: m,
                time: 0.0,
            });
        }
    }
    out
}

/// Moonraker announcements not dismissed (`server.announcements.list`).
pub fn news_messages(result: &Value) -> Vec<Message> {
    result["entries"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter(|e| e["dismissed"].as_bool() != Some(true))
        .filter_map(|e| {
            Some(Message {
                kind: "news".into(),
                title: e["title"].as_str()?.to_owned(),
                sub: e["description"].as_str().unwrap_or_default().to_owned(),
                time: e["date"].as_f64().unwrap_or(0.0),
            })
        })
        .collect()
}

/// Last finished job from `server.history.list`.
#[derive(Debug, Clone, PartialEq)]
pub struct LastPrint {
    pub filename: String,
    /// Unix time the job ended.
    pub ended: f64,
    pub duration_secs: u64,
    pub completed: bool,
    /// Filament type from the slicer (`metadata.filament_type`), empty when not sent.
    pub material: String,
    /// Grams of filament the slicer planned (`metadata.filament_weight_total`), 0 if not sent.
    pub grams: f64,
    /// The slicer's filament profile name ("Elegoo Pink Pla"), else empty.
    pub filament_name: String,
    /// Its colour ("#FF80FF"), else empty.
    pub color: String,
    /// Its nozzle temperature, 0 when not sent.
    pub nozzle_temp: f64,
    /// Unix time the job started.
    pub started: f64,
}

/// Newest job out of a `server.history.list` result (newest first).
pub fn parse_last_print(result: &Value) -> Option<LastPrint> {
    let job = result.get("jobs")?.as_array()?.first()?;
    Some(LastPrint {
        filename: job.get("filename")?.as_str()?.to_owned(),
        ended: job.get("end_time").and_then(Value::as_f64).unwrap_or(0.0),
        duration_secs: job
            .get("total_duration")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            .max(0.0) as u64,
        completed: job.get("status").and_then(Value::as_str) == Some("completed"),
        material: job
            .pointer("/metadata/filament_type")
            .and_then(Value::as_str)
            .map(slicer_types)
            .unwrap_or_default(),
        grams: job
            .pointer("/metadata/filament_weight_total")
            .and_then(Value::as_f64)
            .unwrap_or(0.0),
        filament_name: job
            .pointer("/metadata/filament_name")
            .and_then(Value::as_str)
            .map(slicer_first)
            .unwrap_or_default(),
        color: ["/metadata/filament_colors/0", "/metadata/extruder_colors/0"]
            .iter()
            .find_map(|p| {
                job.pointer(p)
                    .and_then(Value::as_str)
                    .filter(|c| !c.is_empty())
            })
            .unwrap_or_default()
            .to_owned(),
        nozzle_temp: job
            .pointer("/metadata/filament_temps/0")
            .and_then(Value::as_f64)
            .or_else(|| {
                job.pointer("/metadata/first_layer_extr_temp")
                    .and_then(Value::as_f64)
            })
            .unwrap_or(0.0),
        started: job.get("start_time").and_then(Value::as_f64).unwrap_or(0.0),
    })
}

impl PrinterState {
    /// Replace everything with the full status from a subscribe response.
    pub fn replace_status(&mut self, status: Value) {
        self.status = status;
    }

    /// Merge a `notify_status_update` delta.
    pub fn apply_status(&mut self, delta: Value) {
        merge(&mut self.status, delta);
    }

    /// Add a console line, keeping the last 200.
    pub fn push_console(&mut self, text: String, kind: LineKind) {
        self.console.push((text, kind));
        if self.console.len() > 200 {
            self.console.remove(0);
        }
        self.console_revision += 1;
    }

    /// Follow a macro's `action:prompt_*` lines. Returns true when the shown
    /// prompt changed.
    pub fn prompt_line(&mut self, line: &str) -> bool {
        let Some(rest) = line.trim().strip_prefix("// action:prompt_") else {
            return false;
        };
        let (cmd, arg) = rest.split_once(' ').unwrap_or((rest, ""));
        let arg = arg.trim();
        match cmd {
            "begin" => {
                self.prompt_draft = Some(Prompt {
                    title: arg.to_owned(),
                    ..Prompt::default()
                });
                return false;
            }
            "text" => {
                if let Some(d) = self.prompt_draft.as_mut() {
                    d.text.push(arg.to_owned());
                }
                return false;
            }
            "button" | "footer_button" => {
                if let Some(d) = self.prompt_draft.as_mut() {
                    d.buttons.push(PromptButton::parse(arg));
                }
                return false;
            }
            "show" => self.prompt = self.prompt_draft.clone(),
            "end" => {
                self.prompt = None;
                self.prompt_draft = None;
            }
            _ => return false,
        }
        self.prompt_revision += 1;
        true
    }

    pub fn clear_console(&mut self) {
        self.console.clear();
        self.console_revision += 1;
    }

    /// Lights found on the printer (`led …` object names).
    pub fn set_leds(&mut self, leds: impl IntoIterator<Item = String>) {
        self.leds = leds.into_iter().collect();
    }

    /// Show a Z-offset change before Klipper reports it back.
    pub fn adjust_z_offset(&mut self, delta: f64) {
        let origin = self
            .status
            .get_mut("gcode_move")
            .and_then(|g| g.get_mut("homing_origin"))
            .and_then(Value::as_array_mut);
        if let Some(z) = origin.and_then(|o| o.get_mut(2)) {
            *z = json!(z.as_f64().unwrap_or(0.0) + delta);
        }
    }

    pub fn clear_status(&mut self) {
        self.status = Value::Null;
    }

    fn field(&self, object: &str, field: &str) -> Option<&Value> {
        self.status.get(object)?.get(field)
    }

    fn f64(&self, object: &str, field: &str) -> f64 {
        self.field(object, field)
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
    }

    fn str(&self, object: &str, field: &str) -> &str {
        self.field(object, field)
            .and_then(Value::as_str)
            .unwrap_or_default()
    }

    /// First three numbers of an `[x, y, z, e]` array.
    fn axes(&self, object: &str, field: &str) -> [f64; 3] {
        let mut out = [0.0; 3];
        if let Some(a) = self.field(object, field).and_then(Value::as_array) {
            for (o, v) in out.iter_mut().zip(a) {
                *o = v.as_f64().unwrap_or(0.0);
            }
        }
        out
    }

    fn heater(&self, object: &str) -> Heater {
        Heater {
            temperature: self.f64(object, "temperature"),
            target: self.f64(object, "target"),
        }
    }

    /// The part cooling fan's object. Z-Mod drives it as `fan_generic fanM106`
    /// (its M106 macro sets that one); else Klipper's own `fan`.
    fn part_fan(&self) -> Option<String> {
        self.fan_objects
            .iter()
            .find(|o| o.eq_ignore_ascii_case("fan_generic fanM106"))
            .cloned()
            .or_else(|| self.status.get("fan").map(|_| "fan".to_owned()))
    }

    fn fans(&self) -> Vec<Fan> {
        let mut fans = Vec::new();
        let part = self.part_fan();
        if let Some(object) = &part {
            fans.push(Fan {
                object: object.clone(),
                name: "fan".to_owned(),
                kind: FanKind::Part,
                percent: self.f64(object, "speed") * 100.0,
                trigger: FanTrigger::None,
            });
        }
        for object in &self.fan_objects {
            // The part fan is shown once, as the part fan.
            if part.as_deref() == Some(object.as_str()) {
                continue;
            }
            let Some((kind, name)) = object.split_once(' ') else {
                continue;
            };
            let (kind, trigger) = match kind {
                "fan_generic" => (FanKind::Generic, FanTrigger::None),
                "heater_fan" => (
                    FanKind::Auto,
                    FanTrigger::HeaterAbove(
                        self.fan_heater_temps.get(object).copied().unwrap_or(0.0),
                    ),
                ),
                "controller_fan" => (FanKind::Auto, FanTrigger::Motors),
                _ => (FanKind::Auto, FanTrigger::Holds(self.f64(object, "target"))),
            };
            fans.push(Fan {
                object: object.clone(),
                name: name.to_owned(),
                kind,
                percent: self.f64(object, "speed") * 100.0,
                trigger,
            });
        }
        fans
    }

    /// Chamber air circulation, told from the two circulation fans.
    fn air_mode(&self) -> Option<u8> {
        // A KlipperMod convention: only offered when the macros and both fans exist.
        let has = |m: &str| self.macros.iter().any(|n| n == m);
        let has_fan = |o: &str| self.fan_objects.iter().any(|n| n == o);
        if !(has("AIR_CIRCULATION_INTERNAL")
            && has("AIR_CIRCULATION_EXTERNAL")
            && has("AIR_CIRCULATION_STOP")
            && has_fan("fan_generic internal_fan")
            && has_fan("fan_generic external_fan"))
        {
            return None;
        }
        Some(if self.f64("fan_generic internal_fan", "speed") > 0.0 {
            1
        } else if self.f64("fan_generic external_fan", "speed") > 0.0 {
            2
        } else {
            0
        })
    }

    fn print_objects(&self) -> Vec<PrintObject> {
        let names = |field: &str| -> Vec<&str> {
            self.field("exclude_object", field)
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default()
        };
        let excluded = names("excluded_objects");
        let current = self.str("exclude_object", "current_object");
        self.field("exclude_object", "objects")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .filter_map(|o| {
                let name = o.get("name")?.as_str()?;
                let point = |p: &Value| Some([p.get(0)?.as_f64()?, p.get(1)?.as_f64()?]);
                let mut points: Vec<[f64; 2]> = o
                    .get("polygon")
                    .and_then(Value::as_array)
                    .map(|ps| ps.iter().filter_map(point).collect())
                    .unwrap_or_default();
                // No outline: a small square around the centre.
                if points.is_empty() {
                    let [x, y] = point(o.get("center")?)?;
                    points = vec![
                        [x - 5.0, y - 5.0],
                        [x + 5.0, y - 5.0],
                        [x + 5.0, y + 5.0],
                        [x - 5.0, y + 5.0],
                    ];
                }
                let min = points
                    .iter()
                    .fold([f64::MAX; 2], |m, p| [m[0].min(p[0]), m[1].min(p[1])]);
                let max = points
                    .iter()
                    .fold([f64::MIN; 2], |m, p| [m[0].max(p[0]), m[1].max(p[1])]);
                let center = o
                    .get("center")
                    .and_then(point)
                    .unwrap_or([(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0]);
                Some(PrintObject {
                    name: name.to_owned(),
                    polygon: points,
                    center,
                    min,
                    max,
                    excluded: excluded.contains(&name),
                    current: current == name,
                })
            })
            .collect()
    }

    pub fn snapshot(&self) -> Snapshot {
        // M73-driven progress wins over file-position progress when present.
        let display = self.f64("display_status", "progress");
        let progress = if display > 0.0 {
            display
        } else {
            self.f64("virtual_sdcard", "progress")
        };

        let duration = self.f64("print_stats", "print_duration");
        // Time left from the pace so far; too early in a print it is noise.
        let remaining_secs = (progress >= 0.02 && duration > 0.0)
            .then(|| (duration * (1.0 - progress) / progress) as u64);
        let layer = |field: &str| {
            self.field("print_stats", "info")
                .and_then(|i| i.get(field))
                .and_then(Value::as_u64)
                .map(|n| n as u32)
        };

        let light = self.leds.first().map(|object| {
            let channels = self
                .field(object, "color_data")
                .and_then(|c| c.get(0))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let level = channels
                .iter()
                .filter_map(Value::as_f64)
                .fold(0.0, f64::max);
            Light {
                name: object.trim_start_matches("led ").to_owned(),
                on: level > 0.0,
                level,
                white: channels.len() == 4,
            }
        });

        Snapshot {
            connection: self.connection,
            klippy: self.klippy,
            klippy_message: self.str("webhooks", "state_message").trim().to_owned(),
            extruder: self.heater("extruder"),
            bed: self.heater("heater_bed"),
            print_state: self.str("print_stats", "state").to_owned(),
            filename: self.str("print_stats", "filename").to_owned(),
            progress: progress.clamp(0.0, 1.0),
            print_duration: duration,
            remaining_secs,
            layer: layer("current_layer"),
            total_layers: layer("total_layer"),
            speed_percent: self.f64("gcode_move", "speed_factor") * 100.0,
            flow_percent: self.f64("gcode_move", "extrude_factor") * 100.0,
            z_offset: self
                .field("gcode_move", "homing_origin")
                .and_then(|o| o.get(2))
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
            pressure_advance: self.f64("extruder", "pressure_advance"),
            pa_smooth: self.f64("extruder", "smooth_time"),
            live_velocity: self.f64("motion_report", "live_velocity"),
            fan_percent: self
                .part_fan()
                .map_or(0.0, |o| self.f64(&o, "speed") * 100.0),
            fans: self.fans(),
            air_mode: self.air_mode(),
            light,
            caps: self.caps.clone(),
            retraction: Retraction {
                length: self.f64("firmware_retraction", "retract_length"),
                speed: self.f64("firmware_retraction", "retract_speed"),
                extra: self.f64("firmware_retraction", "unretract_extra_length"),
                unretract_speed: self.f64("firmware_retraction", "unretract_speed"),
            },
            pause_at_layer: self
                .field(PAUSE_MACRO, "pause_at_layer")
                .filter(|v| v["enable"].as_bool().unwrap_or(false))
                .and_then(|v| v["layer"].as_u64())
                .map(|n| n as u32),
            pause_next_layer: self
                .field(PAUSE_MACRO, "pause_next_layer")
                .is_some_and(|v| v["enable"].as_bool().unwrap_or(false)),
            pins: self
                .pins
                .iter()
                .map(|p| {
                    (
                        p.trim_start_matches("output_pin ").to_owned(),
                        self.f64(p, "value"),
                    )
                })
                .collect(),
            limits: self.limits,
            position: self.axes("toolhead", "position"),
            gcode_z: self
                .field("gcode_move", "gcode_position")
                .and_then(|p| p.get(2))
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
            homed_axes: self.str("toolhead", "homed_axes").to_owned(),
            axis_min: self.axes("toolhead", "axis_minimum"),
            axis_max: self.axes("toolhead", "axis_maximum"),
            can_extrude: self
                .field("extruder", "can_extrude")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            objects: self.print_objects(),
            runout: self.status.as_object().and_then(|objects| {
                objects
                    .iter()
                    .find(|(n, _)| is_filament_sensor(n))
                    .map(|(n, v)| {
                        (
                            n.split_once(' ')
                                .map_or(n.as_str(), |(_, name)| name)
                                .to_owned(),
                            v["enabled"].as_bool().unwrap_or(true),
                            v["filament_detected"].as_bool().unwrap_or(false),
                        )
                    })
            }),
            air: self
                .status
                .get("flashforge_tvoc")
                .filter(|v| v["status"].as_str().is_some_and(|s| s != "unknown"))
                .map(|v| {
                    [
                        v["tvoc"].as_f64().unwrap_or(0.0),
                        v["co2"].as_f64().unwrap_or(0.0),
                        v["hcho"].as_f64().unwrap_or(0.0),
                    ]
                }),
            load_g: self
                .field("flashforge_loadcell", "force_g")
                .and_then(Value::as_f64),
            board_temps: self
                .status
                .as_object()
                .map(|objects| {
                    objects
                        .iter()
                        .filter(|(n, _)| is_board_sensor(n))
                        .filter_map(|(n, v)| {
                            Some((
                                n.trim_start_matches("temperature_sensor ").to_owned(),
                                v["temperature"].as_f64()?,
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default(),
            probe_range: self.probe_range,
            max_velocity: self.f64("toolhead", "max_velocity"),
            max_accel: self.f64("toolhead", "max_accel"),
            corner_velocity: self.f64("toolhead", "square_corner_velocity"),
            cruise_ratio: self
                .field("toolhead", "minimum_cruise_ratio")
                .and_then(Value::as_f64),
            filament_out: self.status.as_object().is_some_and(|objects| {
                objects.iter().any(|(name, v)| {
                    is_filament_sensor(name)
                        && v["enabled"].as_bool().unwrap_or(true)
                        && v["filament_detected"].as_bool() == Some(false)
                })
            }),
        }
    }
}

/// Deep-merge `src` into `dst`. Objects merge key by key; anything else
/// (numbers, strings, arrays) replaces the old value, matching how Klipper
/// reports changed fields.
pub fn merge(dst: &mut Value, src: Value) {
    match (dst, src) {
        (dst @ &mut Value::Object(_), Value::Object(src)) => {
            let Some(dst) = dst.as_object_mut() else {
                unreachable!()
            };
            for (key, value) in src {
                merge(dst.entry(key).or_insert(Value::Null), value);
            }
        }
        (dst, src) => *dst = src,
    }
}

#[cfg(test)]
mod file_tests {
    use super::*;

    #[test]
    fn rename_keeps_folder_and_extension() {
        assert_eq!(
            renamed("a/Benchy.gcode", "Boat").as_deref(),
            Some("a/Boat.gcode")
        );
        assert_eq!(
            renamed("Benchy.gcode", " Boat.GCODE ").as_deref(),
            Some("Boat.GCODE")
        );
        assert_eq!(renamed("Benchy.gcode", "Benchy"), None);
        assert_eq!(renamed("Benchy.gcode", ""), None);
        assert_eq!(renamed("Benchy.gcode", "x/y"), None);
        assert_eq!(renamed("Benchy.gcode", ".hidden"), None);
    }

    #[test]
    fn move_and_folders() {
        assert_eq!(moved("a/b.gcode", "").as_deref(), Some("b.gcode"));
        assert_eq!(moved("b.gcode", "parts").as_deref(), Some("parts/b.gcode"));
        assert_eq!(moved("parts/b.gcode", "parts"), None);
        let f = |p: &str| FileEntry {
            path: p.into(),
            modified: 0.0,
            size: 0,
        };
        let files = [
            f("x.gcode"),
            f("Parts/a.gcode"),
            f("boxes/b.gcode"),
            f("Parts/c.gcode"),
            f("usb/d.gcode"),
            f("USB/e/f.gcode"),
        ];
        assert_eq!(folders(&files, false), ["boxes", "Parts"]);
        assert_eq!(folders(&files, true), ["usb", "USB/e"]);
    }

    #[test]
    fn job_queue() {
        let q = parse_job_queue(&serde_json::json!({
            "queue_state": "paused",
            "queued_jobs": [
                { "filename": "Benchy.gcode", "job_id": "0001", "time_added": 1.0 },
                { "filename": "bad" }
            ]
        }));
        assert_eq!(q.state, "paused");
        assert_eq!(
            q.jobs,
            [QueuedJob {
                id: "0001".into(),
                path: "Benchy.gcode".into()
            }]
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_shutdowns() {
        assert_eq!(error_kind("MCU 'mcu' shutdown: Timer too close"), "timer");
        assert_eq!(error_kind("Lost communication with MCU 'mcu'"), "lost");
        assert_eq!(error_kind("Something else"), "");
        assert_eq!(error_kind("Shutdown due to webhooks request"), "estop");
        assert_eq!(error_kind("Shutdown due to M112 command"), "estop");
    }

    #[test]
    fn messages_from_sources() {
        let jobs = job_messages(&serde_json::json!({ "jobs": [
            { "filename": "a.gcode", "status": "completed", "end_time": 10.0 },
            { "filename": "b.gcode", "status": "cancelled", "end_time": 5.0 },
            { "filename": "c.gcode", "status": "klippy_shutdown", "end_time": 1.0 },
            { "filename": "d.gcode", "status": "in_progress" }
        ]}));
        let kinds: Vec<&str> = jobs.iter().map(|m| m.kind.as_str()).collect();
        assert_eq!(kinds, ["done", "stopped", "failed"]);
        let standing = standing_messages(
            &serde_json::json!({ "save_config_pending": true, "warnings": [{ "type": "deprecated_option", "message": "Option 'x' is deprecated" }] }),
            &serde_json::json!({ "warnings": ["Moonraker is old"] }),
        );
        let kinds: Vec<&str> = standing.iter().map(|m| m.kind.as_str()).collect();
        assert_eq!(kinds, ["save", "klipper", "moonraker"]);
        assert_eq!(standing[1].sub, "Option 'x' is deprecated");
        let news = news_messages(&serde_json::json!({ "entries": [
            { "title": "New", "description": "d", "date": 3.0, "dismissed": false },
            { "title": "Old", "description": "d", "date": 2.0, "dismissed": true }
        ]}));
        assert_eq!(news.len(), 1);
    }

    #[test]
    fn print_objects() {
        let mut st = PrinterState::default();
        st.apply_status(serde_json::json!({ "exclude_object": {
            "objects": [
                { "name": "CASE.STL_ID_0_COPY_0", "center": [50.0, 50.0],
                  "polygon": [[40.0, 45.0], [60.0, 45.0], [60.0, 55.0], [40.0, 55.0]] },
                { "name": "CLIP", "center": [-10.0, 20.0] }
            ],
            "excluded_objects": ["CLIP"],
            "current_object": "CASE.STL_ID_0_COPY_0"
        }}));
        let o = st.snapshot().objects;
        assert_eq!(o.len(), 2);
        assert_eq!((o[0].min, o[0].max), ([40.0, 45.0], [60.0, 55.0]));
        assert!(o[0].current && !o[0].excluded);
        assert_eq!((o[1].min, o[1].max), ([-15.0, 15.0], [-5.0, 25.0]));
        assert!(o[1].excluded);
        assert_eq!(object_title("CASE.STL_ID_0_COPY_0"), "CASE");
        assert_eq!(object_title("CASE.STL_ID_0_COPY_1"), "CASE 2");
        assert_eq!(object_title("Fan_duct_id_2_copy_0"), "Fan duct");
        assert_eq!(object_title("CLIP"), "CLIP");
    }

    #[test]
    fn macro_prompt() {
        let mut st = PrinterState::default();
        for line in [
            "// action:prompt_begin Filament change",
            "// action:prompt_text Insert the new strand.",
            "// action:prompt_button Unload|UNLOAD_FILAMENT|secondary",
            "// action:prompt_button_group_start",
            "// action:prompt_footer_button Resume|RESUME|primary",
            "// action:prompt_button Close",
        ] {
            assert!(!st.prompt_line(line));
        }
        assert!(st.prompt.is_none());
        assert!(st.prompt_line("// action:prompt_show"));
        let p = st.prompt.clone().unwrap();
        assert_eq!(p.title, "Filament change");
        assert_eq!(p.text, ["Insert the new strand."]);
        assert_eq!(p.buttons.len(), 3);
        assert_eq!(
            (p.buttons[1].gcode.as_str(), p.buttons[1].style.as_str()),
            ("RESUME", "primary")
        );
        assert_eq!(p.buttons[2].gcode, "Close");
        assert!(st.prompt_line("// action:prompt_end"));
        assert!(st.prompt.is_none());
        assert!(!st.prompt_line("ok"));
    }

    #[test]
    fn calibration_info() {
        let cfg: Value =
            serde_json::from_str(include_str!("../tests/fixtures/ad5m-pro/configfile.json"))
                .unwrap();
        let cfg = cfg.get("result").unwrap_or(&cfg);
        let cfg = &cfg["status"]["configfile"]["settings"];
        let objects: Vec<String> = [
            "probe",
            "bed_mesh",
            "screws_tilt_adjust",
            "tmc2209 stepper_x",
            "gcode_macro LOAD_CELL_TARE",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let status = serde_json::json!({
            "bed_mesh": { "profile_name": "", "profiles": { "auto": {} }, "probed_matrix": [[0.1, 0.2], [0.0, -0.1]] },
            "screws_tilt_adjust": { "results": { "screw2": { "z": 2.0, "sign": "CW", "adjust": "00:15", "is_base": false } } }
        });
        let i = CalibInfo::read(cfg, &status, &objects);
        assert!(i.has_probe && i.has_mesh && i.has_screws && i.has_tare);
        // adxl345 on the eboard: not among Klipper's objects, only in the config.
        assert!(i.has_accel && i.corexy);
        assert_eq!(i.sweep_seconds, 95.0);
        assert_eq!(i.z_offset, Some(-0.275));
        assert_eq!(i.mesh_profiles, ["auto"]);
        assert_eq!(i.mesh.len(), 2);
        assert_eq!(i.screws.len(), 4);
        assert_eq!(i.screws[0].name, "Left Near");
        assert_eq!(i.screws[1].turn, Some(("CW".into(), "00:15".into())));
        assert_eq!(i.shaper[0], Some(("MZV".into(), 56.4)));
        assert_eq!(i.pid[0].map(|p| p[0]), Some(39.652));
        assert_eq!(i.drivers[0].1, 0.9);
        assert_eq!(
            probe_range(
                "// probe accuracy results: maximum 2.5, minimum 2.4, range 0.004000, average 2.45"
            ),
            Some(0.004)
        );
    }

    #[test]
    fn sensors_and_limits() {
        let mut st = PrinterState::default();
        st.apply_status(serde_json::json!({
            "flashforge_tvoc": { "tvoc": 0, "co2": 0, "hcho": 0, "status": "unknown" },
            "flashforge_loadcell": { "force_g": -40 },
            "filament_switch_sensor runout_sensor": { "filament_detected": true, "enabled": true },
            "temperature_sensor mcu_temp": { "temperature": 41.5 },
            "toolhead": { "max_velocity": 600.0, "max_accel": 20000.0, "minimum_cruise_ratio": 0.5, "square_corner_velocity": 9.0 }
        }));
        let s = st.snapshot();
        assert_eq!(s.runout, Some(("runout_sensor".into(), true, true)));
        assert_eq!(s.air, None);
        assert_eq!(s.load_g, Some(-40.0));
        assert_eq!(s.board_temps, [("mcu_temp".to_owned(), 41.5)]);
        assert_eq!(
            (
                s.max_velocity,
                s.max_accel,
                s.corner_velocity,
                s.cruise_ratio
            ),
            (600.0, 20000.0, 9.0, Some(0.5))
        );
        let sub = subscription_for(&[
            "flashforge_tvoc".into(),
            "temperature_sensor mcu_temp".into(),
            "temperature_sensor Load_Cell".into(),
        ]);
        assert!(sub["objects"].get("temperature_sensor mcu_temp").is_some());
        assert!(sub["objects"].get("temperature_sensor Load_Cell").is_none());
    }

    #[test]
    fn runout_sensor() {
        let mut st = PrinterState::default();
        st.apply_status(serde_json::json!({
            "filament_switch_sensor runout_sensor": { "enabled": true, "filament_detected": true }
        }));
        assert!(!st.snapshot().filament_out);
        st.apply_status(serde_json::json!({
            "filament_switch_sensor runout_sensor": { "filament_detected": false }
        }));
        assert!(st.snapshot().filament_out);
        st.apply_status(serde_json::json!({
            "filament_switch_sensor runout_sensor": { "enabled": false }
        }));
        assert!(!st.snapshot().filament_out);
    }

    #[test]
    fn pins_switched_by_macros() {
        let config = json!({
            "gcode_macro EXTRUDER_ON": { "gcode": "\nSET_PIN PIN=extruder_heater VALUE=1" },
            "gcode_macro LIGHT": { "gcode": "SET_LED LED=x WHITE=1" },
            "output_pin extruder_heater": { "pin": "PB7" }
        });
        assert_eq!(
            macro_managed_pins(&config),
            vec!["EXTRUDER_HEATER".to_owned()]
        );
    }

    #[test]
    fn merge_keeps_untouched_fields() {
        let mut state = PrinterState::default();
        state.replace_status(json!({
            "extruder": { "temperature": 25.0, "target": 0.0 },
            "heater_bed": { "temperature": 24.0, "target": 0.0 },
        }));
        state.apply_status(json!({ "extruder": { "target": 210.0 } }));

        let snap = state.snapshot();
        assert_eq!(
            snap.extruder,
            Heater {
                temperature: 25.0,
                target: 210.0
            }
        );
        assert_eq!(snap.bed.temperature, 24.0);
    }

    #[test]
    fn arrays_are_replaced_not_merged() {
        let mut v = json!({ "a": [1, 2, 3] });
        merge(&mut v, json!({ "a": [4] }));
        assert_eq!(v, json!({ "a": [4] }));
    }

    #[test]
    fn display_progress_overrides_sdcard() {
        let mut state = PrinterState::default();
        state.replace_status(json!({
            "virtual_sdcard": { "progress": 0.30 },
            "display_status": { "progress": 0.0 },
        }));
        assert_eq!(state.snapshot().progress, 0.30);

        state.apply_status(json!({ "display_status": { "progress": 0.42 } }));
        assert_eq!(state.snapshot().progress, 0.42);
    }

    #[test]
    fn remaining_time_and_layers() {
        let mut state = PrinterState::default();
        state.replace_status(json!({
            "virtual_sdcard": { "progress": 0.25 },
            "print_stats": { "print_duration": 600.0,
                             "info": { "current_layer": null, "total_layer": null } },
        }));
        let snap = state.snapshot();
        assert_eq!(snap.remaining_secs, Some(1800));
        // The slicer did not send layers: nothing to show.
        assert_eq!((snap.layer, snap.total_layers), (None, None));

        state.apply_status(json!({
            "print_stats": { "info": { "current_layer": 76, "total_layer": 180 } },
        }));
        let snap = state.snapshot();
        assert_eq!((snap.layer, snap.total_layers), (Some(76), Some(180)));

        // Too early to estimate.
        state.apply_status(json!({ "virtual_sdcard": { "progress": 0.005 } }));
        assert_eq!(state.snapshot().remaining_secs, None);
    }

    #[test]
    fn last_print_from_history() {
        let result = json!({ "count": 2, "jobs": [
            { "job_id": "000102", "filename": "fan_duct.gcode", "status": "completed",
              "end_time": 1762600000.5, "total_duration": 4980.2, "print_duration": 4900.0 },
            { "job_id": "000101", "filename": "old.gcode", "status": "cancelled" },
        ]});
        let last = parse_last_print(&result).unwrap();
        assert_eq!(last.filename, "fan_duct.gcode");
        assert_eq!(last.duration_secs, 4980);
        assert!(last.completed);
        assert_eq!(parse_last_print(&json!({ "jobs": [] })), None);
    }

    #[test]
    fn tuning_values_and_light() {
        let mut state = PrinterState {
            leds: vec!["led chamber_led".into()],
            ..Default::default()
        };
        state.replace_status(json!({
            "gcode_move": { "speed_factor": 1.1, "extrude_factor": 0.95,
                            "homing_origin": [0.0, 0.0, -0.12, 0.0] },
            "extruder": { "pressure_advance": 0.035 },
            "fan": { "speed": 0.6 },
            "led chamber_led": { "color_data": [[0.0, 0.0, 0.0, 0.8]] },
        }));
        let snap = state.snapshot();
        assert_eq!(snap.speed_percent.round(), 110.0);
        assert_eq!(snap.flow_percent.round(), 95.0);
        assert_eq!(snap.z_offset, -0.12);
        assert_eq!(snap.pressure_advance, 0.035);
        assert_eq!(snap.fan_percent.round(), 60.0);
        let light = snap.light.unwrap();
        assert_eq!(light.name, "chamber_led");
        assert!(light.on && light.white);
        assert_eq!(light.level, 0.8);

        state.apply_status(json!({ "led chamber_led": { "color_data": [[0.0, 0.0, 0.0, 0.0]] } }));
        assert!(!state.snapshot().light.unwrap().on);
    }

    #[test]
    fn subscription_adds_only_leds_that_exist() {
        let names = ["extruder".to_owned(), "led chamber_led".to_owned()];
        let sub = subscription_for(&names);
        assert!(sub["objects"].get("led chamber_led").is_some());
        assert!(sub["objects"].get("extruder").is_some());
        assert!(subscription_for(&[])["objects"]
            .get("led chamber_led")
            .is_none());
    }

    #[test]
    fn z_offset_adjust_is_shown_at_once() {
        let mut state = PrinterState::default();
        state.replace_status(json!({ "gcode_move": { "homing_origin": [0.0, 0.0, -0.12, 0.0] } }));
        state.adjust_z_offset(0.05);
        assert!((state.snapshot().z_offset + 0.07).abs() < 1e-9);
    }

    #[test]
    fn file_list_is_gcode_only_newest_first() {
        let result = json!([
            { "path": "old.gcode", "modified": 100.0, "size": 10 },
            { "path": "sub/new.GCODE", "modified": 300.5, "size": 20 },
            { "path": ".thumbs/new-300x300.png", "modified": 400.0, "size": 5 },
            { "path": ".hidden/x.gcode", "modified": 500.0, "size": 5 },
            { "path": "readme.txt", "modified": 600.0, "size": 5 },
        ]);
        let files = parse_file_list(&result);
        let paths: Vec<_> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["sub/new.GCODE", "old.gcode"]);
        assert_eq!(files[0].size, 20);
        assert!(parse_file_list(&Value::Null).is_empty());
    }

    #[test]
    fn multi_filament_values_from_any_slicer() {
        // OrcaSlicer: a JSON list in the string, names escaped.
        assert_eq!(slicer_types(r#"["PETG", "PETG"]"#), "PETG");
        assert_eq!(slicer_types(r#"["PLA","PETG","PLA"]"#), "PLA, PETG");
        assert_eq!(
            slicer_first(r#"["\u041d\u0418\u0422 PETG", "Other"]"#),
            "НИТ PETG"
        );
        // PrusaSlicer and others: `;`.
        assert_eq!(slicer_types("PLA;PLA;PETG"), "PLA, PETG");
        assert_eq!(slicer_first("Generic PLA; Generic PETG"), "Generic PLA");
        // One plain value, empty.
        assert_eq!(slicer_types("PETG"), "PETG");
        assert_eq!(slicer_types(""), "");
        assert_eq!(slicer_types("[]"), "");
    }

    #[test]
    fn file_metadata_fields_are_optional() {
        let meta = parse_file_meta(&json!({
            "estimated_time": 4980.4, "filament_type": "PETG",
            "filament_weight_total": 18.3, "layer_count": 180, "nozzle_diameter": 0.4,
        }));
        assert_eq!(meta.estimated_secs, Some(4980));
        assert_eq!(meta.filament_type.as_deref(), Some("PETG"));
        assert_eq!(meta.layers, Some(180));
        assert_eq!(meta.nozzle_diameter, Some(0.4));
        // A file without slicer data: nothing to show, nothing made up.
        assert_eq!(
            parse_file_meta(&json!({ "filament_type": "", "estimated_time": 0 })),
            FileMeta::default()
        );
    }

    #[test]
    fn thumbnails_from_metadata_and_choice() {
        let meta = parse_file_meta(&json!({ "thumbnails": [
            { "width": 32, "height": 32, "size": 1, "relative_path": ".thumbs/x-32x32.png" },
            { "width": 300, "height": 300, "size": 2, "relative_path": ".thumbs/x-300x300.png" },
            { "width": 600, "height": 600, "size": 3, "relative_path": ".thumbs/x-600.jpg" },
            { "width": 220, "height": 124, "size": 4, "relative_path": ".thumbs/x-220x124.png" },
        ]}));
        assert_eq!(meta.thumbnails.len(), 4);
        // 200 wanted: the smallest PNG that is wide enough, not the jpg.
        assert_eq!(best_thumbnail(&meta.thumbnails, 200).unwrap().width, 220);
        // Nothing wide enough: the largest PNG.
        assert_eq!(best_thumbnail(&meta.thumbnails, 900).unwrap().width, 300);
        assert!(best_thumbnail(&[], 200).is_none());
    }

    #[test]
    fn thumbnail_lives_next_to_the_file() {
        assert_eq!(
            thumbnail_path("a.gcode", ".thumbs/a-300x300.png"),
            ".thumbs/a-300x300.png"
        );
        assert_eq!(
            thumbnail_path("sub/dir/a.gcode", ".thumbs/a-300x300.png"),
            "sub/dir/.thumbs/a-300x300.png"
        );
    }

    #[test]
    fn limits_come_from_config() {
        let l = parse_limits(&json!({
            "extruder": { "max_temp": 350.0, "nozzle_diameter": 0.4 },
            "heater_bed": { "max_temp": 130.0 },
        }));
        assert_eq!(
            (l.extruder_max, l.bed_max, l.nozzle_diameter),
            (350.0, 130.0, 0.4)
        );
        assert_eq!(parse_limits(&Value::Null), Limits::default());
    }

    #[test]
    fn toolhead_position_axes_and_extrude() {
        let mut state = PrinterState::default();
        state.replace_status(json!({
            "toolhead": { "position": [10.5, -20.0, 3.25, 7.0], "homed_axes": "xy",
                          "axis_minimum": [-112.0, -112.0, -10.0, 0.0],
                          "axis_maximum": [112.0, 112.0, 230.0, 0.0] },
            "extruder": { "can_extrude": true },
        }));
        let s = state.snapshot();
        assert_eq!(s.position, [10.5, -20.0, 3.25]);
        assert_eq!(s.homed_axes, "xy");
        assert_eq!((s.axis_min[2], s.axis_max[2]), (-10.0, 230.0));
        assert!(s.can_extrude);
        assert_eq!(PrinterState::default().snapshot().homed_axes, "");
    }

    #[test]
    fn console_lines_and_macros() {
        assert_eq!(line_kind("!! Move out of range"), LineKind::Error);
        assert_eq!(line_kind("// Notice"), LineKind::Notice);
        assert_eq!(line_kind("ok T:24.1"), LineKind::Response);
        let lines = parse_gcode_store(&json!({ "gcode_store": [
            { "message": "G28", "time": 1.0, "type": "command" },
            { "message": "!! Must home axis first", "time": 2.0, "type": "response" },
        ]}));
        assert_eq!(lines[0], ("> G28".to_owned(), LineKind::Command));
        assert_eq!(lines[1].1, LineKind::Error);

        let all = [
            "extruder",
            "gcode_macro PAUSE",
            "gcode_macro _HELPER",
            "gcode_macro BELTS",
        ]
        .map(String::from);
        assert_eq!(macro_names(&all), ["BELTS", "PAUSE"]);
    }

    #[test]
    fn console_keeps_the_last_200_lines() {
        let mut state = PrinterState::default();
        for i in 0..250 {
            state.push_console(format!("line {i}"), LineKind::Response);
        }
        assert_eq!(state.console.len(), 200);
        assert_eq!(state.console[0].0, "line 50");
        let rev = state.console_revision;
        state.clear_console();
        assert!(state.console.is_empty() && state.console_revision > rev);
    }

    #[test]
    fn z_mod_part_fan_is_fan_m106() {
        // Z-Mod's M106 drives fan_generic fanM106: that is the part fan, shown
        // once, and the Fans screen's part fan follows it.
        let mut state = PrinterState {
            fan_objects: vec![
                "fan_generic fanM106".to_owned(),
                "fan_generic internal_fan".to_owned(),
            ],
            ..Default::default()
        };
        state.replace_status(json!({
            "fan": { "speed": 0.0 },
            "fan_generic fanM106": { "speed": 1.0 },
            "fan_generic internal_fan": { "speed": 0.0 },
        }));
        let snap = state.snapshot();
        assert_eq!(snap.fan_percent.round(), 100.0);
        let parts: Vec<_> = snap
            .fans
            .iter()
            .filter(|f| f.kind == FanKind::Part)
            .collect();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].object, "fan_generic fanM106");
        assert!(!snap
            .fans
            .iter()
            .any(|f| f.kind != FanKind::Part && f.object == "fan_generic fanM106"));
    }

    fn fans_of_the_ad5m_pro() {
        let available = [
            "fan",
            "heater_fan hotend_fan",
            "controller_fan stepper_driver_fan",
            "fan_generic internal_fan",
            "fan_generic chamber_fan",
            "fan_generic external_fan",
            "gcode_macro AIR_CIRCULATION_INTERNAL",
            "gcode_macro AIR_CIRCULATION_EXTERNAL",
            "gcode_macro AIR_CIRCULATION_STOP",
        ]
        .map(String::from);
        // Settable fans first, then the automatic ones, each group by name.
        let names = fan_object_names(&available);
        assert_eq!(
            names,
            [
                "fan_generic chamber_fan",
                "fan_generic external_fan",
                "fan_generic internal_fan",
                "controller_fan stepper_driver_fan",
                "heater_fan hotend_fan",
            ]
        );
        let sub = subscription_for(&available);
        assert!(sub["objects"]["fan_generic chamber_fan"].is_null());
        assert!(sub["objects"].get("heater_fan hotend_fan").is_some());

        let mut state = PrinterState {
            fan_objects: names,
            macros: macro_names(&available),
            ..Default::default()
        };
        state.fan_heater_temps = parse_fan_heater_temps(&json!({
            "heater_fan hotend_fan": { "heater_temp": 50.0 },
            "fan": { "pin": "x" },
        }));
        state.replace_status(json!({
            "fan": { "speed": 0.6 },
            "fan_generic chamber_fan": { "speed": 0.0 },
            "fan_generic internal_fan": { "speed": 0.0 },
            "fan_generic external_fan": { "speed": 1.0 },
            "heater_fan hotend_fan": { "speed": 1.0 },
            "controller_fan stepper_driver_fan": { "speed": 0.0 },
        }));
        let snap = state.snapshot();
        let by = |o: &str| snap.fans.iter().find(|f| f.object == o).unwrap().clone();
        assert_eq!(snap.fans[0].object, "fan");
        assert_eq!(snap.fans[0].kind, FanKind::Part);
        assert_eq!(snap.fans[0].percent.round(), 60.0);
        assert_eq!(by("fan_generic external_fan").percent, 100.0);
        assert_eq!(by("fan_generic chamber_fan").name, "chamber_fan");
        assert_eq!(
            by("heater_fan hotend_fan").trigger,
            FanTrigger::HeaterAbove(50.0)
        );
        assert_eq!(
            by("controller_fan stepper_driver_fan").trigger,
            FanTrigger::Motors
        );
        assert_eq!(snap.fans.len(), 6);
        // Exhaust runs → air mode 2; macros missing → no air mode at all.
        assert_eq!(snap.air_mode, Some(2));
        state.macros.clear();
        assert_eq!(state.snapshot().air_mode, None);
    }

    #[test]
    fn parses_klippy_states() {
        assert_eq!(Klippy::parse("ready"), Klippy::Ready);
        assert_eq!(Klippy::parse("shutdown"), Klippy::Shutdown);
        assert_eq!(Klippy::parse("weird"), Klippy::Unknown);
    }

    #[test]
    fn resonance_reads_shaper_calibrate_output() {
        let mut r = Resonance::default();
        for line in [
            "// Calculating the best input shaper parameters for x axis",
            "// Fitted shaper 'zv' frequency = 56.2 Hz (vibrations = 4.2%, smoothing ~= 0.075)",
            "// To avoid too much smoothing with 'zv', suggested max_accel <= 13000 mm/sec^2",
            "// Fitted shaper 'mzv' frequency = 54.2 Hz (vibrations = 0.7%, smoothing ~= 0.090)",
            "// To avoid too much smoothing with 'mzv', suggested max_accel <= 9700 mm/sec^2",
            "// Recommended shaper_type_x = mzv, shaper_freq_x = 54.2 Hz",
            "// Shaper calibration data written to /tmp/calibration_data_x_20261112_101010.csv file",
            "// Calculating the best input shaper parameters for y axis",
            "// Fitted shaper 'ei' frequency = 41.8 Hz (vibrations = 1.1%, smoothing ~= 0.140)",
            "// Recommended shaper_type_y = ei, shaper_freq_y = 41.8 Hz",
            "// Resonances data written to /tmp/resonances_axis=1.000,1.000_belt_a.csv file",
            "// Resonances data written to /tmp/resonances_axis=1.000,-1.000_belt_b.csv file",
        ] {
            r.feed(line);
        }
        let x = &r.axes[0];
        assert_eq!(x.fits.len(), 2);
        assert_eq!(x.fits[1].name, "mzv");
        assert_eq!(x.fits[1].freq, 54.2);
        assert_eq!(x.fits[1].vibrations, 0.7);
        assert_eq!(x.fits[1].smoothing, 0.09);
        assert_eq!(x.fits[1].max_accel, Some(9700.0));
        assert_eq!(x.recommended, Some(("mzv".into(), 54.2)));
        assert_eq!(
            x.csv.as_deref(),
            Some("/tmp/calibration_data_x_20261112_101010.csv")
        );
        assert_eq!(r.axes[1].recommended, Some(("ei".into(), 41.8)));
        assert_eq!(r.axes[1].fits[0].max_accel, None);
        assert!(r.belts[0].as_deref().unwrap().ends_with("belt_a.csv"));
        assert!(r.belts[1].as_deref().unwrap().ends_with("belt_b.csv"));
    }

    #[test]
    fn psd_csv_and_belts() {
        let a = Psd::parse(
            "freq,psd_x,psd_y,psd_z,psd_xyz,zv(56.2),mzv(54.2)\n\
             10.0,1e-1,0,0,1.000e+02,0.9,0.8\n\
             50.0,1e-1,0,0,9.000e+02,0.2,0.1\n\
             90.0,1e-1,0,0,2.000e+02,0.5,0.4\n",
        )
        .unwrap();
        assert_eq!(a.freq, [10.0, 50.0, 90.0]);
        assert_eq!(a.psd[1], 900.0);
        assert_eq!(a.shapers[1].0, "mzv");
        assert_eq!(a.shapers[1].1, [0.8, 0.1, 0.4]);
        assert_eq!(a.peak(5.0, 100.0), Some(50.0));
        let same = belt_similarity(&a, &a, 5.0, 100.0).unwrap();
        assert!((same - 100.0).abs() < 1e-9);
        let b = Psd::parse(
            "freq,psd_x,psd_y,psd_z,psd_xyz\n10,0,0,0,900\n50,0,0,0,100\n90,0,0,0,200\n",
        )
        .unwrap();
        assert!(b.shapers.is_empty());
        assert!(belt_similarity(&a, &b, 5.0, 100.0).unwrap() < 50.0);
        assert!(Psd::parse("nothing").is_none());
    }

    #[test]
    fn shapers_worked_out_from_the_ad5m_csv() {
        // The AD5M Pro's own measurement; Klipper picked MZV 56.4 Hz for X and
        // ZV 46.0 Hz for Y (what its printer.cfg says).
        let x = Psd::parse(include_str!(
            "../tests/fixtures/ad5m-pro/resonance/calibration_data_x_20260726_132933.csv"
        ))
        .unwrap();
        let y = Psd::parse(include_str!(
            "../tests/fixtures/ad5m-pro/resonance/calibration_data_y_20260726_133151.csv"
        ))
        .unwrap();
        assert_eq!(x.shaper_freqs, [61.0, 56.4, 66.4, 84.0, 102.0]);
        // Klipper calibrates with a square corner velocity of 5 unless told
        // otherwise; with the printer's 9, Y would come out as MZV.
        for scv in [SHAPER_SCV] {
            let fx = AxisShapers::from_csv(&x, scv);
            let fy = AxisShapers::from_csv(&y, scv);
            for f in fx.fits.iter().chain(&fy.fits) {
                println!(
                    "scv {scv}: {} {:.1} Hz vib {:.1}% smooth {:.3} accel {:.0}",
                    f.name,
                    f.freq,
                    f.vibrations,
                    f.smoothing,
                    f.max_accel.unwrap()
                );
            }
            assert_eq!(fx.recommended, Some(("mzv".into(), 56.4)));
            assert_eq!(fy.recommended, Some(("zv".into(), 46.0)));
        }
    }

    #[test]
    fn wifi_scan_saved_and_status() {
        let scan = "bssid / frequency / signal level / flags / ssid\n\
            aa:aa:aa:aa:aa:01\t5200\t-55\t[WPA-PSK-CCMP+TKIP][WPA2-PSK-CCMP+TKIP][WPS][ESS]\tHome\n\
            aa:aa:aa:aa:aa:02\t2457\t-65\t[WPA2-PSK-CCMP][ESS]\tHome\n\
            aa:aa:aa:aa:aa:03\t2452\t-73\t[WPA2-PSK+SAE-CCMP][SAE-H2E][ESS]\tMixed\n\
            aa:aa:aa:aa:aa:04\t2412\t-80\t[SAE-CCMP][ESS]\tOnly WPA3\n\
            aa:aa:aa:aa:aa:05\t2437\t-60\t[ESS]\tCafe \\\"free\\\"\n\
            aa:aa:aa:aa:aa:06\t2437\t-50\t[WPA2-PSK-CCMP][ESS]\t\\x00\\x00\n\
            aa:aa:aa:aa:aa:07\t2437\t-90\t[WEP][ESS]\t\\xd0\\x94\\xd0\\xbe\\xd0\\xbc\n";
        let nets = parse_wifi_scan(scan);
        let names: Vec<&str> = nets.iter().map(|n| n.ssid.as_str()).collect();
        assert_eq!(
            names,
            ["Home", "Cafe \"free\"", "Mixed", "Only WPA3", "Дом"]
        );
        assert_eq!(nets[0].freq, 5200);
        assert_eq!(nets[0].auth, WifiAuth::Psk);
        assert_eq!(nets[1].auth, WifiAuth::Open);
        assert_eq!(nets[2].auth, WifiAuth::Psk);
        assert_eq!(nets[3].auth, WifiAuth::Sae);
        assert_eq!(nets[4].auth, WifiAuth::Other);
        let saved = parse_wifi_saved(
            "network id / ssid / bssid / flags\n0\tHome\tany\t[CURRENT]\n1\tOld net\tany\t[DISABLED]\n",
        );
        assert_eq!(
            saved,
            [(0, "Home".into(), true), (1, "Old net".into(), false)]
        );
        assert_eq!(wpa_hex("Дом"), "d094d0bed0bc");
        let st = parse_wpa_status("wpa_state=COMPLETED\nssid=Home\nip_address=192.168.0.20\n");
        assert_eq!(st["wpa_state"], "COMPLETED");
    }

    #[test]
    fn layers_from_z() {
        let plan = layer_plan(&serde_json::json!({
            "layer_height": 0.2, "first_layer_height": 0.2, "layer_count": 103, "object_height": 20.6
        }));
        assert_eq!(plan, Some((0.2, 0.2, 103)));
        let mut t = LayerTracker::new(0.2, 0.2, 103).unwrap();
        // Z readings four times a second, as Moonraker sends them.
        let mut now = 0.0;
        let mut feed = |t: &mut LayerTracker, secs: f64, z: f64| {
            let mut got = Vec::new();
            let end = now + secs;
            while now < end {
                got.extend(t.update(now, z));
                now += 0.25;
            }
            got
        };
        // Up high while the bed heats: nothing.
        assert!(feed(&mut t, 30.0, 10.0).is_empty());
        // Down at the first layer: layer 1 after a moment.
        assert_eq!(feed(&mut t, 20.0, 0.2), [1]);
        // A travel hop of half a second does not count.
        assert!(feed(&mut t, 0.5, 0.6).is_empty());
        assert!(feed(&mut t, 10.0, 0.2).is_empty());
        // The next layer once Z stayed there for two seconds.
        assert_eq!(feed(&mut t, 15.0, 0.4), [2]);
        // A jump of two layers announces both.
        assert_eq!(feed(&mut t, 15.0, 0.8), [3, 4]);
        // Never back down; the head going up at the end counts no layers.
        assert!(feed(&mut t, 5.0, 0.4).is_empty());
        assert!(feed(&mut t, 5.0, 50.0).is_empty());
    }

    #[test]
    fn macro_parameters() {
        let gcode = "{% set ENABLE = params.ENABLE|int != 0 if params.ENABLE is defined\n\
            else params.LAYER is defined %}\n\
            {% set LAYER = params.LAYER|default(pause_at_layer.layer)|int %}\n\
            {% set TEMP = params.TEMP|default(220)|float %}\n\
            {% set MAT = params[\"MATERIAL\"]|default(\"PLA\") %}\n\
            {% if params.get('X') %}{% endif %}";
        assert_eq!(
            macro_params(gcode),
            [
                ("ENABLE".into(), "".into()),
                ("LAYER".into(), "".into()),
                ("TEMP".into(), "220".into()),
                ("MATERIAL".into(), "PLA".into()),
            ]
        );
        assert!(macro_params("G28\nM104 S0").is_empty());
    }
}
