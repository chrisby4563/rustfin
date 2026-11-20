use relm4::{once_cell::sync::Lazy, SharedState};

use std::collections::HashMap;

use uuid::Uuid;

use crate::{
    config::Config, jellyfin_api::api::item::StreamSettings, utils::shift_state::ShiftState,
};

pub static CONFIG: Lazy<SharedState<Config>> = Lazy::new(|| {
    let state = SharedState::new();
    *state.write() = Config::new().expect("Failed to load config");
    state
});

pub static SHIFT_STATE: Lazy<SharedState<ShiftState>> = Lazy::new(|| {
    let state = SharedState::new();
    *state.write() = ShiftState::default();
    state
});

/// Qualität, Version und Server-Tonspur des laufenden Streams.
pub static STREAM_SETTINGS: Lazy<SharedState<StreamSettings>> = Lazy::new(SharedState::new);

/// Auf der Detailseite gewählte Version (Element-ID → MediaSource-ID).
pub static SELECTED_VERSIONS: Lazy<SharedState<HashMap<Uuid, String>>> =
    Lazy::new(SharedState::new);
