//! Use the firmware's audio path and avoid competing with its END_PRINT tune.
use serde_json::Value;

/// PLAY_MIDI's default file is defined by the firmware (different on each mod).
pub fn test_gcode(settings: &Value) -> &'static str {
    if settings.get("gcode_macro play_midi").is_some() {
        "PLAY_MIDI"
    } else {
        "M300 S1000 P500"
    }
}

/// Follow helper macros too: Z-Mod's end macro delegates to other macros.
/// Treat conditional sound as firmware-owned; its own mute settings must win.
pub fn end_print_has_sound(settings: &Value) -> bool {
    fn visit(settings: &Value, name: &str, seen: &mut Vec<String>) -> bool {
        let name = name.to_ascii_lowercase();
        if seen.contains(&name) {
            return false;
        }
        seen.push(name.clone());
        let Some(code) = settings[format!("gcode_macro {name}")]["gcode"].as_str() else {
            return false;
        };
        code.lines().any(|line| {
            let line = line.split(['#', ';']).next().unwrap_or_default().trim();
            let command = line.split_whitespace().next().unwrap_or_default();
            matches!(
                command.to_ascii_uppercase().as_str(),
                "PLAY_MIDI" | "M300" | "BEEP" | "TONE"
            ) || (command.eq_ignore_ascii_case("RUN_SHELL_COMMAND")
                && line.to_ascii_lowercase().contains("cmd=audio"))
                || visit(settings, command, seen)
        })
    }
    visit(settings, "END_PRINT", &mut Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn klipper_mod_already_plays_a_midi() {
        let cfg: Value = serde_json::from_str(include_str!(
            "../../model/tests/fixtures/ad5m-pro/configfile.json"
        ))
        .unwrap();
        let settings = &cfg["result"]["status"]["configfile"]["settings"];
        assert!(end_print_has_sound(settings));
        assert_eq!(test_gcode(settings), "PLAY_MIDI");
    }

    #[test]
    fn follows_helpers_and_stops_at_cycles() {
        let mut cfg = json!({
            "gcode_macro end_print": {"gcode": "_FINISH"},
            "gcode_macro _finish": {"gcode": "END_PRINT\nPLAY_MIDI FILE=test.mid"}
        });
        assert!(end_print_has_sound(&cfg));
        cfg["gcode_macro _finish"]["gcode"] = json!("END_PRINT\nM104 S0 ; PLAY_MIDI");
        assert!(!end_print_has_sound(&cfg));
        assert_eq!(test_gcode(&cfg), "M300 S1000 P500");
    }

    #[test]
    fn conditional_firmware_audio_keeps_its_own_mute_policy() {
        assert!(end_print_has_sound(&json!({
            "gcode_macro end_print": {"gcode": "{% if enabled %}\n M300 S440 P200\n{% endif %}"}
        })));
        assert!(!end_print_has_sound(&Value::Null));
    }
}
