//! Bilder weich einblenden, sobald sie geladen sind (statt aufzuploppen).

use adw::prelude::*;
use relm4::{adw, gtk};

/// Blendet `picture` jedes Mal ein, wenn es ein neues Bild bekommt.
/// Bis dahin pulsiert die Fläche dahinter als Platzhalter (`.skeleton`).
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

/// Widget in 300 ms von unsichtbar auf sichtbar.
pub fn fade_in(widget: &gtk::Widget) {
    let target = adw::CallbackAnimationTarget::new({
        let widget = widget.clone();
        move |value| widget.set_opacity(value)
    });
    adw::TimedAnimation::new(widget, 0.0, 1.0, 300, target).play();
}
