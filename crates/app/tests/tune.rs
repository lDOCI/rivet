//! Light, speed and Z-offset screens send the right values.

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
fn light_speed_and_z_offset() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    rivet::apply_language("en");
    ui.set_link_state("ready".into());
    ui.show().unwrap();

    let light = Rc::new(RefCell::new(Vec::<f32>::new()));
    let sink = light.clone();
    ui.on_set_light_level(move |v| sink.borrow_mut().push(v));
    let speed = Rc::new(RefCell::new(Vec::<f32>::new()));
    let sink = speed.clone();
    ui.on_set_speed(move |v| sink.borrow_mut().push(v));
    let z = Rc::new(RefCell::new(Vec::<f32>::new()));
    let sink = z.clone();
    ui.on_adjust_z(move |d| sink.borrow_mut().push(d));

    // Light bar: + goes up by 10 from the current level.
    ui.set_has_light(true);
    ui.set_light_level(30.0);
    ui.set_screen("light".into());
    tap(&ui, "+");
    assert_eq!(*light.borrow(), [40.0]);

    // Speed: the +10 button from 100 %.
    ui.set_speed_percent(100.0);
    ui.set_screen("speed".into());
    tap(&ui, "+10");
    assert_eq!(*speed.borrow(), [110.0]);

    // Z-offset: fine step down.
    ui.set_screen("zoffset".into());
    tap(&ui, "−0.01");
    assert_eq!(*z.borrow(), [-0.01]);
}
