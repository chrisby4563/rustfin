use std::{cell::OnceCell, collections::VecDeque, sync::Arc};

use adw::prelude::*;
use gdk::Texture;
use gtk::gdk;
use jellyfin_api::types::BaseItemDto;
use relm4::prelude::*;

use crate::{
    app::{AppInput, APP_BROKER},
    jellyfin_api::api_client::ApiClient,
    library::LIBRARY_REFRESH_QUEUED,
    media_details::watched_state::{watched_label, Played},
    tr,
};

use super::{watched_state::toggle_watched, MediaDetailsInput, MEDIA_DETAILS_BROKER};

/// Folgenkarte: Vorschaubild 16:9.
const EPISODE_THUMB_W: i32 = 200;
const EPISODE_THUMB_H: i32 = 112;

pub(crate) struct Episode {
    media: BaseItemDto,
    api_client: Arc<ApiClient>,
    thumbnail: OnceCell<Controller<EpisodeThumbnail>>,
    /// Medieninfo schon geladen (Popover am Info-Knopf).
    info_loaded: bool,
    versions_loaded: bool,
    hidden: bool,
}

#[derive(Debug)]
pub(crate) enum EpisodeInput {
    ToggleWatched(bool),
    LoadMediaInfo,
    LoadVersions,
    Delete,
    Deleted,
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

            // Erste Zeile: Laufzeit / Rest, dann erster Absatz der Beschreibung.
            set_subtitle: &episode_subtitle(&model.media),
            set_subtitle_lines: 4,

            #[watch]
            set_visible: !model.hidden,

            // Mehrere Fassungen der Folge: Auswahl wie bei Filmen.
            add_suffix = &gtk::MenuButton {
                set_icon_name: "view-paged-symbolic",
                set_css_classes: &["image-button", "flat"],
                set_valign: gtk::Align::Center,
                set_tooltip: tr!("media-details-version"),
                set_visible: model.media.media_source_count.unwrap_or(1) > 1,

                #[wrap(Some)]
                #[name = "versions_popover"]
                set_popover = &gtk::Popover {
                    connect_show[sender] => move |_| {
                        sender.input(EpisodeInput::LoadVersions);
                    },
                    gtk::Spinner {
                        set_spinning: true,
                        set_margin_all: 12,
                    },
                },
            },

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
            versions_loaded: false,
            hidden: false,
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

        // Rechtsklick / lange drücken: Löschen (nur wenn das Konto darf).
        if crate::utils::delete_item::can_delete() {
            let menu = gtk::Popover::new();
            menu.set_parent(&root);
            menu.set_has_arrow(true);
            let delete = gtk::Button::builder()
                .label(tr!("delete-menu"))
                .css_classes(["flat", "destructive-action"])
                .build();
            delete.connect_clicked({
                let sender = sender.clone();
                let menu = menu.clone();
                move |_| {
                    menu.popdown();
                    sender.input(EpisodeInput::Delete);
                }
            });
            menu.set_child(Some(&delete));
            let show = {
                let menu = menu.clone();
                move |x: f64, y: f64| {
                    menu.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
                    menu.popup();
                }
            };
            let click = gtk::GestureClick::new();
            click.set_button(gtk::gdk::BUTTON_SECONDARY);
            click.connect_pressed({
                let show = show.clone();
                move |_, _, x, y| show(x, y)
            });
            root.add_controller(click);
            let long_press = gtk::GestureLongPress::new();
            long_press.set_touch_only(true);
            long_press.connect_pressed(move |_, x, y| show(x, y));
            root.add_controller(long_press);
            root.connect_destroy(move |_| menu.unparent());
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
            EpisodeInput::LoadVersions => {
                if self.versions_loaded {
                    return;
                }
                let Some(id) = self.media.id else {
                    return;
                };
                let child: gtk::Widget = match self.api_client.get_item(&id).await {
                    Ok(item) => versions_list(id, &item),
                    Err(err) => gtk::Label::new(Some(&err.to_string())).upcast(),
                };
                widgets.versions_popover.set_child(Some(&child));
                self.versions_loaded = true;
            }
            EpisodeInput::Delete => {
                let sender = sender.clone();
                crate::utils::delete_item::confirm_delete(
                    &self.media,
                    self.api_client.clone(),
                    move || sender.input(EpisodeInput::Deleted),
                );
            }
            EpisodeInput::Deleted => {
                self.hidden = true;
            }
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
    progress: f64,
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
                set_margin_top: 6,
                set_margin_bottom: 6,
                set_margin_end: 6,
                set_overflow: gtk::Overflow::Hidden,
                add_css_class: "episode-thumbnail",

                gtk::Picture {
                    #[watch]
                    set_paintable: model.thumbnail.as_ref(),
                    set_size_request: (EPISODE_THUMB_W, EPISODE_THUMB_H),
                    set_content_fit: gtk::ContentFit::Cover,
                },

                // Fortschritt als Akzentbalken unten.
                add_overlay = &gtk::ProgressBar {
                    set_fraction: model.progress,
                    set_visible: model.progress > 0.0,
                    set_valign: gtk::Align::End,
                    add_css_class: "episode-progress",
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
            api_client.get_episode_primary_image_url(&media, EPISODE_THUMB_H * 2)
        {
            sender.oneshot_command(async {
                let img_bytes: VecDeque<u8> = match reqwest::get(img_url).await {
                    Ok(response) => response
                        .bytes()
                        .await
                        .map(|b| b.into_iter().collect())
                        .unwrap_or_default(),
                    Err(_) => VecDeque::new(),
                };
                EpisodeThumbnailCommandOutput::LoadThumbnail(img_bytes)
            });
        }

        let progress = media
            .user_data
            .as_ref()
            .and_then(|d| if d.played.unwrap_or(false) { Some(100.0) } else { d.played_percentage })
            .map(|p| (p / 100.0).clamp(0.0, 1.0))
            .unwrap_or(0.0);
        let model = Self {
            media,
            thumbnail: None,
            progress,
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
                let bytes = gtk::glib::Bytes::from_owned(Vec::from(img_bytes));
                if let Ok(texture) = gdk::Texture::from_bytes(&bytes) {
                    self.thumbnail = Some(texture);
                }
            }
        }
    }
}

/// Fassungen einer Folge als Auswahlliste (wie die Versionsauswahl bei Filmen).
fn versions_list(item_id: uuid::Uuid, item: &BaseItemDto) -> gtk::Widget {
    use crate::globals::SELECTED_VERSIONS;
    let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    list.set_margin_top(6);
    list.set_margin_bottom(6);
    let sources = item.media_sources.clone().unwrap_or_default();
    let chosen = SELECTED_VERSIONS.read().get(&item_id).cloned();
    let mut group: Option<gtk::CheckButton> = None;
    for (index, source) in sources.iter().enumerate() {
        let name = source
            .name
            .clone()
            .unwrap_or_else(|| format!("Version {}", index + 1));
        let check = gtk::CheckButton::with_label(&name);
        if let Some(first) = &group {
            check.set_group(Some(first));
        } else {
            group = Some(check.clone());
        }
        let is_chosen = match &chosen {
            Some(chosen) => source.id.as_ref() == Some(chosen),
            None => index == 0,
        };
        check.set_active(is_chosen);
        let id = source.id.clone();
        check.connect_toggled(move |check| {
            if let (true, Some(id)) = (check.is_active(), &id) {
                SELECTED_VERSIONS.write().insert(item_id, id.clone());
            }
        });
        list.append(&check);
    }
    list.upcast()
}

fn episode_subtitle(media: &BaseItemDto) -> String {
    let mut meta: Vec<String> = Vec::new();
    if let Some(ticks) = media.run_time_ticks {
        meta.push(format!("{} min", ticks / 600_000_000));
        let position = media
            .user_data
            .as_ref()
            .and_then(|d| d.playback_position_ticks)
            .unwrap_or(0);
        if position > 0 && position < ticks {
            meta.push(
                tr!("media-details-episode-remaining", {"minutes" => (ticks - position) / 600_000_000})
                    .to_string(),
            );
        }
    }
    let overview = media
        .overview
        .as_deref()
        .map(|o| o.split_once('\n').map_or(o, |(first, _)| first))
        .unwrap_or("");
    match (meta.is_empty(), overview.is_empty()) {
        (true, _) => overview.to_string(),
        (false, true) => meta.join(" · "),
        (false, false) => format!("{}\n{}", meta.join(" · "), overview),
    }
}
