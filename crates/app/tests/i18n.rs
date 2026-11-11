//! The Russian translation is bundled and switches the interface.

use i_slint_backend_testing::ElementHandle;
use rivet::AppWindow;
use slint::ComponentHandle;

#[test]
fn russian_interface() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    ui.set_link_state("ready".into());
    ui.set_screen("control".into());
    ui.show().unwrap();
    let has = |label: &str| {
        ElementHandle::find_by_accessible_label(&ui, label)
            .next()
            .is_some()
    };

    rivet::apply_language("en");
    assert!(has("Heating") && !has("Нагрев"));
    rivet::apply_language("ru");
    assert!(
        has("Нагрев") && has("Управление"),
        "labels follow the language"
    );
    assert!(!has("Heating"));
}
