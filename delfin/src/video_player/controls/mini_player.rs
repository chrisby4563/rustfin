//! Knopf für den Mini-Player (nur unter Hyprland sichtbar).

use gtk::prelude::*;
use relm4::prelude::*;

use crate::{
    tr,
    utils::{main_window::get_main_window, mini_player},
};

pub(crate) struct MiniPlayerButton {
    active: bool,
}

#[relm4::component(pub(crate))]
impl SimpleComponent for MiniPlayerButton {
    type Init = ();
    type Input = ();
    type Output = ();

    view! {
        gtk::Button {
            set_focus_on_click: false,
            set_visible: mini_player::available(),
            #[watch]
            set_icon_name: if model.active { "window-restore-symbolic" } else { "view-pin-symbolic" },
            #[watch]
            set_tooltip_text: Some(&if model.active {
                tr!("vp-mini-player-leave").to_string()
            } else {
                tr!("vp-mini-player-enter").to_string()
            }),
            connect_clicked => (),
        }
    }

    fn init(_: Self::Init, root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = MiniPlayerButton {
            active: mini_player::active(),
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, _: Self::Input, _sender: ComponentSender<Self>) {
        // Aus dem Vollbild zuerst heraus, sonst greift Schweben nicht.
        if !mini_player::active() {
            if let Some(window) = get_main_window() {
                if window.is_fullscreen() {
                    window.unfullscreen();
                }
            }
        }
        mini_player::toggle();
        self.active = mini_player::active();
    }
}
