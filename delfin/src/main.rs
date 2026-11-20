use std::{fmt::Debug, path::PathBuf};

use anyhow::{Context, Result};
use delfin::{
    app::{App, APP_BROKER},
    meson_config::{APP_ID, BUILDDIR, RESOURCES_FILE},
};
use gtk::{gdk, gio, glib, prelude::*};
use relm4::{gtk, RelmApp};
use tracing::log::LevelFilter;

fn main() {
    // Log lines never contain access tokens (stream URLs carry `ApiKey=`).
    env_logger::builder()
        .filter_level(LevelFilter::Warn)
        .parse_default_env()
        .format(|buf, record| {
            use std::io::Write;
            writeln!(
                buf,
                "[{} {:<5} {}] {}",
                buf.timestamp(),
                record.level(),
                record.target(),
                delfin::utils::redact::redact(&record.args().to_string())
            )
        })
        .init();

    #[cfg(feature = "gst")]
    {
        video_player_gst::init_gst();
    }

    load_resources().expect("Error loading resources");

    let app = RelmApp::new(APP_ID);
    relm4_icons::initialize_icons();
    load_css(&app);
    follow_desktop_theme();
    app.with_broker(&APP_BROKER).run::<App>(());
    delfin::syncplay::leave_on_exit();
}

fn load_css<T: Debug>(app: &RelmApp<T>) {
    let css = include_str!(concat!(env!("OUT_DIR"), "/app.css"));
    app.set_global_css(css);
}

/// Omarchy rewrites `~/.config/gtk-4.0/omarchy-theme.css` when changing themes.
/// GTK reads it (included via `gtk.css`) only at startup, so load it here and
/// reload it on every change so the colors update immediately.
/// mitwechseln.
fn follow_desktop_theme() {
    let path = glib::user_config_dir().join("gtk-4.0/omarchy-theme.css");
    let Some(display) = gdk::Display::default() else {
        return;
    };

    let provider = gtk::CssProvider::new();
    if path.exists() {
        provider.load_from_path(&path);
    }
    #[allow(deprecated)]
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_USER + 1,
    );

    let Ok(monitor) = gio::File::for_path(&path)
        .monitor_file(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
    else {
        return;
    };
    monitor.connect_changed(move |_, _, _, event| {
        if matches!(
            event,
            gio::FileMonitorEvent::ChangesDoneHint
                | gio::FileMonitorEvent::Created
                | gio::FileMonitorEvent::MovedIn
                | gio::FileMonitorEvent::Renamed
        ) && path.exists()
        {
            provider.load_from_path(&path);
        }
    });
    // Monitoring runs for the lifetime of the program.
    std::mem::forget(monitor);
}

fn load_resources() -> Result<()> {
    let res = match gio::Resource::load(RESOURCES_FILE) {
        Ok(res) => res,
        // Not installed (test build directly from the project directory): the
        // Ressourcen aus dem Bauordner nehmen.
        Err(_) => gio::Resource::load(PathBuf::from(BUILDDIR).join("data/resources.gresource"))
            .context("Could not load gresource file")?,
    };

    gio::resources_register(&res);
    Ok(())
}
