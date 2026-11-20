use std::{cell::OnceCell, collections::VecDeque, sync::Arc};

use adw::prelude::*;
use gdk::Texture;
use gtk::{gdk, gdk_pixbuf};
use jellyfin_api::types::BaseItemDto;
use relm4::{
    gtk::gdk_pixbuf::{InterpType, Pixbuf},
    prelude::*,
};

use crate::{
    app::{AppInput, APP_BROKER},
    jellyfin_api::api_client::ApiClient,
    library::LIBRARY_REFRESH_QUEUED,
    media_details::watched_state::{watched_label, Played},
    tr,
};

use super::{watched_state::toggle_watched, MediaDetailsInput, MEDIA_DETAILS_BROKER};

const EPISODE_THUMBNAIL_SIZE: i32 = 75;

pub(crate) struct Episode {
    media: BaseItemDto,
    api_client: Arc<ApiClient>,
    thumbnail: OnceCell<Controller<EpisodeThumbnail>>,
    /// Medieninfo schon geladen (Popover am Info-Knopf).
    info_loaded: bool,
}

#[derive(Debug)]
pub(crate) enum EpisodeInput {
    ToggleWatched(bool),
    LoadMediaInfo,
}

#[relm4::component(pub(crate) async)]
impl AsyncComponent for Episode {
    type Init = (BaseItemDto, Arc<ApiClient>);
    type Input = EpisodeInput;
    type Output = ();
    type CommandOutput = ();

    view! {
        adw::ActionRow {
            set_title: &title,
            set_title_lines: 1,
            set_use_markup: false,

            set_subtitle: if let Some(overview) = &model.media.overview {
                // Limit description to first paragraph, otherwise ActionRow
                // doesn't ellipsize the text properly
                overview.split_once('\n').unwrap_or((overview, "")).0
            } else { "" },
            set_subtitle_lines: 3,

            // Medieninfo der Folge (Bild, Ton, Untertitel, Datei).
            add_suffix = &gtk::MenuButton {
                set_icon_name: "help-about-symbolic",
                set_css_classes: &["image-button", "flat"],
                set_valign: gtk::Align::Center,
                set_tooltip: tr!("media-details-media-info"),

                #[wrap(Some)]
                #[name = "info_popover"]
                set_popover = &gtk::Popover {
                    set_width_request: 420,
                    connect_show[sender] => move |_| {
                        sender.input(EpisodeInput::LoadMediaInfo);
                    },
                    gtk::Spinner {
                        set_spinning: true,
                        set_margin_all: 12,
                    },
                },
            },

            add_suffix = &gtk::ToggleButton {
                set_icon_name: "eye-open-negative-filled",
                set_css_classes: &["image-button", "flat", "btn-watched"],
                set_valign: gtk::Align::Center,
                #[watch]
                set_tooltip: &watched_label(model.media.played()),

                #[watch]
                #[block_signal(toggle_handler)]
                set_active: model.media.played(),
                connect_toggled[sender] => move |btn| {
                    sender.input(EpisodeInput::ToggleWatched(btn.is_active()));
                } @toggle_handler,
            },

            set_activatable: true,
            connect_activated[media] => move |_| {
                APP_BROKER.send(AppInput::PlayVideo(media.clone()));
            },
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let (media, api_client) = init;

        let model = Self {
            media: media.clone(),
            api_client: api_client.clone(),
            thumbnail: OnceCell::new(),
            info_loaded: false,
        };

        let title = match (&model.media.index_number, &model.media.name) {
            (Some(index_number), Some(name)) => format!("{index_number}. {name}"),
            (_, Some(name)) => name.clone(),
            _ => tr!("media-details-unnamed-episode").to_string(),
        };

        let widgets = view_output!();

        let thumbnail = EpisodeThumbnail::builder()
            .launch((media, api_client))
            .detach();
        root.add_prefix(thumbnail.widget());

        model.thumbnail.set(thumbnail).unwrap();

        AsyncComponentParts { model, widgets }
    }

    async fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            EpisodeInput::LoadMediaInfo => {
                if self.info_loaded {
                    return;
                }
                let Some(id) = self.media.id else {
                    return;
                };
                let child: gtk::Widget = match self.api_client.get_item(&id).await {
                    Ok(item) => super::media_info::media_info_widget(&item, true)
                        .unwrap_or_else(|| gtk::Label::new(Some("–")).upcast()),
                    Err(err) => gtk::Label::new(Some(&err.to_string())).upcast(),
                };
                widgets.info_popover.set_child(Some(&child));
                self.info_loaded = true;
            }
            EpisodeInput::ToggleWatched(watched) => {
                self.media.user_data =
                    match toggle_watched(&self.media, &self.api_client, watched).await {
                        Ok(user_data) => Some(user_data),
                        Err(err) => {
                            tracing::error!(
                                "Failed to mark episode as {}: {err}",
                                watched_label(watched)
                            );
                            APP_BROKER.send(AppInput::Toast(
                                tr!("media-details-toggle-watched-error", {
                                    "type" => "episode",
                                    "watched" => watched.to_string(),
                                })
                                .to_owned(),
                                None,
                            ));
                            self.update_view(widgets, sender);
                            return;
                        }
                    };
                *LIBRARY_REFRESH_QUEUED.write() = true;
                MEDIA_DETAILS_BROKER.send(MediaDetailsInput::UpdatePlayNext);
            }
        }
        self.update_view(widgets, sender);
    }
}

#[derive(Debug)]
struct EpisodeThumbnail {
    media: BaseItemDto,
    thumbnail: Option<Texture>,
}
#[derive(Debug)]
enum EpisodeThumbnailCommandOutput {
    LoadThumbnail(VecDeque<u8>),
}

#[relm4::component]
impl Component for EpisodeThumbnail {
    type Init = (BaseItemDto, Arc<ApiClient>);
    type Input = ();
    type Output = ();
    type CommandOutput = EpisodeThumbnailCommandOutput;

    view! {
        gtk::Box {
            gtk::Overlay {
                set_margin_top: 8,
                set_margin_bottom: 8,

                gtk::Picture {
                    #[watch]
                    set_paintable: model.thumbnail.as_ref(),

                    add_css_class: "episode-thumbnail",
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,
                    set_width_request: EPISODE_THUMBNAIL_SIZE,
                    set_height_request: EPISODE_THUMBNAIL_SIZE,
                    set_content_fit: gtk::ContentFit::Cover,
                },

                add_overlay = &gtk::Box {
                    #[watch]
                    set_visible: !model.media.user_data.as_ref()
                        .and_then(|user_data| user_data.play_count)
                        .unwrap_or(0) == 0,
                    set_tooltip: "This episode has never been played",
                    add_css_class: "episode-unplayed-indicator",
                    set_halign: gtk::Align::End,
                    set_valign: gtk::Align::Start,
                    set_width_request: 12,
                    set_height_request: 12,
                },

                add_overlay = &gtk::Spinner {
                    #[watch]
                    set_visible: model.thumbnail.is_none(),
                    set_spinning: true,
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,
                    set_width_request: 24,
                    set_height_request: 24,
                },
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let (media, api_client) = init;

        if let Ok(img_url) =
            api_client.get_episode_primary_image_url(&media, EPISODE_THUMBNAIL_SIZE)
        {
            sender.oneshot_command(async {
                let img_bytes: VecDeque<u8> = reqwest::get(img_url)
                    .await
                    .expect("Error getting media tile image: {img_url}")
                    .bytes()
                    .await
                    .expect("Error getting media tile image bytes: {img_url}")
                    .into_iter()
                    .collect();
                EpisodeThumbnailCommandOutput::LoadThumbnail(img_bytes)
            });
        }

        let model = Self {
            media,
            thumbnail: None,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            EpisodeThumbnailCommandOutput::LoadThumbnail(img_bytes) => {
                let pixbuf = Pixbuf::from_read(img_bytes)
                    .expect("Error creating media tile pixbuf: {img_url}");

                // Resize image to match thumbnail widget size
                // This makes widget sizing a bit less wonky
                let resized = Pixbuf::new(
                    gdk_pixbuf::Colorspace::Rgb,
                    false,
                    8,
                    EPISODE_THUMBNAIL_SIZE,
                    EPISODE_THUMBNAIL_SIZE,
                )
                .unwrap();

                let offset_x = ((pixbuf.width() - EPISODE_THUMBNAIL_SIZE) / 2).abs() as f64;

                pixbuf.scale(
                    &resized,
                    0,
                    0,
                    EPISODE_THUMBNAIL_SIZE,
                    EPISODE_THUMBNAIL_SIZE,
                    -offset_x,
                    0.0,
                    1.0,
                    1.0,
                    InterpType::Nearest,
                );

                self.thumbnail = Some(gdk::Texture::for_pixbuf(&resized));
            }
        }
    }
}
