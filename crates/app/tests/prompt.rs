//! A recovery prompt replaces live content and keeps its actions and Stop usable.
use i_slint_backend_testing::ElementHandle;
use rivet::{AppWindow, PromptButton};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[test]
fn recovery_prompt_hides_start_controls_and_live_dial_until_dismissed() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    rivet::apply_language("en");
    ui.set_link_state("ready".into());
    ui.set_screen("home".into());
    ui.set_last_name("previous.gcode".into());
    ui.show().unwrap();
    let find = |label: &str| ElementHandle::find_by_accessible_label(&ui, label).next();
    assert!(find("Print again").is_some());
    ui.set_prompt_title("Interrupted print".into());
    ui.set_prompt_buttons(ModelRc::new(VecModel::from(vec![PromptButton {
        label: "Delete recovery data".into(),
        gcode: "_ZRESTORE_DEL".into(),
        style: "primary".into(),
    }])));
    ui.set_prompt_shown(true);
    assert!(find("Print again").is_none());
    let code = Rc::new(RefCell::new(Vec::new()));
    let sink = code.clone();
    ui.on_run_gcode(move |s| sink.borrow_mut().push(s.to_string()));
    find("Delete recovery data")
        .unwrap()
        .invoke_accessible_default_action();
    assert_eq!(*code.borrow(), ["_ZRESTORE_DEL"]);
    // Do not locally dismiss the prompt when an answer is sent. Wait for Klipper.
    assert!(find("Print again").is_none());
    let stops = Rc::new(Cell::new(0));
    let sink = stops.clone();
    ui.on_emergency_stop(move || sink.set(sink.get() + 1));
    find("Emergency stop")
        .unwrap()
        .invoke_accessible_default_action();
    assert_eq!(stops.get(), 1);

    // Reproduce the photograph's changing background while the dialog is open.
    ui.set_screen("heating".into());
    ui.set_extruder_temp(180.0);
    ui.set_extruder_target(220.0);
    assert!(find("Target temperature").is_none());
    ui.set_extruder_temp(190.0);
    assert!(find("Target temperature").is_none());
    find("Close").unwrap().invoke_accessible_default_action();
    assert_eq!(
        code.borrow().last().unwrap(),
        "RESPOND TYPE=command MSG=action:prompt_end"
    );
    assert!(find("Target temperature").is_none());
    // Simulate the firmware's prompt_end acknowledgement.
    ui.set_prompt_shown(false);
    assert!(find("Target temperature").is_some());
}
