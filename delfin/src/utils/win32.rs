//! Small Windows-only helpers around native window handles.

use std::ffi::c_void;

use relm4::gtk::{self, gdk, glib, glib::translate::ToGlibPtr, prelude::*};

#[repr(C)]
#[derive(Default)]
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
