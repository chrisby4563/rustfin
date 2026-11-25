pub mod bif;
pub mod constants;
pub mod debounce;
pub mod delete_item;
pub mod device_profile;
pub mod display_years;
pub mod empty_component;
pub mod inhibit;
pub mod item_name;
pub mod main_window;
pub mod message_broker;
pub mod mini_player;
pub mod playable;
pub mod rgba;
pub mod round;
pub mod shift_state;
pub mod ticks;
pub mod user_config;
pub mod fade;
pub mod redact;
#[cfg(windows)]
pub mod win32;

/// Factor for stored window sizes (see `win32::text_scale`); 1 elsewhere.
pub fn window_size_scale() -> f64 {
    #[cfg(windows)]
    return win32::text_scale();
    #[cfg(not(windows))]
    return 1.0;
}
