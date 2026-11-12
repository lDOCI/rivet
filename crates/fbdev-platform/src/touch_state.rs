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

type GlideProbe = Box<dyn Fn() -> bool>;

thread_local! {
    /// Asks the UI whether a list is moving right now (set by the app; the event
    /// loop runs on the UI thread).
    static GLIDE_PROBE: std::cell::RefCell<Option<GlideProbe>> = const { std::cell::RefCell::new(None) };
}

/// Tell the touch filter how to see whether a list is still gliding, so a
/// press only stops it when it really moves (not after it hit its end).
pub fn set_glide_probe(probe: impl Fn() -> bool + 'static) {
    GLIDE_PROBE.with(|p| *p.borrow_mut() = Some(Box::new(probe)));
}

fn list_moving() -> Option<bool> {
    GLIDE_PROBE.with(|p| p.borrow().as_ref().map(|f| f()))
}

/// A correction for where touches land, from Rivet's touch calibration:
/// x' = a·x + b·y + c, y' = d·x + e·y + f, applied to every touch point.
static CORRECTION: std::sync::Mutex<Option<[f32; 6]>> = std::sync::Mutex::new(None);

/// Use this correction from now on (None: touches as the panel reports them).
pub fn set_correction(c: Option<[f32; 6]>) {
    if let Ok(mut slot) = CORRECTION.lock() {
        *slot = c;
    }
}

/// The correction in use.
pub fn correction() -> Option<[f32; 6]> {
    CORRECTION.lock().ok().and_then(|c| *c)
}

/// A touch point with the correction applied.
pub fn corrected(x: f32, y: f32) -> (f32, f32) {
    match correction() {
        Some([a, b, c, d, e, f]) => (a * x + b * y + c, d * x + e * y + f),
        None => (x, y),
    }
}

/// Tells a tap from a drag for a finger on this resistive panel (a stylus
/// barely needs it). Tuned on a recording of real fingers on the AD5M
/// (testdata/ad5m-finger.evdev; the panel reports every 10 ms):
///
/// - the first 20 ms (two samples) set where the finger landed: the landing
///   sample may be 20 px off;
/// - a tap wobbles a few pixels, and the sample as the finger lands or lifts
///   often jumps 15–25 px: a drag starts only when two samples in a row are
///   further than `slop` from the touch point, so one wild sample is no drag,
///   and a lift before that is a tap at the touch point;
/// - a scroll is a quick flick (~200 px in 100 ms): from the second sample
///   out of the slop everything passes at once, so the list follows the
///   finger and keeps its speed; a single sample jumping further than a
///   finger can move in 10 ms is dropped;
/// - a press while a flung list still glides only stops it: it is no tap on
///   whatever is under the finger (as on a phone).
#[derive(Debug, Clone, Copy)]
pub struct TapSlop {
    slop: f32,
    /// Where the finger landed (after the landing samples) and where it pressed.
    origin: Option<(f32, f32)>,
    start: (f32, f32),
    pressed_at: Option<std::time::Instant>,
    dragging: bool,
    /// The previous sample, when it was already out of the slop.
    out: bool,
    /// The last point passed on while dragging.
    last_pos: Option<(f32, f32)>,
    /// The last two points passed on while dragging, for the fling speed.
    last: Option<(std::time::Instant, f32, f32)>,
    prev: Option<(std::time::Instant, f32, f32)>,
    /// A list released while moving keeps gliding until about then.
    glide_until: Option<std::time::Instant>,
    /// This touch only stops a glide.
    stopper: bool,
}

/// Slint's Flickable slows a fling by this much, px/s² (i-slint-core DECELERATION).
const DECELERATION: f32 = 2000.0;
/// Slower than this, a release does not glide on.
const MIN_FLING: f32 = 80.0;
/// This many slops from the press point is a drag at once (jumps of a tap
/// reach ~1 slop; a short flick 2.5 within two samples).
const FAR: f32 = 2.5;
/// How long after a press the touch point still follows the finger (two samples).
pub const LANDING: std::time::Duration = std::time::Duration::from_millis(20);
/// Further than this between two samples (10 ms) is a glitch, not a finger.
const MAX_STEP: f32 = 90.0;

impl TapSlop {
    pub fn new(slop: f32) -> Self {
        Self {
            slop,
            origin: None,
            start: (0.0, 0.0),
            pressed_at: None,
            dragging: false,
            out: false,
            last_pos: None,
            last: None,
            prev: None,
            glide_until: None,
            stopper: false,
        }
    }

    /// A finger lands. Returns where, and whether it only stops a glide.
    pub fn press(&mut self, x: f32, y: f32, now: std::time::Instant) -> (f32, f32, bool) {
        // The UI says whether a list moves; without it, the speed of the last
        // flick is the guess.
        self.stopper = list_moving().unwrap_or_else(|| self.glide_until.is_some_and(|t| now < t));
        self.glide_until = None;
        self.origin = Some((x, y));
        self.start = (x, y);
        self.pressed_at = Some(now);
        self.dragging = false;
        self.out = false;
        self.last_pos = None;
        self.last = None;
        self.prev = None;
        (x, y, self.stopper)
    }

    /// The position to pass on, or None while the finger is still a tap.
    pub fn moved(&mut self, x: f32, y: f32, now: std::time::Instant) -> Option<(f32, f32)> {
        let Some((ox, oy)) = self.origin else {
            return Some((x, y));
        };
        if !self.dragging {
            // A finger this far from where it pressed is surely moving: a short
            // flick may give only two samples.
            let far = (x - self.start.0).hypot(y - self.start.1) > FAR * self.slop;
            // The landing sample may be off by 20 px and the finger stay
            // there: for its first moments the touch point follows it.
            if !far
                && self
                    .pressed_at
                    .is_some_and(|t| now.duration_since(t) <= LANDING)
            {
                self.origin = Some((x, y));
                return None;
            }
            let out = (x - ox).hypot(y - oy) > self.slop;
            let confirmed = far || (out && self.out);
            self.out = out;
            if !confirmed {
                return None;
            }
            self.dragging = true;
        } else if let Some((lx, ly)) = self.last_pos {
            if (x - lx).hypot(y - ly) > MAX_STEP {
                return None;
            }
        }
        self.last_pos = Some((x, y));
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
            let (x, y) = self.last_pos.unwrap_or((x, y));
            return (x, y, true);
        }
        let (x, y) = self.origin.unwrap_or((x, y));
        tracing::debug!(x, y, stopper = self.stopper, "tap");
        (x, y, !self.stopper)
    }

    /// Where the finger landed (the touch point a tap clicks at).
    pub fn origin(&self) -> Option<(f32, f32)> {
        self.origin
    }

    /// Whether this touch became a drag.
    pub fn dragging(&self) -> bool {
        self.dragging
    }

    /// Move the touch point (touch targeting moved it onto a touch area).
    pub fn set_origin(&mut self, x: f32, y: f32) {
        self.origin = Some((x, y));
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
        let mut t = TapSlop::new(24.0);
        // A wobbling finger with a wild sample as it lifts: still a tap.
        assert_eq!(t.press(100.0, 200.0, at(0)), (100.0, 200.0, false));
        assert_eq!(t.moved(103.0, 202.0, at(10)), None);
        assert_eq!(t.moved(103.0, 201.0, at(20)), None);
        assert_eq!(t.moved(103.0, 230.0, at(30)), None);
        assert_eq!(t.release(103.0, 231.0, at(40)), (103.0, 201.0, true));
        // A flick: from the second sample out of the slop, everything passes.
        t.press(100.0, 300.0, at(1000));
        assert_eq!(t.moved(100.0, 298.0, at(1010)), None);
        assert_eq!(t.moved(100.0, 296.0, at(1020)), None);
        assert_eq!(t.moved(100.0, 270.0, at(1030)), None);
        assert_eq!(t.moved(100.0, 240.0, at(1040)), Some((100.0, 240.0)));
        assert_eq!(t.moved(100.0, 205.0, at(1050)), Some((100.0, 205.0)));
        // A glitch mid-drag is dropped.
        assert_eq!(t.moved(100.0, 20.0, at(1060)), None);
        assert!(t.release(100.0, 170.0, at(1065)).2);
        // The list glides: a press now only stops it, no click.
        assert!(t.press(120.0, 100.0, at(1150)).2);
        assert_eq!(t.release(120.0, 100.0, at(1200)), (120.0, 100.0, false));
        // Later, a tap is a tap again.
        assert!(!t.press(50.0, 50.0, at(9000)).2);
        assert!(t.near(60.0, 50.0, 40.0));
        assert!(!t.near(150.0, 50.0, 40.0));
        assert_eq!(t.release(55.0, 52.0, at(9050)), (50.0, 50.0, true));
    }

    /// Real fingers on the AD5M (10 minutes of taps and flicks, raw evdev):
    /// every touch that stays within 30 px is a tap, every one that travels
    /// more than 60 px scrolls and hands most of its way to the list.
    #[test]
    fn recorded_fingers() {
        use std::time::{Duration, Instant};
        let raw = include_bytes!("testdata/ad5m-finger.evdev");
        let base = Instant::now();
        let mut s = state(TouchTransform::default());
        let mut slop = TapSlop::new(24.0);
        let (mut taps, mut drags) = (0, 0);
        let mut start = (0.0, 0.0);
        let mut max_dev: f32 = 0.0;
        let mut dragged = false;
        let mut first_drag: Option<(f32, f32)> = None;
        let mut last_seen = (0.0, 0.0);
        for ev in raw.chunks_exact(16) {
            let sec = u32::from_le_bytes(ev[0..4].try_into().unwrap());
            let usec = u32::from_le_bytes(ev[4..8].try_into().unwrap());
            let kind = u16::from_le_bytes(ev[8..10].try_into().unwrap());
            let code = u16::from_le_bytes(ev[10..12].try_into().unwrap());
            let value = i32::from_le_bytes(ev[12..16].try_into().unwrap());
            let now = base + Duration::from_micros(u64::from(sec) * 1_000_000 + u64::from(usec));
            match s.process(kind, code, value) {
                Some(TouchAction::Pressed { x, y }) => {
                    slop.press(x, y, now);
                    start = (x, y);
                    max_dev = 0.0;
                    dragged = false;
                    first_drag = None;
                }
                Some(TouchAction::Moved { x, y }) => {
                    max_dev = max_dev.max((x - start.0).hypot(y - start.1));
                    last_seen = (x, y);
                    if let Some(p) = slop.moved(x, y, now) {
                        dragged = true;
                        first_drag.get_or_insert(p);
                    }
                }
                Some(TouchAction::Released { x, y }) => {
                    let (fx, fy) = last_seen;
                    let _ = (x, y);
                    slop.release(x, y, now);
                    if max_dev <= 30.0 {
                        taps += 1;
                        assert!(!dragged, "a tap ({max_dev:.0} px) became a scroll");
                    } else if max_dev > 60.0 {
                        drags += 1;
                        assert!(dragged, "a flick of {max_dev:.0} px did not scroll");
                        // The list gets the way from where the drag started.
                        let (dx, dy) = first_drag.unwrap();
                        let handed = (fx - dx).hypot(fy - dy) + (dx - start.0).hypot(dy - start.1);
                        assert!(
                            handed >= max_dev * 0.7,
                            "flick {max_dev:.0} px, list got {handed:.0}"
                        );
                    }
                }
                None => {}
            }
        }
        assert!(taps >= 30 && drags >= 30, "taps {taps}, drags {drags}");
    }
}
