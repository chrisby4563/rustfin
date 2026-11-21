//! Fade images in softly once they are loaded (instead of popping in).

use adw::prelude::*;
use relm4::{adw, gtk};

/// Fades in `picture` whenever it receives a new image.
/// Until then, the area behind it pulses as a placeholder (`.skeleton`).
pub fn fade_in_on_load(picture: &gtk::Picture) {
    let set_skeleton = |picture: &gtk::Picture, on: bool| {
        if let Some(parent) = picture.parent() {
            if on {
                parent.add_css_class("skeleton");
            } else {
                parent.remove_css_class("skeleton");
            }
        }
    };
    if picture.paintable().is_none() {
        picture.set_opacity(0.0);
        set_skeleton(picture, true);
    }
    picture.connect_paintable_notify(move |picture| {
        if picture.paintable().is_none() {
            picture.set_opacity(0.0);
            set_skeleton(picture, true);
            return;
        }
        set_skeleton(picture, false);
        fade_in(picture.upcast_ref());
    });
}

/// Fade a widget from invisible to visible in 300 ms.
pub fn fade_in(widget: &gtk::Widget) {
    let target = adw::CallbackAnimationTarget::new({
        let widget = widget.clone();
        move |value| widget.set_opacity(value)
    });
    adw::TimedAnimation::new(widget, 0.0, 1.0, 300, target).play();
}
