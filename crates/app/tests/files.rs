//! Selecting files and deleting them, driven like a finger would.

use std::{cell::RefCell, rc::Rc};

use i_slint_backend_testing::ElementHandle;
use rivet::{AppWindow, FileItem};
use slint::{ComponentHandle, Model, ModelRc, VecModel};

fn tap(ui: &AppWindow, label: &str) {
    ElementHandle::find_by_accessible_label(ui, label)
        .next()
        .unwrap_or_else(|| panic!("nothing labelled {label:?}"))
        .invoke_accessible_default_action();
}

#[test]
fn select_several_and_delete() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    // The labels below are English whatever language the machine uses.
    rivet::apply_language("en");
    let items = ["a", "b", "c"].map(|n| FileItem {
        path: format!("{n}.gcode").as_str().into(),
        name: n.into(),
        days_ago: 1,
        size_kb: 10,
        ..Default::default()
    });
    ui.set_files(ModelRc::new(VecModel::from(items.to_vec())));
    ui.set_link_state("ready".into());
    ui.set_screen("files".into());
    ui.show().unwrap();

    let deleted = Rc::new(RefCell::new(Vec::<String>::new()));
    let sink = deleted.clone();
    let weak = ui.as_weak();
    ui.on_delete_selected(move || {
        let ui = weak.upgrade().unwrap();
        sink.borrow_mut().extend(
            ui.get_files()
                .iter()
                .filter(|f| f.selected)
                .map(|f| f.path.to_string()),
        );
    });

    tap(&ui, "Select several");
    tap(&ui, "a");
    tap(&ui, "c");
    assert_eq!(ui.get_selected_count(), 2);
    // Tap again to unselect.
    tap(&ui, "c");
    assert_eq!(ui.get_selected_count(), 1);

    tap(&ui, "Delete 1");
    // The confirm dialog is up; nothing is deleted yet.
    assert!(deleted.borrow().is_empty());
    assert_eq!(ui.get_overlay(), "delete-files");
    tap(&ui, "Delete");
    assert_eq!(*deleted.borrow(), ["a.gcode"]);
    assert_eq!(ui.get_overlay(), "");
    assert_eq!(ui.get_selected_count(), 0);
}
