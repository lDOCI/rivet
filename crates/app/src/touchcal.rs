//! Touch calibration: where the finger landed against where it was asked to
//! touch, turned into an affine correction (x' = a·x + b·y + c, y' = d·x + e·y + f)
//! that the platform applies to every touch. Saved next to rivet.toml.
//!
//! The points are measured as the UI sees them, that is with the correction in
//! use applied; the new correction is the fit composed with that one, so a
//! second calibration refines the first.

use std::{cell::RefCell, path::PathBuf};

/// Where the nine crosses are (a 3×3 grid), logical px of the 800×480 screen.
/// Nine, not four: one careless tap among four bends the whole correction.
pub const TARGETS: [(f32, f32); 9] = [
    (70.0, 70.0),
    (400.0, 70.0),
    (730.0, 70.0),
    (70.0, 240.0),
    (400.0, 240.0),
    (730.0, 240.0),
    (70.0, 410.0),
    (400.0, 410.0),
    (730.0, 410.0),
];

pub type Affine = [f32; 6];

const IDENTITY: Affine = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];

thread_local! {
    /// Taps so far, by cross; and the correction in use when calibration began.
    static TAPS: RefCell<[Option<(f32, f32)>; 9]> = const { RefCell::new([None; 9]) };
    static BEFORE: RefCell<Option<Affine>> = const { RefCell::new(None) };
}

fn file() -> PathBuf {
    crate::config::config_dir().join("rivet-touch.json")
}

/// The saved correction, if any and sane.
pub fn load() -> Option<Affine> {
    let text = std::fs::read_to_string(file()).ok()?;
    let v: Vec<f32> = serde_json::from_str(&text).ok()?;
    let c: Affine = v.try_into().ok()?;
    sane(&c).then_some(c)
}

fn save(c: Option<Affine>) -> std::io::Result<()> {
    match c {
        Some(c) => std::fs::write(file(), serde_json::to_string(&c.to_vec())?),
        None => match std::fs::remove_file(file()) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        },
    }
}

/// A correction that only fixes a panel, not one that turns the screen around.
fn sane(c: &Affine) -> bool {
    let [a, b, _, d, e, _] = *c;
    (0.7..=1.3).contains(&a) && (0.7..=1.3).contains(&e) && b.abs() <= 0.2 && d.abs() <= 0.2
}

/// Least-squares affine fit taking `from` points onto `to` points.
pub fn fit(from: &[(f32, f32)], to: &[(f32, f32)]) -> Option<Affine> {
    if from.len() < 3 || from.len() != to.len() {
        return None;
    }
    // Normal equations: [Σxx Σxy Σx; Σxy Σyy Σy; Σx Σy n] · [a b c] = [Σx·X Σy·X ΣX].
    let (mut sxx, mut sxy, mut syy, mut sx, mut sy, n) =
        (0.0, 0.0, 0.0, 0.0, 0.0, from.len() as f64);
    for &(x, y) in from {
        let (x, y) = (f64::from(x), f64::from(y));
        sxx += x * x;
        sxy += x * y;
        syy += y * y;
        sx += x;
        sy += y;
    }
    let m = [[sxx, sxy, sx], [sxy, syy, sy], [sx, sy, n]];
    let solve = |pick: fn(&(f32, f32)) -> f32| -> Option<[f64; 3]> {
        let (mut bx, mut by, mut b1) = (0.0, 0.0, 0.0);
        for (&(x, y), t) in from.iter().zip(to) {
            let v = f64::from(pick(t));
            bx += f64::from(x) * v;
            by += f64::from(y) * v;
            b1 += v;
        }
        solve3(m, [bx, by, b1])
    };
    let [a, b, c] = solve(|p| p.0)?;
    let [d, e, f] = solve(|p| p.1)?;
    Some([a as f32, b as f32, c as f32, d as f32, e as f32, f as f32])
}

/// Cramer's rule for a 3×3 system.
fn solve3(m: [[f64; 3]; 3], v: [f64; 3]) -> Option<[f64; 3]> {
    let det = |m: [[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let d = det(m);
    if d.abs() < 1e-9 {
        return None;
    }
    let col = |i: usize| {
        let mut c = m;
        for (row, value) in c.iter_mut().zip(v) {
            row[i] = value;
        }
        det(c) / d
    };
    Some([col(0), col(1), col(2)])
}

/// A fit that leaves out one tap far worse than the rest (a slip), then fits again.
fn robust_fit(from: &[(f32, f32)], to: &[(f32, f32)]) -> Option<Affine> {
    let first = fit(from, to)?;
    let miss: Vec<f32> = from
        .iter()
        .zip(to)
        .map(|(&p, t)| {
            let q = apply(&first, p);
            (q.0 - t.0).hypot(q.1 - t.1)
        })
        .collect();
    let mut sorted = miss.clone();
    sorted.sort_by(f32::total_cmp);
    let median = sorted[sorted.len() / 2];
    let (worst, &w) = miss.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1))?;
    if w > 15.0 && w > 2.5 * median {
        let keep = |i: usize| i != worst;
        let f: Vec<_> = from
            .iter()
            .enumerate()
            .filter(|(i, _)| keep(*i))
            .map(|(_, p)| *p)
            .collect();
        let t: Vec<_> = to
            .iter()
            .enumerate()
            .filter(|(i, _)| keep(*i))
            .map(|(_, p)| *p)
            .collect();
        return fit(&f, &t);
    }
    Some(first)
}

fn apply(c: &Affine, (x, y): (f32, f32)) -> (f32, f32) {
    (c[0] * x + c[1] * y + c[2], c[3] * x + c[4] * y + c[5])
}

/// `outer` after `inner`.
fn compose(outer: &Affine, inner: &Affine) -> Affine {
    let [a, b, c, d, e, f] = *outer;
    let [p, q, r, s, t, u] = *inner;
    [
        a * p + b * s,
        a * q + b * t,
        a * r + b * u + c,
        d * p + e * s,
        d * q + e * t,
        d * r + e * u + f,
    ]
}

/// Calibration starts: touches as corrected so far are what gets measured.
pub fn begin() {
    TAPS.with(|t| *t.borrow_mut() = [None; 9]);
    BEFORE.with(|b| *b.borrow_mut() = current());
}

/// The finger landed here for cross `i`.
pub fn tap(i: usize, x: f32, y: f32) {
    TAPS.with(|t| {
        if let Some(slot) = t.borrow_mut().get_mut(i) {
            *slot = Some((x, y));
        }
    });
}

/// All four taps in: fit, check, apply. The mean miss in px before and after,
/// or None when the taps do not make a sane correction (a cross missed).
pub fn finish() -> Option<(f32, f32)> {
    let taps: Vec<(f32, f32)> = TAPS.with(|t| t.borrow().iter().flatten().copied().collect());
    if taps.len() != TARGETS.len() {
        return None;
    }
    let fix = robust_fit(&taps, &TARGETS)?;
    let before = BEFORE.with(|b| *b.borrow()).unwrap_or(IDENTITY);
    let new = compose(&fix, &before);
    if !sane(&new) {
        return None;
    }
    let miss = |f: &dyn Fn((f32, f32)) -> (f32, f32)| {
        taps.iter()
            .zip(TARGETS)
            .map(|(&p, t)| {
                let q = f(p);
                (q.0 - t.0).hypot(q.1 - t.1)
            })
            .sum::<f32>()
            / taps.len() as f32
    };
    let was = miss(&|p| p);
    let now = miss(&|p| apply(&fix, p));
    set_current(Some(new));
    Some((was, now))
}

/// Keep the correction for good.
pub fn keep() -> std::io::Result<()> {
    save(current())
}

/// Back to the correction from before this calibration.
pub fn undo() {
    set_current(BEFORE.with(|b| *b.borrow()));
}

/// No correction at all: touches as the panel reports them.
pub fn reset() -> std::io::Result<()> {
    set_current(None);
    save(None)
}

#[cfg(feature = "fbdev")]
fn current() -> Option<Affine> {
    fbdev_platform::touch_state::correction()
}

#[cfg(feature = "fbdev")]
fn set_current(c: Option<Affine>) {
    fbdev_platform::touch_state::set_correction(c);
}

// On a computer the mouse needs no correction; keep it for the screen's flow.
#[cfg(not(feature = "fbdev"))]
thread_local! {
    static DESKTOP: RefCell<Option<Affine>> = const { RefCell::new(None) };
}

#[cfg(not(feature = "fbdev"))]
fn current() -> Option<Affine> {
    DESKTOP.with(|d| *d.borrow())
}

#[cfg(not(feature = "fbdev"))]
fn set_current(c: Option<Affine>) {
    DESKTOP.with(|d| *d.borrow_mut() = c);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 0.5 && (a.1 - b.1).abs() < 0.5
    }

    #[test]
    fn fits_a_shifted_and_stretched_panel() {
        // Taps land 22 px right, 4 px low and 3 % wide of the crosses.
        let land = |(x, y): (f32, f32)| (x * 1.03 + 22.0, y + 4.0);
        let taps: Vec<_> = TARGETS.iter().map(|&t| land(t)).collect();
        let fix = fit(&taps, &TARGETS).unwrap();
        for &t in &TARGETS {
            assert!(close(apply(&fix, land(t)), t));
        }
        assert!(sane(&fix));
        // Composed with an earlier correction, the result is still exact.
        let earlier: Affine = [1.01, 0.0, -5.0, 0.0, 0.99, 3.0];
        let total = compose(&fix, &earlier);
        let raw = |t: (f32, f32)| {
            // The point the panel reports, before the earlier correction.
            let (x, y) = land(t);
            ((x + 5.0) / 1.01, (y - 3.0) / 0.99)
        };
        for &t in &TARGETS {
            assert!(close(apply(&total, raw(t)), t));
        }
    }

    #[test]
    fn a_missed_cross_is_no_correction() {
        begin();
        // The crosses tapped in a mirrored order: no panel does that.
        for (i, (x, y)) in TARGETS.iter().enumerate() {
            tap(i, 800.0 - x, 480.0 - y);
        }
        assert!(finish().is_none());
        // One slip among nine is left out: the others still fit exactly.
        begin();
        for (i, (x, y)) in TARGETS.iter().enumerate() {
            let slip = if i == 4 { 60.0 } else { 0.0 };
            tap(i, x + 20.0 + slip, y + 3.0);
        }
        let (_, now) = finish().unwrap();
        assert!(now < 8.0, "{now}");
        undo();
        // Taps a little off: a correction, and the miss goes down.
        begin();
        for (i, (x, y)) in TARGETS.iter().enumerate() {
            tap(i, x + 20.0, y + 3.0);
        }
        let (was, now) = finish().unwrap();
        assert!(was > 19.0 && now < 1.0, "{was} → {now}");
        undo();
    }
}
