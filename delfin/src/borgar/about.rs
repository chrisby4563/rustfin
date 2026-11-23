use gtk::prelude::*;
use relm4::{prelude::*, ComponentParts, SimpleComponent};

use crate::meson_config::{APP_ID, VERSION};

pub(crate) struct About;

#[relm4::component(pub(crate))]
impl SimpleComponent for About {
    type Init = ();
    type Input = ();
    type Output = ();

    view! {
        about_window() -> adw::AboutWindow {
            set_modal: true,
            set_visible: true,
            set_version: VERSION,
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: relm4::ComponentSender<Self>,
    ) -> relm4::ComponentParts<Self> {
        let widgets = view_output!();
        let model = About;
        ComponentParts { model, widgets }
    }
}

#[cfg(target_os = "linux")]
fn about_window() -> adw::AboutWindow {
    adw::AboutWindow::from_appdata(
        &format!("/io/github/chrisby4563/Rustfin/{}.metainfo.xml", APP_ID),
        Some(VERSION),
    )
}

/// libadwaita can only read AppStream metadata on Linux.
#[cfg(not(target_os = "linux"))]
fn about_window() -> adw::AboutWindow {
    adw::AboutWindow::builder()
        .application_name("Rustfin")
        .application_icon(APP_ID)
        .developer_name("chrisby4563")
        .website("https://github.com/chrisby4563/rustfin")
        .issue_url("https://github.com/chrisby4563/rustfin/issues")
        .license_type(gtk::License::Gpl30Only)
        .build()
}
