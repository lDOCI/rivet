//! Calibration data to the screens' properties, on the UI thread.

use slint::{Color, ModelRc, SharedString, VecModel};

use crate::{AppWindow, FormRow, MeshCell, ScrewDial};

pub fn show(ui: &AppWindow, info: model::CalibInfo) {
    ui.set_calib_has_probe(info.has_probe);
    ui.set_calib_has_mesh(info.has_mesh);
    // Screws are planned from a mesh: any printer with a probe and a mesh.
    ui.set_calib_has_screws(info.has_mesh && info.has_probe);
    ui.set_calib_has_accel(info.has_accel);
    ui.set_calib_corexy(info.corexy);
    crate::resonance::set_range(info.freq_range.0, info.freq_range.1);
    ui.set_sweep_minutes((info.sweep_seconds / 60.0).ceil().max(1.0) as i32);
    ui.set_calib_has_tare(info.has_tare);
    ui.set_calib_mesh_macro(info.mesh_macro.clone().into());
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

    // Screws from the mesh, as the calibration assistant plans them.
    let plan = model::screws::plan(
        &info.mesh,
        info.mesh_min,
        info.mesh_max,
        &model::screws::ScrewOptions::default(),
    );
    let dials: Vec<ScrewDial> = plan
        .as_ref()
        .map(|p| {
            p.turns
                .iter()
                .map(|t| ScrewDial {
                    minutes: t.minutes as i32,
                    cw: t.clockwise,
                    limited: t.limited,
                })
                .collect()
        })
        .unwrap_or_default();
    ui.set_screw_dials(ModelRc::new(VecModel::from(dials)));
    ui.set_screws_before(plan.as_ref().map_or(-1.0, |p| p.range_before as f32));
    ui.set_screws_after(plan.as_ref().map_or(-1.0, |p| p.range_after as f32));

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
    fn numbers_and_names() {
        assert_eq!(signed(-0.094), "−0.09");
        assert_eq!(signed(0.001), "0.00");
        assert_eq!(signed(0.04), "+0.04");
        assert_eq!(stepper_title("stepper_x"), "X");
        assert_eq!(stepper_title("extruder"), "Extruder");
    }
}
