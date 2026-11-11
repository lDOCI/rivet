//! Typing on the on-screen keyboard and running macros.

use std::{cell::RefCell, rc::Rc};

use i_slint_backend_testing::ElementHandle;
use rivet::{AppWindow, ConsoleLine};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

fn tap(ui: &AppWindow, label: &str) {
    ElementHandle::find_by_accessible_label(ui, label)
        .next()
        .unwrap_or_else(|| panic!("nothing labelled {label:?}"))
        .invoke_accessible_default_action();
}

#[test]
fn console_keyboard_and_macros() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    rivet::apply_language("en");
    ui.set_link_state("ready".into());
    ui.on_apply_key(|t, k| rivet::apply_key(&t, &k).into());
    let sent = Rc::new(RefCell::new(Vec::<String>::new()));
    let sink = sent.clone();
    ui.on_run_gcode(move |s| sink.borrow_mut().push(s.to_string()));
    ui.set_console_lines(ModelRc::new(VecModel::from(vec![ConsoleLine {
        text: "ok".into(),
        kind: 1,
    }])));
    ui.set_macros(ModelRc::new(VecModel::from(vec![
        SharedString::from("PAUSE"),
        SharedString::from("BELTS"),
    ])));
    ui.set_screen("console".into());
    ui.show().unwrap();

    // Shift starts on: capitals. Type G28, fix a typo with backspace, send.
    tap(&ui, "Command, tap to type");
    for k in ["G", "2", "9", "Erase", "8"] {
        tap(&ui, k);
    }
    assert_eq!(ui.get_console_text(), "G28");
    tap(&ui, "Done");
    assert_eq!(*sent.borrow(), ["G28"]);
    assert_eq!(ui.get_console_text(), "");
    assert!(!ui.get_console_keyboard());

    // A quick button runs at once.
    tap(&ui, "M105");
    assert_eq!(sent.borrow().last().unwrap(), "M105");

    // Macros ask first.
    ui.set_screen("macros".into());
    tap(&ui, "PAUSE");
    assert_eq!(ui.get_overlay(), "run-macro");
    assert_eq!(sent.borrow().len(), 2);
    tap(&ui, "Run");
    assert_eq!(sent.borrow().last().unwrap(), "PAUSE");
    assert_eq!(ui.get_overlay(), "");
}
