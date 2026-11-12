//! Themes: an accent colour and a background, picked in Settings › Screen ›
//! Appearance, saved in Moonraker's database, set into the UI's Tokens.

use slint::{Color, ComponentHandle, ModelRc, SharedString, VecModel};

use crate::{AppWindow, Swatch, Tokens};

/// Accent: name (msgid), the colour, a lighter one for text, the ink on it.
const ACCENTS: [(&str, u32, u32, u32); 8] = [
    ("Teal", 0x2dd4bf, 0x5eead4, 0x042f2a),
    ("Orange", 0xfb923c, 0xfdba74, 0x431407),
    ("Blue", 0x60a5fa, 0x93c5fd, 0x0b1d3a),
    ("Violet", 0xa78bfa, 0xc4b5fd, 0x1e1046),
    ("Pink", 0xf472b6, 0xf9a8d4, 0x4a0d2a),
    ("Lime", 0xa3e635, 0xbef264, 0x1a2e05),
    ("Amber", 0xfbbf24, 0xfcd34d, 0x3d2a00),
    ("Cyan", 0x22d3ee, 0x67e8f9, 0x053340),
];

/// Background: name, then bg, rail, surface 1–3, well.
const BASES: [(&str, [u32; 6]); 3] = [
    (
        "Graphite",
        [0x0b0d10, 0x0f1215, 0x15181d, 0x1f242b, 0x252a31, 0x0b0d10],
    ),
    (
        "Black",
        [0x000000, 0x050505, 0x111214, 0x1b1c1f, 0x242529, 0x000000],
    ),
    (
        "Navy",
        [0x0a0f1a, 0x0d1322, 0x131b2c, 0x1b2539, 0x222e45, 0x0a0f1a],
    ),
];

fn rgb(c: u32) -> Color {
    Color::from_rgb_u8((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// Words for the swatch names, translated (the same msgids as in theme.slint's
/// callers: the names are passed through @tr in app.slint's Strings).
fn name(ui: &AppWindow, key: &str) -> SharedString {
    let words = ui.global::<crate::Strings>();
    match key {
        "Teal" => words.get_theme_teal(),
        "Orange" => words.get_theme_orange(),
        "Blue" => words.get_theme_blue(),
        "Violet" => words.get_theme_violet(),
        "Pink" => words.get_theme_pink(),
        "Lime" => words.get_theme_lime(),
        "Amber" => words.get_theme_amber(),
        "Cyan" => words.get_theme_cyan(),
        "Graphite" => words.get_theme_graphite(),
        "Black" => words.get_theme_black(),
        _ => words.get_theme_navy(),
    }
}

/// The swatches for the Appearance screen.
pub fn show_choices(ui: &AppWindow) {
    let base = BASES[0].1;
    let accents: Vec<Swatch> = ACCENTS
        .iter()
        .map(|(n, c, _, _)| Swatch {
            name: name(ui, n),
            color: rgb(*c),
            under: rgb(base[2]),
        })
        .collect();
    let teal = rgb(ACCENTS[0].1);
    let bases: Vec<Swatch> = BASES
        .iter()
        .map(|(n, b)| Swatch {
            name: name(ui, n),
            color: teal,
            under: rgb(b[0]),
        })
        .collect();
    ui.set_theme_accents(ModelRc::new(VecModel::from(accents)));
    ui.set_theme_bases(ModelRc::new(VecModel::from(bases)));
}

/// Set the theme into the UI. Unknown indices fall back to the default.
pub fn apply(ui: &AppWindow, accent: usize, base: usize) {
    let (_, a, hi, ink) = ACCENTS.get(accent).copied().unwrap_or(ACCENTS[0]);
    let (_, b) = BASES.get(base).copied().unwrap_or(BASES[0]);
    let t = ui.global::<Tokens>();
    t.set_accent(rgb(a));
    t.set_accent_hi(rgb(hi));
    t.set_accent_ink(rgb(ink));
    // 13 % of the accent, as the default theme's accent-soft.
    t.set_accent_soft(rgb(a).with_alpha(0.13));
    t.set_bg(rgb(b[0]));
    t.set_rail(rgb(b[1]));
    t.set_surface_1(rgb(b[2]));
    t.set_surface_2(rgb(b[3]));
    t.set_surface_3(rgb(b[4]));
    t.set_well(rgb(b[5]));
    ui.set_theme_accent(accent.min(ACCENTS.len() - 1) as i32);
    ui.set_theme_base(base.min(BASES.len() - 1) as i32);
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_default_theme_is_the_design_codes() {
        // Teal on graphite: the values tokens.slint starts with.
        assert_eq!(super::ACCENTS[0].1, 0x2dd4bf);
        assert_eq!(super::BASES[0].1[0], 0x0b0d10);
        assert_eq!(super::BASES[0].1[2], 0x15181d);
    }
}
