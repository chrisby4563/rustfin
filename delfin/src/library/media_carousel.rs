use std::sync::Arc;

use adw::prelude::*;
use gtk::{gdk, glib};
use jellyfin_api::types::BaseItemDto;
use relm4::{prelude::*, ComponentParts};

use crate::{globals::SHIFT_STATE, jellyfin_api::api_client::ApiClient};

use super::{
    media_button::MediaButton,
    media_tile::{MediaTile, MediaTileDisplay},
};

const MIN_PADDING: i32 = 24;
/// Dauer der Animation, wenn die Pfeilknöpfe eine Bildschirmbreite weiterblättern.
const PAGE_ANIMATION_MS: u32 = 300;
/// Anteil der sichtbaren Breite, den ein Pfeilklick weiterblättert. Etwas
/// weniger als eine volle Breite, damit eine angeschnittene Kachel sichtbar
/// bleibt und man die Orientierung behält.
const PAGE_FRACTION: f64 = 0.9;

pub(crate) enum MediaCarouselItem {
    Tile(AsyncController<MediaTile>),
    Button(Controller<MediaButton>),
}

impl MediaCarouselItem {
    fn widget(&self) -> &gtk::Widget {
        match self {
            MediaCarouselItem::Tile(media_tile) => media_tile.widget().upcast_ref(),
            MediaCarouselItem::Button(media_button) => media_button.widget().upcast_ref(),
        }
    }
}

pub(crate) enum MediaCarouselType {
    Tiles,
    Buttons,
}

/// Eine waagerechte Reihe von Kacheln.
///
/// Früher ein `adw::Carousel` mit festen Seiten: Wischen auf dem Touchpad zog
/// eine ganze Seite und liess sie einrasten. Jetzt ein frei scrollendes
/// `gtk::ScrolledWindow` – Touchpad wischt fliessend (mit Schwung), die
/// Pfeilknöpfe blättern animiert um gut eine Bildschirmbreite, Shift+Mausrad
/// scrollt die Reihe, das normale Mausrad scrollt die Seite.
pub(crate) struct MediaCarousel {
    media_tile_display: MediaTileDisplay,
    media_tiles: Vec<MediaCarouselItem>,
    /// Passt die Reihe nicht vollständig in die Breite? Nur dann Pfeile zeigen.
    overflowing: bool,
    animation: Option<adw::TimedAnimation>,
}

pub(crate) struct MediaCarouselInit {
    pub(crate) media: Vec<BaseItemDto>,
    pub(crate) media_tile_display: MediaTileDisplay,
    pub(crate) carousel_type: MediaCarouselType,
    pub(crate) api_client: Arc<ApiClient>,
    pub(crate) label: String,
    pub(crate) label_clickable: bool,
}

#[derive(Debug)]
pub(crate) enum MediaCarouselInput {
    Left,
    Right,
    Overflowing(bool),
}

#[derive(Debug)]
pub(crate) enum MediaCarouselOutput {
    LabelClicked,
}

#[relm4::component(pub(crate))]
impl Component for MediaCarousel {
    type Init = MediaCarouselInit;
    type Input = MediaCarouselInput;
    type Output = MediaCarouselOutput;
    type CommandOutput = ();

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 16,

            add_css_class: "media-carousel",

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,

                gtk::Box {
                    set_spacing: 4,
                    set_cursor_from_name: if label_clickable { Some("pointer") } else { None },
                    add_css_class?: label_clickable.then_some("clickable-title"),

                    gtk::Label {
                        set_label: label.as_str(),
                        add_css_class: "title-2",
                        set_halign: gtk::Align::Start,
                    },

                    gtk::Image::from_icon_name("right") {
                        set_visible: label_clickable,
                    },

                    add_controller?: if label_clickable {
                        let controller = gtk::GestureClick::new();
                        controller.connect_released({
                            let sender = sender.clone();
                            move |_, _, _, _| {
                                sender.output(MediaCarouselOutput::LabelClicked).unwrap();
                            }
                        });
                        Some(controller)
                    } else {
                        None
                    },
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_halign: gtk::Align::End,
                    set_hexpand: true,
                    add_css_class: "linked",
                    #[watch]
                    set_visible: model.overflowing,

                    gtk::Button {
                        set_icon_name: "left",
                        add_css_class: "flat",
                        connect_clicked[sender] => move |_| {
                            sender.input(MediaCarouselInput::Left);
                        },
                    },

                    gtk::Button {
                        set_icon_name: "right",
                        add_css_class: "flat",
                        connect_clicked[sender] => move |_| {
                            sender.input(MediaCarouselInput::Right);
                        },
                    },
                },
            },

            #[name = "scroller"]
            gtk::ScrolledWindow {
                set_hscrollbar_policy: gtk::PolicyType::Automatic,
                set_vscrollbar_policy: gtk::PolicyType::Never,
                set_propagate_natural_height: true,
                set_kinetic_scrolling: true,
                set_hexpand: true,
                set_min_content_height: model.media_tile_display.height(),

                // Mausrad (nicht Touchpad) gehört der Seite: ein waagerechtes
                // ScrolledWindow würde das senkrechte Rad sonst in waagerechtes
                // Scrollen umdeuten und die Seite blockieren. Mit Shift scrollt
                // das Rad die Reihe.
                add_controller = gtk::EventControllerScroll {
                    set_flags: gtk::EventControllerScrollFlags::VERTICAL,
                    set_propagation_phase: gtk::PropagationPhase::Capture,
                    connect_scroll => move |controller, _dx, dy| {
                        if controller.unit() != gdk::ScrollUnit::Wheel {
                            return glib::Propagation::Proceed;
                        }
                        let Ok(scroller) = controller
                            .widget()
                            .downcast::<gtk::ScrolledWindow>()
                        else {
                            return glib::Propagation::Proceed;
                        };
                        if SHIFT_STATE.read().pressed() {
                            scroll_by_wheel(&scroller.hadjustment(), dy);
                        } else if let Some(page) = scroller
                            .parent()
                            .and_then(|parent| parent.ancestor(gtk::ScrolledWindow::static_type()))
                            .and_downcast::<gtk::ScrolledWindow>()
                        {
                            scroll_by_wheel(&page.vadjustment(), dy);
                        }
                        glib::Propagation::Stop
                    },
                },

                #[name = "row"]
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: MIN_PADDING,
                    set_halign: gtk::Align::Start,
                },
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: relm4::ComponentSender<Self>,
    ) -> relm4::ComponentParts<Self> {
        let MediaCarouselInit {
            api_client,
            media,
            media_tile_display,
            carousel_type,
            label,
            label_clickable,
        } = init;

        let media_tiles: Vec<MediaCarouselItem> = media
            .iter()
            .map(|media| match carousel_type {
                MediaCarouselType::Tiles => MediaCarouselItem::Tile(
                    MediaTile::builder()
                        .launch((media.clone(), media_tile_display, api_client.clone()))
                        .detach(),
                ),
                MediaCarouselType::Buttons => MediaCarouselItem::Button(
                    MediaButton::builder()
                        .launch((media.clone(), media_tile_display))
                        .detach(),
                ),
            })
            .collect();

        let model = MediaCarousel {
            media_tile_display,
            media_tiles,
            overflowing: false,
            animation: None,
        };

        let widgets = view_output!();

        for tile in &model.media_tiles {
            widgets.row.append(tile.widget());
        }

        // Pfeile nur zeigen, wenn die Reihe breiter ist als der sichtbare Teil.
        let hadjustment = widgets.scroller.hadjustment();
        let update_overflow = {
            let sender = sender.clone();
            move |adjustment: &gtk::Adjustment| {
                sender.input(MediaCarouselInput::Overflowing(
                    adjustment.upper() - adjustment.page_size() > 1.0,
                ));
            }
        };
        hadjustment.connect_changed(update_overflow.clone());
        update_overflow(&hadjustment);

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
            MediaCarouselInput::Overflowing(overflowing) => {
                if self.overflowing == overflowing {
                    return;
                }
                self.overflowing = overflowing;
            }
            MediaCarouselInput::Left => self.page(&widgets.scroller, -1.0),
            MediaCarouselInput::Right => self.page(&widgets.scroller, 1.0),
        }
        self.update_view(widgets, sender);
    }
}

impl MediaCarousel {
    /// Blättert animiert um gut eine sichtbare Breite. Am Ende angekommen geht
    /// es wie beim alten Karussell wieder an den Anfang (und umgekehrt).
    fn page(&mut self, scroller: &gtk::ScrolledWindow, direction: f64) {
        let adjustment = scroller.hadjustment();
        let max = (adjustment.upper() - adjustment.page_size()).max(0.0);
        if max <= 0.0 {
            return;
        }

        // Läuft noch eine Animation, von deren Ziel aus weiterrechnen, damit
        // schnelle Doppelklicks nicht verschluckt werden.
        let from = adjustment.value();
        let base = match &self.animation {
            Some(animation) if animation.state() == adw::AnimationState::Playing => {
                animation.value_to()
            }
            _ => from,
        };

        let step = adjustment.page_size() * PAGE_FRACTION;
        let at_start = base <= 0.5;
        let at_end = base >= max - 0.5;
        let to = if direction > 0.0 && at_end {
            0.0
        } else if direction < 0.0 && at_start {
            max
        } else {
            (base + direction * step).clamp(0.0, max)
        };

        if let Some(animation) = self.animation.take() {
            animation.skip();
        }

        let target = adw::CallbackAnimationTarget::new({
            let adjustment = adjustment.clone();
            move |value| adjustment.set_value(value)
        });
        let animation =
            adw::TimedAnimation::new(scroller, adjustment.value(), to, PAGE_ANIMATION_MS, target);
        animation.set_easing(adw::Easing::EaseOutCubic);
        animation.play();
        self.animation = Some(animation);
    }
}

/// Scrollt eine Achse um so viel, wie GTK selbst für einen Mausrad-Raster
/// nimmt (page_size^(2/3)), damit es sich wie überall sonst anfühlt.
pub(crate) fn scroll_by_wheel(adjustment: &gtk::Adjustment, delta: f64) {
    let step = adjustment.page_size().powf(2.0 / 3.0);
    let max = adjustment.upper() - adjustment.page_size();
    adjustment.set_value((adjustment.value() + delta * step).clamp(0.0, max.max(0.0)));
}
