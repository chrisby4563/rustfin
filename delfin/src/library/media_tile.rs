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
    /// Image is still loading (spinner). Afterwards, no image means an empty tile.
    loading: bool,
    /// Context menu (right-click / long press).
    menu: gtk::Popover,
    /// Removed from “Continue Watching”: hide the tile.
    hidden: bool,
}

#[derive(Debug)]
pub enum MediaTileInput {
    Play,
    ShowDetails,
    ShowMenu(f64, f64),
    SetPlayed(bool),
    RemoveFromResume,
    GoToSeries,
    Delete,
    Deleted,
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
            #[watch]
            set_visible: !model.hidden,

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

                // Right-click and long press: context menu.
                add_controller = gtk::GestureClick {
                    set_button: gdk::BUTTON_SECONDARY,
                    connect_pressed[sender] => move |gesture, _, x, y| {
                        gesture.set_state(gtk::EventSequenceState::Claimed);
                        sender.input(MediaTileInput::ShowMenu(x, y));
                    },
                },
                add_controller = gtk::GestureLongPress {
                    set_touch_only: true,
                    connect_pressed[sender] => move |_, x, y| {
                        sender.input(MediaTileInput::ShowMenu(x, y));
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
                    set_visible: false,
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

                // Watched indicator at top right: checkmark = watched,
                // number = unplayed episodes in a series/season.
                add_overlay = &gtk::Box {
                    add_css_class: "watched-badge",
                    set_halign: gtk::Align::End,
                    set_valign: gtk::Align::Start,
                    set_margin_top: 6,
                    set_margin_end: 6,
                    set_can_target: false,
                    #[watch]
                    set_visible: model.watched_badge().is_some(),

                    gtk::Image {
                        set_icon_name: Some("object-select-symbolic"),
                        #[watch]
                        set_visible: matches!(model.watched_badge(), Some(WatchedBadge::Played)),
                    },

                    gtk::Label {
                        #[watch]
                        set_visible: matches!(model.watched_badge(), Some(WatchedBadge::Unplayed(_))),
                        #[watch]
                        set_label: &match model.watched_badge() {
                            Some(WatchedBadge::Unplayed(count)) => count.to_string(),
                            _ => String::new(),
                        },
                    },
                },

                add_overlay = &gtk::ProgressBar {
                    set_valign: gtk::Align::End,
                    #[watch]
                    set_visible: model.media.user_data.as_ref().and_then(|user_data| user_data.played_percentage).is_some_and(|p| p > 0.0),
                    #[watch]
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
        crate::utils::fade::fade_in_on_load(&widgets.image);
        model.menu.set_parent(&widgets.overlay);
        model.menu.set_has_arrow(false);
        model.menu.set_position(gtk::PositionType::Bottom);
        // The popover belongs to the overlay and must be detached before teardown.
        widgets.overlay.connect_destroy({
            let menu = model.menu.clone();
            move |_| menu.unparent()
        });

        let scale = crate::utils::pixel_scale::pixel_scale();
        sender.oneshot_command({
            let media = model.media.clone();
            async move {
                let thumbnail = get_thumbnail(api_client, &media, &tile_display, scale).await;
                MediaTileCommandOutput::ThumbnailLoaded(thumbnail)
            }
        });

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        message: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            MediaTileInput::ShowMenu(x, y) => {
                if matches!(self.media.type_, Some(BaseItemKind::CollectionFolder)) {
                    return;
                }
                self.build_menu(&sender);
                self.menu
                    .set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
                self.menu.popup();
            }
            MediaTileInput::SetPlayed(played) => {
                self.menu.popdown();
                match crate::media_details::watched_state::toggle_watched(
                    &self.media,
                    &self.api_client,
                    played,
                )
                .await
                {
                    Ok(user_data) => {
                        self.media.user_data = Some(user_data);
                        let message = if played {
                            tr!("library-tile-marked-played")
                        } else {
                            tr!("library-tile-marked-unplayed")
                        };
                        APP_BROKER.send(AppInput::Toast(message.to_string(), Some(2)));
                    }
                    Err(err) => {
                        warn!("Error marking item played={played}: {err}");
                        APP_BROKER.send(AppInput::Toast(err.to_string(), None));
                    }
                }
            }
            MediaTileInput::RemoveFromResume => {
                self.menu.popdown();
                let Some(item_id) = self.media.id else {
                    return;
                };
                match self.api_client.clear_playback_position(item_id).await {
                    Ok(()) => {
                        self.hidden = true;
                        APP_BROKER.send(AppInput::Toast(
                            tr!("library-tile-removed-from-resume").to_string(),
                            Some(2),
                        ));
                    }
                    Err(err) => {
                        warn!("Error removing item from resume: {err}");
                        APP_BROKER.send(AppInput::Toast(err.to_string(), None));
                    }
                }
            }
            MediaTileInput::GoToSeries => {
                self.menu.popdown();
                let Some(series_id) = self.media.series_id else {
                    return;
                };
                match self.api_client.get_item(&series_id).await {
                    Ok(series) => APP_BROKER.send(AppInput::ShowDetails(series)),
                    Err(err) => warn!("Error loading series {series_id}: {err}"),
                }
            }
            MediaTileInput::Play => {
                match self.media.type_ {
                    Some(BaseItemKind::CollectionFolder) => {
                        APP_BROKER.send(AppInput::ShowCollection(self.media.clone()));
                    }
                    // Movie/series/season: show details first (information and
                    // season/episode selection) instead of playing immediately.
                    // Playback is available from the header button. Individual
                    // episodes (Continue Watching, Next Up) still start directly.
                    Some(BaseItemKind::Movie | BaseItemKind::Series | BaseItemKind::Season) => {
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
            MediaTileInput::Delete => {
                self.menu.popdown();
                let sender = sender.clone();
                crate::utils::delete_item::confirm_delete(
                    &self.media,
                    self.api_client.clone(),
                    move || sender.input(MediaTileInput::Deleted),
                );
            }
            MediaTileInput::Deleted => {
                self.hidden = true;
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
            menu: gtk::Popover::new(),
            hidden: false,
        }
    }

    /// Rebuild menu entries for the current state.
    fn build_menu(&self, sender: &AsyncComponentSender<Self>) {
        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let add = |label: &str, input: fn() -> MediaTileInput| {
            let button = gtk::Button::builder()
                .label(label)
                .css_classes(["flat"])
                .build();
            if let Some(child) = button.child().and_downcast::<gtk::Label>() {
                child.set_xalign(0.0);
            }
            button.connect_clicked({
                let sender = sender.clone();
                move |_| sender.input(input())
            });
            list.append(&button);
        };

        let user_data = self.media.user_data.as_ref();
        let played = user_data.and_then(|d| d.played).unwrap_or(false);
        let is_container = matches!(
            self.media.type_,
            Some(BaseItemKind::Series | BaseItemKind::Season)
        );
        if played {
            add(tr!("library-tile-mark-unplayed"), || MediaTileInput::SetPlayed(false));
        } else if is_container {
            add(tr!("library-tile-mark-all-played"), || MediaTileInput::SetPlayed(true));
        } else {
            add(tr!("library-tile-mark-played"), || MediaTileInput::SetPlayed(true));
        }
        if is_container && !played && user_data.and_then(|d| d.played_percentage).is_some() {
            add(tr!("library-tile-mark-unplayed"), || MediaTileInput::SetPlayed(false));
        }
        if user_data
            .and_then(|d| d.playback_position_ticks)
            .is_some_and(|ticks| ticks > 0)
        {
            add(tr!("library-tile-remove-from-resume"), || MediaTileInput::RemoveFromResume);
        }
        if self.media.series_id.is_some()
            && matches!(
                self.media.type_,
                Some(BaseItemKind::Episode | BaseItemKind::Season)
            )
        {
            add(tr!("library-tile-go-to-series"), || MediaTileInput::GoToSeries);
        }
        add(tr!("library-tile-details"), || MediaTileInput::ShowDetails);
        if crate::utils::delete_item::can_delete()
            && matches!(
                self.media.type_,
                Some(
                    BaseItemKind::Movie
                        | BaseItemKind::Series
                        | BaseItemKind::Season
                        | BaseItemKind::Episode
                )
            )
        {
            let delete = gtk::Button::builder()
                .label(tr!("delete-menu"))
                .css_classes(["flat", "destructive-action"])
                .build();
            if let Some(child) = delete.child().and_downcast::<gtk::Label>() {
                child.set_xalign(0.0);
            }
            delete.connect_clicked({
                let sender = sender.clone();
                move |_| sender.input(MediaTileInput::Delete)
            });
            list.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            list.append(&delete);
        }

        self.menu.set_child(Some(&list));
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
    scale: f64,
) -> Option<gdk::Texture> {
    use crate::utils::pixel_scale::scaled;
    // In screen pixels, so the image is as sharp as the tile.
    let height = scaled(tile_display.height(), scale);
    let img_url = match tile_display {
        MediaTileDisplay::Wide => {
            let config = CONFIG.read();
            let aspect_ratio_good = media.primary_image_aspect_ratio.unwrap_or_default() > 1.5;

            if config.general.use_episode_image && aspect_ratio_good {
                api_client.get_episode_primary_image_url(media, height)
            } else if aspect_ratio_good {
                api_client.get_parent_or_item_thumbnail_url(media, height)
            } else {
                api_client.get_episode_thumbnail_or_backdrop_url(media, height)
            }
        }
        MediaTileDisplay::Cover | MediaTileDisplay::CoverLarge => api_client
            .get_parent_or_item_primary_image_url(media, scaled(tile_display.width(), scale)),
        MediaTileDisplay::CollectionWide | MediaTileDisplay::Buttons => {
            api_client.get_collection_thumbnail_url(media)
        }
    };

    // Try the originally selected image first, then fallbacks: episodes in
    // “Continue Watching” on Jellyfin 12 often have neither their own image ID
    // nor a parent thumbnail, which would otherwise leave the tile empty.
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
                    .append_pair("fillHeight", &height.to_string())
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
            // Check that it can be decoded; do not retain the pixbuf across an
            // await (it is not Send).
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

/// Fetches image data; `None` on network failure or a missing image (404).
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

/// What appears at the tile's top right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WatchedBadge {
    Played,
    Unplayed(i32),
}

impl MediaTile {
    fn watched_badge(&self) -> Option<WatchedBadge> {
        let user_data = self.media.user_data.as_ref()?;
        if user_data.played == Some(true) {
            return Some(WatchedBadge::Played);
        }
        let is_container = matches!(
            self.media.type_,
            Some(BaseItemKind::Series | BaseItemKind::Season)
        );
        match user_data.unplayed_item_count {
            Some(count) if is_container && count > 0 => Some(WatchedBadge::Unplayed(count)),
            _ => None,
        }
    }
}
