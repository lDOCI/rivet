//! Vibration measurements: what Klipper printed while measuring, the CSV files
//! it wrote, and the curves for the result and belt screens.
//!
//! Klipper writes the CSVs to its own /tmp. Rivet runs on the printer, so it
//! reads them from there; from a Mac against a remote printer there are no
//! files to read and the screens show the table without a graph.

use std::sync::{LazyLock, Mutex};

use model::{AxisShapers, Psd, Resonance};
use slint::{ModelRc, SharedString, VecModel};

use crate::{AppWindow, BeltsResult, ShaperAxis, ShaperRow};

/// The plot spans 5..150 Hz, as the prototype.
const F_LO: f64 = 5.0;
const F_HI: f64 = 150.0;

struct Data {
    log: Resonance,
    /// X, Y, belt A, belt B, as read from the CSVs.
    psd: [Option<Psd>; 4],
    /// The measured range (resonance_tester min_freq..max_freq).
    range: (f64, f64),
    aspect: f64,
}

static DATA: LazyLock<Mutex<Data>> = LazyLock::new(|| {
    Mutex::new(Data {
        log: Resonance::default(),
        psd: Default::default(),
        range: (5.0, 133.33),
        aspect: 1.2,
    })
});

/// One console line from Klipper.
pub fn feed(line: &str) {
    if let Ok(mut d) = DATA.lock() {
        d.log.feed(line);
    }
}

pub fn set_range(lo: f64, hi: f64) {
    if let Ok(mut d) = DATA.lock() {
        d.range = (lo, hi);
    }
}

pub fn set_aspect(aspect: f64) {
    if let Ok(mut d) = DATA.lock() {
        d.aspect = aspect.clamp(0.5, 5.0);
    }
}

/// Read the CSVs the last measurements named. A file that is not there (another
/// machine) leaves that curve out.
pub fn load() {
    let Ok(mut d) = DATA.lock() else { return };
    let paths = [
        d.log.axes[0].csv.clone(),
        d.log.axes[1].csv.clone(),
        d.log.belts[0].clone(),
        d.log.belts[1].clone(),
    ];
    for (slot, path) in d.psd.iter_mut().zip(paths) {
        *slot = path
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| Psd::parse(&t));
    }
}

/// Whether SHAPER_CALIBRATE has fitted anything yet.
pub fn has_shapers() -> bool {
    DATA.lock()
        .map(|d| d.log.axes.iter().any(|a| !a.fits.is_empty()))
        .unwrap_or(false)
}

fn x_at(f: f64) -> f64 {
    ((f - F_LO) / (F_HI - F_LO)).clamp(0.0, 1.0)
}

/// Path commands for one curve, scaled so that `top` reaches the plot's top.
fn path(freq: &[f64], vals: impl Iterator<Item = f64>, top: f64, width: f64) -> String {
    let mut d = String::new();
    for (f, v) in freq.iter().zip(vals) {
        if *f < F_LO || *f > F_HI {
            continue;
        }
        let x = x_at(*f) * width;
        let y = 1000.0 - (v / top).clamp(0.0, 1.0) * 950.0;
        d.push_str(&format!(
            "{}{x:.1} {y:.1} ",
            if d.is_empty() { "M" } else { "L" }
        ));
    }
    d
}

fn grouped(v: f64) -> String {
    let s = format!("{:.0}", v);
    let n = s.len();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (n - i) % 3 == 0 {
            out.push('\u{a0}');
        }
        out.push(c);
    }
    out
}

/// The table and curves of one axis; the recommended shaper first.
fn axis(a: &AxisShapers, psd: Option<&Psd>, range: (f64, f64), width: f64) -> Plain {
    let mut fits: Vec<&model::ShaperFit> = a.fits.iter().collect();
    if let Some((best, _)) = &a.recommended {
        if let Some(i) = fits.iter().position(|f| &f.name == best) {
            let f = fits.remove(i);
            fits.insert(0, f);
        }
    }
    let best = a.recommended.as_ref().map(|r| r.0.as_str());
    let rows: Vec<ShaperRow> = fits
        .iter()
        .map(|f| ShaperRow {
            name: f.name.to_ascii_uppercase().into(),
            freq: format!("{:.1}", f.freq).into(),
            vib: format!("{:.1}%", f.vibrations).into(),
            accel: f.max_accel.map_or_else(|| "—".into(), grouped).into(),
            best: Some(f.name.as_str()) == best,
            kind: f.name.clone().into(),
            hz: format!("{:.1}", f.freq).into(),
        })
        .collect();
    let (raw, curves, peak, peak_at) = match psd {
        Some(p) => {
            let top = p
                .freq
                .iter()
                .zip(&p.psd)
                .filter(|(f, _)| **f >= F_LO && **f <= F_HI)
                .fold(0.0_f64, |m, (_, v)| m.max(*v))
                .max(f64::MIN_POSITIVE);
            let curves: Vec<SharedString> = fits
                .iter()
                .map(|f| {
                    let resp = p.shapers.iter().find(|(n, _)| *n == f.name);
                    match resp {
                        Some((_, r)) => {
                            path(&p.freq, p.psd.iter().zip(r).map(|(v, k)| v * k), top, width)
                        }
                        None => String::new(),
                    }
                    .into()
                })
                .collect();
            let peak = p.peak(range.0, range.1);
            (
                path(&p.freq, p.psd.iter().copied(), top, width),
                curves,
                peak.map(|f| format!("{f:.1}")).unwrap_or_default(),
                peak.map_or(0.0, x_at),
            )
        }
        None => (String::new(), Vec::new(), String::new(), 0.0),
    };
    Plain {
        shown: !rows.is_empty(),
        rows,
        raw: raw.into(),
        curves,
        peak: peak.into(),
        peak_at: peak_at as f32,
    }
}

fn belts(
    a: Option<&Psd>,
    b: Option<&Psd>,
    measured: bool,
    range: (f64, f64),
    width: f64,
) -> BeltsResult {
    let (Some(a), Some(b)) = (a, b) else {
        return BeltsResult {
            shown: measured,
            similarity: -1,
            ..Default::default()
        };
    };
    // Both on one scale, so the stronger response looks stronger.
    let top = a
        .psd
        .iter()
        .chain(&b.psd)
        .fold(0.0_f64, |m, v| m.max(*v))
        .max(f64::MIN_POSITIVE);
    let (pa, pb) = (a.peak(range.0, range.1), b.peak(range.0, range.1));
    let weaker = match (pa, pb) {
        // A looser belt resonates lower.
        (Some(x), Some(y)) if (x - y).abs() >= 1.0 => {
            if x < y {
                "A"
            } else {
                "B"
            }
        }
        _ => "",
    };
    BeltsResult {
        shown: true,
        a: path(&a.freq, a.psd.iter().copied(), top, width).into(),
        b: path(&b.freq, b.psd.iter().copied(), top, width).into(),
        peak_a: pa.map(|f| format!("{f:.0}")).unwrap_or_default().into(),
        peak_b: pb.map(|f| format!("{f:.0}")).unwrap_or_default().into(),
        peak_a_at: pa.map_or(0.0, x_at) as f32,
        peak_b_at: pb.map_or(0.0, x_at) as f32,
        similarity: model::belt_similarity(a, b, range.0, range.1).map_or(-1, |s| s.round() as i32),
        weaker: weaker.into(),
    }
}

/// Everything the two screens show, built from the last measurements.
pub fn show(ui: &slint::Weak<AppWindow>) {
    let (axes, belts) = build();
    let _ = ui.upgrade_in_event_loop(move |ui| apply(&ui, axes, belts));
}

/// The same, on the UI thread (for snapshots and tests).
pub fn show_now(ui: &AppWindow) {
    let (axes, belts) = build();
    apply(ui, axes, belts);
}

fn build() -> (Vec<Plain>, BeltsResult) {
    let Ok(d) = DATA.lock() else {
        return (Vec::new(), BeltsResult::default());
    };
    let width = 1000.0 * d.aspect;
    let axes = vec![
        axis(&d.log.axes[0], d.psd[0].as_ref(), d.range, width),
        axis(&d.log.axes[1], d.psd[1].as_ref(), d.range, width),
    ];
    let measured = d.log.belts.iter().all(Option::is_some);
    let belts = belts(
        d.psd[2].as_ref(),
        d.psd[3].as_ref(),
        measured,
        d.range,
        width,
    );
    (axes, belts)
}

fn apply(ui: &AppWindow, axes: Vec<Plain>, belts: BeltsResult) {
    let axes: Vec<ShaperAxis> = axes.into_iter().map(Plain::into_axis).collect();
    ui.set_shaper_result(ModelRc::new(VecModel::from(axes)));
    ui.set_belts(belts);
}

/// A ShaperAxis without Slint models (they live on the UI thread only).
struct Plain {
    shown: bool,
    rows: Vec<ShaperRow>,
    raw: SharedString,
    curves: Vec<SharedString>,
    peak: SharedString,
    peak_at: f32,
}

impl Plain {
    fn into_axis(self) -> ShaperAxis {
        ShaperAxis {
            shown: self.shown,
            rows: ModelRc::new(VecModel::from(self.rows)),
            raw: self.raw,
            curves: ModelRc::new(VecModel::from(self.curves)),
            peak: self.peak,
            peak_at: self.peak_at,
        }
    }
}
