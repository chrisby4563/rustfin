//! Player chapter button: list every chapter with preview, time, and name; a
//! click seeks there. The button stays hidden without chapters.

use std::{cell::RefCell, sync::Arc};

use gtk::{gdk, glib, prelude::*};
use jellyfin_api::types::ChapterInfo;
use relm4::prelude::*;
use uuid::Uuid;

use crate::{
    jellyfin_api::api_client::ApiClient,
    tr,
    utils::{message_broker::ResettableMessageBroker, ticks::ticks_to_seconds},
    video_player::backends::VideoPlayerBackend,
};

use super::scrubber::seconds_to_timestamp;

const THUMB_WIDTH: i32 = 160;
const THUMB_HEIGHT: i32 = 90;

pub(crate) static CHAPTERS_BROKER: ResettableMessageBroker<ChaptersInput> =
    ResettableMessageBroker::new();

pub(crate) struct Chapters {
    video_player: Arc<RefCell<dyn VideoPlayerBackend>>,
    /// Start time in seconds per row.
    starts: Vec<usize>,
    thumbnails: Vec<gtk::Picture>,
    visible: bool,
}

#[derive(Debug)]
pub(crate) enum ChaptersInput {
    Reset,
    Set {
        api_client: Arc<ApiClient>,
        item_id: Uuid,
        chapters: Vec<ChapterInfo>,
    },
    Activate(usize),
}

#[derive(Debug)]
pub(crate) enum ChaptersCommandOutput {
    Thumbnail(usize, Vec<u8>),
}

#[relm4::component(pub(crate))]
impl Component for Chapters {
    type Init = Arc<RefCell<dyn VideoPlayerBackend>>;
    type Input = ChaptersInput;
    type Output = ();
    type CommandOutput = ChaptersCommandOutput;

    view! {
        gtk::MenuButton {
            set_icon_name: "view-list-bullet-symbolic",
            set_tooltip_text: Some(tr!("vp-chapters-tooltip")),
            set_direction: gtk::ArrowType::Up,
            #[watch]
            set_visible: model.visible,

            #[wrap(Some)]
            #[name = "popover"]
            set_popover = &gtk::Popover {
                gtk::ScrolledWindow {
                    set_hscrollbar_policy: gtk::PolicyType::Never,
                    set_propagate_natural_height: true,
                    set_propagate_natural_width: true,
                    set_max_content_height: 480,

                    #[name = "list"]
                    gtk::ListBox {
                        set_selection_mode: gtk::SelectionMode::None,
                        add_css_class: "navigation-sidebar",
                        connect_row_activated[sender] => move |_, row| {
                            sender.input(ChaptersInput::Activate(row.index() as usize));
                        },
                    },
                },
            },
        }
    }

    fn init(
        video_player: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Chapters {
            video_player,
            starts: vec![],
            thumbnails: vec![],
            visible: false,
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            ChaptersInput::Reset => {
                widgets.list.remove_all();
                self.starts.clear();
                self.thumbnails.clear();
                self.visible = false;
            }
            ChaptersInput::Set {
                api_client,
                item_id,
                chapters,
            } => {
                widgets.list.remove_all();
                self.starts.clear();
                self.thumbnails.clear();

                let mut chapters: Vec<(usize, ChapterInfo)> = chapters
                    .into_iter()
                    .enumerate()
                    .filter(|(_, c)| c.start_position_ticks.is_some())
                    .collect();
                chapters.sort_by_key(|(_, c)| c.start_position_ticks);

                for (row_index, (server_index, chapter)) in chapters.into_iter().enumerate() {
                    let start = ticks_to_seconds(chapter.start_position_ticks.unwrap()) as usize;
                    let name = chapter
                        .name
                        .clone()
                        .filter(|n| !n.is_empty())
                        .unwrap_or_else(|| format!("{} {}", tr!("vp-chapter"), row_index + 1));

                    let picture = gtk::Picture::builder()
                        .width_request(THUMB_WIDTH)
                        .height_request(THUMB_HEIGHT)
                        .content_fit(gtk::ContentFit::Cover)
                        .css_classes(["chapter-thumbnail"])
                        .build();
                    let time = gtk::Label::builder()
                        .label(seconds_to_timestamp(start))
                        .css_classes(["dim-label", "numeric"])
                        .halign(gtk::Align::Start)
                        .build();
                    let title = gtk::Label::builder()
                        .label(&name)
                        .halign(gtk::Align::Start)
                        .ellipsize(gtk::pango::EllipsizeMode::End)
                        .max_width_chars(28)
                        .build();
                    let text = gtk::Box::builder()
                        .orientation(gtk::Orientation::Vertical)
                        .valign(gtk::Align::Center)
                        .spacing(2)
                        .build();
                    text.append(&title);
                    text.append(&time);
                    let row = gtk::Box::builder().spacing(12).build();
                    row.append(&picture);
                    row.append(&text);
                    widgets.list.append(&row);

                    if let Some(tag) = chapter.image_tag.clone() {
                        if let Ok(url) =
                            api_client.get_chapter_image_url(&item_id, server_index, &tag)
                        {
                            sender.oneshot_command(async move {
                                let bytes = match reqwest::get(&url).await {
                                    Ok(r) if r.status().is_success() => {
                                        r.bytes().await.map(|b| b.to_vec()).unwrap_or_default()
                                    }
                                    _ => vec![],
                                };
                                ChaptersCommandOutput::Thumbnail(row_index, bytes)
                            });
                        }
                    } else {
                        picture.set_visible(false);
                    }

                    self.starts.push(start);
                    self.thumbnails.push(picture);
                }
                // A single chapter (“Chapter 1” at 0:00) is useless.
                self.visible = self.starts.len() > 1;
            }
            ChaptersInput::Activate(index) => {
                if let Some(start) = self.starts.get(index) {
                    if !crate::syncplay::intercept_seek(*start) {
                        self.video_player.borrow().seek_to(*start);
                    }
                }
                widgets.popover.popdown();
            }
        }
        self.update_view(widgets, sender);
    }

    fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            ChaptersCommandOutput::Thumbnail(index, bytes) => {
                let Some(picture) = self.thumbnails.get(index) else {
                    return;
                };
                match gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes)) {
                    Ok(texture) => picture.set_paintable(Some(&texture)),
                    Err(_) => picture.set_visible(false),
                }
            }
        }
    }
}
