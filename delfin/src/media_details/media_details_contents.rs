use std::{cell::OnceCell, sync::Arc};

use gtk::prelude::*;
use jellyfin_api::types::{BaseItemDto, BaseItemKind};
use relm4::{
    component::{AsyncComponent, AsyncComponentController, AsyncComponentParts, AsyncController},
    loading_widgets::LoadingWidgets,
    prelude::*,
    view, AsyncComponentSender,
};
use uuid::Uuid;

use crate::{
    jellyfin_api::api_client::ApiClient,
    media_details::{media_details_header::MediaDetailsHeaderInit, seasons::SeasonsInit},
};

use super::{
    cast::{cast_people, CastRow},
    media_details_header::{MediaDetailsHeader, MediaDetailsHeaderInput, MediaDetailsHeaderOutput},
    seasons::{Seasons, SeasonsOutput},
};

pub struct MediaDetailsContents {
    api_client: Arc<ApiClient>,
    item: BaseItemDto,
    series_id: Option<Uuid>,
    header: OnceCell<AsyncController<MediaDetailsHeader>>,
    seasons: Option<AsyncController<Seasons>>,
    initial_season_id: Option<Uuid>,
    selected_season_index: Option<usize>,
    cast: Option<Controller<CastRow>>,
}

#[derive(Debug)]
pub enum MediaDetailsContentsInput {
    RefreshSeasons,
    SetSelectedSeasonIndex(usize),
    UpdatePlayNext,
}

#[relm4::component(pub async)]
impl AsyncComponent for MediaDetailsContents {
    type Init = (Arc<ApiClient>, BaseItemDto);
    type Input = MediaDetailsContentsInput;
    type Output = ();
    type CommandOutput = ();

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 16,

            adw::Clamp {
                set_maximum_size: 1100,
                set_margin_bottom: 32,
                set_margin_start: 12,
                set_margin_end: 12,

                #[name = "container"]
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 16,

                }
            },
        }
    }

    fn init_loading_widgets(root: Self::Root) -> Option<LoadingWidgets> {
        view! {
            #[local_ref]
            root {
                #[name(spinner)]
                gtk::Spinner {
                    set_spinning: true,
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,
                    set_hexpand: true,
                    set_vexpand: true,
                    set_width_request: 48,
                    set_height_request: 48,
                }
            }
        }
        Some(LoadingWidgets::new(root, spinner))
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let (api_client, media) = init;

        let series_id = media
            .series_id
            .or(if let Some(BaseItemKind::Series) = media.type_ {
                media.id
            } else {
                None
            });

        let item = if let Some(item_id) = media.series_id.or(media.id) {
            match api_client.get_item(&item_id).await {
                Ok(item) => item,
                Err(err) => {
                    tracing::error!("Failed to load media details: {err:#}");
                    media.clone()
                }
            }
        } else {
            tracing::error!("Media details received an item without an ID");
            media.clone()
        };

        let mut model = MediaDetailsContents {
            api_client: api_client.clone(),
            item: item.clone(),
            series_id,
            header: OnceCell::new(),
            seasons: None,
            initial_season_id: match media.type_ {
                Some(BaseItemKind::Season) => media.id,
                _ => media.season_id,
            },
            selected_season_index: None,
            cast: None,
        };

        let widgets = view_output!();
        let container = &widgets.container;

        let header = MediaDetailsHeader::builder()
            .launch(MediaDetailsHeaderInit {
                api_client,
                media,
                item,
            })
            .forward(sender.input_sender(), |m| m.into());
        root.prepend(header.widget());
        model.header.set(header).unwrap();

        // Movie layout: chapters and cast on the left, media information on the right.
        let is_movie = matches!(model.item.type_, Some(BaseItemKind::Movie));
        let movie_left = gtk::Box::new(gtk::Orientation::Vertical, 16);
        movie_left.set_hexpand(true);
        if is_movie {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 32);
            if let Some(chapters) = chapters_section(&model.api_client, &model.item) {
                movie_left.append(&chapters);
            }
            row.append(&movie_left);
            if let Some(info) = super::media_info::media_info_box(&model.item) {
                row.append(&info);
            }
            container.append(&row);
        }

        model.load_seasons(&sender, container);

        // Cast below everything else, wider than the text block.
        let people = cast_people(model.item.people.as_deref().unwrap_or_default());
        if !people.is_empty() {
            let cast = CastRow::builder()
                .launch((model.api_client.clone(), people))
                .detach();
            if is_movie {
                movie_left.append(cast.widget());
            } else {
                let clamp = adw::Clamp::builder()
                    .maximum_size(1200)
                    .margin_start(12)
                    .margin_end(12)
                    .margin_bottom(32)
                    .child(cast.widget())
                    .build();
                root.append(&clamp);
            }
            model.cast = Some(cast);
        }

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
            MediaDetailsContentsInput::RefreshSeasons => {
                self.load_seasons(&sender, &widgets.container);
            }
            MediaDetailsContentsInput::SetSelectedSeasonIndex(selected_season_index) => {
                self.selected_season_index = Some(selected_season_index);
            }
            MediaDetailsContentsInput::UpdatePlayNext => {
                if let Some(header) = self.header.get() {
                    header.emit(MediaDetailsHeaderInput::UpdatePlayNext);
                }
            }
        }
        self.update_view(widgets, sender);
    }
}

impl MediaDetailsContents {
    fn load_seasons(&mut self, sender: &AsyncComponentSender<Self>, container: &gtk::Box) {
        if let Some(seasons) = self.seasons.take() {
            container.remove(seasons.widget());
        }

        if let Some(series_id) = self.series_id {
            let seasons = Seasons::builder()
                .launch(SeasonsInit {
                    api_client: self.api_client.clone(),
                    series_id,
                    initial_selected_season_index: self.selected_season_index,
                    initial_season_id: self.initial_season_id,
                })
                .forward(sender.input_sender(), |m| m.into());
            container.append(seasons.widget());
            self.seasons = Some(seasons);
        }
    }
}

impl From<SeasonsOutput> for MediaDetailsContentsInput {
    fn from(val: SeasonsOutput) -> Self {
        match val {
            SeasonsOutput::SeasonSelected(selected_season_index) => {
                MediaDetailsContentsInput::SetSelectedSeasonIndex(selected_season_index)
            }
            SeasonsOutput::Refresh => MediaDetailsContentsInput::RefreshSeasons,
        }
    }
}

impl From<MediaDetailsHeaderOutput> for MediaDetailsContentsInput {
    fn from(_: MediaDetailsHeaderOutput) -> Self {
        Self::RefreshSeasons
    }
}

/// Movie chapters as a row with preview images; clicking plays from that position.
fn chapters_section(api_client: &Arc<ApiClient>, item: &BaseItemDto) -> Option<gtk::Widget> {
    let chapters = item.chapters.as_ref().filter(|c| c.len() > 1)?;
    let item_id = item.id?;
    let section = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let heading = gtk::Label::builder()
        .label(crate::tr!("media-details-chapters"))
        .xalign(0.0)
        .css_classes(["title-3"])
        .build();
    section.append(&heading);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    for (index, chapter) in chapters.iter().enumerate() {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
        card.set_width_request(200);
        let picture = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .width_request(200)
            .height_request(112)
            .overflow(gtk::Overflow::Hidden)
            .css_classes(["episode-thumbnail"])
            .build();
        if let (Some(tag), Ok(url)) = (
            chapter.image_tag.as_ref(),
            api_client.get_chapter_image_url(
                &item_id,
                index,
                chapter.image_tag.as_deref().unwrap_or_default(),
            ),
        ) {
            let _ = tag;
            let picture = picture.clone();
            gtk::glib::spawn_future_local(async move {
                let (tx, rx) = relm4::tokio::sync::oneshot::channel();
                relm4::spawn(async move {
                    let bytes = match reqwest::get(url).await {
                        Ok(r) => r.bytes().await.ok().map(|b| b.to_vec()),
                        Err(_) => None,
                    };
                    let _ = tx.send(bytes);
                });
                if let Ok(Some(bytes)) = rx.await {
                    let bytes = gtk::glib::Bytes::from_owned(bytes);
                    if let Ok(texture) = gtk::gdk::Texture::from_bytes(&bytes) {
                        picture.set_paintable(Some(&texture));
                    }
                }
            });
        }
        crate::utils::fade::fade_in_on_load(&picture);
        card.append(&picture);
        let name = chapter.name.clone().unwrap_or_default();
        card.append(
            &gtk::Label::builder()
                .label(&name)
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .css_classes(["heading"])
                .build(),
        );
        let seconds = chapter.start_position_ticks.unwrap_or(0) / 10_000_000;
        let time = format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            (seconds % 3600) / 60,
            seconds % 60
        );
        card.append(
            &gtk::Label::builder()
                .label(&time)
                .xalign(0.0)
                .css_classes(["dim-label", "caption"])
                .build(),
        );
        let button = gtk::Button::builder()
            .child(&card)
            .css_classes(["flat", "chapter-card"])
            .build();
        button.connect_clicked({
            let item = item.clone();
            let ticks = chapter.start_position_ticks.unwrap_or(0);
            move |_| {
                let mut item = item.clone();
                if let Some(user_data) = item.user_data.as_mut() {
                    user_data.playback_position_ticks = Some(ticks);
                }
                crate::app::APP_BROKER.send(crate::app::AppInput::PlayVideo(item));
            }
        });
        row.append(&button);
    }
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .child(&row)
        .build();
    section.append(&scroller);
    Some(section.upcast())
}
