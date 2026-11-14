//! Keypad logic, driven the way a finger would: find a key by its accessible
//! label and activate it.

use std::{cell::RefCell, rc::Rc, sync::Once};

use i_slint_backend_testing::ElementHandle;
use rivet::AppWindow;
use slint::ComponentHandle;

static INIT: Once = Once::new();

struct Pad {
    ui: AppWindow,
    done: Rc<RefCell<Vec<f32>>>,
}

impl Pad {
    fn open(min: f32, max: f32, negative: bool) -> Pad {
        INIT.call_once(i_slint_backend_testing::init_no_event_loop);
        let ui = AppWindow::new().unwrap();
        // The labels below are English whatever language the machine uses.
        rivet::apply_language("en");
        ui.set_keypad_min(min);
        ui.set_keypad_max(max);
        ui.set_keypad_negative(negative);
        ui.set_overlay("keypad".into());
        ui.show().unwrap();
        let done = Rc::new(RefCell::new(Vec::new()));
        let sink = done.clone();
        ui.on_keypad_done(move |v| sink.borrow_mut().push(v));
        Pad { ui, done }
    }

    fn tap(&self, label: &str) {
        let key = ElementHandle::find_by_accessible_label(&self.ui, label)
            .next()
            .unwrap_or_else(|| panic!("no key {label:?}"));
        key.invoke_accessible_default_action();
    }

    fn type_keys(&self, keys: &str) {
        for k in keys.chars() {
            self.tap(&k.to_string());
        }
    }

    fn finish(&self) -> Option<f32> {
        self.tap("Done");
        self.done.borrow().last().copied()
    }
}

fn integers_and_decimals() {
    let pad = Pad::open(0.0, 350.0, false);
    pad.type_keys("240");
    assert_eq!(pad.finish(), Some(240.0));

    let pad = Pad::open(-1.0, 1.0, true);
    pad.type_keys("0.05");
    assert_eq!(pad.finish(), Some(0.05));
}

fn negative_numbers() {
    let pad = Pad::open(-1.0, 1.0, true);
    pad.type_keys("−0.12");
    assert_eq!(pad.finish(), Some(-0.12));

    // Not allowed: the sign key does nothing.
    let pad = Pad::open(0.0, 10.0, false);
    pad.type_keys("−5");
    assert_eq!(pad.finish(), Some(5.0));
}

fn backspace_edits_and_empties() {
    let pad = Pad::open(0.0, 350.0, false);
    pad.type_keys("2");
    pad.tap("⌫");
    // Empty again: Done does nothing.
    assert_eq!(pad.finish(), None);
    pad.type_keys("19");
    pad.tap("⌫");
    pad.type_keys("5");
    assert_eq!(pad.finish(), Some(15.0));

    let pad = Pad::open(0.0, 100.0, false);
    pad.type_keys("1.25");
    pad.tap("⌫");
    pad.tap("⌫");
    pad.tap("⌫");
    pad.type_keys("3");
    assert_eq!(pad.finish(), Some(13.0));
}

fn out_of_range_cannot_be_confirmed() {
    let pad = Pad::open(0.0, 350.0, false);
    pad.type_keys("400");
    assert_eq!(pad.finish(), None);
    pad.tap("⌫");
    assert_eq!(pad.finish(), Some(40.0));
}

fn only_one_decimal_point_and_three_decimals() {
    let pad = Pad::open(0.0, 10.0, false);
    pad.type_keys("1.2.3456");
    assert_eq!(pad.finish(), Some(1.234));
}

fn done_keeps_the_value_shown() {
    let pad = Pad::open(1.0, 500.0, false);
    pad.ui.set_keypad_current("150".into());
    assert_eq!(pad.finish(), Some(150.0));
    // Shown but out of range: nothing to confirm.
    let pad = Pad::open(1.0, 100.0, false);
    pad.ui.set_keypad_current("150".into());
    assert_eq!(pad.finish(), None);
}

/// The Extruder screen's own length: Custom… opens the keypad, Done sets it.
fn custom_extrude_length() {
    let pad = Pad::open(0.0, 1.0, false);
    let ui = &pad.ui;
    ui.set_overlay("".into());
    ui.set_link_state("ready".into());
    ui.set_screen("extruder".into());
    pad.tap("Custom…");
    assert_eq!(ui.get_overlay(), "keypad");
    // Digits the screen behind has no button for (its speeds are 2, 5, 10, 15).
    pad.type_keys("130");
    pad.tap("Done");
    assert_eq!(ui.get_overlay(), "");
    assert_eq!(ui.get_extrude_custom(), 130);
    assert_eq!(ui.get_extrude_length(), 4);
}

/// One test: the Slint platform is set once per process and windows belong to
/// the thread that made them, so scenarios run one after another.
#[test]
fn keypad() {
    integers_and_decimals();
    negative_numbers();
    backspace_edits_and_empties();
    out_of_range_cannot_be_confirmed();
    only_one_decimal_point_and_three_decimals();
    done_keeps_the_value_shown();
    custom_extrude_length();
}
