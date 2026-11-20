#![cfg_attr(docsrs, feature(doc_cfg))]

/// No-op.
#[macro_export]
macro_rules! skip_assert_initialized {
    () => {};
}

/// Asserts that this is the main thread and either `gdk::init` or `gtk::init` has been called.
macro_rules! assert_initialized_main_thread {
    () => {
        if !::gtk::is_initialized_main_thread() {
            if ::gtk::is_initialized() {
                panic!("GTK may only be used from the main thread.");
            } else {
                panic!("GTK has not been initialized. Call `gtk::init` first.");
            }
        }
    };
}

pub use auto::*;
mod auto;

use glib::translate::ToGlibPtr;

impl VideoPlayerMpv {
    /// Seek with sub-second precision (added manually, not generated from GIR).
    pub fn seek_to_precise(&self, seconds: f64) {
        unsafe {
            ffi::vpm_video_player_mpv_seek_to_precise(self.to_glib_none().0, seconds);
        }
    }
}
