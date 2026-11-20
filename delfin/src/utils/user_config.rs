//! Jellyfin-Benutzereinstellungen (gelten für alle Clients, auch die
//! Weboberfläche): Tonsprache, Untertitelmodus, nächste Folge, Startseite …
//! Gelesen bei der Anmeldung, geschrieben aus den Einstellungen.

use std::{collections::HashMap, sync::Arc};

use relm4::once_cell::sync::Lazy;
use serde_json::Value;
use tracing::{info, warn};

use crate::jellyfin_api::api_client::ApiClient;

/// `UserConfiguration` des angemeldeten Kontos.
static USER_CONFIG: Lazy<std::sync::RwLock<Value>> =
    Lazy::new(|| std::sync::RwLock::new(Value::Null));
/// CustomPrefs der Anzeige-Einstellungen (`DisplayPreferences/usersettings`, Client `emby`).
static DISPLAY_PREFS: Lazy<std::sync::RwLock<Value>> =
    Lazy::new(|| std::sync::RwLock::new(Value::Null));

pub const DISPLAY_CLIENT: &str = "emby";

pub fn load(api_client: Arc<ApiClient>) {
    // Startqualität aus den Einstellungen.
    crate::globals::STREAM_SETTINGS.write().quality =
        crate::globals::CONFIG.read().video_player.default_quality;
    relm4::spawn(async move {
        if let Some(config) = fetch_user_config(&api_client).await {
            info!("User configuration loaded");
            *USER_CONFIG.write().unwrap() = config;
        }
        if let Some(prefs) = fetch_display_prefs(&api_client).await {
            *DISPLAY_PREFS.write().unwrap() = prefs;
        }
    });
}

/// Beide Einstellungsquellen neu holen (wartet darauf).
pub async fn refresh(api_client: &ApiClient) {
    if let Some(config) = fetch_user_config(api_client).await {
        *USER_CONFIG.write().unwrap() = config;
    }
    if let Some(prefs) = fetch_display_prefs(api_client).await {
        *DISPLAY_PREFS.write().unwrap() = prefs;
    }
}

pub async fn fetch_user_config(api_client: &ApiClient) -> Option<Value> {
    let url = api_client.root.join("Users/Me").ok()?;
    let user: Value = api_client.client.get(url).send().await.ok()?.json().await.ok()?;
    Some(user["Configuration"].clone())
}

pub async fn fetch_display_prefs(api_client: &ApiClient) -> Option<Value> {
    let mut url = api_client.root.join("DisplayPreferences/usersettings").ok()?;
    url.query_pairs_mut()
        .append_pair("userId", &api_client.account.id.to_string())
        .append_pair("client", DISPLAY_CLIENT);
    api_client.client.get(url).send().await.ok()?.json().await.ok()
}

pub fn get_bool(key: &str, default: bool) -> bool {
    USER_CONFIG.read().unwrap()[key].as_bool().unwrap_or(default)
}

pub fn get_str(key: &str) -> String {
    USER_CONFIG.read().unwrap()[key]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

pub fn get_list(key: &str) -> Vec<String> {
    USER_CONFIG.read().unwrap()[key]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_lowercase)).collect())
        .unwrap_or_default()
}

/// Ein Wert der Benutzereinstellungen ändern und das Ganze zurückschreiben.
pub async fn save(api_client: Arc<ApiClient>, key: String, value: Value) -> anyhow::Result<()> {
    let config = {
        let mut config = USER_CONFIG.write().unwrap();
        if config.is_null() {
            anyhow::bail!("user configuration not loaded");
        }
        config[key.as_str()] = value;
        config.clone()
    };
    let mut url = api_client.root.join("Users/Configuration")?;
    url.query_pairs_mut()
        .append_pair("userId", &api_client.account.id.to_string());
    api_client
        .client
        .post(url)
        .json(&config)
        .send()
        .await?
        .error_for_status()?;
    info!("Saved user configuration: {key}");
    Ok(())
}

pub fn display_pref(key: &str) -> Option<String> {
    DISPLAY_PREFS.read().unwrap()["CustomPrefs"][key]
        .as_str()
        .map(str::to_string)
}

/// Eine Anzeige-Einstellung (CustomPrefs) ändern und zurückschreiben.
pub async fn save_display_prefs(
    api_client: Arc<ApiClient>,
    changes: HashMap<String, String>,
) -> anyhow::Result<()> {
    let prefs = {
        let mut prefs = DISPLAY_PREFS.write().unwrap();
        if prefs.is_null() {
            anyhow::bail!("display preferences not loaded");
        }
        if !prefs["CustomPrefs"].is_object() {
            prefs["CustomPrefs"] = Value::Object(Default::default());
        }
        for (k, v) in changes {
            prefs["CustomPrefs"][k.as_str()] = Value::String(v);
        }
        prefs.clone()
    };
    let mut url = api_client.root.join("DisplayPreferences/usersettings")?;
    url.query_pairs_mut()
        .append_pair("userId", &api_client.account.id.to_string())
        .append_pair("client", DISPLAY_CLIENT);
    api_client
        .client
        .post(url)
        .json(&prefs)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

/// Sprachen vom Server (für Auswahllisten): (Anzeigename, Drei-Buchstaben-Code, Zwei-Buchstaben-Code).
pub async fn cultures(api_client: &ApiClient) -> Vec<(String, String, String)> {
    let Ok(url) = api_client.root.join("Localization/Cultures") else {
        return vec![];
    };
    let list: Vec<Value> = match api_client.client.get(url).send().await {
        Ok(r) => r.json().await.unwrap_or_default(),
        Err(err) => {
            warn!("Could not load cultures: {err}");
            vec![]
        }
    };
    let mut out: Vec<(String, String, String)> = list
        .iter()
        .filter_map(|c| {
            Some((
                c["DisplayName"].as_str()?.to_string(),
                c["ThreeLetterISOLanguageName"].as_str()?.to_string(),
                c["TwoLetterISOLanguageName"].as_str().unwrap_or_default().to_string(),
            ))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// mpv-Sprachliste zu einem Jellyfin-Sprachcode (z. B. `ger` → `ger,deu,de`).
pub fn mpv_languages(code: &str) -> String {
    match code {
        "" => String::new(),
        "ger" | "deu" => "ger,deu,de".into(),
        "eng" => "eng,en".into(),
        "jpn" => "jpn,ja".into(),
        "fre" | "fra" => "fre,fra,fr".into(),
        "spa" => "spa,es".into(),
        "ita" => "ita,it".into(),
        other => other.to_string(),
    }
}

/// mpv-Optionen für Ton- und Untertitelwahl (direkt abgespielte Dateien).
pub fn mpv_track_options(local_audio: &str) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    // Tonspur: Jellyfin-Sprache, sonst lokale Liste; „Standardspur“ = mpv entscheidet nach Flag.
    if !get_bool("PlayDefaultAudioTrack", false) {
        let server = mpv_languages(&get_str("AudioLanguagePreference"));
        out.push(("alang", if server.is_empty() { local_audio.to_string() } else { server }));
    } else {
        out.push(("alang", String::new()));
    }
    let slang = mpv_languages(&get_str("SubtitleLanguagePreference"));
    out.push(("slang", slang));
    // Untertitelmodus wie Jellyfin (Default/Smart/OnlyForced/Always/None).
    let mode = get_str("SubtitleMode");
    let (sid, fallback, forced, matching) = match mode.as_str() {
        "None" => ("no", "no", "no", "no"),
        "Always" => ("auto", "yes", "yes", "yes"),
        "OnlyForced" => ("auto", "no", "always", "forced"),
        "Smart" => ("auto", "default", "yes", "no"),
        _ => ("auto", "default", "yes", "yes"),
    };
    out.push(("sid", sid.to_string()));
    out.push(("subs-fallback", fallback.to_string()));
    out.push(("subs-fallback-forced", forced.to_string()));
    out.push(("subs-with-matching-audio", matching.to_string()));
    out
}
