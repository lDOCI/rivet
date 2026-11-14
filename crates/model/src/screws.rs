//! Bed screws from a bed mesh, as Flashforge Calibration Assistant computes
//! them (github.com/lDOCI/Flashforge-Calibration-Assistant-web, mechanics.ts).
//!
//! The heater plate sits on four springs, one screw per corner. A stiff plate
//! on four springs settles on the least-squares plane through the four support
//! heights, so the screws can only tilt it; the plate's own bumps stay. The
//! moves are the smallest that give the best tilt: a least-squares fit with a
//! small penalty on the moves, the common offset free (it only shifts Z).
//! Moves under `min_adjust` are dropped, over `max_adjust` clamped, and the
//! rest solved again.
//!
//! Mesh values follow Klipper: higher means the bed is higher there.

/// Corners in this order everywhere: front left, front right, back left, back right.
pub const CORNERS: usize = 4;

/// The AD5M's bed: screw thread pitch (mm per turn), how far the screws sit
/// from the bed's centre, and what is worth turning.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrewOptions {
    pub pitch: f64,
    pub offset: [f64; 2],
    pub min_adjust: f64,
    pub max_adjust: f64,
    /// Turns that improve the spread by less than this are not suggested, mm.
    pub threshold: f64,
}

impl Default for ScrewOptions {
    fn default() -> Self {
        Self {
            pitch: 0.7,
            offset: [105.0, 105.0],
            min_adjust: 0.02,
            max_adjust: 2.0,
            threshold: 0.03,
        }
    }
}

/// One screw: clock minutes to turn (60 = a full turn) and which way, holding
/// the nut from below and turning the screw from above. Clockwise drives the
/// screw in and lowers that corner. 0 minutes: leave it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScrewTurn {
    pub minutes: u32,
    pub clockwise: bool,
    /// Asked for more than `max_adjust`: check again after this turn.
    pub limited: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScrewPlan {
    pub turns: [ScrewTurn; CORNERS],
    /// Highest minus lowest point of the mesh, now and after the turns, mm.
    pub range_before: f64,
    pub range_after: f64,
}

/// `mesh[row][col]`: row 0 at `min[1]` (the front), col 0 at `min[0]` (the left).
pub fn plan(
    mesh: &[Vec<f64>],
    min: [f64; 2],
    max: [f64; 2],
    opts: &ScrewOptions,
) -> Option<ScrewPlan> {
    let rows = mesh.len();
    let cols = mesh.first()?.len();
    if rows < 2 || cols < 2 || mesh.iter().any(|r| r.len() != cols) {
        return None;
    }
    let axis = |lo: f64, hi: f64, n: usize| -> Vec<f64> {
        (0..n)
            .map(|i| lo + (hi - lo) * i as f64 / (n - 1) as f64)
            .collect()
    };
    let xs = axis(min[0], max[0], cols);
    let ys = axis(min[1], max[1], rows);
    // Klipper printers put the origin either at a corner or in the middle.
    let center = if min[0] < 0.0 || min[1] < 0.0 {
        [0.0, 0.0]
    } else {
        [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0]
    };
    let [ox, oy] = opts.offset;
    let corners = [
        [center[0] - ox, center[1] - oy],
        [center[0] + ox, center[1] - oy],
        [center[0] - ox, center[1] + oy],
        [center[0] + ox, center[1] + oy],
    ];
    let nodes: Vec<[f64; 2]> = ys
        .iter()
        .flat_map(|&y| xs.iter().map(move |&x| [x, y]))
        .collect();
    let flat: Vec<f64> = mesh.iter().flatten().copied().collect();
    // Height change at every node when one corner moves 1 mm and the others stay.
    let responses: Vec<Vec<f64>> = (0..CORNERS)
        .map(|k| {
            let unit: Vec<f64> = (0..CORNERS).map(|i| f64::from(u8::from(i == k))).collect();
            let plane = fit_plane(&corners, &unit);
            nodes.iter().map(|&[x, y]| plane(x, y)).collect()
        })
        .collect();
    let target: Vec<f64> = flat.iter().map(|v| -v).collect();
    let n = target.len();

    let mut fixed: [Option<f64>; CORNERS] = [None; CORNERS];
    let mut limited = [false; CORNERS];
    let mut moves = [0.0; CORNERS];
    for _ in 0..5 {
        let free: Vec<usize> = (0..CORNERS).filter(|&i| fixed[i].is_none()).collect();
        let b: Vec<f64> = (0..n)
            .map(|r| {
                target[r]
                    - (0..CORNERS)
                        .map(|i| fixed[i].unwrap_or(0.0) * responses[i][r])
                        .sum::<f64>()
            })
            .collect();
        let a: Vec<Vec<f64>> = (0..n)
            .map(|r| free.iter().map(|&i| responses[i][r]).chain([1.0]).collect())
            .collect();
        let mut ridge: Vec<f64> = free.iter().map(|_| 1e-3 * n as f64).collect();
        ridge.push(0.0);
        let sol = if free.is_empty() {
            Vec::new()
        } else {
            least_squares(&a, &b, &ridge)
        };
        for i in 0..CORNERS {
            moves[i] = fixed[i].unwrap_or(0.0);
        }
        for (k, &i) in free.iter().enumerate() {
            moves[i] = sol[k];
        }
        let mut changed = false;
        for &i in &free {
            if moves[i].abs() > opts.max_adjust {
                fixed[i] = Some(moves[i].signum() * opts.max_adjust);
                limited[i] = true;
                changed = true;
            } else if moves[i].abs() < opts.min_adjust {
                fixed[i] = Some(0.0);
                changed = true;
            }
        }
        if !changed {
            break;
        }
        for i in 0..CORNERS {
            if let Some(f) = fixed[i] {
                moves[i] = f;
            }
        }
    }

    let per_minute = opts.pitch / 60.0;
    let mut turns = [ScrewTurn::default(); CORNERS];
    let mut after = flat.clone();
    for i in 0..CORNERS {
        let minutes = (moves[i].abs() / per_minute).round();
        let rounded = moves[i].signum() * minutes * per_minute;
        for (v, r) in after.iter_mut().zip(&responses[i]) {
            *v += rounded * r;
        }
        turns[i] = ScrewTurn {
            minutes: minutes as u32,
            clockwise: moves[i] < 0.0,
            limited: limited[i],
        };
    }
    let range_before = range(&flat);
    let range_after = range(&after);
    if range_before - range_after < opts.threshold {
        return Some(ScrewPlan {
            turns: [ScrewTurn::default(); CORNERS],
            range_before,
            range_after: range_before,
        });
    }
    Some(ScrewPlan {
        turns,
        range_before,
        range_after,
    })
}

fn range(v: &[f64]) -> f64 {
    let (lo, hi) = v
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), &x| (a.min(x), b.max(x)));
    hi - lo
}

/// Least-squares plane `z = a + b·x + c·y` through the points.
fn fit_plane(points: &[[f64; 2]], values: &[f64]) -> impl Fn(f64, f64) -> f64 {
    let a: Vec<Vec<f64>> = points.iter().map(|&[x, y]| vec![1.0, x, y]).collect();
    let s = least_squares(&a, values, &[]);
    move |x, y| s[0] + s[1] * x + s[2] * y
}

/// `min |A·x - b|² + Σ ridge[k]·x[k]²` by the normal equations (a few unknowns).
fn least_squares(a: &[Vec<f64>], b: &[f64], ridge: &[f64]) -> Vec<f64> {
    let p = a.first().map_or(0, Vec::len);
    let mut m = vec![vec![0.0; p + 1]; p];
    for (row, &rhs) in a.iter().zip(b) {
        for i in 0..p {
            m[i][p] += row[i] * rhs;
            for j in 0..p {
                m[i][j] += row[i] * row[j];
            }
        }
    }
    for (i, r) in ridge.iter().enumerate().take(p) {
        m[i][i] += r;
    }
    for col in 0..p {
        let pivot = (col..p)
            .max_by(|&x, &y| m[x][col].abs().total_cmp(&m[y][col].abs()))
            .unwrap_or(col);
        m.swap(col, pivot);
        let d = m[col][col];
        if d.abs() < 1e-12 {
            continue;
        }
        for r in 0..p {
            if r == col {
                continue;
            }
            let f = m[r][col] / d;
            let pivot_row = m[col].clone();
            for (cell, v) in m[r].iter_mut().zip(&pivot_row).skip(col) {
                *cell -= f * v;
            }
        }
    }
    (0..p)
        .map(|i| {
            if m[i][i].abs() < 1e-12 {
                0.0
            } else {
                m[i][p] / m[i][i]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: [f64; 2] = [-105.0, -105.0];
    const MAX: [f64; 2] = [105.0, 105.0];

    fn tilted(dx: f64, dy: f64) -> Vec<Vec<f64>> {
        (0..5)
            .map(|r| {
                (0..5)
                    .map(|c| dx * (c as f64 - 2.0) / 2.0 + dy * (r as f64 - 2.0) / 2.0)
                    .collect()
            })
            .collect()
    }

    #[test]
    fn a_flat_bed_needs_nothing() {
        let p = plan(&tilted(0.0, 0.0), MIN, MAX, &ScrewOptions::default()).unwrap();
        assert!(p.turns.iter().all(|t| t.minutes == 0));
    }

    #[test]
    fn a_high_right_side_lowers_the_right_screws() {
        // Right edge 0.35 mm higher than the middle, left 0.35 lower.
        let p = plan(&tilted(0.35, 0.0), MIN, MAX, &ScrewOptions::default()).unwrap();
        let [fl, fr, bl, br] = p.turns;
        assert!(fr.clockwise && br.clockwise, "{p:?}");
        assert!(!fl.clockwise && !bl.clockwise, "{p:?}");
        // 0.35 mm at the screws is half a turn of 0.7 mm: some 30 minutes each side
        // (the smallest moves: split between both sides).
        assert!(p.range_after < 0.05, "{p:?}");
        assert!(fr.minutes > 5 && fr.minutes <= 40, "{p:?}");
    }

    // The assistant's own tests (calibration.test.ts), same numbers.
    #[test]
    fn assistant_rigid_plate_against_one_low_corner() {
        let opts = ScrewOptions {
            min_adjust: 0.0,
            max_adjust: 10.0,
            threshold: 0.0,
            ..ScrewOptions::default()
        };
        let p = plan(&[vec![-1.0, 0.0], vec![0.0, 0.0]], MIN, MAX, &opts).unwrap();
        assert!((p.range_after - 0.5).abs() < 0.05, "{p:?}");
    }

    #[test]
    fn assistant_removes_a_pure_tilt() {
        let mesh: Vec<Vec<f64>> = (0..5)
            .map(|r| {
                let y = -105.0 + 52.5 * r as f64;
                (0..5)
                    .map(|c| 0.0015 * (-105.0 + 52.5 * c as f64) - 0.001 * y)
                    .collect()
            })
            .collect();
        let p = plan(&mesh, MIN, MAX, &ScrewOptions::default()).unwrap();
        assert!(p.range_after < 0.03, "{p:?}");
        let [_, fr, bl, _] = p.turns;
        assert!(
            fr.clockwise && fr.minutes > 0 && !bl.clockwise && bl.minutes > 0,
            "{p:?}"
        );
    }

    #[test]
    fn a_bad_mesh_is_none() {
        assert!(plan(&[vec![0.0]], MIN, MAX, &ScrewOptions::default()).is_none());
        assert!(plan(&[], MIN, MAX, &ScrewOptions::default()).is_none());
    }
}
