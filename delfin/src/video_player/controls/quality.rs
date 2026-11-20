//! Qualitäts-Knopf im Player: Automatisch, Original oder feste Stufe, die der
//! Server umwandelt. Die Wahl gilt für die Sitzung (auch nächste Folgen).

use gtk::prelude::*;
use jellyfin_api::types::BaseItemDto;
use relm4::prelude::*;

use crate::{
    globals::STREAM_SETTINGS,
    jellyfin_api::{
        api::item::{is_disc, StreamQuality},
        api_client::ApiClient,
    },
    tr,
    video_player::{VideoPlayerInput, VIDEO_PLAYER_BROKER},
};

pub(crate) struct Quality {
    buttons: Vec<gtk::CheckButton>,
    /// Beim Setzen der Auswahl per Code keinen Wechsel auslösen.
    syncing: bool,
}

#[derive(Debug)]
pub(crate) enum QualityInput {
    /// Neuer Titel: Automatik-Beschreibung und Auswahl auffrischen.
    SetItem(Box<BaseItemDto>),
    Selected(usize),
}

#[relm4::component(pub(crate))]
impl Component for Quality {
    type Init = ();
    type Input = QualityInput;
    type Output = ();
    type CommandOutput = ();

    view! {
        gtk::MenuButton {
            set_icon_name: "video-display-symbolic",
            set_tooltip_text: Some(tr!("vp-quality-tooltip")),
            set_direction: gtk::ArrowType::Up,

            #[wrap(Some)]
            #[name = "popover"]
            set_popover = &gtk::Popover {
                #[name = "list"]
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 4,
                    set_margin_all: 6,

                    gtk::Label {
                        set_label: tr!("vp-quality-tooltip"),
                        add_css_class: "heading",
                        set_halign: gtk::Align::Start,
                        set_margin_bottom: 4,
                    },
                },
            },
        }
    }

    fn init(_: Self::Init, root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let mut model = Quality {
            buttons: vec![],
            syncing: false,
        };
        let widgets = view_output!();

        let mut group: Option<gtk::CheckButton> = None;
        for (index, quality) in StreamQuality::ALL.iter().enumerate() {
            let button = gtk::CheckButton::with_label(&quality_label(*quality, None));
            if let Some(group) = &group {
                button.set_group(Some(group));
            } else {
                group = Some(button.clone());
            }
            button.set_active(*quality == StreamQuality::default());
            button.connect_toggled({
                let sender = sender.clone();
                move |button| {
                    if button.is_active() {
                        sender.input(QualityInput::Selected(index));
                    }
                }
            });
            widgets.list.append(&button);
            model.buttons.push(button);
        }

        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            QualityInput::SetItem(item) => {
                let settings = STREAM_SETTINGS.read().clone();
                let auto_transcoded = ApiClient::is_transcoded(
                    &item,
                    &crate::jellyfin_api::api::item::StreamSettings {
                        quality: StreamQuality::Auto,
                        ..settings.clone()
                    },
                );
                self.syncing = true;
                for (button, quality) in self.buttons.iter().zip(StreamQuality::ALL) {
                    button.set_label(Some(&quality_label(quality, Some(auto_transcoded))));
                    // Disc-Abbilder lassen sich nicht als Datei abspielen.
                    if quality == StreamQuality::Original {
                        button.set_sensitive(!is_disc(&item));
                    }
                    if quality == settings.quality {
                        button.set_active(true);
                    }
                }
                self.syncing = false;
            }
            QualityInput::Selected(index) => {
                if self.syncing {
                    return;
                }
                if let Some(quality) = StreamQuality::ALL.get(index) {
                    VIDEO_PLAYER_BROKER.send(VideoPlayerInput::SetQuality(*quality));
                }
                widgets.popover.popdown();
            }
        }
    }
}

fn quality_label(quality: StreamQuality, auto_transcoded: Option<bool>) -> String {
    match quality {
        StreamQuality::Auto => match auto_transcoded {
            Some(true) => format!("{} ({})", tr!("vp-quality-auto"), tr!("vp-quality-auto-server")),
            Some(false) => format!("{} ({})", tr!("vp-quality-auto"), tr!("vp-quality-auto-direct")),
            None => tr!("vp-quality-auto").to_string(),
        },
        StreamQuality::Original => tr!("vp-quality-original").to_string(),
        StreamQuality::P1080 => "1080p · 20 Mbit/s".to_string(),
        StreamQuality::P720 => "720p · 8 Mbit/s".to_string(),
        StreamQuality::P480 => "480p · 3 Mbit/s".to_string(),
    }
}
