//! Moves only on homed axes; extrusion only when Klipper says it can.

use std::{cell::RefCell, rc::Rc};

use i_slint_backend_testing::ElementHandle;
use rivet::AppWindow;
use slint::ComponentHandle;

fn tap(ui: &AppWindow, label: &str) {
    ElementHandle::find_by_accessible_label(ui, label)
        .next()
        .unwrap_or_else(|| panic!("nothing labelled {label:?}"))
        .invoke_accessible_default_action();
}

#[test]
fn motion_and_extruder() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    ui.set_link_state("ready".into());
    ui.show().unwrap();

    let moves = Rc::new(RefCell::new(Vec::<(i32, f32)>::new()));
    let sink = moves.clone();
    ui.on_move_axis(move |a, d| sink.borrow_mut().push((a, d)));
    let pushes = Rc::new(RefCell::new(Vec::<(f32, f32)>::new()));
    let sink = pushes.clone();
    ui.on_extrude(move |l, s| sink.borrow_mut().push((l, s)));

    // Motion: only X is homed.
    ui.set_screen("motion".into());
    ui.set_homed_x(true);
    tap(&ui, "X +10");
    tap(&ui, "Y +10");
    tap(&ui, "Z −1");
    assert_eq!(*moves.borrow(), [(0, 10.0)]);

    // Extruder: cold nozzle, then hot with the default 25 mm at 5 mm/s.
    ui.set_screen("extruder".into());
    tap(&ui, "Extrude");
    assert!(pushes.borrow().is_empty());
    ui.set_can_extrude(true);
    tap(&ui, "Extrude");
    tap(&ui, "Retract");
    assert_eq!(*pushes.borrow(), [(25.0, 5.0), (-25.0, 5.0)]);
}
