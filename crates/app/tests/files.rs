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

#[test]
fn previews_follow_the_scroll() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    // 86 files, as on the AD5M Pro, every one with a preview already.
    let items: Vec<FileItem> = (0..86)
        .map(|i| FileItem {
            path: format!("f{i}.gcode").as_str().into(),
            name: format!("f{i}").as_str().into(),
            rank: i,
            has_thumb: true,
            ..Default::default()
        })
        .collect();
    rivet::files::set_all(&ui, items);
    // Scrolled down to card 60: far previews go, the near ones stay.
    let wanted = rivet::files::window(&ui, 60);
    assert!(wanted.is_empty(), "all near ones were loaded: {wanted:?}");
    let files = ui.get_files();
    let kept: Vec<usize> = (0..files.row_count())
        .filter(|&i| files.row_data(i).unwrap().has_thumb)
        .collect();
    assert_eq!(kept, (54..78).collect::<Vec<_>>());
    // Back at the top: the first screen is asked for first, nearest first.
    let wanted = rivet::files::window(&ui, 0);
    assert_eq!(wanted.first().map(String::as_str), Some("f0.gcode"));
    assert_eq!(wanted.len(), 15);
    // The end of the list: the last cards, then the row above them.
    let wanted = rivet::files::window(&ui, 84);
    assert_eq!(
        wanted,
        ["f84", "f85", "f83", "f82", "f81"].map(|n| format!("{n}.gcode"))
    );
}
