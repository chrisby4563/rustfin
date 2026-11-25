// No console window next to the app in Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::{fmt::Debug, path::PathBuf};

use anyhow::{Context, Result};
use rustfin::{
    app::{App, APP_BROKER},
    meson_config::{APP_ID, BUILDDIR, RESOURCES_FILE},
};
use gtk::{gdk, gio, glib, prelude::*};
use relm4::{gtk, RelmApp};
use tracing::log::LevelFilter;

fn main() {
    // Before anything else (the log file is recreated on every start).
    #[cfg(windows)]
    if !rustfin::utils::win32::claim_single_instance(APP_ID) {
        return;
    }
    // Windows has no session bus; GLib would start its own (`gdbus.exe`) that
    // keeps running after Rustfin is closed. Nothing here needs it.
    #[cfg(windows)]
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        std::env::set_var("DBUS_SESSION_BUS_ADDRESS", "disabled:");
    }

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
                rustfin::utils::redact::redact(&record.args().to_string())
            )
        })
        .target(log_target())
        .init();

    #[cfg(feature = "gst")]
    {
        video_player_gst::init_gst();
    }

    load_resources().expect("Error loading resources");

    let app = RelmApp::new(APP_ID);
    // Uniqueness is handled by `claim_single_instance` (no session bus).
    #[cfg(windows)]
    relm4::main_application().set_flags(gio::ApplicationFlags::NON_UNIQUE);
    relm4_icons::initialize_icons();
    load_css(&app);
    follow_desktop_theme();
    #[cfg(windows)]
    rustfin::utils::win32::center_dialogs_on_parent();
    app.with_broker(&APP_BROKER).run::<App>(());
    rustfin::syncplay::leave_on_exit();
}

/// A Windows GUI program started from Explorer or the Start menu has no
/// stderr; log into `%LOCALAPPDATA%/rustfin/rustfin.log` instead
/// (overwritten on every start). Redirected stderr is still honoured.
#[cfg(windows)]
fn log_target() -> env_logger::Target {
    extern "system" {
        fn GetStdHandle(std_handle: u32) -> *mut std::ffi::c_void;
    }
    const STD_ERROR_HANDLE: u32 = -12i32 as u32;

    let handle = unsafe { GetStdHandle(STD_ERROR_HANDLE) };
    if !handle.is_null() && handle as isize != -1 {
        return env_logger::Target::Stderr;
    }

    let file = std::env::var_os("LOCALAPPDATA")
        .map(|local| PathBuf::from(local).join("rustfin"))
        .and_then(|dir| {
            std::fs::create_dir_all(&dir).ok()?;
            std::fs::File::create(dir.join("rustfin.log")).ok()
        });
    match file {
        Some(file) => env_logger::Target::Pipe(Box::new(file)),
        None => env_logger::Target::Stderr,
    }
}

#[cfg(not(windows))]
fn log_target() -> env_logger::Target {
    env_logger::Target::Stderr
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

/// On Windows the installation can live anywhere, so the resources are looked
/// up next to the executable (`<prefix>/bin/rustfin.exe` →
/// `<prefix>/share/rustfin/resources.gresource`).
#[cfg(windows)]
fn resources_file() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent()?.parent().map(PathBuf::from))
        .map(|prefix| prefix.join("share").join("rustfin").join("resources.gresource"))
        .unwrap_or_else(|| PathBuf::from(RESOURCES_FILE))
}

#[cfg(not(windows))]
fn resources_file() -> PathBuf {
    PathBuf::from(RESOURCES_FILE)
}

fn load_resources() -> Result<()> {
    let res = match gio::Resource::load(resources_file()) {
        Ok(res) => res,
        // Not installed (test build directly from the project directory): the
        // resources come from the build directory.
        Err(_) => gio::Resource::load(PathBuf::from(BUILDDIR).join("data/resources.gresource"))
            .context("Could not load gresource file")?,
    };

    gio::resources_register(&res);
    Ok(())
}
