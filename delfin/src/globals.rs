use relm4::{once_cell::sync::Lazy, SharedState};

use std::collections::HashMap;

use uuid::Uuid;

use crate::{
    config::Config, jellyfin_api::api::item::StreamSettings, utils::shift_state::ShiftState,
};

pub static CONFIG: Lazy<SharedState<Config>> = Lazy::new(|| {
    let state = SharedState::new();
    *state.write() = Config::new().unwrap_or_else(|err| {
        tracing::error!("Failed to load configuration: {err}");
        Config::default()
    });
    state
});

pub static SHIFT_STATE: Lazy<SharedState<ShiftState>> = Lazy::new(|| {
    let state = SharedState::new();
    *state.write() = ShiftState::default();
    state
});

/// Quality, version, and server audio track of the current stream.
pub static STREAM_SETTINGS: Lazy<SharedState<StreamSettings>> = Lazy::new(SharedState::new);

/// Version selected on the details page (item ID → media source ID).
pub static SELECTED_VERSIONS: Lazy<SharedState<HashMap<Uuid, String>>> =
    Lazy::new(SharedState::new);
