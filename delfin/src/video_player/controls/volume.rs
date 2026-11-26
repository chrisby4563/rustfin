use std::{cell::RefCell, sync::Arc, time::Duration};

use gtk::prelude::*;
use relm4::{gtk, ComponentParts, SimpleComponent};

use crate::{
    globals::CONFIG,
    tr,
    utils::{debounce::Debounce, message_broker::ResettableMessageBroker},
    video_player::backends::VideoPlayerBackend,
};

/// Mouse wheel notch, arrow keys on the slider.
const VOLUME_STEP: f64 = 0.05;

pub static VOLUME_BROKER: ResettableMessageBroker<VolumeInput> = ResettableMessageBroker::new();

pub struct Volume {
    video_player: Arc<RefCell<dyn VideoPlayerBackend>>,
    muted: bool,
    volume: f64,
    save_debounce: Debounce,
}

#[derive(Debug)]
pub enum VolumeInput {
    // Update displayed values
    MuteUpdated(bool),
    VolumeUpdated(f64),
    // Modify player volume/mute
    ToggleMute,
    SetVolume(f64),
    ChangeVolume(f64),

    SaveSettings,
    LoadSettings,
}

#[relm4::component(pub)]
impl SimpleComponent for Volume {
    type Init = Arc<RefCell<dyn VideoPlayerBackend>>;
    type Input = VolumeInput;
    type Output = ();

    view! {
        gtk::Box {
            gtk::Separator { add_css_class: "spacer" },

            // The slider sits in a popover so it is there in every window
            // width (inline it had to make room for the other buttons).
            gtk::MenuButton {
                set_focus_on_click: false,
                set_direction: gtk::ArrowType::Up,
                #[watch]
                set_icon_name: model.icon_name(),
                #[watch]
                set_tooltip_text: Some(tr!(
                    "vp-volume-tooltip",
                    {"percent" => model.percent()},
                )),

                // Mouse wheel over the button changes the volume directly.
                add_controller = gtk::EventControllerScroll {
                    set_flags: gtk::EventControllerScrollFlags::VERTICAL,
                    connect_scroll[sender] => move |_, _, dy| {
                        sender.input(VolumeInput::ChangeVolume(-dy * VOLUME_STEP));
                        gtk::glib::Propagation::Stop
                    },
                },

                #[wrap(Some)]
                set_popover = &gtk::Popover {
                    add_css_class: "volume-popover",

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 6,

                        gtk::Label {
                            add_css_class: "numeric",
                            #[watch]
                            set_label: &format!("{} %", model.percent()),
                        },

                        gtk::Scale {
                            set_orientation: gtk::Orientation::Vertical,
                            set_inverted: true,
                            set_focus_on_click: false,
                            set_range: (0.0, 1.0),
                            set_increments: (VOLUME_STEP, 0.1),
                            set_height_request: 160,
                            set_halign: gtk::Align::Center,
                            #[watch]
                            #[block_signal(volume_changed_handler)]
                            set_value: model.volume,
                            connect_value_changed[sender] => move |scale| {
                                sender.input(VolumeInput::SetVolume(scale.value()));
                            } @volume_changed_handler,
                        },

                        gtk::Button {
                            set_focus_on_click: false,
                            add_css_class: "flat",
                            #[watch]
                            set_icon_name: if model.muted {
                                "audio-volume-muted"
                            } else {
                                "audio-volume-high"
                            },
                            #[watch]
                            set_tooltip_text: Some(tr!(
                                "vp-volume-mute-tooltip",
                                {"muted" => model.muted.to_string()},
                            )),
                            connect_clicked[sender] => move |_| {
                                sender.input(VolumeInput::ToggleMute);
                            },
                        },
                    },
                },
            },

            gtk::Separator { add_css_class: "spacer" },
        }
    }

    fn init(
        video_player: Self::Init,
        root: Self::Root,
        sender: relm4::ComponentSender<Self>,
    ) -> relm4::ComponentParts<Self> {
        let config = CONFIG.read();

        let save_debounce = Debounce::new(
            Duration::from_secs(3),
            Box::new({
                let sender = sender.clone();
                move || {
                    sender.input(VolumeInput::SaveSettings);
                }
            }),
        );

        let model = Volume {
            video_player: video_player.clone(),
            muted: config.video_player.muted,
            volume: config.video_player.volume,
            save_debounce,
        };

        let widgets = view_output!();

        model.video_player.borrow_mut().connect_mute_updated({
            let sender = sender.clone();
            Box::new(move |muted| {
                sender.input(VolumeInput::MuteUpdated(muted));
            })
        });

        model
            .video_player
            .borrow_mut()
            .connect_volume_updated(Box::new(move |volume| {
                sender.input(VolumeInput::VolumeUpdated(volume));
            }));

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: relm4::ComponentSender<Self>) {
        match message {
            VolumeInput::MuteUpdated(muted) => self.muted = muted,
            VolumeInput::VolumeUpdated(volume) => self.volume = volume,
            VolumeInput::ToggleMute => {
                self.muted = !self.muted;
                self.video_player.borrow().set_muted(self.muted);
                self.save_debounce.debounce();
            }
            VolumeInput::SetVolume(volume) => {
                self.unmute();
                self.volume = volume;
                self.video_player.borrow().set_volume(volume);
                self.save_debounce.debounce();
            }
            VolumeInput::ChangeVolume(amount) => {
                self.unmute();
                let volume = f64::clamp(self.volume + amount, 0.0, 1.0);
                self.volume = volume;
                self.video_player.borrow().set_volume(volume);
                self.save_debounce.debounce();
            }
            VolumeInput::SaveSettings => {
                self.save_settings();
            }
            VolumeInput::LoadSettings => {
                let config = CONFIG.read();
                self.volume = config.video_player.volume;
                self.muted = config.video_player.muted;
                self.video_player.borrow().set_muted(self.muted);
                self.video_player.borrow().set_volume(self.volume);
            }
        }
    }

    fn shutdown(&mut self, _widgets: &mut Self::Widgets, _output: relm4::Sender<Self::Output>) {
        self.save_settings();
    }
}

impl Volume {
    fn icon_name(&self) -> &'static str {
        if self.muted || self.volume <= 0.0 {
            "audio-volume-muted"
        } else if self.volume < 0.34 {
            "audio-volume-low"
        } else if self.volume < 0.67 {
            "audio-volume-medium"
        } else {
            "audio-volume-high"
        }
    }

    fn percent(&self) -> i64 {
        (self.volume * 100.0).round() as i64
    }

    /// Changing the volume while muted makes it audible again.
    fn unmute(&mut self) {
        if self.muted {
            self.muted = false;
            self.video_player.borrow().set_muted(false);
        }
    }

    fn save_settings(&self) {
        let mut config = CONFIG.write();
        config.video_player.volume = self.volume;
        config.video_player.muted = self.muted;
        if let Err(err) = config.save() {
            tracing::error!("Failed to save volume settings: {err}");
        }
    }
}
