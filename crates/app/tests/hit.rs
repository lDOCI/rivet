//! Back takes the taps that land beside it: fingers on the AD5M's panel land
//! right of and below where they aim (a recording of taps on Back).

use rivet::AppWindow;
use slint::{
    platform::{PointerEventButton, WindowEvent},
    ComponentHandle, LogicalPosition,
};

fn tap(ui: &AppWindow, x: f32, y: f32) {
    let position = LogicalPosition::new(x, y);
    let w = ui.window();
    w.dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    w.dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
    w.dispatch_event(WindowEvent::PointerExited);
}

#[test]
fn back_takes_taps_beside_it() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    ui.window().set_size(slint::PhysicalSize::new(800, 480));
    ui.set_link_state("ready".into());
    ui.show().unwrap();
    // Recorded taps on Back (the button draws at x 104–168, y 16–80).
    for (x, y) in [
        (152.0, 29.0),
        (175.0, 54.0),
        (183.0, 55.0),
        (148.0, 75.0),
        (176.0, 56.0),
    ] {
        ui.set_screen("light".into());
        tap(&ui, x, y);
        assert_eq!(ui.get_screen(), "control", "a tap at {x},{y} missed Back");
    }
    // Further right is the title: no back.
    ui.set_screen("light".into());
    tap(&ui, 320.0, 48.0);
    assert_eq!(ui.get_screen(), "light");
}

#[test]
fn touch_calibration_measures_and_fixes_the_miss() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    ui.window().set_size(slint::PhysicalSize::new(800, 480));
    ui.set_link_state("ready".into());
    ui.on_cal_begin(rivet::touchcal::begin);
    ui.on_cal_point(|i, x, y| rivet::touchcal::tap(i as usize, x, y));
    let weak = ui.as_weak();
    ui.on_cal_finish(move || {
        let ui = weak.upgrade().unwrap();
        let (was, now) = rivet::touchcal::finish().unwrap_or((-1.0, -1.0));
        ui.set_cal_miss_before(was);
        ui.set_cal_miss_after(now);
    });
    ui.on_cal_undo(rivet::touchcal::undo);
    ui.show().unwrap();
    ui.set_overlay("touch-cal".into());
    // The finger lands 20 px right and 3 px low of each cross.
    for (x, y) in rivet::touchcal::TARGETS {
        tap(&ui, x + 20.0, y + 3.0);
    }
    assert!(
        (ui.get_cal_miss_before() - 20.2).abs() < 0.5,
        "{}",
        ui.get_cal_miss_before()
    );
    assert!(ui.get_cal_miss_after() < 1.0);
    rivet::touchcal::undo();
}
