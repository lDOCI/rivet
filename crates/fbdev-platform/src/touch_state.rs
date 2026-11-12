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

/// Tells a tap from a drag the way phones do, for a finger on this resistive
/// panel (a stylus barely needs it):
///
/// - a light, broad fingertip makes the reported point jump by several pixels,
///   most in the first moments of a press: positions go through a median of
///   the last three, and the first `SETTLE` after a press never start a drag;
/// - until the finger has gone further than `slop` from where it touched, its
///   wobble is not passed on, and a lift there is a tap at the touch point;
/// - a press while a flung list still glides only stops it: it is no tap on
///   whatever is under the finger (as on a phone).
///
/// Without it every wobble reached Slint's Flickable, which takes 8 px for a
/// scroll and cancels the button under the finger.
#[derive(Debug, Clone, Copy)]
pub struct TapSlop {
    slop: f32,
    origin: Option<(f32, f32)>,
    pressed_at: Option<std::time::Instant>,
    dragging: bool,
    /// The last raw points of this touch, for the median.
    recent: [(f32, f32); 3],
    count: usize,
    /// The last two points passed on while dragging, for the fling speed.
    last: Option<(std::time::Instant, f32, f32)>,
    prev: Option<(std::time::Instant, f32, f32)>,
    /// A list released while moving keeps gliding until about then.
    glide_until: Option<std::time::Instant>,
    /// This touch only stops a glide.
    stopper: bool,
}

/// How long after a press the reported point is still settling.
const SETTLE: std::time::Duration = std::time::Duration::from_millis(60);
/// Slint's Flickable slows a fling by this much, px/s² (i-slint-core DECELERATION).
const DECELERATION: f32 = 2000.0;
/// Slower than this, a release does not glide on.
const MIN_FLING: f32 = 80.0;

impl TapSlop {
    pub fn new(slop: f32) -> Self {
        Self {
            slop,
            origin: None,
            pressed_at: None,
            dragging: false,
            recent: [(0.0, 0.0); 3],
            count: 0,
            last: None,
            prev: None,
            glide_until: None,
            stopper: false,
        }
    }

    fn median(&mut self, x: f32, y: f32) -> (f32, f32) {
        self.recent[self.count % 3] = (x, y);
        self.count += 1;
        if self.count < 3 {
            return (x, y);
        }
        let mid = |mut v: [f32; 3]| {
            v.sort_by(f32::total_cmp);
            v[1]
        };
        (mid(self.recent.map(|p| p.0)), mid(self.recent.map(|p| p.1)))
    }

    /// A finger lands. Returns where, and whether it only stops a glide.
    pub fn press(&mut self, x: f32, y: f32, now: std::time::Instant) -> (f32, f32, bool) {
        self.stopper = self.glide_until.is_some_and(|t| now < t);
        self.glide_until = None;
        self.origin = Some((x, y));
        self.pressed_at = Some(now);
        self.dragging = false;
        self.count = 0;
        self.median(x, y);
        self.last = None;
        self.prev = None;
        (x, y, self.stopper)
    }

    /// The position to pass on, or None while the finger is still a tap.
    pub fn moved(&mut self, x: f32, y: f32, now: std::time::Instant) -> Option<(f32, f32)> {
        // The point is still settling: these samples do not count at all.
        if !self.dragging
            && self
                .pressed_at
                .is_some_and(|t| now.duration_since(t) < SETTLE)
        {
            return None;
        }
        let (x, y) = self.median(x, y);
        let Some((ox, oy)) = self.origin else {
            return Some((x, y));
        };
        if !self.dragging {
            if (x - ox).hypot(y - oy) <= self.slop {
                return None;
            }
            self.dragging = true;
        }
        self.prev = self.last;
        self.last = Some((now, x, y));
        Some((x, y))
    }

    /// Where the lift lands (the touch point for a tap), and whether it may
    /// click: not when the touch only stopped a glide. The state stays until
    /// the next press: a finger that loses contact for a moment and comes back
    /// goes on with the same touch.
    pub fn release(&mut self, x: f32, y: f32, now: std::time::Instant) -> (f32, f32, bool) {
        if self.dragging {
            // Released while moving: the list glides on for v / deceleration.
            if let (Some((t1, x1, y1)), Some((t0, x0, y0))) = (self.last, self.prev) {
                let dt = t1.duration_since(t0).as_secs_f32().max(0.005);
                let recent = now.duration_since(t1).as_secs_f32() < 0.1;
                let speed = (x1 - x0).hypot(y1 - y0) / dt;
                if recent && speed > MIN_FLING {
                    let secs = (speed / DECELERATION).min(3.0);
                    self.glide_until = Some(now + std::time::Duration::from_secs_f32(secs));
                }
            }
            return (x, y, true);
        }
        let (x, y) = self.origin.unwrap_or((x, y));
        (x, y, !self.stopper)
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
        use std::time::{Duration, Instant};
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        let mut t = TapSlop::new(14.0);
        // A wobbling finger: nothing passes, the lift lands where it touched.
        assert_eq!(t.press(100.0, 200.0, at(0)), (100.0, 200.0, false));
        assert_eq!(t.moved(106.0, 195.0, at(80)), None);
        assert_eq!(t.moved(92.0, 204.0, at(100)), None);
        assert_eq!(t.release(109.0, 208.0, at(120)), (100.0, 200.0, true));
        // A jump in the first moments of a press is no drag.
        t.press(100.0, 200.0, at(1000));
        assert_eq!(t.moved(100.0, 240.0, at(1020)), None);
        // One wild sample later on is taken out by the median.
        assert_eq!(t.moved(101.0, 201.0, at(1100)), None);
        assert_eq!(t.moved(100.0, 260.0, at(1110)), None);
        assert_eq!(t.release(100.0, 200.0, at(1130)), (100.0, 200.0, true));
        // A finger that travels: moves pass once it leaves the slop.
        t.press(100.0, 300.0, at(2000));
        assert_eq!(t.moved(100.0, 290.0, at(2070)), None);
        assert_eq!(t.moved(100.0, 270.0, at(2080)), None); // median still 290
        assert!(t.moved(100.0, 250.0, at(2090)).is_some());
        assert!(t.moved(100.0, 230.0, at(2100)).is_some());
        // Released fast: the list glides; a press now only stops it, no click.
        assert!(t.release(100.0, 225.0, at(2105)).2);
        assert!(t.press(120.0, 100.0, at(2200)).2);
        assert_eq!(t.release(120.0, 100.0, at(2260)), (120.0, 100.0, false));
        // The next touch is a tap again.
        assert!(!t.press(50.0, 50.0, at(9000)).2);
        assert!(t.near(60.0, 50.0, 40.0));
        assert!(!t.near(150.0, 50.0, 40.0));
        assert_eq!(t.release(55.0, 52.0, at(9050)), (50.0, 50.0, true));
    }
}
