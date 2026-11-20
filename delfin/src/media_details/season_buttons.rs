use gtk::prelude::*;
use jellyfin_api::types::BaseItemDto;
use relm4::prelude::*;

use crate::tr;

use super::seasons::SeasonsInput;

pub(crate) struct SeasonButtons;

#[relm4::component(pub(crate))]
impl SimpleComponent for SeasonButtons {
    type Init = (Vec<BaseItemDto>, usize);
    type Input = ();
    type Output = SeasonsInput;

    view! {
        gtk::FlowBox {
            add_css_class: "seasons-box",
            set_homogeneous: false,
            set_halign: gtk::Align::Start,
            set_selection_mode: gtk::SelectionMode::None,
            set_column_spacing: 8,
            set_row_spacing: 8,
            set_max_children_per_line: 20,
        }
    }

    fn init(
        init: Self::Init,
        seasons_box: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let (seasons, initial_selected_season_index) = init;

        let model = SeasonButtons;

        let widgets = view_output!();

        let btn_toggle_handler = |index: usize| {
            let sender = sender.clone();
            move |btn: &gtk::ToggleButton| {
                if btn.is_active() {
                    sender.output(SeasonsInput::SeasonActivated(index)).unwrap();
                }
            }
        };

        // First button will is used to group remaining buttons
        let first_btn = create_season_btn(&seasons[0]);
        add_season_menu(&first_btn, &seasons[0], 0, &sender);
        first_btn.set_active(initial_selected_season_index == 0);
        first_btn.connect_toggled(btn_toggle_handler(0));
        seasons_box.append(&first_btn);

        for (index, season) in seasons.iter().enumerate().skip(1) {
            let season_btn = create_season_btn(season);
            add_season_menu(&season_btn, season, index, &sender);
            season_btn.set_group(Some(&first_btn));
            season_btn.set_active(initial_selected_season_index == index);
            season_btn.connect_toggled(btn_toggle_handler(index));
            seasons_box.append(&season_btn);
        }

        ComponentParts { model, widgets }
    }
}

/// Rechtsklick auf eine Staffel: ganze Staffel als (un)gesehen markieren.
fn add_season_menu(
    button: &gtk::ToggleButton,
    season: &BaseItemDto,
    index: usize,
    sender: &ComponentSender<SeasonButtons>,
) {
    let played = season
        .user_data
        .as_ref()
        .and_then(|d| d.played)
        .unwrap_or(false);
    let action = gtk::Button::builder()
        .label(if played {
            tr!("media-details-season-mark-unplayed").to_string()
        } else {
            tr!("media-details-season-mark-played").to_string()
        })
        .css_classes(["flat"])
        .build();
    let popover = gtk::Popover::builder()
        .child(&action)
        .has_arrow(false)
        .build();
    popover.set_parent(button);
    button.connect_destroy({
        let popover = popover.clone();
        move |_| popover.unparent()
    });
    action.connect_clicked({
        let sender = sender.clone();
        let popover = popover.clone();
        move |_| {
            popover.popdown();
            sender
                .output(SeasonsInput::MarkSeasonPlayed(index, !played))
                .unwrap();
        }
    });

    let gesture = gtk::GestureClick::builder()
        .button(gtk::gdk::BUTTON_SECONDARY)
        .build();
    gesture.connect_pressed(move |gesture, _, x, y| {
        gesture.set_state(gtk::EventSequenceState::Claimed);
        popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        popover.popup();
    });
    button.add_controller(gesture);
}

fn create_season_btn(season: &BaseItemDto) -> gtk::ToggleButton {
    let btn_contents = gtk::Overlay::new();

    let name = season
        .name
        .clone()
        .unwrap_or(tr!("media-details-unnamed-season").to_string());

    btn_contents.set_child(Some(
        &gtk::Label::builder().label(name).build(),
    ));

    if let Some(user_data) = &season.user_data {
        if !user_data.played.unwrap_or(false) {
            btn_contents.add_overlay(
                &gtk::Box::builder()
                    .css_classes(["season-unplayed-indicator"])
                    .width_request(8)
                    .height_request(8)
                    .halign(gtk::Align::End)
                    .valign(gtk::Align::Start)
                    .tooltip_text(
                        if let Some(unplayed_item_count) = user_data.unplayed_item_count {
                            tr!("media-details-season-tooltip", {"unplayedItemCount" => unplayed_item_count}).to_string()
                        } else {
                            tr!("media-details-season-tooltip.unknown-item-count").to_string()
                        },
                    )
                    .build(),
            );
        }
    }

    gtk::ToggleButton::builder()
        .child(&btn_contents)
        .css_classes(["season-pill"])
        .build()
}
