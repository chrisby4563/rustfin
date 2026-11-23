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

extern "system" {
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

/// GTK on Windows places transient windows (dialogs) at the top left of the
/// screen instead of over their parent. Center every such window on its
/// parent once it is shown.
pub fn center_dialogs_on_parent() {
    let toplevels = gtk::Window::toplevels();
    toplevels.connect_items_changed(|list, position, _removed, added| {
        for i in position..position + added {
            let Some(window) = list.item(i).and_downcast::<gtk::Window>() else {
                continue;
            };
            window.connect_map(|window| {
                let window = window.clone();
                glib::idle_add_local_once(move || center_on_parent(&window));
            });
        }
    });
}

fn center_on_parent(window: &gtk::Window) {
    let Some(parent) = window.transient_for() else {
        return;
    };
    let (Some(own), Some(parent)) = (hwnd(window), hwnd(&parent)) else {
        return;
    };
    let (Some(own_rect), Some(parent_rect)) = (window_rect(own), window_rect(parent)) else {
        return;
    };

    let width = own_rect.right - own_rect.left;
    let height = own_rect.bottom - own_rect.top;
    let x = parent_rect.left + (parent_rect.right - parent_rect.left - width) / 2;
    let y = parent_rect.top + (parent_rect.bottom - parent_rect.top - height) / 2;

    unsafe {
        SetWindowPos(
            own,
            std::ptr::null_mut(),
            x.max(0),
            y.max(0),
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}
