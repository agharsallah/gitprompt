//! Where the recall window opens: horizontally centred near the top of the screen with the
//! pointer, like a launcher. Pure geometry in physical pixels.

/// A rectangle on the desktop: its top-left corner and its size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Area {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// How far down the screen the window's top edge sits, in percent of the screen's height.
const TOP_PERCENT: i64 = 18;

/// The top-left corner for a `window`-sized (width, height) window on `screen`: centred across,
/// `TOP_PERCENT` down, and never past the screen's edges (a window larger than the screen sits at
/// its top-left corner).
pub(crate) fn top_center(screen: Area, window: (u32, u32)) -> (i32, i32) {
    let spare = |screen: u32, window: u32| i64::from(screen) - i64::from(window);
    let x = i64::from(screen.x) + spare(screen.width, window.0).max(0) / 2;
    let down = i64::from(screen.height) * TOP_PERCENT / 100;
    let y = i64::from(screen.y) + down.min(spare(screen.height, window.1).max(0));
    (clamp(x), clamp(y))
}

/// Screen coordinates fit `i32` on every platform; a value outside it can only be garbage.
fn clamp(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(if value < 0 { i32::MIN } else { i32::MAX })
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn screen(x: i32, y: i32, width: u32, height: u32) -> Area {
        Area { x, y, width, height }
    }

    #[test]
    fn the_window_is_centred_across_and_a_fifth_down() {
        assert_eq!(top_center(screen(0, 0, 2000, 1000), (800, 400)), (600, 180));
    }

    #[test]
    fn a_second_screen_is_placed_by_its_own_origin() {
        assert_eq!(top_center(screen(2000, 0, 2000, 1000), (800, 400)), (2600, 180));
        assert_eq!(top_center(screen(-1920, -200, 1920, 1080), (960, 480)), (-1440, -200 + 194));
    }

    #[test]
    fn the_window_stays_on_a_screen_too_short_for_its_offset() {
        // 18 % of the height is 90 and 100 pixels are spare: it fits. At 450 only 50 are spare.
        assert_eq!(top_center(screen(0, 0, 1000, 500), (800, 400)), (100, 90));
        assert_eq!(top_center(screen(0, 0, 1000, 450), (800, 400)), (100, 50));
    }

    #[test]
    fn a_window_larger_than_the_screen_sits_at_its_corner() {
        assert_eq!(top_center(screen(10, 20, 600, 300), (800, 400)), (10, 20));
    }
}
