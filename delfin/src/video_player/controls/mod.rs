mod audio_tracks;
pub(crate) mod chapters;
pub(super) mod fullscreen;
mod mini_player;
pub(super) mod next_prev_episode;
pub(super) mod play_pause;
pub(super) mod playback_speed;
pub(crate) mod quality;
pub(super) mod scrubber;
pub(super) mod skip_forwards_backwards;
pub(super) mod subtitles;
pub(super) mod video_player_menu;
pub(super) mod volume;

use std::{
    cell::{OnceCell, RefCell},
    sync::Arc,
};

use crate::{
    app::APP_BROKER,
    jellyfin_api::api_client::ApiClient,
    video_player::{
        backends::VideoPlayerBackend,
        controls::{
            fullscreen::FULLSCREEN_BROKER,
            next_prev_episode::{
                NextPrevEpisodeDirection, NextPrevEpisodeOutput, NEXT_EPISODE_BROKER,
                PREV_EPISODE_BROKER,
            },
            play_pause::PLAY_PAUSE_BROKER,
            scrubber::SCRUBBER_BROKER,
            skip_forwards_backwards::{
                SkipForwardsBackwardsDirection, SKIP_BACKWARDS_BROKER, SKIP_FORWARDS_BROKER,
            },
            subtitles::SUBTITLES_BROKER,
            volume::VOLUME_BROKER,
        },
        next_up::NEXT_UP_VISIBILE,
    },
};
use gtk::{glib, prelude::*};
use jellyfin_api::types::BaseItemDto;
use playback_speed::PLAYBACK_SPEED_BROKER;
use relm4::{gtk, Component, ComponentController, ComponentParts, Controller, SimpleComponent};
use video_player_menu::{VideoPlayerMenu, VIDEO_PLAYER_MENU_BROKER};

use self::{
    audio_tracks::{AudioTracks, AudioTracksInput},
    chapters::{Chapters, CHAPTERS_BROKER},
    fullscreen::Fullscreen,
    mini_player::MiniPlayerButton,
    next_prev_episode::{NextPrevEpisode, NextPrevEpisodeInput},
    play_pause::PlayPause,
    playback_speed::PlaybackSpeed,
    quality::{Quality, QualityInput},
    scrubber::Scrubber,
    skip_forwards_backwards::SkipForwardsBackwards,
    subtitles::{Subtitles, SubtitlesInput},
    volume::Volume,
};

pub struct VideoPlayerControls {
    show_controls: bool,
    next_prev_episodes: (Option<BaseItemDto>, Option<BaseItemDto>),
    // We need to keep these controllers around, even if we don't read them
    scrubber: Option<Controller<Scrubber>>,
    play_pause: Option<Controller<PlayPause>>,
    skip_forwards_backwards: OnceCell<(
        Controller<SkipForwardsBackwards>,
        Controller<SkipForwardsBackwards>,
    )>,
    next_prev_episode_controls:
        OnceCell<(Controller<NextPrevEpisode>, Controller<NextPrevEpisode>)>,
    volume: Option<Controller<Volume>>,
    subtitles: OnceCell<Controller<Subtitles>>,
    audio_tracks: OnceCell<Controller<AudioTracks>>,
    chapters: OnceCell<Controller<Chapters>>,
    quality: OnceCell<Controller<Quality>>,
    mini_player: OnceCell<Controller<MiniPlayerButton>>,
    fullscreen: Option<Controller<Fullscreen>>,
    playback_speed: OnceCell<Controller<PlaybackSpeed>>,
    menu: OnceCell<Controller<VideoPlayerMenu>>,
}

pub struct VideoPlayerControlsInit {
    pub player: Arc<RefCell<dyn VideoPlayerBackend>>,
    pub default_show_controls: bool,
}

#[derive(Debug)]
pub enum VideoPlayerControlsInput {
    Noop,
    SetShowControls(bool),
    SetPlaying {
        api_client: Arc<ApiClient>,
        item: Box<BaseItemDto>,
    },
    SetNextPreviousEpisodes(Box<Option<BaseItemDto>>, Box<Option<BaseItemDto>>),
    PlayPreviousEpisode,
    PlayNextEpisode,
    RevealerClicked,
}

#[derive(Debug)]
pub enum VideoPlayerControlsOutput {
    ShowControls,
}

#[relm4::component(pub)]
impl SimpleComponent for VideoPlayerControls {
    type Init = VideoPlayerControlsInit;
    type Input = VideoPlayerControlsInput;
    type Output = VideoPlayerControlsOutput;

    view! {
        gtk::Revealer {
            #[watch]
            set_reveal_child: model.show_controls,
            set_transition_type: gtk::RevealerTransitionType::Crossfade,
            set_valign: gtk::Align::End,

            add_controller = gtk::GestureClick {
                connect_released[sender] => move |_, _, _, _| {
                    sender.input(VideoPlayerControlsInput::RevealerClicked);
                },
            },

            #[name = "controls"]
            #[wrap(Some)]
            set_child = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                add_css_class: "toolbar",
                add_css_class: "osd",
                add_css_class: "video-player-controls",

                #[name = "second_row"]
                gtk::Box {},
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: relm4::ComponentSender<Self>,
    ) -> relm4::ComponentParts<Self> {
        let VideoPlayerControlsInit {
            player,
            default_show_controls,
        } = init;

        let mut model = VideoPlayerControls {
            show_controls: default_show_controls,
            skip_forwards_backwards: OnceCell::new(),
            next_prev_episodes: (None, None),
            scrubber: None,
            play_pause: None,
            next_prev_episode_controls: OnceCell::new(),
            volume: None,
            subtitles: OnceCell::new(),
            audio_tracks: OnceCell::new(),
            chapters: OnceCell::new(),
            quality: OnceCell::new(),
            mini_player: OnceCell::new(),
            fullscreen: None,
            playback_speed: OnceCell::new(),
            menu: OnceCell::new(),
        };

        let widgets = view_output!();
        let controls = &widgets.controls;
        let second_row = &widgets.second_row;

        FULLSCREEN_BROKER.reset();
        PLAY_PAUSE_BROKER.reset();
        SCRUBBER_BROKER.reset();
        SKIP_BACKWARDS_BROKER.reset();
        SKIP_FORWARDS_BROKER.reset();
        PREV_EPISODE_BROKER.reset();
        NEXT_EPISODE_BROKER.reset();
        SUBTITLES_BROKER.reset();
        CHAPTERS_BROKER.reset();
        VOLUME_BROKER.reset();
        VIDEO_PLAYER_MENU_BROKER.reset();

        let scrubber = Scrubber::builder()
            .launch_with_broker(player.clone(), &SCRUBBER_BROKER.read())
            .detach();
        controls.prepend(scrubber.widget());
        model.scrubber = Some(scrubber);

        let prev_episode = NextPrevEpisode::builder()
            .launch_with_broker(
                NextPrevEpisodeDirection::Previous,
                &PREV_EPISODE_BROKER.read(),
            )
            .forward(sender.input_sender(), |msg| match msg {
                NextPrevEpisodeOutput::Play => VideoPlayerControlsInput::PlayPreviousEpisode,
            });
        second_row.append(prev_episode.widget());

        let skip_backwards = SkipForwardsBackwards::builder()
            .launch_with_broker(
                (SkipForwardsBackwardsDirection::Backwards, player.clone()),
                &SKIP_BACKWARDS_BROKER.read(),
            )
            .detach();
        second_row.append(skip_backwards.widget());

        let play_pause = PlayPause::builder()
            .launch_with_broker(player.clone(), &PLAY_PAUSE_BROKER.read())
            .detach();
        second_row.append(play_pause.widget());
        model.play_pause = Some(play_pause);

        let skip_forwards = SkipForwardsBackwards::builder()
            .launch_with_broker(
                (SkipForwardsBackwardsDirection::Forwards, player.clone()),
                &SKIP_FORWARDS_BROKER.read(),
            )
            .detach();
        second_row.append(skip_forwards.widget());
        model
            .skip_forwards_backwards
            .set((skip_backwards, skip_forwards))
            .unwrap();

        let next_episode = NextPrevEpisode::builder()
            .launch_with_broker(NextPrevEpisodeDirection::Next, &NEXT_EPISODE_BROKER.read())
            .forward(sender.input_sender(), |msg| match msg {
                NextPrevEpisodeOutput::Play => VideoPlayerControlsInput::PlayNextEpisode,
            });
        second_row.append(next_episode.widget());
        model
            .next_prev_episode_controls
            .set((prev_episode, next_episode))
            .unwrap();

        // Push remaining controls to end
        second_row.append(&gtk::Box::builder().hexpand(true).build());

        let menu = VideoPlayerMenu::builder()
            .launch_with_broker(player.clone(), &VIDEO_PLAYER_MENU_BROKER.read())
            .detach();
        second_row.append(menu.widget());
        model.menu.set(menu).unwrap();

        let chapters = Chapters::builder()
            .launch_with_broker(player.clone(), &CHAPTERS_BROKER.read())
            .detach();
        second_row.append(chapters.widget());
        let _ = model.chapters.set(chapters);

        let quality = Quality::builder().launch(()).detach();
        second_row.append(quality.widget());
        let _ = model.quality.set(quality);

        let playback_speed = PlaybackSpeed::builder()
            .launch_with_broker(player.clone(), &PLAYBACK_SPEED_BROKER.read())
            .detach();
        second_row.append(playback_speed.widget());
        model.playback_speed.set(playback_speed).unwrap();

        let subtitles = Subtitles::builder()
            .launch_with_broker(player.clone(), &SUBTITLES_BROKER.read())
            .detach();
        second_row.append(subtitles.widget());
        model.subtitles.set(subtitles).unwrap();

        let audio_tracks = AudioTracks::builder().launch(player.clone()).detach();
        second_row.append(audio_tracks.widget());
        model.audio_tracks.set(audio_tracks).unwrap();

        let volume = Volume::builder()
            .launch_with_broker(player.clone(), &VOLUME_BROKER.read())
            .detach();
        second_row.append(volume.widget());
        model.volume = Some(volume);

        let mut syncplay = crate::syncplay::ui::SyncPlayButton::builder()
            .launch(())
            .detach();
        syncplay.widget().set_direction(gtk::ArrowType::Up);
        second_row.append(syncplay.widget());
        syncplay.detach_runtime();

        let mini_player = MiniPlayerButton::builder().launch(()).detach();
        second_row.append(mini_player.widget());
        let _ = model.mini_player.set(mini_player);

        let fullscreen = Fullscreen::builder()
            .launch_with_broker((), &FULLSCREEN_BROKER.read())
            .detach();
        second_row.append(fullscreen.widget());
        model.fullscreen = Some(fullscreen);

        fit_to_width(&second_row);

        NEXT_UP_VISIBILE.subscribe(sender.input_sender(), |visible| {
            if *visible {
                VideoPlayerControlsInput::SetShowControls(false)
            } else {
                VideoPlayerControlsInput::Noop
            }
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: relm4::ComponentSender<Self>) {
        match message {
            VideoPlayerControlsInput::SetShowControls(show_controls) => {
                self.show_controls = show_controls;
            }
            VideoPlayerControlsInput::SetPlaying { api_client, item } => {
                if let Some(quality) = self.quality.get() {
                    quality.emit(QualityInput::SetItem(item.clone()));
                }
                if let Some(audio_tracks) = self.audio_tracks.get() {
                    audio_tracks.emit(AudioTracksInput::Reset {
                        api_client: api_client.clone(),
                        item: item.clone(),
                    });
                }
                if let Some(subtitles) = self.subtitles.get() {
                    subtitles.emit(SubtitlesInput::Reset { api_client, item });
                }
            }
            VideoPlayerControlsInput::SetNextPreviousEpisodes(prev, next) => {
                if let Some((prev_control, next_control)) = self.next_prev_episode_controls.get() {
                    prev_control.emit(if prev.is_some() {
                        NextPrevEpisodeInput::Show
                    } else {
                        NextPrevEpisodeInput::Hide
                    });
                    next_control.emit(if next.is_some() {
                        NextPrevEpisodeInput::Show
                    } else {
                        NextPrevEpisodeInput::Hide
                    });
                }

                self.next_prev_episodes = (*prev, *next);
            }
            VideoPlayerControlsInput::PlayPreviousEpisode => {
                if let (Some(previous), _) = &self.next_prev_episodes {
                    APP_BROKER.send(crate::app::AppInput::PlayVideo(previous.clone()));
                }
            }
            VideoPlayerControlsInput::PlayNextEpisode => {
                if let (_, Some(next)) = &self.next_prev_episodes {
                    APP_BROKER.send(crate::app::AppInput::PlayVideo(next.clone()));
                }
            }
            VideoPlayerControlsInput::Noop => {}
            VideoPlayerControlsInput::RevealerClicked => {
                if !self.show_controls {
                    sender
                        .output(VideoPlayerControlsOutput::ShowControls)
                        .unwrap();
                }
            }
        }
    }
}

/// In narrow windows the row is wider than the window and the right-hand
/// buttons are cut off. Then hide the volume slider (the mute button stays)
/// and the label of the “Together” button (its icon stays); show them again
/// once there is room.
fn fit_to_width(row: &gtk::Box) {
    let optional: Vec<gtk::Widget> = descendants(row.upcast_ref())
        .into_iter()
        .filter(|widget| {
            widget.is::<gtk::Scale>()
                || (widget.is::<gtk::Label>()
                    && widget
                        .ancestor(gtk::MenuButton::static_type())
                        .is_some_and(|button| button.has_css_class("header-pill")))
        })
        .collect();
    if optional.is_empty() {
        return;
    }
    // Minimum width with everything shown, measured while shown.
    let full_width = std::cell::Cell::new(0);
    let last_available = std::cell::Cell::new(-1);
    row.add_tick_callback(move |row, _| {
        // The controls span the window; their own allocation grows with the
        // overflowing row, so compare with the window instead.
        let (Some(root), Some(controls)) = (row.root(), row.parent()) else {
            return glib::ControlFlow::Continue;
        };
        let available = root.width();
        if available <= 0 || available == last_available.get() {
            return glib::ControlFlow::Continue;
        }
        last_available.set(available);
        let shown = optional[0].is_visible();
        if shown {
            // Including the padding of the controls box.
            full_width.set(controls.measure(gtk::Orientation::Horizontal, -1).0);
        }
        let fits = available >= full_width.get();
        if fits != shown {
            for widget in &optional {
                widget.set_visible(fits);
            }
        }
        glib::ControlFlow::Continue
    });
}

fn descendants(widget: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut out = Vec::new();
    let mut child = widget.first_child();
    while let Some(current) = child {
        out.push(current.clone());
        out.extend(descendants(&current));
        child = current.next_sibling();
    }
    out
}
