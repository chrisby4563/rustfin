//! Startseite: großes Titelbild „Weiterschauen“ (Konzept Startseite).

use std::{cell::RefCell, rc::Rc, sync::Arc};

use gtk::prelude::*;
use jellyfin_api::types::{BaseItemDto, BaseItemKind};
use relm4::{adw, gtk};
use adw::prelude::*;

use crate::{
    app::{AppInput, APP_BROKER},
    jellyfin_api::{api::latest::GetNextUpOptionsBuilder, api_client::ApiClient},
    tr,
};

const HERO_HEIGHT: i32 = 400;
const HERO_MAX: usize = 5;

struct Slot {
    /// Zwei Bilder im Stapel: das neue wird hinten gefüllt, dann übergeblendet.
    stack: gtk::Stack,
    pictures: [gtk::Picture; 2],
    front: usize,
    hovered: bool,
    text: gtk::Box,
    title: gtk::Label,
    meta: gtk::Label,
    progress: gtk::ProgressBar,
    dots: gtk::Box,
    items: Vec<BaseItemDto>,
    textures: Vec<Option<gtk::gdk::Texture>>,
    current: usize,
}

/// Baut das Titelbild. Bleibt unsichtbar, solange nichts zum Weiterschauen da ist.
pub fn hero(api_client: Arc<ApiClient>) -> gtk::Widget {
    let overlay = gtk::Overlay::builder()
        .height_request(HERO_HEIGHT)
        .overflow(gtk::Overflow::Hidden)
        .css_classes(["home-hero"])
        .visible(false)
        .build();
    let make_picture = || {
        gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .can_shrink(true)
            .hexpand(true)
            .height_request(HERO_HEIGHT)
            .build()
    };
    let pictures = [make_picture(), make_picture()];
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .transition_duration(600)
        .build();
    stack.add_named(&pictures[0], Some("a"));
    stack.add_named(&pictures[1], Some("b"));
    overlay.set_child(Some(&stack));
    overlay.add_overlay(
        &gtk::Box::builder()
            .css_classes(["home-hero-shade"])
            .hexpand(true)
            .vexpand(true)
            .build(),
    );

    let text = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .valign(gtk::Align::End)
        .halign(gtk::Align::Start)
        .margin_start(32)
        .margin_bottom(36)
        .width_request(320)
        .build();
    let kicker = gtk::Label::builder()
        .label(tr!("home-hero-kicker"))
        .xalign(0.0)
        .css_classes(["home-hero-kicker"])
        .build();
    let title = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .lines(2)
        .max_width_chars(28)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["home-hero-title"])
        .build();
    let meta = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .max_width_chars(60)
        .css_classes(["detail-meta"])
        .build();
    let progress = gtk::ProgressBar::builder()
        .halign(gtk::Align::Start)
        .width_request(320)
        .css_classes(["detail-progress"])
        .build();
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    buttons.set_margin_top(8);
    let play = gtk::Button::builder()
        .css_classes(["pill", "suggested-action"])
        .build();
    let play_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    play_box.append(&gtk::Image::from_icon_name("play-filled"));
    play_box.append(&gtk::Label::new(Some(tr!("home-hero-resume"))));
    play.set_child(Some(&play_box));
    let details = gtk::Button::builder()
        .label(tr!("home-hero-details"))
        .css_classes(["pill", "detail-secondary"])
        .build();
    buttons.append(&play);
    buttons.append(&details);
    for w in [kicker.upcast_ref::<gtk::Widget>(), title.upcast_ref(), meta.upcast_ref(), progress.upcast_ref(), buttons.upcast_ref()] {
        text.append(w);
    }
    overlay.add_overlay(&text);

    let dots = gtk::Box::builder()
        .spacing(6)
        .halign(gtk::Align::End)
        .valign(gtk::Align::End)
        .margin_end(32)
        .margin_bottom(28)
        .build();
    overlay.add_overlay(&dots);

    let slot = Rc::new(RefCell::new(Slot {
        stack,
        pictures,
        front: 0,
        hovered: false,
        text: text.clone(),
        title,
        meta,
        progress,
        dots,
        items: Vec::new(),
        textures: Vec::new(),
        current: 0,
    }));

    // Maus darüber: nicht weiterblättern.
    let motion = gtk::EventControllerMotion::new();
    motion.connect_enter({
        let slot = slot.clone();
        move |_, _, _| slot.borrow_mut().hovered = true
    });
    motion.connect_leave({
        let slot = slot.clone();
        move |_| slot.borrow_mut().hovered = false
    });
    overlay.add_controller(motion);

    // Alle 8 s zum nächsten Titel; endet, wenn die Startseite weg ist.
    let weak = overlay.downgrade();
    gtk::glib::timeout_add_seconds_local(8, {
        let slot = slot.clone();
        move || {
            let Some(overlay) = weak.upgrade() else {
                return gtk::glib::ControlFlow::Break;
            };
            let (hovered, count, current) = {
                let s = slot.borrow();
                (s.hovered, s.items.len(), s.current)
            };
            if !hovered && count > 1 && overlay.is_mapped() {
                show(&slot, (current + 1) % count);
            }
            gtk::glib::ControlFlow::Continue
        }
    });

    play.connect_clicked({
        let slot = slot.clone();
        move |_| {
            if let Some(item) = slot.borrow().items.get(slot.borrow().current) {
                APP_BROKER.send(AppInput::PlayVideo(item.clone()));
            }
        }
    });
    details.connect_clicked({
        let slot = slot.clone();
        move |_| {
            if let Some(item) = slot.borrow().items.get(slot.borrow().current) {
                APP_BROKER.send(AppInput::ShowDetails(item.clone()));
            }
        }
    });

    // Laden: Liste holen, dann Bilder nacheinander.
    gtk::glib::spawn_future_local({
        let overlay = overlay.clone();
        let slot = slot.clone();
        async move {
            let items = {
                let api_client = api_client.clone();
                run_bg(async move {
                    api_client
                        .get_continue_watching(
                            GetNextUpOptionsBuilder::default()
                                .limit(HERO_MAX)
                                .build()
                                .unwrap(),
                        )
                        .await
                        .unwrap_or_default()
                })
                .await
                .unwrap_or_default()
            };
            if items.is_empty() {
                return;
            }
            {
                let mut s = slot.borrow_mut();
                s.textures = vec![None; items.len()];
                s.items = items.clone();
            }
            build_dots(&slot);
            show(&slot, 0);
            overlay.set_visible(true);
            for (index, item) in items.iter().enumerate() {
                let Ok(url) = api_client.get_wide_backdrop_url(item) else {
                    continue;
                };
                let bytes = run_bg(async move {
                    match reqwest::get(url).await {
                        Ok(r) => r.bytes().await.ok().map(|b| b.to_vec()),
                        Err(_) => None,
                    }
                })
                .await
                .flatten();
                if let Some(bytes) = bytes {
                    if let Ok(texture) =
                        gtk::gdk::Texture::from_bytes(&gtk::glib::Bytes::from_owned(bytes))
                    {
                        slot.borrow_mut().textures[index] = Some(texture);
                        if slot.borrow().current == index {
                            show(&slot, index);
                        }
                    }
                }
            }
        }
    });

    overlay.upcast()
}

async fn run_bg<T: Send + 'static>(
    future: impl std::future::Future<Output = T> + Send + 'static,
) -> Option<T> {
    let (tx, rx) = relm4::tokio::sync::oneshot::channel();
    relm4::spawn(async move {
        let _ = tx.send(future.await);
    });
    rx.await.ok()
}

fn build_dots(slot: &Rc<RefCell<Slot>>) {
    let count = slot.borrow().items.len();
    if count < 2 {
        return;
    }
    let dots = slot.borrow().dots.clone();
    for index in 0..count {
        let dot = gtk::Button::builder()
            .css_classes(["home-hero-dot"])
            .valign(gtk::Align::Center)
            .build();
        dot.connect_clicked({
            let slot = slot.clone();
            move |_| show(&slot, index)
        });
        dots.append(&dot);
    }
}

fn show(slot: &Rc<RefCell<Slot>>, index: usize) {
    let mut s = slot.borrow_mut();
    let Some(item) = s.items.get(index).cloned() else {
        return;
    };
    let changed = s.current != index;
    s.current = index;
    if changed {
        // Neues Bild hinten einsetzen und überblenden.
        let back = 1 - s.front;
        s.pictures[back].set_paintable(s.textures[index].as_ref());
        s.stack.set_visible_child(&s.pictures[back]);
        s.front = back;
        // Text weich einblenden.
        let text = s.text.clone();
        let target = adw::CallbackAnimationTarget::new(move |value| text.set_opacity(value));
        adw::TimedAnimation::new(&s.text, 0.0, 1.0, 450, target).play();
    } else {
        s.pictures[s.front].set_paintable(s.textures[index].as_ref());
    }
    let title = item
        .series_name
        .clone()
        .or(item.name.clone())
        .unwrap_or_default();
    s.title.set_label(&title);
    let meta = meta_line(&item);
    s.meta.set_visible(!meta.is_empty());
    s.meta.set_label(&meta);
    let fraction = item
        .user_data
        .as_ref()
        .and_then(|d| d.played_percentage)
        .map(|p| (p / 100.0).clamp(0.0, 1.0))
        .unwrap_or(0.0);
    s.progress.set_fraction(fraction);
    s.progress.set_visible(fraction > 0.0);
    let mut child = s.dots.first_child();
    let mut i = 0;
    while let Some(dot) = child {
        if i == index {
            dot.add_css_class("active");
        } else {
            dot.remove_css_class("active");
        }
        child = dot.next_sibling();
        i += 1;
    }
}

fn meta_line(item: &BaseItemDto) -> String {
    let mut parts: Vec<String> = Vec::new();
    if matches!(item.type_, Some(BaseItemKind::Episode)) {
        if let Some(season) = item.parent_index_number {
            parts.push(tr!("home-hero-season", {"number" => season}).to_string());
        }
        if let Some(episode) = item.index_number {
            parts.push(tr!("home-hero-episode", {"number" => episode}).to_string());
        }
        if let Some(name) = &item.name {
            parts.push(name.clone());
        }
    } else if let Some(year) = item.production_year {
        parts.push(year.to_string());
    }
    if let Some(ticks) = item.run_time_ticks {
        let position = item
            .user_data
            .as_ref()
            .and_then(|d| d.playback_position_ticks)
            .unwrap_or(0);
        if position > 0 && position < ticks {
            parts.push(
                tr!("media-details-episode-remaining", {"minutes" => (ticks - position) / 600_000_000})
                    .to_string(),
            );
        }
    }
    parts.join(" · ")
}
