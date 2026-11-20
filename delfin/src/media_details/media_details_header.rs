use std::{collections::VecDeque, matches, sync::Arc};

use adw::{prelude::*, BreakpointCondition};
use jellyfin_api::types::{BaseItemDto, BaseItemKind};
use relm4::{
    gtk::{gdk::Texture, gdk_pixbuf::Pixbuf},
    prelude::*,
};

use crate::{
    globals::SELECTED_VERSIONS,
    app::{AppInput, APP_BROKER},
    jellyfin_api::api_client::ApiClient,
    library::LIBRARY_REFRESH_QUEUED,
    media_details::watched_state::{watched_label, Played},
    tr,
    utils::{constants::MAX_LIBRARY_WIDTH, playable::get_next_playable_media},
};

use super::watched_state::toggle_watched;

pub const MEDIA_DETAILS_BACKDROP_HEIGHT: i32 = 480;

#[derive(Debug)]
pub(crate) struct MediaDetailsHeader {
    api_client: Arc<ApiClient>,
    media: BaseItemDto,
    item: BaseItemDto,
    backdrop: Option<Texture>,
    poster: Option<Texture>,
    play_next_label: Option<String>,
    play_next_media: Option<BaseItemDto>,
}

pub(crate) struct MediaDetailsHeaderInit {
    pub(crate) api_client: Arc<ApiClient>,
    pub(crate) media: BaseItemDto,
    pub(crate) item: BaseItemDto,
}

#[derive(Debug)]
pub enum MediaDetailsHeaderInput {
    PlayNext,
    PlayFromStart,
    ToggleWatched(bool),
    UpdatePlayNext,
}

#[derive(Debug)]
pub enum MediaDetailsHeaderOutput {
    RefreshEpisodes,
}

#[derive(Debug)]
pub enum MediaDetailsHeaderCommandOutput {
    PlayNextLoaded(Box<(String, Option<BaseItemDto>)>),
    BackdropLoaded(VecDeque<u8>),
    PosterLoaded(VecDeque<u8>),
}

#[relm4::component(pub(crate) async)]
impl AsyncComponent for MediaDetailsHeader {
    type Init = MediaDetailsHeaderInit;
    type Input = MediaDetailsHeaderInput;
    type Output = MediaDetailsHeaderOutput;
    type CommandOutput = MediaDetailsHeaderCommandOutput;

    view! {
        adw::BreakpointBin {
            set_size_request: (150, MEDIA_DETAILS_BACKDROP_HEIGHT),

            #[wrap(Some)]
            set_child = &gtk::Overlay {
                set_height_request: MEDIA_DETAILS_BACKDROP_HEIGHT,
                add_css_class: "media-details-header",
                set_overflow: gtk::Overflow::Hidden,

                // Hintergrundbild über die ganze Breite.
                gtk::Picture {
                    #[watch]
                    set_paintable: model.backdrop.as_ref(),
                    set_content_fit: gtk::ContentFit::Cover,
                    set_can_shrink: true,
                    set_hexpand: true,
                    set_height_request: MEDIA_DETAILS_BACKDROP_HEIGHT,
                },

                add_overlay = &gtk::Spinner {
                    #[watch]
                    set_visible: model.backdrop.is_none(),
                    set_spinning: true,
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,
                    set_width_request: 24,
                    set_height_request: 24,
                },

                add_overlay = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_valign: gtk::Align::Fill,
                    set_hexpand: true,
                    #[watch]
                    set_css_classes: &{
                        let mut classes = vec!["media-details-header-overlay"];
                        if model.backdrop.is_some() {
                            classes.push("gradient");
                        }
                        classes
                    },

                    adw::Clamp {
                        set_maximum_size: MAX_LIBRARY_WIDTH,
                        set_tightening_threshold: MAX_LIBRARY_WIDTH,
                        set_valign: gtk::Align::End,
                        set_vexpand: true,

                        #[name = "details"]
                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_valign: gtk::Align::End,
                            set_spacing: 28,
                            set_margin_start: 32,
                            set_margin_end: 32,
                            set_margin_bottom: 28,

                            #[name = "poster_box"]
                            gtk::Box {
                                set_valign: gtk::Align::End,
                                gtk::Picture {
                                    #[watch]
                                    set_paintable: model.poster.as_ref(),
                                    #[watch]
                                    set_visible: model.poster.is_some(),
                                    set_content_fit: gtk::ContentFit::Cover,
                                    set_size_request: (160, 240),
                                    set_overflow: gtk::Overflow::Hidden,
                                    add_css_class: "detail-poster",
                                },
                            },

                            gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_valign: gtk::Align::End,
                                set_hexpand: true,
                                set_spacing: 10,

                                gtk::Label {
                                    set_label: &title,
                                    set_tooltip: &title,
                                    set_xalign: 0.0,
                                    set_wrap: true,
                                    set_lines: 2,
                                    set_ellipsize: gtk::pango::EllipsizeMode::End,
                                    add_css_class: "media-details-header-title",
                                },

                                gtk::Label {
                                    set_label: &meta,
                                    set_visible: !meta.is_empty(),
                                    set_xalign: 0.0,
                                    set_wrap: true,
                                    add_css_class: "detail-meta",
                                },

                                gtk::Label {
                                    set_label: &overview,
                                    set_visible: !overview.is_empty(),
                                    set_xalign: 0.0,
                                    set_wrap: true,
                                    set_lines: 3,
                                    set_max_width_chars: 90,
                                    set_ellipsize: gtk::pango::EllipsizeMode::End,
                                    add_css_class: "detail-overview",
                                },

                                gtk::ProgressBar {
                                    set_fraction: resume_fraction,
                                    set_visible: resume_fraction > 0.0,
                                    set_halign: gtk::Align::Start,
                                    set_width_request: 320,
                                    add_css_class: "detail-progress",
                                },

                                #[name = "header_buttons"]
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_halign: gtk::Align::Start,
                                    set_spacing: 10,
                                    set_margin_top: 6,

                                    #[name = "btn_play_next"]
                                    gtk::Button {
                                        add_css_class: "pill",
                                        add_css_class: "suggested-action",
                                        set_valign: gtk::Align::Center,

                                        #[watch]
                                        set_visible: model.play_next_label.is_some() && model.play_next_media.is_some(),

                                        connect_clicked[sender] => move |_| {
                                            sender.input(MediaDetailsHeaderInput::PlayNext);
                                        },

                                        #[wrap(Some)]
                                        set_child = &gtk::Box {
                                            set_orientation: gtk::Orientation::Horizontal,
                                            set_spacing: 8,

                                            gtk::Image::from_icon_name("play-filled"),

                                            if model.play_next_label.is_some() {
                                                gtk::Label {
                                                    #[watch]
                                                    set_label: model.play_next_label.as_ref().unwrap(),
                                                }
                                            } else { gtk::Spinner { set_spinning: true } },
                                        },
                                    },

                                    // Film angefangen: zusätzlich „Von vorn“.
                                    gtk::Button {
                                        set_label: tr!("media-details-play-from-start"),
                                        add_css_class: "pill",
                                        add_css_class: "detail-secondary",
                                        set_valign: gtk::Align::Center,
                                        set_visible: is_movie && resume_fraction > 0.0,
                                        connect_clicked[sender] => move |_| {
                                            sender.input(MediaDetailsHeaderInput::PlayFromStart);
                                        },
                                    },

                                    #[name = "btn_watched"]
                                    gtk::ToggleButton {
                                        set_icon_name: "eye-open-negative-filled",
                                        set_css_classes: &["circular", "detail-round", "btn-watched"],
                                        set_valign: gtk::Align::Center,
                                        #[watch]
                                        set_tooltip: &watched_label(model.item.played()),

                                        #[watch]
                                        #[block_signal(toggle_handler)]
                                        set_active: model.item.played(),
                                        connect_toggled[sender] => move |btn| {
                                            sender.input(MediaDetailsHeaderInput::ToggleWatched(btn.is_active()));
                                        } @toggle_handler,
                                    },
                                },
                            },
                        },
                    },
                },
            },

            add_breakpoint = adw::Breakpoint::new(BreakpointCondition::new_length(
                adw::BreakpointConditionLengthType::MaxWidth,
                700.0,
                adw::LengthUnit::Px
            )) {
                add_setter: (&poster_box, "visible", &false.into()),
                add_setter: (&details, "margin_start", &16.0.into()),
                add_setter: (&details, "margin_end", &16.0.into()),
            }
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let MediaDetailsHeaderInit {
            api_client,
            media,
            item,
        } = init;

        if let Ok(img_url) = api_client.get_backdrop_url(&item) {
            sender.oneshot_command(async {
                let img_bytes: VecDeque<u8> = reqwest::get(img_url)
                    .await
                    .expect("Error getting media tile image: {img_url}")
                    .bytes()
                    .await
                    .expect("Error getting media tile image bytes: {img_url}")
                    .into_iter()
                    .collect();
                MediaDetailsHeaderCommandOutput::BackdropLoaded(img_bytes)
            });
        }

        let model = MediaDetailsHeader {
            api_client,
            media,
            item,
            backdrop: None,
            poster: None,
            play_next_label: None,
            play_next_media: None,
        };

        model.update_play_next(&sender);

        let title = model
            .media
            .series_name
            .clone()
            .or(model.media.name.clone())
            .unwrap_or(tr!("media-details-unnamed-item").to_string());

        let is_movie = matches!(model.item.type_, Some(BaseItemKind::Movie));
        let meta = meta_line(&model.item);
        let overview = model
            .item
            .overview
            .as_deref()
            .map(|o| o.split_once('\n').map_or(o, |(first, _)| first).to_string())
            .unwrap_or_default();
        let resume_fraction = if is_movie {
            model
                .item
                .user_data
                .as_ref()
                .and_then(|d| d.played_percentage)
                .map(|p| (p / 100.0).clamp(0.0, 1.0))
                .unwrap_or(0.0)
        } else {
            0.0
        };

        if let Ok(poster_url) = model.api_client.get_parent_or_item_primary_image_url(&model.item) {
            sender.oneshot_command(async move {
                let bytes: VecDeque<u8> = match reqwest::get(poster_url).await {
                    Ok(response) => response.bytes().await.map(|b| b.into_iter().collect()).unwrap_or_default(),
                    Err(_) => VecDeque::new(),
                };
                MediaDetailsHeaderCommandOutput::PosterLoaded(bytes)
            });
        }

        let widgets = view_output!();

        // Mehrere Versionen (z. B. 4K und 1080p): Auswahl neben dem Abspielknopf.
        if let (Some(item_id), Some(sources)) = (model.item.id, model.item.media_sources.as_ref()) {
            if sources.len() > 1 {
                let ids: Vec<Option<String>> = sources.iter().map(|s| s.id.clone()).collect();
                let names: Vec<String> = sources
                    .iter()
                    .enumerate()
                    .map(|(i, s)| s.name.clone().unwrap_or_else(|| format!("Version {}", i + 1)))
                    .collect();
                let names: Vec<&str> = names.iter().map(String::as_str).collect();
                let dropdown = gtk::DropDown::from_strings(&names);
                dropdown.set_valign(gtk::Align::Center);
                dropdown.set_tooltip_text(Some(tr!("media-details-version")));
                if let Some(chosen) = SELECTED_VERSIONS.read().get(&item_id) {
                    if let Some(pos) = ids.iter().position(|id| id.as_ref() == Some(chosen)) {
                        dropdown.set_selected(pos as u32);
                    }
                }
                dropdown.connect_selected_notify(move |dropdown| {
                    if let Some(Some(id)) = ids.get(dropdown.selected() as usize) {
                        SELECTED_VERSIONS.write().insert(item_id, id.clone());
                    }
                });
                if let Some(parent) = widgets.btn_play_next.parent().and_downcast::<gtk::Box>() {
                    parent.append(&dropdown);
                }
            }
        }

        // Löschen (nur wenn das Konto darf): Film, Serie oder Staffel.
        if crate::utils::delete_item::can_delete() {
            let delete = gtk::Button::builder()
                .icon_name("user-trash-symbolic")
                .tooltip_text(tr!("delete-menu"))
                .valign(gtk::Align::Center)
                .css_classes(["circular", "detail-round", "detail-delete"])
                .build();
            delete.connect_clicked({
                let item = model.item.clone();
                let api_client = model.api_client.clone();
                move |_| {
                    crate::utils::delete_item::confirm_delete(&item, api_client.clone(), || {
                        APP_BROKER.send(AppInput::NavigateBack);
                    });
                }
            });
            if let Some(parent) = widgets.btn_play_next.parent().and_downcast::<gtk::Box>() {
                parent.append(&delete);
            }
        }

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        message: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            MediaDetailsHeaderInput::PlayNext => {
                if let Some(play_next_media) = &self.play_next_media {
                    APP_BROKER.send(crate::app::AppInput::PlayVideo(play_next_media.clone()));
                }
            }
            MediaDetailsHeaderInput::PlayFromStart => {
                let mut item = self.item.clone();
                if let Some(user_data) = item.user_data.as_mut() {
                    user_data.playback_position_ticks = Some(0);
                }
                APP_BROKER.send(crate::app::AppInput::PlayVideo(item));
            }
            MediaDetailsHeaderInput::ToggleWatched(watched) => {
                self.item.user_data = match toggle_watched(&self.item, &self.api_client, watched)
                    .await
                {
                    Ok(user_data) => Some(user_data),
                    Err(err) => {
                        tracing::error!(
                            "Failed to mark media as {}: {err}",
                            watched_label(watched)
                        );
                        APP_BROKER.send(AppInput::Toast(
                                tr!("media-details-toggle-watched-error", {
                                    "type" => if let Some(BaseItemKind::Movie) = self.item.type_ { "movie" } else { "series" },
                                    "watched" => watched.to_string(),
                                })
                                .to_owned(),
                                None,
                            ));
                        return;
                    }
                };
                let _ = sender.output(MediaDetailsHeaderOutput::RefreshEpisodes);
                self.update_play_next(&sender);
                *LIBRARY_REFRESH_QUEUED.write() = true;
            }
            MediaDetailsHeaderInput::UpdatePlayNext => {
                self.update_play_next(&sender);
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
            MediaDetailsHeaderCommandOutput::PlayNextLoaded(play_next) => {
                let (play_next_label, play_next_media) = *play_next;
                self.play_next_label = Some(play_next_label);
                self.play_next_media = play_next_media;
            }
            MediaDetailsHeaderCommandOutput::PosterLoaded(img_bytes) => {
                if let Ok(pixbuf) = Pixbuf::from_read(img_bytes) {
                    self.poster = Some(Texture::for_pixbuf(&pixbuf));
                }
            }
            MediaDetailsHeaderCommandOutput::BackdropLoaded(img_bytes) => {
                let pixbuf = match Pixbuf::from_read(img_bytes) {
                    Ok(pixbuf) => pixbuf,
                    Err(err) => {
                        APP_BROKER.send(AppInput::Toast(
                            tr!("media-details-backdrop-error").to_owned(),
                            None,
                        ));
                        tracing::error!("Error creating media tile pixbuf: {err}");
                        return;
                    }
                };
                self.backdrop = Some(Texture::for_pixbuf(&pixbuf));
            }
        }
    }
}

impl MediaDetailsHeader {
    fn update_play_next(&self, sender: &AsyncComponentSender<Self>) {
        sender.oneshot_command({
            let api_client = self.api_client.clone();
            let item = self.item.clone();
            async move {
                let play_next = get_play_next(&api_client, &item).await;
                MediaDetailsHeaderCommandOutput::PlayNextLoaded(Box::new(play_next))
            }
        });
    }
}

async fn get_play_next(
    api_client: &Arc<ApiClient>,
    item: &BaseItemDto,
) -> (String, Option<BaseItemDto>) {
    let Some(next_playable) = get_next_playable_media(api_client.clone(), item.clone()).await
    else {
        return (
            tr!("media-details-play-button.next-episode").to_string(),
            None,
        );
    };

    let resume = next_playable
        .user_data
        .clone()
        .and_then(|user_data| user_data.playback_position_ticks)
        .unwrap_or(0)
        != 0;

    if !(matches!(
        next_playable.type_,
        Some(BaseItemKind::Episode) | Some(BaseItemKind::Series)
    )) {
        return (
            tr!("media-details-play-button", {"resume" => resume.to_string()}).to_string(),
            Some(next_playable),
        );
    }

    if let (Some(parent_index_number), Some(index_number)) = (
        &next_playable.parent_index_number,
        &next_playable.index_number,
    ) {
        return (
            tr!(
                "media-details-play-button.with-episode-and-season",
                {
                    "resume" => resume.to_string(),
                    "seasonNumber" => parent_index_number,
                    "episodeNumber" => index_number,
                },
            )
            .to_string(),
            Some(next_playable),
        );
    }

    (
        tr!("media-details-play-button.next-episode").to_string(),
        Some(next_playable),
    )
}

/// Infozeile unter dem Titel: Jahre · Laufzeit · Genres · FSK · Bewertung · Ende.
fn meta_line(item: &BaseItemDto) -> String {
    use crate::{media_details::run_time::RunTime, utils::display_years::DisplayYears};
    let mut parts: Vec<String> = Vec::new();
    if let Some(years) = item.display_years() {
        parts.push(years.to_string());
    }
    let is_movie = matches!(item.type_, Some(BaseItemKind::Movie));
    if is_movie {
        if let Some(run_time) = item.run_time() {
            parts.push(run_time);
        }
    } else if let Some(count) = item.child_count.filter(|c| *c > 0) {
        parts.push(tr!("media-details-season-count", {"count" => count}).to_string());
    }
    if let Some(genres) = &item.genres {
        if !genres.is_empty() {
            parts.push(genres.iter().take(3).cloned().collect::<Vec<_>>().join(", "));
        }
    }
    if let Some(rating) = &item.official_rating {
        parts.push(rating.clone());
    }
    if let Some(rating) = item.community_rating {
        parts.push(format!("★ {rating:.1}"));
    }
    if is_movie {
        if let Some(ticks) = item.run_time_ticks {
            let done = item
                .user_data
                .as_ref()
                .and_then(|d| d.playback_position_ticks)
                .unwrap_or(0);
            let remaining = ((ticks - done).max(0) / 10_000_000) as i64;
            let end = chrono::Local::now() + chrono::Duration::seconds(remaining);
            parts.push(tr!("media-details-ends-at", {"time" => end.format("%H:%M").to_string()}).to_string());
        }
    }
    parts.join(" · ")
}
