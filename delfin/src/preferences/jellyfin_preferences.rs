//! Einstellungen wie im Jellyfin-Client: Wiedergabe (Ton, Untertitel, Ablauf,
//! sections) and Home (areas, “Next Up”, libraries, display). Values marked
//! (Jellyfin) reside on the server and also apply to the web UI; the rest is
//! only for Delfin.

use std::{collections::HashMap, sync::Arc};

use adw::prelude::*;
use relm4::{adw, gtk};
use serde_json::Value;

use crate::{
    config::video_player_config::SegmentAction,
    globals::CONFIG,
    jellyfin_api::{api::item::StreamQuality, api_client::ApiClient},
    tr,
    utils::user_config,
};

async fn run_bg<T: Send + 'static>(
    future: impl std::future::Future<Output = T> + Send + 'static,
) -> Option<T> {
    let (tx, rx) = relm4::tokio::sync::oneshot::channel();
    relm4::spawn(async move {
        let _ = tx.send(future.await);
    });
    rx.await.ok()
}

fn toast(window: &adw::PreferencesWindow, text: &str) {
    window.add_toast(adw::Toast::new(text));
}

/// Save a Jellyfin user-preference value (in the background).
fn save_server(api_client: &Arc<ApiClient>, window: &adw::PreferencesWindow, key: &str, value: Value) {
    let api_client = api_client.clone();
    let window = window.clone();
    let key = key.to_string();
    gtk::glib::spawn_future_local(async move {
        let ok = run_bg(user_config::save(api_client, key, value))
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false);
        if !ok {
            toast(&window, tr!("prefs-jf-save-failed"));
        }
    });
}

fn save_display(api_client: &Arc<ApiClient>, window: &adw::PreferencesWindow, changes: HashMap<String, String>) {
    let api_client = api_client.clone();
    let window = window.clone();
    gtk::glib::spawn_future_local(async move {
        let ok = run_bg(user_config::save_display_prefs(api_client, changes))
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false);
        if !ok {
            toast(&window, tr!("prefs-jf-save-failed"));
        }
    });
}

fn server_switch(
    group: &adw::PreferencesGroup,
    api_client: &Arc<ApiClient>,
    window: &adw::PreferencesWindow,
    key: &'static str,
    default: bool,
    title: &str,
    subtitle: &str,
) {
    let row = adw::SwitchRow::builder()
        .title(title)
        .subtitle(subtitle)
        .active(user_config::get_bool(key, default))
        .build();
    let api_client = api_client.clone();
    let window = window.clone();
    row.connect_active_notify(move |row| {
        save_server(&api_client, &window, key, Value::Bool(row.is_active()));
    });
    group.add(&row);
}

fn combo(title: &str, subtitle: &str, labels: &[String], selected: usize) -> adw::ComboRow {
    let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
    let row = adw::ComboRow::builder()
        .title(title)
        .subtitle(subtitle)
        .model(&gtk::StringList::new(&labels))
        .build();
    row.set_selected(selected as u32);
    row
}

fn save_local(change: impl FnOnce(&mut crate::config::Config)) {
    let mut config = CONFIG.write();
    change(&mut config);
    if let Err(err) = config.save() {
        tracing::error!("Error saving config: {err}");
    }
}

/// Sprachauswahl (Server-Kulturliste), gespeichert als Drei-Buchstaben-Code.
fn language_combo(
    group: &adw::PreferencesGroup,
    api_client: &Arc<ApiClient>,
    window: &adw::PreferencesWindow,
    key: &'static str,
    title: &str,
    also_local_audio: bool,
) {
    let row = adw::ComboRow::builder()
        .title(title)
        .subtitle(tr!("prefs-jf-synced"))
        .sensitive(false)
        .build();
    group.add(&row);
    let api_client = api_client.clone();
    let window = window.clone();
    gtk::glib::spawn_future_local(async move {
        let cultures = {
            let api_client = api_client.clone();
            run_bg(async move { user_config::cultures(&api_client).await })
                .await
                .unwrap_or_default()
        };
        let mut labels = vec![tr!("prefs-jf-any-language").to_string()];
        let mut codes = vec![String::new()];
        for (name, three, _) in cultures {
            labels.push(name);
            codes.push(three);
        }
        let current = user_config::get_str(key);
        let selected = codes.iter().position(|c| *c == current).unwrap_or(0);
        let labels_ref: Vec<&str> = labels.iter().map(String::as_str).collect();
        row.set_model(Some(&gtk::StringList::new(&labels_ref)));
        row.set_enable_search(true);
        row.set_selected(selected as u32);
        row.set_sensitive(true);
        row.connect_selected_notify(move |row| {
            let code = codes.get(row.selected() as usize).cloned().unwrap_or_default();
            if also_local_audio && !code.is_empty() {
                let langs = user_config::mpv_languages(&code);
                save_local(|c| c.video_player.preferred_audio_languages = langs);
            }
            save_server(&api_client, &window, key, Value::String(code));
        });
    });
}

pub fn playback_page(api_client: Arc<ApiClient>, window: &adw::PreferencesWindow) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title(tr!("prefs-jf-playback-page"))
        .icon_name("media-playback-start-symbolic")
        .build();

    // ── Ton & Untertitel ──
    let tracks = adw::PreferencesGroup::builder()
        .title(tr!("prefs-jf-tracks"))
        .description(tr!("prefs-jf-tracks-description"))
        .build();
    language_combo(&tracks, &api_client, window, "AudioLanguagePreference", tr!("prefs-jf-audio-language"), true);
    server_switch(&tracks, &api_client, window, "PlayDefaultAudioTrack", false,
        tr!("prefs-jf-default-audio.title"), tr!("prefs-jf-default-audio.subtitle"));
    language_combo(&tracks, &api_client, window, "SubtitleLanguagePreference", tr!("prefs-jf-subtitle-language"), false);

    let modes = ["Default", "Smart", "OnlyForced", "Always", "None"];
    let mode_labels: Vec<String> = vec![
        tr!("prefs-jf-subtitle-mode-default").to_string(),
        tr!("prefs-jf-subtitle-mode-smart").to_string(),
        tr!("prefs-jf-subtitle-mode-forced").to_string(),
        tr!("prefs-jf-subtitle-mode-always").to_string(),
        tr!("prefs-jf-subtitle-mode-none").to_string(),
    ];
    let current = user_config::get_str("SubtitleMode");
    let mode_row = combo(
        tr!("prefs-jf-subtitle-mode.title"),
        tr!("prefs-jf-subtitle-mode.subtitle"),
        &mode_labels,
        modes.iter().position(|m| *m == current).unwrap_or(0),
    );
    mode_row.connect_selected_notify({
        let api_client = api_client.clone();
        let window = window.clone();
        move |row| {
            let mode = modes.get(row.selected() as usize).copied().unwrap_or("Default");
            save_server(&api_client, &window, "SubtitleMode", Value::String(mode.into()));
        }
    });
    tracks.add(&mode_row);
    server_switch(&tracks, &api_client, window, "RememberAudioSelections", true,
        tr!("prefs-jf-remember-audio.title"), tr!("prefs-jf-remember.subtitle"));
    server_switch(&tracks, &api_client, window, "RememberSubtitleSelections", true,
        tr!("prefs-jf-remember-subtitles.title"), tr!("prefs-jf-remember.subtitle"));
    page.add(&tracks);

    // ── Ablauf ──
    let flow = adw::PreferencesGroup::builder()
        .title(tr!("prefs-jf-flow"))
        .build();
    server_switch(&flow, &api_client, window, "EnableNextEpisodeAutoPlay", true,
        tr!("prefs-jf-next-episode.title"), tr!("prefs-jf-next-episode.subtitle"));
    let quality_labels: Vec<String> = StreamQuality::ALL
        .iter()
        .map(|q| crate::video_player::controls::quality::quality_label(*q, None))
        .collect();
    let current_quality = CONFIG.read().video_player.default_quality;
    let quality_row = combo(
        tr!("prefs-jf-quality.title"),
        tr!("prefs-jf-quality.subtitle"),
        &quality_labels,
        StreamQuality::ALL.iter().position(|q| *q == current_quality).unwrap_or(0),
    );
    quality_row.connect_selected_notify(|row| {
        if let Some(quality) = StreamQuality::ALL.get(row.selected() as usize).copied() {
            save_local(|c| c.video_player.default_quality = quality);
            crate::globals::STREAM_SETTINGS.write().quality = quality;
        }
    });
    flow.add(&quality_row);
    page.add(&flow);

    // ── Abschnitte (Media Segments) ──
    let segments = adw::PreferencesGroup::builder()
        .title(tr!("prefs-jf-segments"))
        .description(tr!("prefs-jf-segments-description"))
        .build();
    let action_labels: Vec<String> = vec![
        tr!("prefs-jf-segment-skip").to_string(),
        tr!("prefs-jf-segment-ask").to_string(),
        tr!("prefs-jf-segment-nothing").to_string(),
    ];
    let actions = CONFIG.read().video_player.segment_actions;
    let rows: [(&str, SegmentAction, fn(&mut crate::config::Config, SegmentAction)); 4] = [
        (tr!("prefs-jf-segment-intro"), actions.intro, |c, a| c.video_player.segment_actions.intro = a),
        (tr!("prefs-jf-segment-recap"), actions.recap, |c, a| c.video_player.segment_actions.recap = a),
        (tr!("prefs-jf-segment-outro"), actions.outro, |c, a| c.video_player.segment_actions.outro = a),
        (tr!("prefs-jf-segment-preview"), actions.preview, |c, a| c.video_player.segment_actions.preview = a),
    ];
    for (title, current, set) in rows {
        let row = combo(
            title,
            "",
            &action_labels,
            SegmentAction::ALL.iter().position(|a| *a == current).unwrap_or(1),
        );
        row.connect_selected_notify(move |row| {
            if let Some(action) = SegmentAction::ALL.get(row.selected() as usize).copied() {
                save_local(|c| set(c, action));
            }
        });
        segments.add(&row);
    }
    page.add(&segments);

    page
}

const HOME_SECTIONS: [&str; 10] = [
    "smalllibrarytiles",
    "librarybuttons",
    "resume",
    "nextup",
    "latestmedia",
    "livetv",
    "activerecordings",
    "resumeaudio",
    "resumebook",
    "none",
];
const HOME_DEFAULTS: [&str; 7] = [
    "smalllibrarytiles",
    "resume",
    "resumeaudio",
    "resumebook",
    "livetv",
    "nextup",
    "latestmedia",
];

pub fn home_page(api_client: Arc<ApiClient>, window: &adw::PreferencesWindow) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title(tr!("prefs-jf-home-page"))
        .icon_name("go-home-symbolic")
        .build();

    // ── Startseite ──
    let home = adw::PreferencesGroup::builder()
        .title(tr!("prefs-jf-home"))
        .description(tr!("prefs-jf-home-description"))
        .build();
    let hero = adw::SwitchRow::builder()
        .title(tr!("prefs-jf-hero.title"))
        .subtitle(tr!("prefs-jf-hero.subtitle"))
        .active(CONFIG.read().general.home_hero)
        .build();
    hero.connect_active_notify(|row| {
        let active = row.is_active();
        save_local(|c| c.general.home_hero = active);
    });
    home.add(&hero);
    let section_labels: Vec<String> = vec![
        tr!("prefs-jf-section-my-media").to_string(),
        tr!("prefs-jf-section-my-media-small").to_string(),
        tr!("prefs-jf-section-resume").to_string(),
        tr!("prefs-jf-section-next-up").to_string(),
        tr!("prefs-jf-section-latest").to_string(),
        tr!("prefs-jf-section-live-tv").to_string(),
        tr!("prefs-jf-section-recordings").to_string(),
        tr!("prefs-jf-section-resume-audio").to_string(),
        tr!("prefs-jf-section-resume-book").to_string(),
        tr!("prefs-jf-section-none").to_string(),
    ];
    for (index, default) in HOME_DEFAULTS.iter().enumerate() {
        let key = format!("homesection{index}");
        let current = user_config::display_pref(&key).unwrap_or_else(|| default.to_string());
        let row = combo(
            &tr!("prefs-jf-section", {"number" => index + 1}).to_string(),
            "",
            &section_labels,
            HOME_SECTIONS.iter().position(|s| *s == current).unwrap_or(9),
        );
        row.connect_selected_notify({
            let api_client = api_client.clone();
            let window = window.clone();
            move |row| {
                let value = HOME_SECTIONS.get(row.selected() as usize).copied().unwrap_or("none");
                save_display(&api_client, &window, HashMap::from([(key.clone(), value.to_string())]));
            }
        });
        home.add(&row);
    }
    page.add(&home);

    // ── Next Up ──
    let next_up = adw::PreferencesGroup::builder()
        .title(tr!("prefs-jf-next-up"))
        .build();
    let max_days: f64 = user_config::display_pref("maxDaysForNextUp")
        .and_then(|d| d.parse().ok())
        .unwrap_or(365.0);
    let days = adw::SpinRow::with_range(0.0, 1000.0, 1.0);
    days.set_title(tr!("prefs-jf-max-days.title"));
    days.set_subtitle(tr!("prefs-jf-max-days.subtitle"));
    days.set_value(max_days);
    days.connect_value_notify({
        let api_client = api_client.clone();
        let window = window.clone();
        move |row| {
            save_display(
                &api_client,
                &window,
                HashMap::from([("maxDaysForNextUp".to_string(), (row.value() as i64).to_string())]),
            );
        }
    });
    next_up.add(&days);
    let rewatch = adw::SwitchRow::builder()
        .title(tr!("prefs-jf-rewatching.title"))
        .subtitle(tr!("prefs-jf-rewatching.subtitle"))
        .active(user_config::display_pref("enableRewatchingInNextUp").as_deref() == Some("true"))
        .build();
    rewatch.connect_active_notify({
        let api_client = api_client.clone();
        let window = window.clone();
        move |row| {
            save_display(
                &api_client,
                &window,
                HashMap::from([("enableRewatchingInNextUp".to_string(), row.is_active().to_string())]),
            );
        }
    });
    next_up.add(&rewatch);
    let episode_image = adw::SwitchRow::builder()
        .title(tr!("prefs-general-use-episode-image.title"))
        .subtitle(tr!("prefs-general-use-episode-image.subtitle"))
        .active(CONFIG.read().general.use_episode_image)
        .build();
    episode_image.connect_active_notify(|row| {
        let active = row.is_active();
        save_local(|c| c.general.use_episode_image = active);
    });
    next_up.add(&episode_image);
    page.add(&next_up);

    // ── Anzeige ──
    let display = adw::PreferencesGroup::builder()
        .title(tr!("prefs-jf-display"))
        .build();
    server_switch(&display, &api_client, window, "DisplayMissingEpisodes", false,
        tr!("prefs-jf-missing.title"), tr!("prefs-jf-missing.subtitle"));
    server_switch(&display, &api_client, window, "HidePlayedInLatest", true,
        tr!("prefs-jf-hide-played.title"), tr!("prefs-jf-synced"));
    page.add(&display);

    // ── Bibliotheken ──
    let libraries = adw::PreferencesGroup::builder()
        .title(tr!("prefs-jf-libraries"))
        .description(tr!("prefs-jf-libraries-description"))
        .build();
    page.add(&libraries);
    gtk::glib::spawn_future_local({
        let api_client = api_client.clone();
        let window = window.clone();
        let libraries = libraries.clone();
        async move {
            let views = {
                let api_client = api_client.clone();
                run_bg(async move { api_client.get_user_views(None, None).await.ok() })
                    .await
                    .flatten()
                    .map(|(views, _)| views)
                    .unwrap_or_default()
            };
            for view in views {
                let id = view.id().simple().to_string();
                let row = adw::ActionRow::builder().title(view.name()).build();
                for (key, label) in [
                    ("MyMediaExcludes", tr!("prefs-jf-library-my-media")),
                    ("LatestItemsExcludes", tr!("prefs-jf-library-latest")),
                ] {
                    let check = gtk::CheckButton::builder()
                        .label(label)
                        .active(!user_config::get_list(key).contains(&id))
                        .valign(gtk::Align::Center)
                        .build();
                    let api_client = api_client.clone();
                    let window = window.clone();
                    let id = id.clone();
                    check.connect_toggled(move |check| {
                        let mut list = user_config::get_list(key);
                        list.retain(|x| *x != id);
                        if !check.is_active() {
                            list.push(id.clone());
                        }
                        save_server(
                            &api_client,
                            &window,
                            key,
                            Value::Array(list.into_iter().map(Value::String).collect()),
                        );
                    });
                    row.add_suffix(&check);
                }
                libraries.add(&row);
            }
        }
    });

    page
}
