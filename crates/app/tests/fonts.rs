//! The printer has no system fonts: only the Inter faces built into the binary.
//! A Text asking for any other family finds nothing, and Slint's software
//! renderer panics on it (`systemfonts.rs`, `fallbackfont`: unwrap on None) —
//! the Klipper error screen did this with `font-family: "monospace"`.

use std::path::Path;

fn slint_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            slint_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "slint") {
            out.push(path);
        }
    }
}

#[test]
fn only_the_built_in_font_is_asked_for() {
    let ui = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui");
    let mut files = Vec::new();
    slint_files(&ui, &mut files);
    assert!(!files.is_empty());
    let mut bad = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        for (n, line) in text.lines().enumerate() {
            if let Some(rest) = line.split("font-family:").nth(1) {
                let family = rest.trim().trim_start_matches('"');
                if !family.starts_with("Inter\"") {
                    bad.push(format!("{}:{}: {}", file.display(), n + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "fonts the printer does not have:\n{}",
        bad.join("\n")
    );
}

/// Every character the screens can show: Slint string literals (English) and
/// the Russian translations. The Rust side's own strings are numbers and names.
fn shown_text() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    let mut files = Vec::new();
    slint_files(&root.join("ui"), &mut files);
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        for (n, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            for lit in code.split('"').skip(1).step_by(2) {
                out.push((format!("{}:{}", file.display(), n + 1), lit.to_owned()));
            }
        }
    }
    let po = root.join("i18n/ru/LC_MESSAGES/rivet.po");
    for (n, line) in std::fs::read_to_string(&po).unwrap().lines().enumerate() {
        if let Some(rest) = line.strip_prefix("msgstr \"") {
            out.push((
                format!("rivet.po:{}", n + 1),
                rest.trim_end_matches('"').to_owned(),
            ));
        }
    }
    out
}

#[test]
fn every_character_shown_is_in_the_built_in_font() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let faces: Vec<Vec<u8>> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "ttf"))
        .map(|p| std::fs::read(p).unwrap())
        .collect();
    assert!(!faces.is_empty());
    let faces: Vec<ttf_parser::Face> = faces
        .iter()
        .map(|d| ttf_parser::Face::parse(d, 0).unwrap())
        .collect();
    let mut missing = Vec::new();
    for (place, text) in shown_text() {
        for c in text.chars() {
            if c.is_control() || c == '\\' {
                continue;
            }
            if faces.iter().any(|f| f.glyph_index(c).is_none()) {
                missing.push(format!("{place}: {c:?} (U+{:04X}) in {text:?}", c as u32));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "characters the printer's font does not have (it has no other):\n{}",
        missing.join("\n")
    );
}
