//! Rivet UI and engine. The `rivet` binary picks a backend (desktop window or
//! framebuffer); `ui-snapshot` renders the same UI to PNG for review.

slint::include_modules!();

pub mod calib;
pub mod config;
pub mod engine;
pub mod files;
pub mod logfile;
pub mod resonance;
pub mod theme;
pub mod thumbs;
pub mod timezone;
pub mod touchcal;
pub mod wifi;

/// Interface language: `--lang ru`, else `RIVET_LANG`, else the `LANG` of the
/// system (`ru_RU.UTF-8` -> `ru`), else English.
pub fn language(arg: Option<&str>, rivet_lang: Option<&str>, system_lang: Option<&str>) -> String {
    arg.or(rivet_lang)
        .or(system_lang)
        .map(|l| {
            l.split(['_', '.', '-'])
                .next()
                .unwrap_or("")
                .to_ascii_lowercase()
        })
        .filter(|l| !l.is_empty() && l != "c" && l != "posix")
        .unwrap_or_else(|| "en".to_owned())
}

/// Switch the interface to `lang`. Call it after the first window exists.
/// English is the source language; it is selected explicitly because the
/// bundle may start in the system's language.
/// Whether a language was set at start (command line, RIVET_LANG, rivet.toml or
/// the system's LANG, which S80rivet fills from Z-Mod's setting); if not, and
/// none was picked on the screen, Rivet follows Fluidd's or Mainsail's.
pub static LANGUAGE_SET: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn apply_language(lang: &str) {
    if let Err(err) = slint::select_bundled_translation(lang) {
        eprintln!("no translation for {lang:?}: {err}");
        let _ = slint::select_bundled_translation("en");
    }
}

/// Longest text the on-screen keyboard builds.
const MAX_TEXT: usize = 200;

/// New text after pressing `key` on the on-screen keyboard: "⌫" erases the
/// last character, anything else is added. Enter, shift and the symbol switch
/// are handled by the keyboard itself.
pub fn apply_key(text: &str, key: &str) -> String {
    match key {
        "⌫" => {
            let mut chars = text.chars();
            chars.next_back();
            chars.as_str().to_owned()
        }
        _ if text.chars().count() + key.chars().count() > MAX_TEXT => text.to_owned(),
        _ => format!("{text}{key}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_key, language};

    #[test]
    fn keyboard_editing() {
        assert_eq!(apply_key("G2", "8"), "G28");
        assert_eq!(apply_key("G28", "⌫"), "G2");
        assert_eq!(apply_key("", "⌫"), "");
        // Backspace removes a whole character, not a byte.
        assert_eq!(apply_key("Ёж", "⌫"), "Ё");
        assert_eq!(apply_key(&"a".repeat(200), "b").len(), 200);
    }

    #[test]
    fn language_choice_order() {
        assert_eq!(language(Some("ru"), Some("en"), Some("de_DE.UTF-8")), "ru");
        assert_eq!(language(None, Some("RU"), Some("de_DE.UTF-8")), "ru");
        assert_eq!(language(None, None, Some("ru_RU.UTF-8")), "ru");
        assert_eq!(language(None, None, Some("C")), "en");
        assert_eq!(language(None, None, None), "en");
    }
}
