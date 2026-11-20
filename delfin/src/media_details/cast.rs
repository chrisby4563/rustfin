//! Cast on the detail page: horizontal row with circular portraits, names, and roles.

use std::sync::Arc;

use adw::prelude::*;
use gtk::{gdk, glib};
use jellyfin_api::types::{BaseItemPerson, PersonKind};
use relm4::prelude::*;

use crate::{
    globals::SHIFT_STATE, jellyfin_api::api_client::ApiClient,
    library::media_carousel::scroll_by_wheel, tr,
};

/// The Jellyfin web UI does not usefully show more either.
const MAX_PEOPLE: usize = 40;
const AVATAR_SIZE: i32 = 96;
const PERSON_WIDTH: i32 = 112;

pub(crate) struct CastRow {
    avatars: Vec<adw::Avatar>,
}

#[derive(Debug)]
pub(crate) enum CastCommandOutput {
    Image(usize, Vec<u8>),
}

#[relm4::component(pub(crate))]
impl Component for CastRow {
    type Init = (Arc<ApiClient>, Vec<BaseItemPerson>);
    type Input = ();
    type Output = ();
    type CommandOutput = CastCommandOutput;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 12,

            gtk::Label {
                set_label: tr!("media-details-cast"),
                add_css_class: "title-3",
                set_halign: gtk::Align::Start,
            },

            #[name = "scroller"]
            gtk::ScrolledWindow {
                set_hscrollbar_policy: gtk::PolicyType::Automatic,
                set_vscrollbar_policy: gtk::PolicyType::Never,
                set_propagate_natural_height: true,

                // The mouse wheel belongs to the page; Shift+wheel scrolls the row
                // (as with home page rows).
                add_controller = gtk::EventControllerScroll {
                    set_flags: gtk::EventControllerScrollFlags::VERTICAL,
                    set_propagation_phase: gtk::PropagationPhase::Capture,
                    connect_scroll => move |controller, _dx, dy| {
                        if controller.unit() != gdk::ScrollUnit::Wheel {
                            return glib::Propagation::Proceed;
                        }
                        let Ok(scroller) = controller.widget().downcast::<gtk::ScrolledWindow>() else {
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
                    set_spacing: 12,
                    set_margin_bottom: 8,
                },
            },
        }
    }

    fn init(
        (api_client, people): Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mut model = CastRow { avatars: vec![] };
        let widgets = view_output!();

        for (index, person) in people.into_iter().take(MAX_PEOPLE).enumerate() {
            let name = person.name.clone().unwrap_or_default();
            let avatar = adw::Avatar::new(AVATAR_SIZE, Some(&name), true);

            let name_label = gtk::Label::builder()
                .label(&name)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .max_width_chars(1)
                .css_classes(["heading"])
                .build();
            let role_label = gtk::Label::builder()
                .label(role_text(&person))
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .max_width_chars(1)
                .css_classes(["dim-label", "caption"])
                .build();

            let tile = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(4)
                .width_request(PERSON_WIDTH)
                .tooltip_text(format!("{name}\n{}", role_text(&person)))
                .build();
            tile.append(&avatar);
            tile.append(&name_label);
            tile.append(&role_label);
            widgets.row.append(&tile);
            model.avatars.push(avatar);

            if let (Some(id), Some(tag)) = (person.id, person.primary_image_tag.as_deref()) {
                if let Ok(url) = api_client.get_person_image_url(&id, tag) {
                    sender.oneshot_command(async move {
                        let bytes = match reqwest::get(&url).await {
                            Ok(response) if response.status().is_success() => {
                                response.bytes().await.map(|b| b.to_vec()).unwrap_or_default()
                            }
                            _ => vec![],
                        };
                        CastCommandOutput::Image(index, bytes)
                    });
                }
            }
        }

        ComponentParts { model, widgets }
    }

    fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            CastCommandOutput::Image(index, bytes) => {
                if bytes.is_empty() {
                    return;
                }
                let Some(avatar) = self.avatars.get(index) else {
                    return;
                };
                if let Ok(texture) = gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes)) {
                    avatar.set_custom_image(Some(&texture));
                    crate::utils::fade::fade_in(avatar.upcast_ref());
                }
            }
        }
    }
}

/// People that belong in the cast row: director and writer first,
/// dann Darsteller in Server-Reihenfolge (Hauptrollen vorn).
pub(crate) fn cast_people(people: &[BaseItemPerson]) -> Vec<BaseItemPerson> {
    let rank = |person: &BaseItemPerson| match person.type_ {
        PersonKind::Director => Some(0),
        PersonKind::Writer => Some(1),
        PersonKind::Actor | PersonKind::GuestStar => Some(2),
        _ => None,
    };
    let mut selected: Vec<(usize, &BaseItemPerson)> = people
        .iter()
        .filter_map(|person| rank(person).map(|r| (r, person)))
        .collect();
    selected.sort_by_key(|(r, _)| *r); // stabil: Reihenfolge innerhalb bleibt
    selected.into_iter().map(|(_, p)| p.clone()).collect()
}

fn role_text(person: &BaseItemPerson) -> String {
    match person.type_ {
        PersonKind::Director => tr!("media-details-cast-director").to_string(),
        PersonKind::Writer => tr!("media-details-cast-writer").to_string(),
        _ => person.role.clone().filter(|r| !r.is_empty()).unwrap_or_default(),
    }
}
