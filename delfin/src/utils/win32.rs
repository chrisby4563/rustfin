//! Small Windows-only helpers around native window handles.

use std::ffi::c_void;

use relm4::gtk::{self, gdk, glib, glib::translate::ToGlibPtr, prelude::*};

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

extern "C" {
    fn gdk_win32_surface_get_handle(surface: *mut gdk::ffi::GdkSurface) -> *mut c_void;
}

#[repr(C)]
struct MonitorInfo {
    size: u32,
    monitor: Rect,
    work: Rect,
    flags: u32,
}

extern "system" {
    fn MonitorFromWindow(hwnd: *mut c_void, flags: u32) -> *mut c_void;
    fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
    fn GetWindowRect(hwnd: *mut c_void, rect: *mut Rect) -> i32;
    fn SetWindowPos(
        hwnd: *mut c_void,
        insert_after: *mut c_void,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
}

const MONITOR_DEFAULTTONEAREST: u32 = 2;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;

/// Native handle (HWND) of a realized GTK window.
pub fn hwnd(window: &impl IsA<gtk::Window>) -> Option<*mut c_void> {
    let surface = window.as_ref().surface()?;
    let handle = unsafe { gdk_win32_surface_get_handle(surface.to_glib_none().0) };
    (!handle.is_null()).then_some(handle)
}

fn window_rect(hwnd: *mut c_void) -> Option<Rect> {
    let mut rect = Rect::default();
    (unsafe { GetWindowRect(hwnd, &mut rect) } != 0).then_some(rect)
}

/// GTK on Windows places every new window at the top left of the screen.
/// Center dialogs on their parent and other windows on their monitor's work
/// area once they are first shown.
pub fn center_dialogs_on_parent() {
    let toplevels = gtk::Window::toplevels();
    toplevels.connect_items_changed(|list, position, _removed, added| {
        for i in position..position + added {
            let Some(window) = list.item(i).and_downcast::<gtk::Window>() else {
                continue;
            };
            let handler = std::rc::Rc::new(std::cell::Cell::new(None));
            let id = window.connect_map({
                let handler = handler.clone();
                move |window| {
                    // Only the first time; later the user's placement wins.
                    if let Some(id) = handler.take() {
                        window.disconnect(id);
                    }
                    let window = window.clone();
                    glib::idle_add_local_once(move || center(&window));
                }
            });
            handler.set(Some(id));
        }
    });
}

fn center(window: &gtk::Window) {
    let Some(own) = hwnd(window) else {
        return;
    };
    let Some(own_rect) = window_rect(own) else {
        return;
    };
    tracing::debug!(
        "Window {:?}: {}x{} logical, scale factor {}, surface scale {:?}, {}x{} physical",
        window.title(),
        window.width(),
        window.height(),
        window.scale_factor(),
        window.surface().map(|s| s.scale()),
        own_rect.right - own_rect.left,
        own_rect.bottom - own_rect.top,
    );

    let area = match window.transient_for().and_then(|parent| hwnd(&parent)) {
        Some(parent) => window_rect(parent),
        None => {
            let monitor = unsafe { MonitorFromWindow(own, MONITOR_DEFAULTTONEAREST) };
            let mut info = MonitorInfo {
                size: std::mem::size_of::<MonitorInfo>() as u32,
                monitor: Rect::default(),
                work: Rect::default(),
                flags: 0,
            };
            (unsafe { GetMonitorInfoW(monitor, &mut info) } != 0).then_some(info.work)
        }
    };
    let Some(area) = area else {
        return;
    };

    let width = own_rect.right - own_rect.left;
    let height = own_rect.bottom - own_rect.top;
    let x = area.left + (area.right - area.left - width) / 2;
    let y = area.top + (area.bottom - area.top - height) / 2;

    unsafe {
        SetWindowPos(
            own,
            std::ptr::null_mut(),
            x.max(area.left),
            y.max(area.top),
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

// ---------------------------------------------------------------------------
// Single instance

extern "system" {
    fn CreateMutexW(attributes: *mut c_void, initial_owner: i32, name: *const u16) -> *mut c_void;
    fn GetLastError() -> u32;
    fn EnumWindows(callback: extern "system" fn(*mut c_void, isize) -> i32, param: isize) -> i32;
    fn GetWindowThreadProcessId(hwnd: *mut c_void, pid: *mut u32) -> u32;
    fn IsWindowVisible(hwnd: *mut c_void) -> i32;
    fn IsIconic(hwnd: *mut c_void) -> i32;
    fn GetWindowTextLengthW(hwnd: *mut c_void) -> i32;
    fn ShowWindow(hwnd: *mut c_void, cmd: i32) -> i32;
    fn SetForegroundWindow(hwnd: *mut c_void) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn QueryFullProcessImageNameW(process: *mut c_void, flags: u32, name: *mut u16, size: *mut u32)
        -> i32;
}

const ERROR_ALREADY_EXISTS: u32 = 183;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const SW_RESTORE: i32 = 9;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn process_image(pid: u32) -> Option<std::path::PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return None;
    }
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(process, 0, buf.as_mut_ptr(), &mut len) };
    unsafe { CloseHandle(process) };
    (ok != 0).then(|| std::ffi::OsString::from_wide(&buf[..len as usize]).into())
}

extern "system" fn find_app_window(hwnd: *mut c_void, param: isize) -> i32 {
    let found = unsafe { &mut *(param as *mut Option<*mut c_void>) };
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    if pid == std::process::id()
        || unsafe { IsWindowVisible(hwnd) } == 0
        || unsafe { GetWindowTextLengthW(hwnd) } == 0
    {
        return 1;
    }
    let same_exe = match (process_image(pid), std::env::current_exe()) {
        (Some(other), Ok(own)) => other
            .to_string_lossy()
            .eq_ignore_ascii_case(&own.to_string_lossy()),
        _ => false,
    };
    if same_exe {
        *found = Some(hwnd);
        return 0;
    }
    1
}

/// Windows has no session bus, so GApplication cannot detect a running
/// instance (a second start waited forever for an answer and lingered as an
/// invisible process). A named mutex does it instead.
///
/// Returns `false` if Rustfin is already running; its window is then brought
/// to the front and this process should exit.
pub fn claim_single_instance(app_id: &str) -> bool {
    let name = wide(&format!("Local\\{app_id}"));
    // The handle is kept open for the lifetime of the process.
    let mutex = unsafe { CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr()) };
    if mutex.is_null() || unsafe { GetLastError() } != ERROR_ALREADY_EXISTS {
        return true;
    }

    // The other instance may still be starting: wait a moment for its window.
    for _ in 0..25 {
        let mut found: Option<*mut c_void> = None;
        unsafe { EnumWindows(find_app_window, &mut found as *mut _ as isize) };
        if let Some(hwnd) = found {
            unsafe {
                if IsIconic(hwnd) != 0 {
                    ShowWindow(hwnd, SW_RESTORE);
                }
                SetForegroundWindow(hwnd);
            }
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    false
}

// ---------------------------------------------------------------------------
// Mini player

pub mod mini_player {
    use std::sync::Mutex;

    use relm4::gtk::prelude::*;

    use super::{
        hwnd, window_rect, GetMonitorInfoW, IsIconic, MonitorFromWindow, MonitorInfo, Rect,
        SetWindowPos, ShowWindow, MONITOR_DEFAULTTONEAREST,
    };
    use crate::utils::{
        main_window::get_main_window,
        mini_player::{HEIGHT, MARGIN, WIDTH},
    };

    extern "system" {
        fn GetDpiForWindow(hwnd: *mut std::ffi::c_void) -> u32;
        fn IsZoomed(hwnd: *mut std::ffi::c_void) -> i32;
    }

    const HWND_TOPMOST: isize = -1;
    const HWND_NOTOPMOST: isize = -2;
    const SW_RESTORE: i32 = 9;
    const SW_MAXIMIZE: i32 = 3;

    /// Placement before mini mode, for restoring.
    struct Saved {
        rect: Rect,
        maximized: bool,
    }

    // Rect only holds integers; the window handle is looked up again.
    unsafe impl Send for Saved {}

    static SAVED: Mutex<Option<Saved>> = Mutex::new(None);

    pub fn available() -> bool {
        true
    }

    pub fn active() -> bool {
        SAVED.lock().map(|s| s.is_some()).unwrap_or(false)
    }

    /// Small, bottom right of the work area (above the taskbar), always on top.
    pub fn enter() {
        if active() {
            return;
        }
        let Some(hwnd) = get_main_window().and_then(|window| hwnd(&window)) else {
            return;
        };

        let maximized = unsafe { IsZoomed(hwnd) } != 0;
        if maximized || unsafe { IsIconic(hwnd) } != 0 {
            unsafe { ShowWindow(hwnd, SW_RESTORE) };
        }
        let Some(rect) = window_rect(hwnd) else {
            return;
        };

        let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
        let mut info = MonitorInfo {
            size: std::mem::size_of::<MonitorInfo>() as u32,
            monitor: Rect::default(),
            work: Rect::default(),
            flags: 0,
        };
        if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
            return;
        }

        let scale = f64::from(unsafe { GetDpiForWindow(hwnd) }.max(96)) / 96.0;
        let px = |logical: i64| (logical as f64 * scale).round() as i32;
        let (width, height, margin) = (px(WIDTH), px(HEIGHT), px(MARGIN));
        let x = info.work.right - width - margin;
        let y = info.work.bottom - height - margin;

        *SAVED.lock().unwrap() = Some(Saved { rect, maximized });
        tracing::info!("Mini player on ({x},{y} {width}x{height})");
        unsafe {
            SetWindowPos(hwnd, HWND_TOPMOST as _, x, y, width, height, 0);
        }
    }

    /// Restore the previous size and position.
    pub fn leave() {
        let Some(saved) = SAVED.lock().ok().and_then(|mut s| s.take()) else {
            return;
        };
        let Some(hwnd) = get_main_window().and_then(|window| hwnd(&window)) else {
            return;
        };
        tracing::info!("Mini player off");
        let r = saved.rect;
        unsafe {
            SetWindowPos(
                hwnd,
                HWND_NOTOPMOST as _,
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                0,
            );
            if saved.maximized {
                ShowWindow(hwnd, SW_MAXIMIZE);
            }
        }
    }
}

/// GTK on Windows renders with an integer scale and makes up the rest of the
/// system scaling with the text DPI (stock GTK: scale 1 at 125–175 %, text
/// enlarged; Rustfin's patched GTK: scale 2 from 150 %, text reduced). Window
/// sizes stored in logical pixels are multiplied by this factor so that the
/// window has the same room for its text as on Linux.
pub fn text_scale() -> f64 {
    // With the patched GTK (scale 2 from 150 %) the logical size already
    // matches Linux; the text DPI is set by `apply_linux_look`.
    if monitor_scale() >= 2 {
        return 1.0;
    }
    let dpi = gtk::Settings::default()
        .map(|settings| settings.gtk_xft_dpi())
        .unwrap_or(-1);
    if dpi <= 0 {
        return 1.0;
    }
    (f64::from(dpi) / 1024.0 / 96.0).clamp(1.0, 4.0)
}

/// GTK's integer scale of the primary monitor.
fn monitor_scale() -> i32 {
    gdk::Display::default()
        .and_then(|display| display.monitors().item(0))
        .and_downcast::<gdk::Monitor>()
        .map(|monitor| monitor.scale_factor())
        .unwrap_or(1)
}

// ---------------------------------------------------------------------------
// Look of the Linux version

/// The design asks for Inter / Space Grotesk; where they are missing (as on the
/// Linux machines) GTK falls back to Liberation Sans. Arial has the same metrics
/// and ships with Windows, whose own "sans-serif" fallback is much wider.
const FONT_CSS: &str = r#"
window { font-family: "Inter", "Liberation Sans", "Arial", sans-serif; }
.header-brand, .home-hero-title, .media-details-header-title,
.title-1, .title-2, .title-3, .settings-heading {
  font-family: "Space Grotesk", "Inter", "Liberation Sans", "Arial", sans-serif;
}
/* With Direct Composition the area outside the window is drawn black, not
   transparent: no shadow (the patched GTK then keeps no margin) and square
   corners. */
window.csd { box-shadow: none; border-radius: 0; }
"#;

/// Same fonts as on Linux, text size as in other Windows programs.
pub fn apply_linux_look() {
    let Some(settings) = gtk::Settings::default() else {
        return;
    };
    // Size as in other Windows programs ("Segoe UI 9"); Linux uses 11, which
    // made Rustfin's text bigger than everything else on Windows.
    settings.set_gtk_font_name(Some("Arial 9"));

    if let Some(display) = gdk::Display::default() {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(FONT_CSS);
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
        );
    }

    // Text at the Windows scaling, like the other programs: GTK renders with an
    // integer scale (175 % -> 2), so the text DPI makes up the difference
    // (168 / 2 = 84). Linux's extra text scaling (1.18) made Rustfin bigger than
    // everything else on Windows.
    let gtk_scale = monitor_scale();
    if gtk_scale >= 2 {
        extern "system" {
            fn GetDpiForSystem() -> u32;
        }
        let system_dpi = f64::from(unsafe { GetDpiForSystem() }.max(96));
        settings.set_gtk_xft_dpi((system_dpi / f64::from(gtk_scale) * 1024.0) as i32);
    }
}
