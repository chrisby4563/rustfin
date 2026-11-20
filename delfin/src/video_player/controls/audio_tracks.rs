use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

use gtk::prelude::*;
use relm4::{
    actions::{ActionGroupName, ActionName, RelmAction, RelmActionGroup},
    gtk::{self, gio},
    Component, ComponentParts,
};

use jellyfin_api::types::{BaseItemDto, MediaStreamType};
use tracing::warn;

use crate::{
    globals::CONFIG,
    jellyfin_api::api_client::ApiClient,
    tr,
    video_player::{
        backends::{AudioTrack, VideoPlayerBackend},
        VideoPlayerInput, VIDEO_PLAYER_BROKER,
    },
};

relm4::new_action_group!(AudioTracksActionGroup, "audio_tracks_actions");
relm4::new_stateful_action!(
    SelectedAudioTrackAction,
    AudioTracksActionGroup,
    "selected_audio_track",
    Option<i32>,
    Option<i32>
);

#[derive(Debug)]
pub struct AudioTracks {
    video_player: Arc<RefCell<dyn VideoPlayerBackend>>,
    menu: gio::Menu,
    audio_tracks_available: bool,
    /// Vom Server umgewandelter Stream: Spuren kommen vom Server, ein Wechsel
    /// fordert den Stream neu an (statt mpv-Spur umzuschalten).
    server_mode: Rc<Cell<bool>>,
    /// Do not switch tracks while setting the preselection.
    suppress_switch: Rc<Cell<bool>>,
}

#[derive(Debug)]
pub enum AudioTracksInput {
    Reset {
        api_client: Arc<ApiClient>,
        item: Box<BaseItemDto>,
    },
    AudioTracksUpdated(Vec<AudioTrack>),
}

#[derive(Debug)]
pub enum AudioTracksCommandOutput {
    /// (server index, name), the selected track, and whether it must be switched
    /// to (the server preselection was not the preferred language).
    ServerTracksLoaded(Vec<(i32, String)>, Option<i32>, bool),
}

#[relm4::component(pub)]
impl Component for AudioTracks {
    type Init = Arc<RefCell<dyn VideoPlayerBackend>>;
    type Input = AudioTracksInput;
    type Output = ();
    type CommandOutput = AudioTracksCommandOutput;

    view! {
        gtk::MenuButton {
            set_icon_name: "music-note-single",
            set_menu_model: Some(&model.menu),
            set_focus_on_click: false,
            set_direction: gtk::ArrowType::Up,
            set_tooltip_text: Some(tr!("vp-audio-track-tooltip")),
            #[watch]
            set_visible: model.audio_tracks_available,
        }
    }

    fn init(
        video_player: Self::Init,
        root: Self::Root,
        sender: relm4::ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = AudioTracks {
            video_player,
            menu: gio::Menu::new(),
            audio_tracks_available: false,
            server_mode: Rc::new(Cell::new(false)),
            suppress_switch: Rc::new(Cell::new(false)),
        };

        model
            .video_player
            .borrow_mut()
            .connect_audio_tracks_updated(Box::new(move |tracks| {
                sender.input(AudioTracksInput::AudioTracksUpdated(tracks.clone()));
            }));

        let widgets = view_output!();

        let selected_audio_track_action: RelmAction<SelectedAudioTrackAction> =
            RelmAction::new_stateful_with_target_value(&None, {
                let video_player = model.video_player.clone();
                let server_mode = model.server_mode.clone();
                let suppress_switch = model.suppress_switch.clone();
                move |_, state, value: Option<i32>| {
                    let changed = *state != value;
                    *state = value;
                    if server_mode.get() {
                        if suppress_switch.get() {
                            return;
                        }
                        if let (true, Some(index)) = (changed, value) {
                            VIDEO_PLAYER_BROKER
                                .send(VideoPlayerInput::SwitchServerAudioTrack(index));
                        }
                    } else {
                        video_player
                            .borrow()
                            .set_audio_track(value.map(|id| id as usize));
                    }
                }
            });

        let mut group = RelmActionGroup::<AudioTracksActionGroup>::new();
        group.add_action(selected_audio_track_action);
        group.register_for_widget(root);

        ComponentParts { model, widgets }
    }

    fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        _sender: relm4::ComponentSender<Self>,
        root: &Self::Root,
    ) {
        match message {
            AudioTracksCommandOutput::ServerTracksLoaded(tracks, default, switch) => {
                if let (true, Some(index)) = (switch, default) {
                    VIDEO_PLAYER_BROKER.send(VideoPlayerInput::SwitchServerAudioTrack(index));
                }
                if tracks.len() < 2 {
                    return;
                }
                let menu = gio::Menu::new();
                for (index, name) in &tracks {
                    menu.append_item(
                        &RelmAction::<SelectedAudioTrackAction>::to_menu_item_with_target_value(
                            name,
                            &Some(*index),
                        ),
                    );
                }
                self.menu.remove_all();
                self.menu
                    .append_section(Some(tr!("vp-audio-track-menu")), &menu);
                self.audio_tracks_available = true;

                // Set the preselection without triggering a switch.
                let selected = default.or(tracks.first().map(|(index, _)| *index));
                self.suppress_switch.set(true);
                let _ = root.activate_action(
                    &format!(
                        "{}.{}",
                        AudioTracksActionGroup::NAME,
                        SelectedAudioTrackAction::NAME
                    ),
                    Some(&selected.to_variant()),
                );
                self.suppress_switch.set(false);
            }
        }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: relm4::ComponentSender<Self>,
        root: &Self::Root,
    ) {
        match message {
            AudioTracksInput::Reset { api_client, item } => {
                self.audio_tracks_available = false;
                self.menu.remove_all();
                let settings = crate::globals::STREAM_SETTINGS.read().clone();
                let server_mode = ApiClient::is_transcoded(&item, &settings);
                self.server_mode.set(server_mode);
                if let (true, Some(item_id)) = (server_mode, item.id) {
                    sender.oneshot_command(async move {
                        let info = match api_client.get_playback_info(&item_id).await {
                            Ok(info) => info,
                            Err(err) => {
                                warn!("Error getting audio streams: {err}");
                                return AudioTracksCommandOutput::ServerTracksLoaded(
                                    vec![],
                                    None,
                                    false,
                                );
                            }
                        };
                        let source = match &settings.media_source_id {
                            Some(id) => info
                                .media_sources
                                .into_iter()
                                .find(|s| s.id.as_ref() == Some(id)),
                            None => info.media_sources.into_iter().next(),
                        };
                        let default = source.as_ref().and_then(|s| s.default_audio_stream_index);
                        // Already selected track (after a quality change): retain it.
                        if let Some(chosen) = settings.audio_stream_index {
                            let tracks = source
                                .and_then(|s| s.media_streams)
                                .unwrap_or_default()
                                .into_iter()
                                .filter(|stream| stream.type_ == Some(MediaStreamType::Audio))
                                .filter_map(|stream| {
                                    let name =
                                        stream.display_title.or(stream.language).unwrap_or_else(
                                            || format!("#{}", stream.index.unwrap_or(0)),
                                        );
                                    Some((stream.index?, name))
                                })
                                .collect();
                            return AudioTracksCommandOutput::ServerTracksLoaded(
                                tracks,
                                Some(chosen),
                                false,
                            );
                        }
                        let audio_streams: Vec<_> = source
                            .and_then(|s| s.media_streams)
                            .unwrap_or_default()
                            .into_iter()
                            .filter(|stream| stream.type_ == Some(MediaStreamType::Audio))
                            .collect();

                        // Preferred language: if the server-selected track does
                        // not speak it but another does, switch to that track.
                        let preferred = CONFIG.read().video_player.preferred_audio_language_list();
                        let lang_rank = |stream: &jellyfin_api::types::MediaStream| {
                            let lang = stream.language.clone().unwrap_or_default().to_lowercase();
                            preferred.iter().position(|p| *p == lang)
                        };
                        let default_rank = audio_streams
                            .iter()
                            .find(|s| s.index == default)
                            .and_then(lang_rank);
                        let best = audio_streams
                            .iter()
                            .filter_map(|s| Some((lang_rank(s)?, s.index?)))
                            .min_by_key(|(rank, _)| *rank);
                        let mut selected = default;
                        let mut switch = false;
                        if let Some((rank, index)) = best {
                            if default_rank.map_or(true, |d| rank < d) && Some(index) != default {
                                selected = Some(index);
                                switch = true;
                            }
                        }

                        let tracks = audio_streams
                            .into_iter()
                            .filter_map(|stream| {
                                let name = stream
                                    .display_title
                                    .or(stream.language)
                                    .unwrap_or_else(|| format!("#{}", stream.index.unwrap_or(0)));
                                Some((stream.index?, name))
                            })
                            .collect();
                        AudioTracksCommandOutput::ServerTracksLoaded(tracks, selected, switch)
                    });
                }
            }
            AudioTracksInput::AudioTracksUpdated(audio_tracks) => {
                if audio_tracks.len() < 2 || self.server_mode.get() {
                    return;
                }

                self.audio_tracks_available = true;

                // Build updated audio tracks menu
                let audio_tracks_menu = gio::Menu::new();
                audio_tracks
                    .iter()
                    .map(|audio_track| {
                        RelmAction::<SelectedAudioTrackAction>::to_menu_item_with_target_value(
                            &audio_track.name,
                            &Some(audio_track.id as i32),
                        )
                    })
                    .for_each(|menu_item| audio_tracks_menu.append_item(&menu_item));

                self.menu.remove_all();
                self.menu
                    .append_section(Some(tr!("vp-audio-track-menu")), &audio_tracks_menu);

                // Select current audio track in menu
                if let Some(current_audio_track) = self.video_player.borrow().current_audio_track()
                {
                    root.activate_action(
                        &format!(
                            "{}.{}",
                            AudioTracksActionGroup::NAME,
                            SelectedAudioTrackAction::NAME
                        ),
                        Some(&Some(current_audio_track as i32).to_variant()),
                    )
                    .expect("Error selecting current audio track.");
                }
            }
        }

        self.update_view(widgets, sender);
    }
}
