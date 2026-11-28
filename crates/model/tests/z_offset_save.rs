use model::Caps;
use serde_json::Value;

#[test]
fn zmod_saves_without_applying_to_probe_or_endstop_or_restarting() {
    // These wrappers are defined in Z-Mod's base.cfg. Recognition is based on
    // the connected printer, so desktop clients and remote Moonraker work too.
    let objects = [
        "gcode_macro SET_GCODE_OFFSET",
        "gcode_macro LOAD_GCODE_OFFSET",
    ]
    .map(String::from);
    for homes_on_probe in [false, true] {
        let caps = Caps::new(&objects, &[], homes_on_probe);
        for printing in [false, true] {
            assert_eq!(
                caps.save_z_offset(printing),
                ("SAVE_ZMOD_DATA LOAD_ZOFFSET=1".to_owned(), false)
            );
        }
    }
}

#[test]
fn klipper_mod_keeps_the_existing_endstop_save_path() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/ad5m-pro/objects_list.json")).unwrap();
    let objects: Vec<String> = fixture["result"]["objects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    let caps = Caps::new(&objects, &[], false);
    assert_eq!(
        caps.save_z_offset(true),
        ("Z_OFFSET_APPLY_ENDSTOP".to_owned(), true)
    );
    assert_eq!(
        caps.save_z_offset(false),
        ("Z_OFFSET_APPLY_ENDSTOP\nSAVE_CONFIG".to_owned(), false)
    );
}

#[test]
fn other_klipper_printers_keep_probe_save_and_do_not_match_one_wrapper() {
    let objects = ["gcode_macro SET_GCODE_OFFSET"].map(String::from);
    let caps = Caps::new(&objects, &[], true);
    assert_eq!(
        caps.save_z_offset(true),
        ("Z_OFFSET_APPLY_PROBE".to_owned(), true)
    );
    assert_eq!(
        caps.save_z_offset(false),
        ("Z_OFFSET_APPLY_PROBE\nSAVE_CONFIG".to_owned(), false)
    );
}
