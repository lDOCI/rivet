//! The model against real responses from an AD5M Pro on KlipperMod
//! (Klipper v0.13.0, Moonraker v0.9.3), captured read-only on 2026-11-08.

use model::{Klippy, PrinterState};
use serde_json::Value;

fn load(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/ad5m-pro/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap()
}

#[test]
fn snapshot_from_real_status() {
    let mut status = load("status.json");
    let mut state = PrinterState::default();
    state.klippy = Klippy::Ready;
    state.replace_status(status["result"]["status"].take());
    let snap = state.snapshot();

    assert_eq!(snap.klippy_message, "Printer is ready");
    assert_eq!(snap.print_state, "standby");
    assert_eq!(snap.filename, "");
    assert_eq!(snap.progress, 0.0);
    assert!((15.0..40.0).contains(&snap.extruder.temperature));
    assert!((15.0..40.0).contains(&snap.bed.temperature));
    assert_eq!(snap.extruder.target, 0.0);
}

#[test]
fn every_subscribed_object_exists_on_the_printer() {
    let objects = load("objects_list.json");
    let objects: Vec<&str> = objects["result"]["objects"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let sub = model::subscription();
    for name in sub["objects"].as_object().unwrap().keys() {
        assert!(
            objects.contains(&name.as_str()),
            "{name} is not on the AD5M Pro"
        );
    }
}

#[test]
fn limits_used_by_the_ui_come_from_config() {
    let cfg = load("configfile.json");
    let s = &cfg["result"]["status"]["configfile"]["settings"];
    assert_eq!(s["extruder"]["max_temp"], 350.0);
    assert_eq!(s["heater_bed"]["max_temp"], 130.0);
    assert_eq!(s["extruder"]["min_extrude_temp"], 170.0);
    assert_eq!(s["extruder"]["max_extrude_only_distance"], 1500.0);
    assert_eq!(s["printer"]["max_velocity"], 600.0);
    assert_eq!(s["printer"]["max_accel"], 20000.0);
}
