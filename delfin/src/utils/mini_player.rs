//! Mini player: the Delfin window is small, floating in a corner, and pinned
//! above everything. Wayland does not permit applications to do this directly;
//! under Hyprland it uses `hyprctl` (Lua dispatcher, Hyprland ≥ 0.55).

use std::{process::Command, sync::Mutex};

use serde_json::Value;
use tracing::{info, warn};

const WIDTH: i64 = 768;
const HEIGHT: i64 = 432;
const MARGIN: i64 = 20;

/// State before mini mode, for restoring.
struct Saved {
    address: String,
    was_floating: bool,
    at: (i64, i64),
    size: (i64, i64),
}

static SAVED: Mutex<Option<Saved>> = Mutex::new(None);

/// Is Delfin running under Hyprland?
pub fn available() -> bool {
    std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some()
}

pub fn active() -> bool {
    SAVED.lock().map(|s| s.is_some()).unwrap_or(false)
}

fn hyprctl_json(what: &str) -> Option<Value> {
    let output = Command::new("hyprctl").args(["-j", what]).output().ok()?;
    serde_json::from_slice(&output.stdout).ok()
}

fn dispatch(lua: &str) {
    match Command::new("hyprctl").args(["dispatch", lua]).output() {
        Ok(output) if String::from_utf8_lossy(&output.stdout).trim() == "ok" => {}
        Ok(output) => warn!(
            "hyprctl dispatch {lua}: {}",
            String::from_utf8_lossy(&output.stdout).trim()
        ),
        Err(err) => warn!("hyprctl not runnable: {err}"),
    }
}

fn pair(value: &Value) -> (i64, i64) {
    (
        value[0].as_i64().unwrap_or_default(),
        value[1].as_i64().unwrap_or_default(),
    )
}

/// Find the application's window (by process ID) and monitor.
fn own_window() -> Option<Value> {
    let pid = std::process::id() as i64;
    hyprctl_json("clients")?
        .as_array()?
        .iter()
        .find(|client| client["pid"].as_i64() == Some(pid))
        .cloned()
}

/// Enter mini mode: floating, small, bottom right, pinned.
pub fn enter() {
    if active() || !available() {
        return;
    }
    let Some(window) = own_window() else {
        warn!("Mini player: own window not found");
        return;
    };
    let address = window["address"].as_str().unwrap_or_default().to_string();
    let monitor_id = window["monitor"].as_i64();
    let Some(monitor) = hyprctl_json("monitors").and_then(|monitors| {
        monitors
            .as_array()?
            .iter()
            .find(|m| m["id"].as_i64() == monitor_id)
            .cloned()
    }) else {
        warn!("Mini player: monitor not found");
        return;
    };

    // Coordinates are logical (pixels / scale).
    let scale = monitor["scale"].as_f64().unwrap_or(1.0).max(0.1);
    let width = (monitor["width"].as_f64().unwrap_or(1920.0) / scale) as i64;
    let height = (monitor["height"].as_f64().unwrap_or(1080.0) / scale) as i64;
    let (mx, my) = (
        monitor["x"].as_i64().unwrap_or_default(),
        monitor["y"].as_i64().unwrap_or_default(),
    );
    // reserved = [left, top, right, bottom] (bars, dock)
    let reserved_right = monitor["reserved"][2].as_i64().unwrap_or_default();
    let reserved_bottom = monitor["reserved"][3].as_i64().unwrap_or_default();
    let x = mx + width - reserved_right - WIDTH - MARGIN;
    let y = my + height - reserved_bottom - HEIGHT - MARGIN;

    *SAVED.lock().unwrap() = Some(Saved {
        address: address.clone(),
        was_floating: window["floating"].as_bool().unwrap_or(false),
        at: pair(&window["at"]),
        size: pair(&window["size"]),
    });

    let target = format!("address:{address}");
    info!("Mini player on ({x},{y} {WIDTH}x{HEIGHT})");
    dispatch(&format!(
        r#"hl.dsp.window.float({{ action = "on", window = "{target}" }})"#
    ));
    dispatch(&format!(
        r#"hl.dsp.window.resize({{ x = {WIDTH}, y = {HEIGHT}, window = "{target}" }})"#
    ));
    dispatch(&format!(
        r#"hl.dsp.window.move({{ x = {x}, y = {y}, window = "{target}" }})"#
    ));
    dispatch(&format!(
        r#"hl.dsp.window.pin({{ action = "on", window = "{target}" }})"#
    ));
}

/// Restore the previous size and position.
pub fn leave() {
    let Some(saved) = SAVED.lock().ok().and_then(|mut s| s.take()) else {
        return;
    };
    let target = format!("address:{}", saved.address);
    info!("Mini player off");
    dispatch(&format!(
        r#"hl.dsp.window.pin({{ action = "off", window = "{target}" }})"#
    ));
    if saved.was_floating {
        dispatch(&format!(
            r#"hl.dsp.window.resize({{ x = {}, y = {}, window = "{target}" }})"#,
            saved.size.0, saved.size.1
        ));
        dispatch(&format!(
            r#"hl.dsp.window.move({{ x = {}, y = {}, window = "{target}" }})"#,
            saved.at.0, saved.at.1
        ));
    } else {
        dispatch(&format!(
            r#"hl.dsp.window.float({{ action = "off", window = "{target}" }})"#
        ));
    }
}

pub fn toggle() {
    if active() {
        leave();
    } else {
        enter();
    }
}
