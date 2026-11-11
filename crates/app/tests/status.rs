//! While the printer cannot be used, the status screen replaces everything.

use std::{cell::Cell, rc::Rc};

use i_slint_backend_testing::ElementHandle;
use rivet::AppWindow;
use slint::ComponentHandle;

#[test]
fn status_replaces_screens_until_ready() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    // The labels below are English whatever language the machine uses.
    rivet::apply_language("en");
    ui.set_screen("control".into());
    ui.show().unwrap();

    let label = |l: &str| ElementHandle::find_by_accessible_label(&ui, l).next();

    // No link: no Control tiles, no restart button.
    assert!(label("Heating").is_none());
    assert!(label("Restart firmware").is_none());

    // Klipper stopped: the way out is a firmware restart.
    let restarts = Rc::new(Cell::new(0));
    let count = restarts.clone();
    ui.on_firmware_restart(move || count.set(count.get() + 1));
    ui.set_link_state("error".into());
    assert!(label("Heating").is_none());
    label("Restart firmware")
        .expect("restart button")
        .invoke_accessible_default_action();
    assert_eq!(restarts.get(), 1);

    // Ready: the screen the user was on comes back.
    ui.set_link_state("ready".into());
    assert!(label("Restart firmware").is_none());
    assert!(label("Heating").is_some());
}
