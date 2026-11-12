//! Themes: an accent colour and a background, picked in Settings › Screen ›
//! Appearance — one of the presets or any colour from the picker — saved in
//! Moonraker's database and set into the UI's Tokens. Every other shade (text
//! on the accent, a lighter accent for text, the surfaces) is worked out from
//! those two, so any pick stays readable.

use std::cell::Cell;

use slint::{Color, ComponentHandle, ModelRc, SharedString, VecModel};

use crate::{AppWindow, Swatch, Tokens};

/// Accent presets: name and colour.
const ACCENTS: [(&str, u32); 8] = [
    ("Teal", 0x2dd4bf),
    ("Orange", 0xfb923c),
    ("Blue", 0x60a5fa),
    ("Violet", 0xa78bfa),
    ("Pink", 0xf472b6),
    ("Lime", 0xa3e635),
    ("Amber", 0xfbbf24),
    ("Cyan", 0x22d3ee),
];

/// Background presets: name and the background colour.
const BASES: [(&str, u32); 3] = [
    ("Graphite", 0x0b0d10),
    ("Black", 0x000000),
    ("Navy", 0x0a0f1a),
];

/// The default theme's exact shades (design code), so it looks as designed.
const DEFAULT_ACCENT: (u32, u32, u32) = (0x2dd4bf, 0x5eead4, 0x042f2a);
const DEFAULT_BASE: [u32; 6] = [0x0b0d10, 0x0f1215, 0x15181d, 0x1f242b, 0x252a31, 0x0b0d10];

thread_local! {
    /// "Your own" colours as last picked (also what the picker starts from).
    static OWN_ACCENT: Cell<u32> = const { Cell::new(0xe879f9) };
    static OWN_BASE: Cell<u32> = const { Cell::new(0x14101c) };
}

fn rgb(c: u32) -> Color {
    Color::from_rgb_u8((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

fn hex(c: Color) -> u32 {
    (u32::from(c.red()) << 16) | (u32::from(c.green()) << 8) | u32::from(c.blue())
}

/// `a` moved toward `b` by `t` (0..1).
fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |s: u32| -> u32 {
        let (x, y) = (((a >> s) & 0xff) as f32, ((b >> s) & 0xff) as f32);
        ((x + (y - x) * t).round() as u32) << s
    };
    ch(16) | ch(8) | ch(0)
}

/// Relative luminance, 0 (black) .. 1 (white).
fn luminance(c: u32) -> f32 {
    let lin = |v: u32| {
        let v = v as f32 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin((c >> 16) & 0xff) + 0.7152 * lin((c >> 8) & 0xff) + 0.0722 * lin(c & 0xff)
}

/// An accent's shades: itself, lighter for text, and the ink for text on it
/// (dark on a light accent, white on a dark one).
fn accent_shades(c: u32) -> (u32, u32, u32) {
    if c == DEFAULT_ACCENT.0 {
        return DEFAULT_ACCENT;
    }
    let ink = if luminance(c) > 0.3 {
        mix(c, 0x000000, 0.85)
    } else {
        0xffffff
    };
    (c, mix(c, 0xffffff, 0.35), ink)
}

/// A background's shades: bg, rail, surfaces 1–3, well — each a little lighter.
fn base_shades(c: u32) -> [u32; 6] {
    if c == DEFAULT_BASE[0] {
        return DEFAULT_BASE;
    }
    // On a light background the cards step darker, on a dark one lighter.
    let toward = if light(c) { 0x000000 } else { 0xffffff };
    [
        c,
        mix(c, toward, 0.02),
        mix(c, toward, 0.05),
        mix(c, toward, 0.09),
        mix(c, toward, 0.12),
        c,
    ]
}

/// A background light enough for dark text.
fn light(c: u32) -> bool {
    luminance(c) > 0.35
}

/// Text and the shades that follow the background: text, text-2, text-dim,
/// text-faint, text-mute, track, control, line.
fn ink_shades(bg: u32) -> [u32; 8] {
    if bg == DEFAULT_BASE[0] {
        // The design code's own values.
        return [
            0xf4f6f8, 0xc7cdd4, 0x98a1ac, 0x66707b, 0x7d8792, 0x171b21, 0x3a414a, 0x4a525c,
        ];
    }
    if light(bg) {
        [
            0x111317,
            0x2c323a,
            0x58616c,
            0x8a929c,
            0x6b7480,
            mix(bg, 0x000000, 0.08),
            mix(bg, 0x000000, 0.22),
            mix(bg, 0x000000, 0.32),
        ]
    } else {
        [
            0xf4f6f8,
            0xc7cdd4,
            0x98a1ac,
            0x66707b,
            0x7d8792,
            mix(bg, 0xffffff, 0.06),
            mix(bg, 0xffffff, 0.18),
            mix(bg, 0xffffff, 0.25),
        ]
    }
}

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
        "Navy" => words.get_theme_navy(),
        _ => words.get_theme_own(),
    }
}

/// The swatches for the Appearance screen; the last of each row is "your own".
pub fn show_choices(ui: &AppWindow) {
    let under = rgb(DEFAULT_BASE[2]);
    let mut accents: Vec<Swatch> = ACCENTS
        .iter()
        .map(|(n, c)| Swatch {
            name: name(ui, n),
            color: rgb(*c),
            under,
        })
        .collect();
    accents.push(Swatch {
        name: name(ui, "own"),
        color: rgb(OWN_ACCENT.with(Cell::get)),
        under,
    });
    // A background shows itself and, inside, the shade its cards take.
    let base = |n: SharedString, b: u32| Swatch {
        name: n,
        color: rgb(base_shades(b)[3]),
        under: rgb(b),
    };
    let mut bases: Vec<Swatch> = BASES.iter().map(|(n, b)| base(name(ui, n), *b)).collect();
    bases.push(base(name(ui, "own"), OWN_BASE.with(Cell::get)));
    ui.set_theme_accents(ModelRc::new(VecModel::from(accents)));
    ui.set_theme_bases(ModelRc::new(VecModel::from(bases)));
}

/// The index past the presets: "your own".
pub const OWN_ACCENT_INDEX: usize = ACCENTS.len();
pub const OWN_BASE_INDEX: usize = BASES.len();

/// Remember the user's own colours (from the database at start, or the picker).
pub fn set_own(accent: Option<u32>, base: Option<u32>) {
    if let Some(a) = accent {
        OWN_ACCENT.with(|c| c.set(a));
    }
    if let Some(b) = base {
        OWN_BASE.with(|c| c.set(b & 0xffffff));
    }
}

/// The own colours as "rrggbb", for saving.
pub fn own_hex() -> (String, String) {
    (
        format!("{:06x}", OWN_ACCENT.with(Cell::get)),
        format!("{:06x}", OWN_BASE.with(Cell::get)),
    )
}

/// A picked colour, as `rrggbb`.
pub fn color_hex(c: Color) -> u32 {
    hex(c)
}

/// Set the theme into the UI: a preset index or OWN_*_INDEX for your own.
pub fn apply(ui: &AppWindow, accent: usize, base: usize) {
    let a = ACCENTS.get(accent).map(|p| p.1).unwrap_or_else(|| {
        if accent == OWN_ACCENT_INDEX {
            OWN_ACCENT.with(Cell::get)
        } else {
            ACCENTS[0].1
        }
    });
    let b = BASES.get(base).map(|p| p.1).unwrap_or_else(|| {
        if base == OWN_BASE_INDEX {
            OWN_BASE.with(Cell::get)
        } else {
            BASES[0].1
        }
    });
    let (a, hi, ink) = accent_shades(a);
    let s = base_shades(b);
    let t = ui.global::<Tokens>();
    t.set_accent(rgb(a));
    t.set_accent_hi(rgb(hi));
    t.set_accent_ink(rgb(ink));
    // 13 % of the accent, as the default theme's accent-soft.
    t.set_accent_soft(rgb(a).with_alpha(0.13));
    t.set_bg(rgb(s[0]));
    t.set_rail(rgb(s[1]));
    t.set_surface_1(rgb(s[2]));
    t.set_surface_2(rgb(s[3]));
    t.set_surface_3(rgb(s[4]));
    t.set_well(rgb(s[5]));
    let k = ink_shades(b);
    t.set_text(rgb(k[0]));
    t.set_text_2(rgb(k[1]));
    t.set_text_dim(rgb(k[2]));
    t.set_text_faint(rgb(k[3]));
    t.set_text_mute(rgb(k[4]));
    t.set_track(rgb(k[5]));
    t.set_control(rgb(k[6]));
    t.set_line(rgb(k[7]));
    ui.set_theme_accent(accent.min(OWN_ACCENT_INDEX) as i32);
    ui.set_theme_base(base.min(OWN_BASE_INDEX) as i32);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_theme_is_the_design_codes() {
        assert_eq!(accent_shades(0x2dd4bf), (0x2dd4bf, 0x5eead4, 0x042f2a));
        assert_eq!(base_shades(0x0b0d10)[2], 0x15181d);
    }

    #[test]
    fn own_colours_stay_readable() {
        // A dark accent gets white text on it, a light one dark text.
        assert_eq!(accent_shades(0x1e3a8a).2, 0xffffff);
        assert!(luminance(accent_shades(0xfde047).2) < 0.05);
        // A white background reads dark text, cards a little darker.
        assert!(light(0xffffff) && ink_shades(0xffffff)[0] == 0x111317);
        assert!(luminance(base_shades(0xffffff)[2]) < luminance(0xffffff));
        // Surfaces step lighter than the background.
        let s = base_shades(0x101820);
        assert!(luminance(s[2]) > luminance(s[0]) && luminance(s[4]) > luminance(s[3]));
    }
}
