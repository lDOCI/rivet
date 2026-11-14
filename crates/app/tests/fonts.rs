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
