pub mod general;
mod migrate;
mod versions;
pub mod video_player_config;

use std::{fs, path::PathBuf};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use self::{
    general::GeneralConfig, migrate::ConfigVersions, video_player_config::VideoPlayerConfig,
};

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct Config {
    version: usize,
    #[serde(default)]
    pub window: Window,
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub video_player: VideoPlayerConfig,
    pub servers: Vec<Server>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 2,
            window: Window::default(),
            general: GeneralConfig::default(),
            video_player: VideoPlayerConfig::default(),
            servers: Vec::default(),
        }
    }
}

impl Config {
    pub fn new() -> Result<Self> {
        let config_file = match get_config_file_exists() {
            Some(f) => f,
            None => return Ok(Self::default()),
        };
        let config = fs::read_to_string(config_file)?;
        let config_version = ConfigVersions::new(&config)?;
        Ok(config_version.into())
    }

    pub fn save(&self) -> Result<()> {
        let config_file = match get_config_file(true) {
            Some(f) => f,
            None => return Err(anyhow!("Error getting config file")),
        };
        let toml = toml::to_string_pretty(&self)?;
        fs::write(config_file, toml)?;
        Ok(())
    }

    pub fn to_toml(&self) -> Result<String> {
        Ok(toml::to_string_pretty(&self)?)
    }
}

fn get_config_file(create_dir: bool) -> Option<PathBuf> {
    let config_dir = match dirs::config_dir() {
        Some(d) => d,
        None => return None,
    };

    let dir = config_dir.join("rustfin");
    migrate_from_delfin(&config_dir, &dir);

    if create_dir {
        if let Err(err) = fs::create_dir_all(&dir) {
            tracing::error!("Failed to create config directory {}: {err}", dir.display());
            return None;
        }
    }

    let config_file = dir.join("config.toml");

    Some(config_file)
}

/// Rustfin started as a fork of Delfin: on first start, copy an existing
/// Delfin configuration (servers, accounts, preferences) so nobody has to
/// sign in again. The old file is left in place.
fn migrate_from_delfin(config_dir: &std::path::Path, dir: &std::path::Path) {
    let old = config_dir.join("delfin").join("config.toml");
    let new = dir.join("config.toml");
    if new.exists() || !old.exists() {
        return;
    }
    if let Err(err) = fs::create_dir_all(dir).and_then(|_| fs::copy(&old, &new)) {
        tracing::warn!("Could not copy Delfin configuration: {err}");
    } else {
        tracing::info!("Copied Delfin configuration to {}", new.display());
    }
}

fn get_config_file_exists() -> Option<PathBuf> {
    let config_file = match get_config_file(false) {
        Some(f) => f,
        None => return None,
    };

    let exists = match config_file.try_exists() {
        Ok(exists) => exists,
        Err(err) => {
            tracing::error!(
                "Failed to check config file {}: {err}",
                config_file.display()
            );
            return None;
        }
    };
    if !exists {
        return None;
    }

    Some(config_file)
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Server {
    pub id: Uuid,
    pub url: String,
    pub name: String,
    pub accounts: Vec<Account>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Account {
    pub id: Uuid,
    pub username: String,
    // TODO: move to keyring
    pub access_token: String,
    pub device_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Window {
    pub width: usize,
    pub height: usize,
    pub maximized: bool,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            width: 960,
            height: 540,
            maximized: false,
        }
    }
}
