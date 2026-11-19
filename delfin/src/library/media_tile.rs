use std::{collections::VecDeque, sync::Arc};

use gdk::Texture;
use jellyfin_api::types::{BaseItemDto, BaseItemKind};
use relm4::{
    gtk::{self, gdk, gdk_pixbuf, glib::markup_escape_text, prelude::*},
    prelude::{AsyncComponent, AsyncComponentParts},
    AsyncComponentSender,
};
use tracing::{debug, error, warn};

use crate::{
    app::{AppInput, APP_BROKER},
    globals::CONFIG,
    jellyfin_api::api_client::ApiClient,
    tr,
    utils::{display_years::DisplayYears, item_name::ItemName, playable::get_next_playable_media},
};

#[derive(Clone, Copy)]
pub enum MediaTileDisplay {
    Cover,
    CoverLarge,
    Wide,
    CollectionWide,
    Buttons,
}

impl MediaTileDisplay {
    pub fn width(&self) -> i32 {
        match self {
            Self::Cover => 133,
            Self::CoverLarge => 175,
            Self::Wide => 300,
            Self::CollectionWide => 300,
            Self::Buttons => 300,
        }
    }

    pub fn height(&self) -> i32 {
        match self {
            Self::Cover => 200,
            Self::CoverLarge => 262,
            Self::Wide => 175,
            Self::CollectionWide => 175,
            Self::Buttons => 0,
        }
    }
}

pub struct MediaTile {
    media: BaseItemDto,
    api_client: Arc<ApiClient>,
    thumbnail: Option<Texture>,
    /// Bild wird noch geholt (Ladekreis). Danach ohne Bild: leere Kachel.
    loading: bool,
}

#[derive(Debug)]
pub enum MediaTileInput {
    Play,
    ShowDetails,
}

#[derive(Debug)]
pub enum MediaTileCommandOutput {
    ThumbnailLoaded(Option<Texture>),
}

#[relm4::component(pub async)]
impl AsyncComponent for MediaTile {
    type Init = (BaseItemDto, MediaTileDisplay, Arc<ApiClient>);
    type Input = MediaTileInput;
    type Output = ();
    type CommandOutput = MediaTileCommandOutput;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_halign: gtk::Align::Fill,
            set_valign: gtk::Align::Start,
            set_spacing: 8,
            add_css_class: "media-tile",

            #[name = "overlay"]
            gtk::Overlay {
                set_halign: gtk::Align::Center,
                set_cursor_from_name: Some("pointer"),

                set_accessible_role: gtk::AccessibleRole::Button,
                set_focusable: true,
                connect_has_focus_notify[root, media] => move |overlay| {
                    if overlay.has_focus() && !matches!(media.type_, Some(BaseItemKind::CollectionFolder)) {
                        root.add_css_class("hover");
                    } else {
                        root.remove_css_class("hover");
                    }
                },

                // TODO: progress bar overflows media tile. Hiding the overflow is a workaround, but it makes the
                // progress bar percentage look wrong.
                set_overflow: gtk::Overflow::Hidden,


                add_controller = gtk::GestureClick {
                    connect_released[sender] => move |_, _, _, _| {
                        sender.input(MediaTileInput::Play);
                    },
                },

                add_controller = gtk::EventControllerKey {
                    connect_key_released[sender] => move |_, key, _, _| {
                        if matches!(key, gdk::Key::Return | gdk::Key::space) {
                            sender.input(MediaTileInput::Play);
                        }
                    },
                },

                add_controller = gtk::EventControllerMotion {
                    connect_enter[root, media] => move |_, _, _| {
                        if !matches!(media.type_, Some(BaseItemKind::CollectionFolder)) {
                            root.add_css_class("hover");
                        }
                    },
                    connect_leave[root] => move |_| {
                        root.remove_css_class("hover");
                    },
                },

                #[name = "image"]
                gtk::Picture {
                    #[watch]
                    set_paintable: model.thumbnail.as_ref(),

                    set_content_fit: gtk::ContentFit::Cover,
                    set_width_request: tile_display.width(),
                    set_height_request: tile_display.height(),
                },

                add_overlay = &gtk::Spinner {
                    #[watch]
                    set_visible: model.loading,
                    start: (),
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,
                    set_width_request: 32,
                    set_height_request: 32,
                },

                add_overlay = &gtk::CenterBox {
                    add_css_class: "hover-overlay",

                    set_halign: gtk::Align::Fill,
                    set_valign: gtk::Align::Fill,

                    #[wrap(Some)]
                    set_center_widget = &gtk::Image {
                        set_from_icon_name: Some("play-filled"),

                        set_halign: gtk::Align::Center,
                        set_valign: gtk::Align::Center,
                    }
                },

                add_overlay = &gtk::ProgressBar {
                    set_valign: gtk::Align::End,
                    set_visible: model.media.user_data.as_ref().map(|user_data| user_data.played_percentage).is_some(),
                    set_fraction?: model.media.user_data.as_ref()
                        .and_then(|user_data| user_data.played_percentage)
                        .map(|played_percentage| played_percentage / 100.0),
                    set_overflow: gtk::Overflow::Hidden,
                    set_width_request: tile_display.width(),
                },
            },

            gtk::Label {
                set_halign: gtk::Align::Center,
                set_justify: gtk::Justification::Center,
                set_cursor_from_name: Some("pointer"),
                set_ellipsize: gtk::pango::EllipsizeMode::End,
                set_max_width_chars: 1,
                set_width_request: tile_display.width(),
                #[watch]
                set_markup: &model.get_item_label(),

                set_has_tooltip: true,
                connect_query_tooltip[item_label] => move |_, _, _, _, tooltip| {
                    let label = gtk::Label::builder()
                        .use_markup(true)
                        .label(&item_label)
                        .justify(gtk::Justification::Center)
                        .build();
                    tooltip.set_custom(Some(&label));
                    true
                },

                add_controller = gtk::GestureClick {
                    connect_released[sender] => move |_, _, _, _| {
                        sender.input(MediaTileInput::ShowDetails);
                    },
                },
            },
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let (media, tile_display, api_client) = init;

        let model = Self::new(media.clone(), api_client.clone());
        let item_label = model.get_item_label();

        let widgets = view_output!();

        sender.oneshot_command({
            let media = model.media.clone();
            async move {
                let thumbnail = get_thumbnail(api_client, &media, &tile_display).await;
                MediaTileCommandOutput::ThumbnailLoaded(thumbnail)
            }
        });

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        message: Self::Input,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            MediaTileInput::Play => {
                match self.media.type_ {
                    Some(BaseItemKind::CollectionFolder) => {
                        APP_BROKER.send(AppInput::ShowCollection(self.media.clone()));
                    }
                    // Serie/Staffel: erst die Staffel- und Folgenauswahl zeigen
                    // statt blind eine Folge zu starten. Abspielen geht dort
                    // über den Knopf im Kopfbereich.
                    Some(BaseItemKind::Series | BaseItemKind::Season) => {
                        APP_BROKER.send(AppInput::ShowDetails(self.media.clone()));
                    }
                    _ => {
                        match get_next_playable_media(self.api_client.clone(), self.media.clone())
                            .await
                        {
                            Some(media) => APP_BROKER.send(AppInput::PlayVideo(media)),
                            _ => {
                                let mut message = "No playable media found".to_string();
                                if let Some(name) = self.media.name.as_ref() {
                                    message += &format!(" for {name}");
                                }
                                APP_BROKER.send(AppInput::Toast(message, None))
                            }
                        };
                    }
                };
            }
            MediaTileInput::ShowDetails => {
                match self.media.type_ {
                    Some(BaseItemKind::CollectionFolder) => {
                        APP_BROKER.send(AppInput::ShowCollection(self.media.clone()));
                    }
                    _ => {
                        APP_BROKER.send(AppInput::ShowDetails(self.media.clone()));
                    }
                };
            }
        }
    }

    async fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            MediaTileCommandOutput::ThumbnailLoaded(thumbnail) => {
                self.thumbnail = thumbnail;
                self.loading = false;
            }
        }
    }
}

impl MediaTile {
    pub fn new(media: BaseItemDto, api_client: Arc<ApiClient>) -> Self {
        Self {
            media,
            api_client,
            thumbnail: None,
            loading: true,
        }
    }

    fn get_item_label(&self) -> String {
        match (
            self.media
                .series_name
                .as_ref()
                .map(|s| markup_escape_text(s)),
            self.media
                .episode_name_with_number()
                .as_ref()
                .map(|s| markup_escape_text(s)),
            self.media.display_years(),
        ) {
            (Some(series_name), Some(name), _) => format!(
                r#"{series_name}
<span size="small">{name}</span>"#
            ),
            (_, Some(name), Some(display_years)) => format!(
                r#"{name}
<span size="small">{display_years}</span>"#
            ),
            (_, Some(name), _) => name.to_string(),
            _ => tr!("library-media-tile-unnamed-item").to_string(),
        }
    }
}

async fn get_thumbnail(
    api_client: Arc<ApiClient>,
    media: &BaseItemDto,
    tile_display: &MediaTileDisplay,
) -> Option<gdk::Texture> {
    let img_url = match tile_display {
        MediaTileDisplay::Wide => {
            let config = CONFIG.read();
            let aspect_ratio_good = media.primary_image_aspect_ratio.unwrap_or_default() > 1.5;

            if config.general.use_episode_image && aspect_ratio_good {
                api_client.get_episode_primary_image_url(media, tile_display.height())
            } else if aspect_ratio_good {
                api_client.get_parent_or_item_thumbnail_url(media, tile_display.height())
            } else {
                api_client.get_episode_thumbnail_or_backdrop_url(media, tile_display.height())
            }
        }
        MediaTileDisplay::Cover | MediaTileDisplay::CoverLarge => {
            api_client.get_parent_or_item_primary_image_url(media)
        }
        MediaTileDisplay::CollectionWide | MediaTileDisplay::Buttons => {
            api_client.get_collection_thumbnail_url(media)
        }
    };

    // Erst das vom Original gewählte Bild, dann Ersatzbilder: Folgen in
    // „Weiterschauen“ haben auf Jellyfin 12 oft weder eigene Bildkennungen
    // noch Eltern-Thumb – dann bleibt die Kachel sonst leer.
    let kinds: &[&str] = match tile_display {
        MediaTileDisplay::Cover | MediaTileDisplay::CoverLarge => &["Primary"],
        _ => &["Thumb", "Backdrop", "Primary"],
    };
    let mut candidates: Vec<String> = img_url.ok().into_iter().collect();
    for owner in [
        media.series_id,
        media.parent_thumb_item_id,
        media.parent_backdrop_item_id,
        media.id,
    ]
    .into_iter()
    .flatten()
    {
        for kind in kinds {
            if let Ok(mut url) = api_client.root.join(&format!("Items/{owner}/Images/{kind}")) {
                url.query_pairs_mut()
                    .append_pair("fillHeight", &tile_display.height().to_string())
                    .append_pair("quality", "96");
                let url = url.to_string();
                if !candidates.contains(&url) {
                    candidates.push(url);
                }
            }
        }
    }

    let mut found = None;
    for img_url in candidates {
        if let Some(bytes) = fetch_image_bytes(&img_url, media).await {
            // Prüfen, ob es sich dekodieren lässt; das Pixbuf selbst darf
            // nicht über ein await gehalten werden (nicht Send).
            if gdk_pixbuf::Pixbuf::from_read(VecDeque::from(bytes.clone())).is_ok() {
                found = Some(bytes);
                break;
            }
            error!("Error creating media tile pixbuf: {:#?}", media.id);
        }
    }
    let Some(bytes) = found else {
        debug!("No media tile image at all for {:?}", media.id);
        return None;
    };
    let pixbuf = gdk_pixbuf::Pixbuf::from_read(VecDeque::from(bytes)).ok()?;

    // TODO: merge resizing with how it's done for episode list

    let resized = gdk_pixbuf::Pixbuf::new(
        gdk_pixbuf::Colorspace::Rgb,
        true,
        8,
        tile_display.width(),
        tile_display.height(),
    )?;

    let scale = tile_display.height() as f64 / pixbuf.height() as f64;

    pixbuf.scale(
        &resized,
        0,
        0,
        tile_display.width(),
        tile_display.height(),
        0.0,
        0.0,
        scale,
        scale,
        gdk_pixbuf::InterpType::Bilinear,
    );

    Some(gdk::Texture::for_pixbuf(&resized))
}

/// Holt Bilddaten; `None` bei Netzfehler oder fehlendem Bild (404).
async fn fetch_image_bytes(img_url: &str, media: &BaseItemDto) -> Option<Vec<u8>> {
    let response = match reqwest::get(img_url).await {
        Ok(response) => response,
        Err(err) => {
            warn!("Error getting media tile image for {:?}: {err}", media.id);
            return None;
        }
    };
    if !response.status().is_success() {
        return None;
    }
    match response.bytes().await {
        Ok(bytes) => Some(bytes.to_vec()),
        Err(err) => {
            warn!("Error reading media tile image for {:?}: {err}", media.id);
            None
        }
    }
}
