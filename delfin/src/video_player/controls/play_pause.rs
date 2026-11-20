use std::{cell::RefCell, sync::Arc, time::Duration};

use gtk::prelude::*;
use relm4::{
    gtk,
    gtk::glib::{timeout_add, ControlFlow},
    ComponentParts, ComponentSender, SimpleComponent,
};

use crate::{
    tr, utils::message_broker::ResettableMessageBroker, video_player::backends::VideoPlayerBackend,
};

pub static PLAY_PAUSE_BROKER: ResettableMessageBroker<PlayPauseInput> =
    ResettableMessageBroker::new();

pub(crate) struct PlayPause {
    video_player: Arc<RefCell<dyn VideoPlayerBackend>>,
    loading: bool,
    playing: bool,
    first_click: bool,
    double_click_time: Duration,
}

#[derive(Debug)]
pub enum PlayPauseInput {
    TogglePlaying,
    SetLoading,
    SetPlaying(bool),
    /// Vom Nutzer (z. B. Medientasten) – geht bei SyncPlay an den Server.
    UserSetPlaying(bool),
    /// The player reports its state — display only, no command is sent back.
    /// (Previously this returned as `SetPlaying` and could overwrite a newly
    /// set SyncPlay pause.)
    StateChanged(bool),
    LeftClick,
    LeftClickTimeout,
}

#[relm4::component(pub(crate))]
impl SimpleComponent for PlayPause {
    type Init = Arc<RefCell<dyn VideoPlayerBackend>>;
    type Input = PlayPauseInput;
    type Output = ();

    view! {
        gtk::Button {
            set_focus_on_click: false,
            add_css_class: "player-play",

            #[watch]
            set_icon_name: if model.playing {
                "pause-filled"
            } else {
                "play-filled"
            },
            #[watch]
            set_tooltip_text: Some(tr!(
                "vp-play-pause-tooltip",
                {"playing" => model.playing.to_string()},
            )),
            #[watch]
            set_sensitive: !model.loading,
            connect_clicked[sender] => move |_| {
                sender.input(PlayPauseInput::TogglePlaying);
            },
        }
    }

    fn init(
        video_player: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let double_click_time = gtk::Settings::default()
            .map(|s| s.gtk_double_click_time())
            .unwrap_or(400);
        let model = PlayPause {
            video_player,
            loading: true,
            playing: true,
            first_click: false,
            double_click_time: Duration::from_millis(double_click_time as u64),
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            PlayPauseInput::TogglePlaying => {
                if crate::syncplay::intercept_play(!self.playing) {
                    return;
                }
                if self.playing {
                    self.video_player.borrow().pause();
                    self.playing = false;
                } else {
                    self.video_player.borrow().play();
                    self.playing = true;
                }
            }
            PlayPauseInput::UserSetPlaying(playing) => {
                if crate::syncplay::intercept_play(playing) {
                    return;
                }
                sender.input(PlayPauseInput::SetPlaying(playing));
            }
            PlayPauseInput::StateChanged(playing) => {
                self.playing = playing;
                self.loading = false;
            }
            PlayPauseInput::SetLoading => {
                self.loading = true;
            }
            PlayPauseInput::SetPlaying(playing) => {
                self.playing = playing;
                self.loading = false;
                if playing {
                    self.video_player.borrow().play();
                } else {
                    self.video_player.borrow().pause();
                }
            }
            PlayPauseInput::LeftClick => {
                if !self.first_click {
                    self.first_click = true;
                    _ = timeout_add(self.double_click_time, || {
                        PLAY_PAUSE_BROKER.send(PlayPauseInput::LeftClickTimeout);
                        ControlFlow::Break
                    });
                } else {
                    self.first_click = false;
                }
            }
            PlayPauseInput::LeftClickTimeout => {
                if self.first_click {
                    self.first_click = false;
                    sender.input(PlayPauseInput::TogglePlaying);
                }
            }
        }
    }
}
