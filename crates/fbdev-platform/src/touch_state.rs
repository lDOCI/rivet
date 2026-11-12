//! Platform-independent evdev touch decoding: raw `(type, code, value)`
//! events in, pointer actions in screen coordinates out.

pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_ABS: u16 = 0x03;

pub const SYN_REPORT: u16 = 0x00;

pub const BTN_LEFT: u16 = 0x110;
pub const BTN_TOUCH: u16 = 0x14a;

pub const ABS_X: u16 = 0x00;
pub const ABS_Y: u16 = 0x01;
pub const ABS_PRESSURE: u16 = 0x18;
pub const ABS_MT_POSITION_X: u16 = 0x35;
pub const ABS_MT_POSITION_Y: u16 = 0x36;
pub const ABS_MT_TRACKING_ID: u16 = 0x39;

/// Corrections for panels whose touch axes don't match the display.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TouchTransform {
    pub swap_xy: bool,
    pub invert_x: bool,
    pub invert_y: bool,
}

/// Reported range of one absolute axis (`EVIOCGABS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AxisRange {
    pub min: i32,
    pub max: i32,
}

impl AxisRange {
    fn normalize(self, value: i32) -> f32 {
        if self.max <= self.min {
            return 0.0;
        }
        ((value - self.min) as f32 / (self.max - self.min) as f32).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TouchAction {
    Pressed { x: f32, y: f32 },
    Moved { x: f32, y: f32 },
    Released { x: f32, y: f32 },
}

/// Accumulates events between `SYN_REPORT`s and emits one action per frame.
#[derive(Debug)]
pub struct TouchState {
    range_x: AxisRange,
    range_y: AxisRange,
    /// UI size in logical pixels.
    width: f32,
    height: f32,
    transform: TouchTransform,

    raw_x: i32,
    raw_y: i32,
    down: bool,
    next_down: Option<bool>,
    moved: bool,
}

impl TouchState {
    pub fn new(
        range_x: AxisRange,
        range_y: AxisRange,
        width: f32,
        height: f32,
        transform: TouchTransform,
    ) -> Self {
        Self {
            range_x,
            range_y,
            width,
            height,
            transform,
            raw_x: 0,
            raw_y: 0,
            down: false,
            next_down: None,
            moved: false,
        }
    }

    pub fn is_down(&self) -> bool {
        self.down
    }

    fn position(&self) -> (f32, f32) {
        let mut nx = self.range_x.normalize(self.raw_x);
        let mut ny = self.range_y.normalize(self.raw_y);
        if self.transform.swap_xy {
            std::mem::swap(&mut nx, &mut ny);
        }
        if self.transform.invert_x {
            nx = 1.0 - nx;
        }
        if self.transform.invert_y {
            ny = 1.0 - ny;
        }
        (nx * self.width, ny * self.height)
    }

    /// Feed one raw evdev event.
    pub fn process(&mut self, kind: u16, code: u16, value: i32) -> Option<TouchAction> {
        match (kind, code) {
            (EV_ABS, ABS_X | ABS_MT_POSITION_X) => {
                self.raw_x = value;
                self.moved = true;
            }
            (EV_ABS, ABS_Y | ABS_MT_POSITION_Y) => {
                self.raw_y = value;
                self.moved = true;
            }
            (EV_ABS, ABS_MT_TRACKING_ID) => self.next_down = Some(value >= 0),
            // tslib without BTN_TOUCH signals contact through pressure.
            (EV_ABS, ABS_PRESSURE) => self.next_down = Some(value > 0),
            (EV_KEY, BTN_TOUCH | BTN_LEFT) => self.next_down = Some(value != 0),
            (EV_SYN, SYN_REPORT) => return self.flush(),
            _ => {}
        }
        None
    }

    fn flush(&mut self) -> Option<TouchAction> {
        let (x, y) = self.position();
        let moved = std::mem::take(&mut self.moved);
        match self.next_down.take() {
            Some(true) if !self.down => {
                self.down = true;
                Some(TouchAction::Pressed { x, y })
            }
            Some(false) if self.down => {
                self.down = false;
                Some(TouchAction::Released { x, y })
            }
            _ if self.down && moved => Some(TouchAction::Moved { x, y }),
            _ => None,
        }
    }
}

/// Tells a tap from a drag the way phones do: until the finger has gone
/// further than `slop` from where it touched, its jitter is not passed on, and
/// a lift there counts as a tap at the touch point. Only a finger that clearly
/// travels starts a drag (a list scroll).
///
/// Without it every wobble of a fingertip (more for a broad finger, which rolls
/// as it presses) reaches Slint's Flickable, which takes 8 px of movement for a
/// scroll and cancels the button under the finger.
#[derive(Debug, Clone, Copy)]
pub struct TapSlop {
    slop: f32,
    origin: Option<(f32, f32)>,
    dragging: bool,
}

impl TapSlop {
    pub fn new(slop: f32) -> Self {
        Self {
            slop,
            origin: None,
            dragging: false,
        }
    }

    pub fn press(&mut self, x: f32, y: f32) -> (f32, f32) {
        self.origin = Some((x, y));
        self.dragging = false;
        (x, y)
    }

    /// The position to pass on, or None while the finger is still within the slop.
    pub fn moved(&mut self, x: f32, y: f32) -> Option<(f32, f32)> {
        let Some((ox, oy)) = self.origin else {
            return Some((x, y));
        };
        if !self.dragging && (x - ox).hypot(y - oy) <= self.slop {
            return None;
        }
        self.dragging = true;
        Some((x, y))
    }

    /// Where the lift lands: the touch point for a tap, the finger for a drag.
    /// The state stays until the next press: a finger that loses contact for a
    /// moment and comes back goes on with the same touch.
    pub fn release(&mut self, x: f32, y: f32) -> (f32, f32) {
        match (self.origin, self.dragging) {
            (Some(origin), false) => origin,
            _ => (x, y),
        }
    }

    /// Whether a point is close enough to the touch to be the same finger.
    pub fn near(&self, x: f32, y: f32, distance: f32) -> bool {
        self.origin
            .is_some_and(|(ox, oy)| (x - ox).hypot(y - oy) <= distance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(transform: TouchTransform) -> TouchState {
        TouchState::new(
            AxisRange { min: 0, max: 799 },
            AxisRange { min: 0, max: 479 },
            800.0,
            480.0,
            transform,
        )
    }

    fn tap(s: &mut TouchState, x: i32, y: i32) -> Vec<TouchAction> {
        let events = [
            (EV_ABS, ABS_X, x),
            (EV_ABS, ABS_Y, y),
            (EV_KEY, BTN_TOUCH, 1),
            (EV_SYN, SYN_REPORT, 0),
            (EV_KEY, BTN_TOUCH, 0),
            (EV_SYN, SYN_REPORT, 0),
        ];
        events
            .into_iter()
            .filter_map(|(k, c, v)| s.process(k, c, v))
            .collect()
    }

    #[test]
    fn single_tap_presses_and_releases() {
        let mut s = state(TouchTransform::default());
        let actions = tap(&mut s, 799, 0);
        assert_eq!(
            actions,
            vec![
                TouchAction::Pressed { x: 800.0, y: 0.0 },
                TouchAction::Released { x: 800.0, y: 0.0 },
            ]
        );
        assert!(!s.is_down());
    }

    #[test]
    fn drag_emits_moves_only_while_down() {
        let mut s = state(TouchTransform::default());
        // Hover-like movement without contact: nothing.
        assert_eq!(s.process(EV_ABS, ABS_X, 100), None);
        assert_eq!(s.process(EV_SYN, SYN_REPORT, 0), None);

        s.process(EV_KEY, BTN_TOUCH, 1);
        assert!(matches!(
            s.process(EV_SYN, SYN_REPORT, 0),
            Some(TouchAction::Pressed { .. })
        ));
        s.process(EV_ABS, ABS_X, 200);
        assert!(matches!(
            s.process(EV_SYN, SYN_REPORT, 0),
            Some(TouchAction::Moved { .. })
        ));
        // No new coordinates: no duplicate move.
        assert_eq!(s.process(EV_SYN, SYN_REPORT, 0), None);
    }

    #[test]
    fn multitouch_tracking_id() {
        let mut s = state(TouchTransform::default());
        s.process(EV_ABS, ABS_MT_TRACKING_ID, 5);
        s.process(EV_ABS, ABS_MT_POSITION_X, 400);
        s.process(EV_ABS, ABS_MT_POSITION_Y, 240);
        assert!(matches!(
            s.process(EV_SYN, SYN_REPORT, 0),
            Some(TouchAction::Pressed { .. })
        ));
        s.process(EV_ABS, ABS_MT_TRACKING_ID, -1);
        assert!(matches!(
            s.process(EV_SYN, SYN_REPORT, 0),
            Some(TouchAction::Released { .. })
        ));
    }

    #[test]
    fn transform_inverts_and_swaps() {
        let mut s = state(TouchTransform {
            swap_xy: false,
            invert_x: true,
            invert_y: true,
        });
        let actions = tap(&mut s, 0, 0);
        assert_eq!(actions[0], TouchAction::Pressed { x: 800.0, y: 480.0 });

        // Raw X at 25 % lands on screen Y after swap.
        let mut s = state(TouchTransform {
            swap_xy: true,
            invert_x: false,
            invert_y: false,
        });
        let actions = tap(&mut s, 200, 0);
        match actions[0] {
            TouchAction::Pressed { x, y } => {
                assert_eq!(x, 0.0);
                assert!((y - 480.0 * 200.0 / 799.0).abs() < 0.01);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn pressure_signals_contact() {
        let mut s = state(TouchTransform::default());
        s.process(EV_ABS, ABS_PRESSURE, 255);
        assert!(matches!(
            s.process(EV_SYN, SYN_REPORT, 0),
            Some(TouchAction::Pressed { .. })
        ));
        s.process(EV_ABS, ABS_PRESSURE, 0);
        assert!(matches!(
            s.process(EV_SYN, SYN_REPORT, 0),
            Some(TouchAction::Released { .. })
        ));
    }

    #[test]
    fn tap_slop_keeps_taps_and_lets_drags_through() {
        let mut t = TapSlop::new(14.0);
        // A wobbling finger: nothing passes, the lift lands where it touched.
        assert_eq!(t.press(100.0, 200.0), (100.0, 200.0));
        assert_eq!(t.moved(106.0, 195.0), None);
        assert_eq!(t.moved(92.0, 204.0), None);
        assert_eq!(t.release(109.0, 208.0), (100.0, 200.0));
        // A finger that travels: moves pass once it leaves the slop, and from
        // then on even small ones do.
        t.press(100.0, 300.0);
        assert_eq!(t.moved(100.0, 290.0), None);
        assert_eq!(t.moved(100.0, 280.0), Some((100.0, 280.0)));
        assert_eq!(t.moved(100.0, 278.0), Some((100.0, 278.0)));
        assert_eq!(t.release(100.0, 260.0), (100.0, 260.0));
        // The next touch starts fresh.
        t.press(50.0, 50.0);
        assert!(t.near(60.0, 50.0, 40.0));
        assert!(!t.near(150.0, 50.0, 40.0));
        assert_eq!(t.release(55.0, 52.0), (50.0, 50.0));
    }
}
