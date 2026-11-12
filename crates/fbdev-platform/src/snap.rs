//! Touch targeting, as Windows does it for fingers: a touch is a patch of
//! skin, not a point. When the point lands on no touch area at all (between two
//! buttons, a few pixels off a small one), it moves to the nearest touch area
//! within reach — what the finger was most likely after. A point that is on a
//! touch area stays exactly where it is.
//!
//! The window's items are walked for enabled `TouchArea`s; only the part of
//! each that is on the glass counts (what a scrolled list clips away is not a
//! target).

use std::ops::ControlFlow;

#[cfg(test)]
use i_slint_core::lengths::{LogicalPoint, LogicalSize};
use i_slint_core::{
    item_tree::{ItemRc, ParentItemTraversalMode},
    items::{Flickable, TouchArea},
    lengths::LogicalRect,
    window::WindowInner,
};

/// The item's rectangle in window coordinates.
fn window_rect(item: &ItemRc) -> LogicalRect {
    let g = item.geometry();
    LogicalRect::new(item.map_to_window(g.origin), g.size)
}

/// The part of `item` its clipping ancestors (lists, clipped rectangles) leave visible.
fn visible_rect(item: &ItemRc) -> Option<LogicalRect> {
    let mut rect = window_rect(item);
    let mut cur = item.parent_item(ParentItemTraversalMode::StopAtPopups);
    while let Some(anc) = cur {
        if anc.borrow().as_ref().clips_children() {
            rect = rect.intersection(&window_rect(&anc))?;
        }
        cur = anc.parent_item(ParentItemTraversalMode::StopAtPopups);
    }
    (rect.size.width > 0.0 && rect.size.height > 0.0).then_some(rect)
}

/// Every enabled, visible touch area of the window, as its visible rectangle.
fn targets(window: &slint::Window) -> Vec<LogicalRect> {
    let mut out = Vec::new();
    let Some(root) = WindowInner::from_pub(window).window_item_rc() else {
        return out;
    };
    root.visit_descendants(|item| {
        if let Some(ta) = item.downcast::<TouchArea>() {
            if ta.as_pin_ref().enabled() && item.is_visible() {
                if let Some(r) = visible_rect(item) {
                    out.push(r);
                }
            }
        }
        ControlFlow::<()>::Continue(())
    });
    out
}

/// Whether a finger pressed at (x, y) can drag anything: a list that scrolls
/// under it, or a touch area that follows the finger (a dial, a slider). If not,
/// the touch is a tap however the finger wobbles, as in LVGL, where a press only
/// stops being a click once something has scrolled.
// Used by the Linux event loop only.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn can_drag(window: &slint::Window, x: f32, y: f32) -> bool {
    let Some(root) = WindowInner::from_pub(window).window_item_rc() else {
        return true;
    };
    let mut drag = false;
    root.visit_descendants(|item| {
        let under = || visible_rect(item).is_some_and(|r| distance(&r, x, y) == 0.0);
        if let Some(ta) = item.downcast::<TouchArea>() {
            let ta = ta.as_pin_ref();
            if ta.enabled()
                && (ta.moved.has_handler() || ta.pointer_event.has_handler())
                && item.is_visible()
                && under()
            {
                drag = true;
            }
        } else if let Some(f) = item.downcast::<Flickable>() {
            let f = f.as_pin_ref();
            let g = item.geometry();
            let scrolls = f.content_height().get() > g.height() + 0.5
                || f.content_width().get() > g.width() + 0.5;
            if f.interactive() && scrolls && item.is_visible() && under() {
                drag = true;
            }
        }
        if drag {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    });
    drag
}

fn distance(r: &LogicalRect, x: f32, y: f32) -> f32 {
    let dx = (r.min_x() - x).max(0.0).max(x - r.max_x());
    let dy = (r.min_y() - y).max(0.0).max(y - r.max_y());
    dx.hypot(dy)
}

/// Where a touch at (x, y) should land: None when it is on a touch area already
/// or none is within `reach`; else a point just inside the nearest one.
// Used by the Linux event loop only.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn snap(window: &slint::Window, x: f32, y: f32, reach: f32) -> Option<(f32, f32)> {
    nearest(&targets(window), x, y, reach)
}

fn nearest(targets: &[LogicalRect], x: f32, y: f32, reach: f32) -> Option<(f32, f32)> {
    if targets.iter().any(|r| distance(r, x, y) == 0.0) {
        return None;
    }
    let best = targets
        .iter()
        .map(|r| (distance(r, x, y), r))
        .filter(|(d, _)| *d <= reach)
        .min_by(|a, b| a.0.total_cmp(&b.0))?
        .1;
    // A little inside the edge, so the press is surely within it.
    let inset = |lo: f32, hi: f32| (4.0f32).min((hi - lo) / 2.0);
    let (ix, iy) = (
        inset(best.min_x(), best.max_x()),
        inset(best.min_y(), best.max_y()),
    );
    Some((
        x.clamp(best.min_x() + ix, best.max_x() - ix),
        y.clamp(best.min_y() + iy, best.max_y() - iy),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> LogicalRect {
        LogicalRect::new(LogicalPoint::new(x, y), LogicalSize::new(w, h))
    }

    #[test]
    fn a_miss_goes_to_the_nearest_button_within_reach() {
        // Back at 104–168 × 16–80, a tile further right.
        let t = [
            rect(104.0, 16.0, 64.0, 64.0),
            rect(300.0, 16.0, 120.0, 64.0),
        ];
        // On Back: stays.
        assert_eq!(nearest(&t, 150.0, 40.0, 24.0), None);
        // 15 px right of Back: onto its right edge, a little inside.
        assert_eq!(nearest(&t, 183.0, 55.0, 24.0), Some((164.0, 55.0)));
        // Between the two, nearer the tile.
        assert_eq!(nearest(&t, 290.0, 40.0, 24.0), Some((304.0, 40.0)));
        // Far from both: nothing.
        assert_eq!(nearest(&t, 230.0, 200.0, 24.0), None);
    }
}
