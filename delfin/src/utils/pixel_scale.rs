//! Screen pixels per layout pixel, so images can be fetched as large as they
//! are drawn: on a 200 % screen a tile 175 px high covers 350 pixels, and a
//! 175-pixel image would be stretched (blurry).

use gtk::{gdk, prelude::*};
use relm4::gtk;

/// Largest scale of the connected monitors (1 … 3; fractional scales round
/// up, as GTK renders them). Main thread only.
pub fn pixel_scale() -> f64 {
    let Some(display) = gdk::Display::default() else {
        return 1.0;
    };
    let monitors = display.monitors();
    (0..monitors.n_items())
        .filter_map(|i| monitors.item(i).and_downcast::<gdk::Monitor>())
        .map(|monitor| f64::from(monitor.scale_factor()))
        .fold(1.0, f64::max)
        .min(3.0)
}

/// `size` layout pixels in screen pixels.
pub fn scaled(size: i32, scale: f64) -> i32 {
    (f64::from(size) * scale).ceil() as i32
}
