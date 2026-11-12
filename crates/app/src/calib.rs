//! Calibration data to the screens' properties, on the UI thread.

use slint::{Color, ModelRc, SharedString, VecModel};

use crate::{AppWindow, FormRow, MeshCell, ScrewRow};

pub fn show(ui: &AppWindow, info: model::CalibInfo) {
    ui.set_calib_has_probe(info.has_probe);
    ui.set_calib_has_mesh(info.has_mesh);
    ui.set_calib_has_screws(info.has_screws && !info.screws.is_empty());
    ui.set_calib_has_accel(info.has_accel);
    ui.set_calib_has_tare(info.has_tare);
    ui.set_probe_z_saved(info.z_offset.map_or(-999.0, |z| z as f32));
    ui.set_probe_offset(format!("{} / {}", info.probe_offset[0], info.probe_offset[1]).into());
    ui.set_probe_samples(info.probe_samples as i32);

    // The mesh as seen from above: the back row first.
    let cols = info.mesh.first().map_or(0, Vec::len);
    let peak = info
        .mesh
        .iter()
        .flatten()
        .fold(0.0_f64, |m, v| m.max(v.abs()))
        .max(0.01);
    let cells: Vec<MeshCell> = info
        .mesh
        .iter()
        .rev()
        .flatten()
        .map(|v| MeshCell {
            text: signed(*v).into(),
            color: mesh_color(v / peak),
        })
        .collect();
    let (lo, hi) = info
        .mesh
        .iter()
        .flatten()
        .fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    ui.set_mesh_cols(cols as i32);
    ui.set_mesh_cells(ModelRc::new(VecModel::from(cells)));
    ui.set_mesh_range(if cols > 0 { (hi - lo) as f32 } else { -1.0 });
    ui.set_mesh_profile(info.mesh_profile.as_str().into());
    ui.set_mesh_profiles_text(info.mesh_profiles.join(", ").into());
    let profiles: Vec<SharedString> = info
        .mesh_profiles
        .iter()
        .map(|p| p.as_str().into())
        .collect();
    ui.set_mesh_profiles(ModelRc::new(VecModel::from(profiles)));

    // Screws placed as they sit on the bed.
    let (x0, x1, y0, y1) = info.screws.iter().fold(
        (f64::MAX, f64::MIN, f64::MAX, f64::MIN),
        |(a, b, c, d), s| (a.min(s.x), b.max(s.x), c.min(s.y), d.max(s.y)),
    );
    let screws: Vec<ScrewRow> = info
        .screws
        .iter()
        .map(|s| {
            let (state, amount) = match (&s.turn, s.base) {
                (_, true) => ("base", String::new()),
                (None, _) => ("none", String::new()),
                (Some((sign, adjust)), _) => match turn(adjust) {
                    None => ("level", String::new()),
                    Some(t) => (if sign == "CW" { "cw" } else { "ccw" }, t),
                },
            };
            ScrewRow {
                name: s.name.as_str().into(),
                x: ((s.x - x0) / (x1 - x0).max(1.0)) as f32,
                y: ((y1 - s.y) / (y1 - y0).max(1.0)) as f32,
                state: state.into(),
                amount: amount.into(),
            }
        })
        .collect();
    ui.set_screws(ModelRc::new(VecModel::from(screws)));

    let pid = |p: Option<[f64; 3]>| {
        p.map(|[kp, ki, kd]| format!("Kp {kp:.1} · Ki {ki:.2} · Kd {kd:.0}"))
            .unwrap_or_default()
    };
    ui.set_pid_nozzle(pid(info.pid[0]).into());
    ui.set_pid_bed(pid(info.pid[1]).into());
    let shaper = |s: &Option<(String, f64)>| {
        s.as_ref()
            .map(|(t, f)| format!("{t} · {f:.1}"))
            .unwrap_or_default()
    };
    ui.set_shaper_x(shaper(&info.shaper[0]).into());
    ui.set_shaper_y(shaper(&info.shaper[1]).into());

    let drivers: Vec<FormRow> = info
        .drivers
        .iter()
        .map(|(section, current, quiet)| {
            let (chip, stepper) = section.split_once(' ').unwrap_or((section, ""));
            FormRow {
                kind: "info".into(),
                key: section.as_str().into(),
                title: stepper_title(stepper).into(),
                sub: chip.to_ascii_uppercase().into(),
                value: format!("{current:.2} A{}", if *quiet { " · ~" } else { "" }).into(),
                shown: true,
                ..Default::default()
            }
        })
        .collect();
    ui.set_tmc_rows(ModelRc::new(VecModel::from(drivers)));
}

/// "+0.04", "−0.09", "0.00": the real minus sign.
fn signed(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r == 0.0 {
        "0.00".into()
    } else if r < 0.0 {
        format!("−{:.2}", -r)
    } else {
        format!("+{r:.2}")
    }
}

/// −1..1 to the map's colours: blue below, grey level, orange above.
fn mesh_color(t: f64) -> Color {
    let mix = |a: [u8; 3], b: [u8; 3], k: f64| {
        let c = |i: usize| {
            (f64::from(a[i]) + (f64::from(b[i]) - f64::from(a[i])) * k.clamp(0.0, 1.0)).round()
                as u8
        };
        Color::from_rgb_u8(c(0), c(1), c(2))
    };
    let level = [0x2a, 0x2f, 0x37];
    if t < 0.0 {
        mix(level, [0x25, 0x63, 0xeb], -t)
    } else {
        mix(level, [0xea, 0x58, 0x0c], t)
    }
}

/// Klipper's "HH:MM" (whole turns : minutes of a 60-minute turn) as a person
/// turns: "¼", "1 ½". None when it is level already.
pub fn turn(adjust: &str) -> Option<String> {
    let (h, m) = adjust.split_once(':')?;
    let (h, m): (u32, u32) = (h.trim().parse().ok()?, m.trim().parse().ok()?);
    let marks = [
        (0, ""),
        (8, "⅛"),
        (15, "¼"),
        (20, "⅓"),
        (30, "½"),
        (40, "⅔"),
        (45, "¾"),
        (53, "⅞"),
        (60, ""),
    ];
    let (mark, frac) = marks
        .iter()
        .min_by_key(|(at, _)| (i64::from(*at) - i64::from(m)).abs())
        .copied()?;
    let whole = h + u32::from(mark == 60);
    match (whole, frac) {
        (0, "") => None,
        (0, f) => Some(f.to_owned()),
        (w, "") => Some(w.to_string()),
        (w, f) => Some(format!("{w} {f}")),
    }
}

/// "stepper_x" → "X", "extruder" → "Extruder": how drivers are named on screen.
fn stepper_title(stepper: &str) -> String {
    match stepper.strip_prefix("stepper_") {
        Some(axis) => axis.to_ascii_uppercase(),
        None => {
            let mut c = stepper.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screw_turns() {
        assert_eq!(turn("00:15").as_deref(), Some("¼"));
        assert_eq!(turn("00:20").as_deref(), Some("⅓"));
        assert_eq!(turn("01:30").as_deref(), Some("1 ½"));
        assert_eq!(turn("00:02"), None);
        assert_eq!(turn("00:58").as_deref(), Some("1"));
    }

    #[test]
    fn numbers_and_names() {
        assert_eq!(signed(-0.094), "−0.09");
        assert_eq!(signed(0.001), "0.00");
        assert_eq!(signed(0.04), "+0.04");
        assert_eq!(stepper_title("stepper_x"), "X");
        assert_eq!(stepper_title("extruder"), "Extruder");
    }
}
