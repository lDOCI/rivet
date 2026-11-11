//! The heating dial and keypad path, driven with pointer events.

use std::{cell::RefCell, rc::Rc};

use i_slint_backend_testing::ElementHandle;
use rivet::AppWindow;
use slint::{
    platform::{PointerEventButton, WindowEvent},
    ComponentHandle, LogicalPosition,
};

/// One test: the Slint platform is set once per process.
#[test]
fn heating() {
    i_slint_backend_testing::init_no_event_loop();
    dragging_the_dial_sends_one_target_on_release();
    keypad_value_goes_to_the_heater_that_was_open();
}

fn dragging_the_dial_sends_one_target_on_release() {
    let ui = AppWindow::new().unwrap();
    ui.set_link_state("ready".into());
    ui.set_max_nozzle(350.0);
    ui.set_extruder_temp(25.0);
    ui.set_screen("heating".into());
    ui.show().unwrap();

    let sent = Rc::new(RefCell::new(Vec::<f32>::new()));
    let sink = sent.clone();
    ui.on_set_nozzle(move |t| sink.borrow_mut().push(t));

    let dial = ElementHandle::find_by_accessible_label(&ui, "Target temperature")
        .next()
        .expect("dial");
    let (pos, size) = (dial.absolute_position(), dial.size());
    let (cx, cy) = (pos.x + size.width / 2.0, pos.y + size.height / 2.0);
    let at = |dx: f32, dy: f32| LogicalPosition::new(cx + dx, cy + dy);
    let window = ui.window();

    // Straight up is the middle of the arc: half of 350.
    window.dispatch_event(WindowEvent::PointerPressed {
        position: at(0.0, -110.0),
        button: PointerEventButton::Left,
    });
    // Moving on the way does not send anything yet.
    window.dispatch_event(WindowEvent::PointerMoved {
        position: at(30.0, -105.0),
    });
    assert!(sent.borrow().is_empty());
    // The finger comes back to the top before lifting.
    window.dispatch_event(WindowEvent::PointerMoved {
        position: at(0.0, -110.0),
    });
    window.dispatch_event(WindowEvent::PointerReleased {
        position: at(0.0, -110.0),
        button: PointerEventButton::Left,
    });
    assert_eq!(*sent.borrow(), [175.0]);

    // The dead zone at the bottom of the dial is ignored.
    window.dispatch_event(WindowEvent::PointerPressed {
        position: at(0.0, 110.0),
        button: PointerEventButton::Left,
    });
    window.dispatch_event(WindowEvent::PointerReleased {
        position: at(0.0, 110.0),
        button: PointerEventButton::Left,
    });
    assert_eq!(sent.borrow().len(), 1);
}

fn keypad_value_goes_to_the_heater_that_was_open() {
    let ui = AppWindow::new().unwrap();
    ui.set_link_state("ready".into());
    ui.set_max_bed(130.0);
    ui.set_screen("heating".into());
    ui.set_heating_heater(1);
    ui.show().unwrap();

    let sent = Rc::new(RefCell::new(Vec::<f32>::new()));
    let sink = sent.clone();
    ui.on_set_bed(move |t| sink.borrow_mut().push(t));

    ElementHandle::find_by_accessible_label(&ui, "Enter a number")
        .next()
        .unwrap()
        .invoke_accessible_default_action();
    assert_eq!(ui.get_overlay(), "keypad");
    assert_eq!(ui.get_keypad_max(), 130.0);
    for key in ["6", "0"] {
        ElementHandle::find_by_accessible_label(&ui, key)
            .next()
            .unwrap()
            .invoke_accessible_default_action();
    }
    ElementHandle::find_by_accessible_label(&ui, "Done")
        .next()
        .unwrap()
        .invoke_accessible_default_action();
    assert_eq!(*sent.borrow(), [60.0]);
}
