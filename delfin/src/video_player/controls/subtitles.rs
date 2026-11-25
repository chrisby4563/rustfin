use std::{cell::RefCell, sync::Arc};

use gtk::prelude::*;
use jellyfin_api::types::BaseItemDto;
use relm4::{
    actions::{ActionGroupName, ActionName, RelmAction, RelmActionGroup},
    gtk::{self, gio},
    Component, ComponentParts, ComponentSender,
};
use tracing::warn;

use crate::{
    app::{AppInput, APP_BROKER},
    jellyfin_api::api_client::ApiClient,
    tr,
    utils::message_broker::ResettableMessageBroker,
    video_player::backends::{SubtitleTrack, VideoPlayerBackend},
};

pub static SUBTITLES_BROKER: ResettableMessageBroker<SubtitlesInput> =
    ResettableMessageBroker::new();

relm4::new_action_group!(SubtitleActionGroup, "subtitle_actions");
relm4::new_stateful_action!(
    SelectedSubtitleAction,
    SubtitleActionGroup,
    "selected_subtitle",
    Option<i32>,
    Option<i32>
);

#[derive(Clone)]
pub struct ExternalSubtitleTrack {
    name: String,
    url: String,
    /// Jellyfin's default subtitle for this user (language/mode preferences).
    is_default: bool,
}

impl std::fmt::Debug for ExternalSubtitleTrack {
    // Without the URL (it carries the access token).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalSubtitleTrack")
            .field("name", &self.name)
            .field("is_default", &self.is_default)
            .finish()
    }
}

/// Wait this long before telling that the subtitles take a while.
const SLOW_HINT_AFTER: std::time::Duration = std::time::Duration::from_secs(3);

#[derive(Debug)]
pub struct Subtitles {
    video_player: Arc<RefCell<dyn VideoPlayerBackend>>,
    menu: gio::Menu,
    subtitles_available: bool,
    previous_track: Option<usize>,
    /// Subtitles loaded separately from the stream (external files, and
    /// embedded text subtitles while the server converts the video).
    external: Vec<ExternalSubtitleTrack>,
    /// How many of `external` were added to the current stream.
    external_added: usize,
    /// Separate subtitles still downloading. The first time, the server has
    /// to read the whole video file for them (minutes for large files).
    loading: usize,
    /// Ignore downloads that belong to a previous title.
    generation: u64,
    /// The subtitle tracks of the current stream (from mpv).
    tracks: Vec<SubtitleTrack>,
    /// The current stream is loaded; mpv only accepts `sub-add` then.
    stream_ready: bool,
}

#[derive(Debug)]
pub enum SubtitlesInput {
    Reset {
        api_client: Arc<ApiClient>,
        item: Box<BaseItemDto>,
    },
    SubtitlesUpdated(Vec<SubtitleTrack>),
    ToggleSubtitles,
    /// Same title, new stream (e.g. other audio track): add the separately
    /// loaded subtitles again.
    StreamReloaded,
}

#[derive(Debug)]
pub enum SubtitlesCommandOutput {
    /// This many separate subtitles are being downloaded.
    ExternalSubtitlesFound { generation: u64, count: usize },
    /// One download finished (`None`: failed).
    ExternalSubtitleLoaded {
        generation: u64,
        track: Option<ExternalSubtitleTrack>,
    },
    SlowHint { generation: u64 },
}

#[relm4::component(pub)]
impl Component for Subtitles {
    type Init = Arc<RefCell<dyn VideoPlayerBackend>>;
    type Input = SubtitlesInput;
    type Output = ();
    type CommandOutput = SubtitlesCommandOutput;

    view! {
        gtk::MenuButton {
            set_icon_name: "closed-captioning",
            set_menu_model: Some(&model.menu),
            set_focus_on_click: false,
            set_direction: gtk::ArrowType::Up,
            #[watch]
            set_tooltip_text: Some(tr!(
                "vp-subtitle-track-tooltip",
                {"subtitlesAvailable" => model.subtitles_available.to_string()},
            )),
            #[watch]
            set_sensitive: model.subtitles_available || model.loading > 0,
            #[watch]
            set_has_tooltip: !model.subtitles_available && model.loading == 0,
        }
    }

    fn init(
        video_player: Self::Init,
        root: Self::Root,
        sender: relm4::ComponentSender<Self>,
    ) -> relm4::ComponentParts<Self> {
        let model = Subtitles {
            video_player,
            menu: gio::Menu::new(),
            subtitles_available: false,
            previous_track: None,
            external: Vec::new(),
            external_added: 0,
            loading: 0,
            generation: 0,
            tracks: Vec::new(),
            stream_ready: false,
        };

        model
            .video_player
            .borrow_mut()
            .connect_subtitle_tracks_updated(Box::new(move |tracks| {
                sender.input(SubtitlesInput::SubtitlesUpdated(tracks.clone()));
            }));

        let widgets = view_output!();

        let selected_subtitle_action: RelmAction<SelectedSubtitleAction> =
            RelmAction::new_stateful_with_target_value(&None, {
                let video_player = model.video_player.clone();
                move |_, state, value: Option<i32>| {
                    *state = value;
                    video_player
                        .borrow()
                        .set_subtitle_track(value.map(|id| id as usize));
                }
            });

        let mut group = RelmActionGroup::<SubtitleActionGroup>::new();
        group.add_action(selected_subtitle_action);
        group.register_for_widget(root);

        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: relm4::ComponentSender<Self>,
        root: &Self::Root,
    ) {
        match message {
            SubtitlesInput::Reset { api_client, item } => {
                self.subtitles_available = false;
                self.previous_track = None;
                self.external.clear();
                self.external_added = 0;
                self.loading = 0;
                self.generation += 1;
                self.tracks.clear();
                self.stream_ready = false;
                self.rebuild_menu();
                Subtitles::load_external_subtitles(
                    &sender,
                    &api_client,
                    &item,
                    self.generation,
                );
            }
            SubtitlesInput::StreamReloaded => {
                self.stream_ready = false;
                self.external_added = 0;
            }
            SubtitlesInput::SubtitlesUpdated(subtitle_streams) => {
                // Only sent while a stream is loaded.
                self.stream_ready = true;
                self.add_pending_external();
                if subtitle_streams.is_empty() {
                    return;
                }

                self.subtitles_available = true;
                self.tracks = subtitle_streams;
                self.rebuild_menu();

                // Select current subtitle track in menu
                if let Some(current_subtitle_track) =
                    self.video_player.borrow().current_subtitle_track()
                {
                    root.activate_action(
                        &format!(
                            "{}.{}",
                            SubtitleActionGroup::NAME,
                            SelectedSubtitleAction::NAME
                        ),
                        Some(&Some(current_subtitle_track as i32).to_variant()),
                    )
                    .expect("Error selecting current subtitle track.");
                }
            }
            SubtitlesInput::ToggleSubtitles => 'msg_block: {
                if !self.subtitles_available && self.loading > 0 {
                    APP_BROKER.send(AppInput::Toast(tr!("vp-subtitles-preparing").into(), None));
                    break 'msg_block;
                }
                if !self.subtitles_available {
                    APP_BROKER.send(AppInput::Toast(
                        tr!("vp-no-subtitles-available").into(),
                        None,
                    ));
                    break 'msg_block;
                }

                if let Some(current_subtitle_track) =
                    self.video_player.borrow().current_subtitle_track()
                {
                    self.previous_track = Some(current_subtitle_track);
                    self.video_player.borrow().set_subtitle_track(None);
                } else if let Some(previous_track) = self.previous_track {
                    self.video_player
                        .borrow()
                        .set_subtitle_track(Some(previous_track));
                } else {
                    self.video_player.borrow().set_subtitle_track(Some(0));
                }
            }
        }

        self.update_view(widgets, sender);
    }

    fn update_cmd_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::CommandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            SubtitlesCommandOutput::ExternalSubtitlesFound { generation, count }
                if generation == self.generation =>
            {
                self.loading = count;
                self.rebuild_menu();
            }
            SubtitlesCommandOutput::ExternalSubtitleLoaded { generation, track }
                if generation == self.generation =>
            {
                self.loading = self.loading.saturating_sub(1);
                if let Some(track) = track {
                    self.external.push(track);
                    self.add_pending_external();
                }
                self.rebuild_menu();
            }
            SubtitlesCommandOutput::SlowHint { generation }
                if generation == self.generation && self.loading > 0 =>
            {
                APP_BROKER.send(AppInput::Toast(tr!("vp-subtitles-preparing").into(), None));
            }
            _ => {}
        }
        self.update_view(widgets, sender);
    }
}

impl Subtitles {
    /// Add `external` to the stream once it is loaded (mpv rejects `sub-add`
    /// before that, and drops added tracks when a new stream starts).
    fn add_pending_external(&mut self) {
        if !self.stream_ready {
            return;
        }
        for sub in &self.external[self.external_added..] {
            self.video_player
                .borrow()
                .add_subtitle_track(&sub.url, &sub.name, sub.is_default);
        }
        self.external_added = self.external.len();
    }

    /// Tracks of the stream, "Off", and a note while subtitles still load.
    fn rebuild_menu(&self) {
        self.menu.remove_all();
        if !self.tracks.is_empty() {
            let subs_menu = gio::Menu::new();
            for track in &self.tracks {
                subs_menu.append_item(
                    &RelmAction::<SelectedSubtitleAction>::to_menu_item_with_target_value(
                        &track.name,
                        &Some(track.id as i32),
                    ),
                );
            }
            subs_menu.append_item(
                &RelmAction::<SelectedSubtitleAction>::to_menu_item_with_target_value(
                    tr!("vp-subtitle-track-off"),
                    &None,
                ),
            );
            self.menu
                .append_section(Some(tr!("vp-subtitle-track-menu")), &subs_menu);
        }
        if self.loading > 0 {
            // No action: shown greyed out.
            let note = gio::Menu::new();
            note.append(Some(tr!("vp-subtitles-preparing")), Some("subtitle_actions.loading"));
            self.menu.append_section(None, &note);
        }
    }

    fn load_external_subtitles(
        sender: &ComponentSender<Self>,
        api_client: &Arc<ApiClient>,
        item: &BaseItemDto,
        generation: u64,
    ) {
        let Some(item_id) = item.id else { return };
        let settings = crate::globals::STREAM_SETTINGS.read().clone();
        // A converted stream carries no subtitles: load the file's text
        // subtitles separately, as Jellyfin's web player does.
        let transcoded = ApiClient::is_transcoded(item, &settings);

        let api_client = api_client.clone();
        sender.command(move |out, shutdown| {
            shutdown
                .register(async move {
                    let Some(found) =
                        Self::find_external_subtitles(&api_client, item_id, settings, transcoded)
                            .await
                    else {
                        return;
                    };
                    let _ = out.send(SubtitlesCommandOutput::ExternalSubtitlesFound {
                        generation,
                        count: found.len(),
                    });
                    if found.is_empty() {
                        return;
                    }
                    relm4::spawn({
                        let out = out.clone();
                        async move {
                            relm4::tokio::time::sleep(SLOW_HINT_AFTER).await;
                            let _ = out.send(SubtitlesCommandOutput::SlowHint { generation });
                        }
                    });
                    // Fetched here first so the player knows when they are
                    // ready (mpv's own download reports nothing); mpv then
                    // gets them from the server's cache right away. One after
                    // the other: the server extracts all of a file's
                    // subtitles in the first request anyway.
                    for track in found {
                        let track = match fetch(&api_client, &track.url).await {
                            Ok(()) => Some(track),
                            Err(err) => {
                                warn!("Error loading subtitle {}: {err}", track.name);
                                None
                            }
                        };
                        let _ = out.send(SubtitlesCommandOutput::ExternalSubtitleLoaded {
                            generation,
                            track,
                        });
                    }
                })
                .drop_on_shutdown()
        });
    }

    /// The separately loaded subtitles of the version being played (URLs).
    async fn find_external_subtitles(
        api_client: &ApiClient,
        item_id: uuid::Uuid,
        settings: crate::jellyfin_api::api::item::StreamSettings,
        transcoded: bool,
    ) -> Option<Vec<ExternalSubtitleTrack>> {
        {
            {
                let playback_info = match api_client.get_playback_info(&item_id).await {
                    Ok(playback_info) => playback_info,
                    Err(err) => {
                        warn!("Error getting playback info: {err}");
                        return None;
                    }
                };

                // The version being played (otherwise the first one).
                let Some(source) = playback_info
                    .media_sources
                    .iter()
                    .find(|source| {
                        settings.media_source_id.is_some() && source.id == settings.media_source_id
                    })
                    .or_else(|| playback_info.media_sources.first())
                    .cloned()
                else {
                    return None;
                };
                let source_id = source
                    .id
                    .clone()
                    .unwrap_or_else(|| item_id.simple().to_string());
                let default_index = source.default_subtitle_stream_index;

                let external_subtitles = source
                    .media_streams
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|stream| stream.is_text_subtitle_stream == Some(true))
                    .filter_map(|stream| {
                        let url = match (stream.is_external, stream.delivery_url, stream.index) {
                            (Some(true), Some(delivery_url), _) => {
                                // Strip leading slash
                                let delivery_url =
                                    delivery_url.strip_prefix('/').unwrap_or(&delivery_url);
                                api_client
                                    .root
                                    .join(delivery_url)
                                    .expect("Error getting external subtitle track URL")
                                    .to_string()
                            }
                            (Some(true), None, _) => return None,
                            (_, _, Some(index)) if transcoded => api_client.subtitle_url(
                                &item_id,
                                &source_id,
                                index,
                                stream.codec.as_deref(),
                            ),
                            _ => return None,
                        };
                        let name = stream
                            .display_title
                            .or(stream.language)
                            .unwrap_or(tr!("vp-subtitle-track-external").to_owned());
                        let is_default = stream.index.is_some() && stream.index == default_index;
                        Some(ExternalSubtitleTrack {
                            name,
                            url,
                            is_default,
                        })
                    })
                    .collect();

                Some(external_subtitles)
            }
        }
    }
}

async fn fetch(api_client: &ApiClient, url: &str) -> anyhow::Result<()> {
    let response = api_client.client.get(url).send().await?.error_for_status()?;
    response.bytes().await?;
    Ok(())
}
