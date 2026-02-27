use windows::Win32::Foundation::{POINT, RECT};

use crate::window_manager;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SnapZone {
    /// Left half of monitor
    Left,
    /// Right half of monitor
    Right,
    /// Maximize (full work area)
    Maximize,
    /// Top-left quarter
    TopLeft,
    /// Top-right quarter
    TopRight,
    /// Bottom-left quarter
    BottomLeft,
    /// Bottom-right quarter
    BottomRight,
}

/// Detect if the cursor is in a snap zone (near a monitor edge).
///
/// Returns the detected zone and the destination rect the window should snap to.
/// The `threshold` is the number of pixels from the edge to trigger detection.
pub fn detect_snap_zone(cursor: POINT, threshold: i32) -> Option<(SnapZone, RECT)> {
    let work = window_manager::get_monitor_work_area(cursor)?;

    let near_left = cursor.x - work.left < threshold;
    let near_right = work.right - cursor.x < threshold;
    let near_top = cursor.y - work.top < threshold;
    let near_bottom = work.bottom - cursor.y < threshold;

    // Priority: corners > edges > maximize (top)
    let zone = if near_top && near_left {
        Some(SnapZone::TopLeft)
    } else if near_top && near_right {
        Some(SnapZone::TopRight)
    } else if near_bottom && near_left {
        Some(SnapZone::BottomLeft)
    } else if near_bottom && near_right {
        Some(SnapZone::BottomRight)
    } else if near_top {
        Some(SnapZone::Maximize)
    } else if near_left {
        Some(SnapZone::Left)
    } else if near_right {
        Some(SnapZone::Right)
    } else {
        None
    };

    zone.map(|z| {
        let rect = snap_zone_rect(z, work);
        (z, rect)
    })
}

/// Compute the destination rect for a snap zone within the given work area.
fn snap_zone_rect(zone: SnapZone, work: RECT) -> RECT {
    let w = work.right - work.left;
    let h = work.bottom - work.top;
    let half_w = w / 2;
    let half_h = h / 2;

    match zone {
        SnapZone::Left => RECT {
            left: work.left,
            top: work.top,
            right: work.left + half_w,
            bottom: work.bottom,
        },
        SnapZone::Right => RECT {
            left: work.left + half_w,
            top: work.top,
            right: work.right,
            bottom: work.bottom,
        },
        SnapZone::Maximize => work,
        SnapZone::TopLeft => RECT {
            left: work.left,
            top: work.top,
            right: work.left + half_w,
            bottom: work.top + half_h,
        },
        SnapZone::TopRight => RECT {
            left: work.left + half_w,
            top: work.top,
            right: work.right,
            bottom: work.top + half_h,
        },
        SnapZone::BottomLeft => RECT {
            left: work.left,
            top: work.top + half_h,
            right: work.left + half_w,
            bottom: work.bottom,
        },
        SnapZone::BottomRight => RECT {
            left: work.left + half_w,
            top: work.top + half_h,
            right: work.right,
            bottom: work.bottom,
        },
    }
}
