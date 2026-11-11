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

#[derive(Debug, Default)]
pub struct PrinterState {
    pub connection: Connection,
    pub klippy: Klippy,
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
    fn parses_klippy_states() {
        assert_eq!(Klippy::parse("ready"), Klippy::Ready);
        assert_eq!(Klippy::parse("shutdown"), Klippy::Shutdown);
        assert_eq!(Klippy::parse("weird"), Klippy::Unknown);
    }
}
