//! Printer state built from Moonraker's `printer.objects.subscribe` result
//! and the partial updates that follow in `notify_status_update`.

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
        }
    })
}

/// The base subscription plus what this printer actually has, from
/// `printer.objects.list`: every `led …` object, so Rivet shows only the
/// lights that exist.
pub fn subscription_for(available: &[String]) -> Value {
    let mut sub = subscription();
    if let Some(objects) = sub["objects"].as_object_mut() {
        for name in available.iter().filter(|n| n.starts_with("led ")) {
            objects.insert(name.clone(), Value::Null);
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
    status: Value,
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
    /// Part fan, percent.
    pub fan_percent: f64,
    pub light: Option<Light>,
    pub limits: Limits,
}

/// The first light of the printer.
#[derive(Debug, Clone, PartialEq)]
pub struct Light {
    /// Klipper name without the `led ` prefix.
    pub name: String,
    pub on: bool,
    /// Has a white channel (RGBW). Otherwise it is RGB.
    pub white: bool,
}

/// Heater limits from the printer's own config; 0 = not known.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Limits {
    pub extruder_max: f64,
    pub bed_max: f64,
    pub nozzle_diameter: f64,
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
    pub thumbnails: Vec<ThumbRef>,
}

pub fn parse_file_meta(result: &Value) -> FileMeta {
    let num = |k: &str| result.get(k).and_then(Value::as_f64).filter(|v| *v > 0.0);
    FileMeta {
        estimated_secs: num("estimated_time").map(|v| v as u64),
        filament_type: result
            .get("filament_type")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
        filament_grams: num("filament_weight_total"),
        layers: num("layer_count").map(|v| v as u32),
        nozzle_diameter: num("nozzle_diameter"),
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

/// Last finished job from `server.history.list`.
#[derive(Debug, Clone, PartialEq)]
pub struct LastPrint {
    pub filename: String,
    /// Unix time the job ended.
    pub ended: f64,
    pub duration_secs: u64,
    pub completed: bool,
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

    fn heater(&self, object: &str) -> Heater {
        Heater {
            temperature: self.f64(object, "temperature"),
            target: self.f64(object, "target"),
        }
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
            Light {
                name: object.trim_start_matches("led ").to_owned(),
                on: channels.iter().any(|v| v.as_f64().unwrap_or(0.0) > 0.0),
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
            fan_percent: self.f64("fan", "speed") * 100.0,
            light,
            limits: self.limits,
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
mod tests {
    use super::*;

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
    fn parses_klippy_states() {
        assert_eq!(Klippy::parse("ready"), Klippy::Ready);
        assert_eq!(Klippy::parse("shutdown"), Klippy::Shutdown);
        assert_eq!(Klippy::parse("weird"), Klippy::Unknown);
    }
}
