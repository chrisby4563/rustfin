use std::sync::Arc;

use adw::prelude::*;
use anyhow::Result;
use jellyfin_api::types::BaseItemDto;
use relm4::{
    actions::{AccelsPlus, RelmAction, RelmActionGroup},
    prelude::*,
};

use crate::{
    borgar::borgar_menu::{BorgarMenu, BorgarMenuAuth},
    config::{Account, Server},
    jellyfin_api::{
        api::views::{CollectionQuery, CollectionSort},
        api_client::ApiClient,
    },
    library::{
        media_page::{MediaPage, MediaPageInput},
        media_tile::MediaTileDisplay,
    },
    tr,
    utils::{empty_component::EmptyComponent, main_window::get_main_window},
};

use super::{media_fetcher::Fetcher, media_page::MediaPageInit};

pub struct Collection {
    api_client: Arc<ApiClient>,
    collection: BaseItemDto,
    borgar_menu: Controller<BorgarMenu>,
    media_page: Controller<MediaPage<CollectionItemsFetcher, EmptyComponent>>,
    /// Sortierung/Filter der Ansicht.
    query: CollectionQuery,
    genres: Vec<String>,
    genre_dropdown: gtk::DropDown,
    direction_button: gtk::Button,
    /// Bleiben über Neuladen hinweg bestehen und wandern in die neue Seite.
    filter_bar: gtk::Widget,
    letter_block: gtk::Widget,
}

#[derive(Debug)]
pub enum CollectionInput {
    Refresh,
    SetSort(u32),
    ToggleDirection,
    SetUnplayedOnly(bool),
    SetLetter(Option<char>),
    SetGenre(u32),
}

#[derive(Debug)]
pub enum CollectionCommandOutput {
    GenresLoaded(Vec<String>),
}

#[relm4::component(pub)]
impl Component for Collection {
    type Init = (Arc<ApiClient>, BaseItemDto, Server, Account);
    type Input = CollectionInput;
    type Output = ();
    type CommandOutput = CollectionCommandOutput;

    view! {
        adw::NavigationPage {
            #[name = "toolbar_view"]
            #[wrap(Some)]
            set_child = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    pack_end = model.borgar_menu.widget(),
                    pack_end = &gtk::Button::from_icon_name("refresh") {
                        set_tooltip: tr!("library-collection-refresh-button"),
                        connect_clicked[sender] => move |_| {
                            sender.input(CollectionInput::Refresh);
                        },
                    },
                },

                #[wrap(Some)]
                set_content = model.media_page.widget(),
            },

            set_title: &model.collection
                .name
                .as_ref()
                .unwrap_or(tr!("library-unnamed-collection"))
                .clone(),
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let (api_client, collection, server, account) = init;

        let query = CollectionQuery::default();
        let genre_dropdown = gtk::DropDown::from_strings(&[tr!("library-filter-all-genres")]);
        let direction_button = gtk::Button::from_icon_name(direction_icon(query.descending));
        let filter_bar: gtk::Widget =
            filter_bar(&sender, &genre_dropdown, &direction_button).upcast();
        // Buchstabenleiste, darunter Linie zum Inhalt.
        let letter_block = gtk::Box::new(gtk::Orientation::Vertical, 0);
        letter_block.append(&letter_bar(&sender));
        letter_block.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let letter_block: gtk::Widget = letter_block.upcast();

        let model = Collection {
            query: query.clone(),
            genres: Vec::new(),
            genre_dropdown: genre_dropdown.clone(),
            direction_button: direction_button.clone(),
            filter_bar: filter_bar.clone(),
            letter_block: letter_block.clone(),
            api_client: api_client.clone(),
            collection: collection.clone(),
            borgar_menu: BorgarMenu::builder()
                .launch(Some(BorgarMenuAuth {
                    api_client: api_client.clone(),
                    server,
                    account,
                }))
                .detach(),
            media_page: new_media_page(
                &api_client,
                collection.clone(),
                query,
                &filter_bar,
                &letter_block,
            ),
        };

        model.media_page.emit(MediaPageInput::NextPage);

        let widgets = view_output!();

        sender.oneshot_command({
            let api_client = api_client.clone();
            async move {
                CollectionCommandOutput::GenresLoaded(
                    api_client
                        .get_collection_genres(&collection)
                        .await
                        .unwrap_or_default(),
                )
            }
        });

        model.register_actions(&sender);

        ComponentParts { model, widgets }
    }

    fn shutdown(&mut self, _widgets: &mut Self::Widgets, _output: relm4::Sender<Self::Output>) {
        self.unregister_actions();
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        let previous = self.query.clone();
        match message {
            CollectionInput::Refresh => {
                self.reload(widgets);
            }
            CollectionInput::SetSort(index) => {
                let sort = CollectionSort::ALL
                    .get(index as usize)
                    .copied()
                    .unwrap_or_default();
                if sort != self.query.sort {
                    self.query.sort = sort;
                    self.query.descending = sort.default_descending();
                }
            }
            CollectionInput::ToggleDirection => {
                self.query.descending = !self.query.descending;
            }
            CollectionInput::SetUnplayedOnly(unplayed_only) => {
                self.query.unplayed_only = unplayed_only;
            }
            CollectionInput::SetLetter(letter) => {
                self.query.letter = letter;
            }
            CollectionInput::SetGenre(index) => {
                self.query.genre = index
                    .checked_sub(1)
                    .and_then(|i| self.genres.get(i as usize).cloned());
            }
        }
        if self.query != previous {
            self.direction_button
                .set_icon_name(direction_icon(self.query.descending));
            self.reload(widgets);
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
            CollectionCommandOutput::GenresLoaded(genres) => {
                let mut names = vec![tr!("library-filter-all-genres").to_string()];
                names.extend(genres.iter().cloned());
                let names: Vec<&str> = names.iter().map(String::as_str).collect();
                self.genres = genres;
                self.genre_dropdown
                    .set_model(Some(&gtk::StringList::new(&names)));
                self.genre_dropdown.set_selected(0);
                self.genre_dropdown.set_visible(!self.genres.is_empty());
            }
        }
    }
}

impl Collection {
    fn reload(&mut self, widgets: &mut CollectionWidgets) {
        detach(&self.filter_bar);
        detach(&self.letter_block);
        let media_page = new_media_page(
            &self.api_client,
            self.collection.clone(),
            self.query.clone(),
            &self.filter_bar,
            &self.letter_block,
        );
        widgets.toolbar_view.set_content(Some(media_page.widget()));
        media_page.emit(MediaPageInput::NextPage);
        self.media_page = media_page;
    }

    fn register_actions(&self, sender: &ComponentSender<Self>) {
        relm4::main_application().set_accelerators_for_action::<RefreshAction>(&["<Ctrl>r"]);

        let refresh_action: RelmAction<RefreshAction> = RelmAction::new_stateless({
            let sender = sender.clone();
            move |_| {
                sender.input(CollectionInput::Refresh);
            }
        });
        let mut group = RelmActionGroup::<CollectionActionGroup>::new();
        group.add_action(refresh_action);
        if let Some(main_window) = get_main_window() {
            group.register_for_widget(main_window);
        }
    }

    fn unregister_actions(&self) {
        relm4::main_application().set_accelerators_for_action::<RefreshAction>(&[]);
    }
}

/// Löst ein Widget aus der alten Seite, damit die neue es aufnehmen kann.
fn detach(widget: &gtk::Widget) {
    let Some(parent) = widget.parent() else {
        return;
    };
    if let Some(center_box) = parent.downcast_ref::<gtk::CenterBox>() {
        center_box.set_center_widget(None::<&gtk::Widget>);
    } else if let Some(container) = parent.downcast_ref::<gtk::Box>() {
        container.remove(widget);
    }
}

fn direction_icon(descending: bool) -> &'static str {
    if descending {
        "view-sort-descending-symbolic"
    } else {
        "view-sort-ascending-symbolic"
    }
}

/// Leiste mit Sortierung, Richtung, „Ungesehen“ und Genre.
fn filter_bar(
    sender: &ComponentSender<Collection>,
    genre_dropdown: &gtk::DropDown,
    direction_button: &gtk::Button,
) -> gtk::Box {
    let bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_start(12)
        .margin_end(12)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();

    let sort_names: Vec<String> = CollectionSort::ALL
        .iter()
        .map(|sort| match sort {
            CollectionSort::Name => tr!("library-sort-name").to_string(),
            CollectionSort::Year => tr!("library-sort-year").to_string(),
            CollectionSort::DateAdded => tr!("library-sort-date-added").to_string(),
            CollectionSort::Rating => tr!("library-sort-rating").to_string(),
        })
        .collect();
    let sort_names: Vec<&str> = sort_names.iter().map(String::as_str).collect();
    let sort_dropdown = gtk::DropDown::from_strings(&sort_names);
    sort_dropdown.connect_selected_notify({
        let sender = sender.clone();
        move |dropdown| sender.input(CollectionInput::SetSort(dropdown.selected()))
    });
    // Sortierung + Richtung als verbundene Gruppe.
    let sort_group = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    sort_group.add_css_class("linked");
    sort_group.append(&sort_dropdown);

    direction_button.set_tooltip_text(Some(tr!("library-sort-direction")));
    direction_button.connect_clicked({
        let sender = sender.clone();
        move |_| sender.input(CollectionInput::ToggleDirection)
    });
    sort_group.append(direction_button);
    bar.append(&sort_group);
    bar.append(&group_separator());

    let unplayed = gtk::ToggleButton::with_label(tr!("library-filter-unplayed"));
    unplayed.connect_toggled({
        let sender = sender.clone();
        move |button| sender.input(CollectionInput::SetUnplayedOnly(button.is_active()))
    });
    bar.append(&unplayed);
    bar.append(&group_separator());

    genre_dropdown.set_visible(false);
    genre_dropdown.set_enable_search(true);
    genre_dropdown.connect_selected_notify({
        let sender = sender.clone();
        move |dropdown| sender.input(CollectionInput::SetGenre(dropdown.selected()))
    });
    bar.append(genre_dropdown);

    bar
}

fn group_separator() -> gtk::Separator {
    let separator = gtk::Separator::new(gtk::Orientation::Vertical);
    separator.set_margin_top(6);
    separator.set_margin_bottom(6);
    separator.set_margin_start(4);
    separator.set_margin_end(4);
    separator
}

/// Buchstabenleiste: Alle, #, A–Z.
fn letter_bar(sender: &ComponentSender<Collection>) -> gtk::ScrolledWindow {
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(2)
        .halign(gtk::Align::Center)
        .margin_start(6)
        .margin_end(6)
        .build();
    // Hinterlegt als abgerundete Fläche, damit die Leiste als Einheit wirkt.
    row.add_css_class("card");

    let letters: Vec<Option<char>> = std::iter::once(None)
        .chain(std::iter::once(Some('#')))
        .chain(('A'..='Z').map(Some))
        .collect();

    let mut first: Option<gtk::ToggleButton> = None;
    for letter in letters {
        let label = match letter {
            None => tr!("library-filter-all-letters").to_string(),
            Some(c) => c.to_string(),
        };
        let button = gtk::ToggleButton::with_label(&label);
        button.add_css_class("flat");
        button.add_css_class("letter-button");
        match &first {
            Some(group) => button.set_group(Some(group)),
            None => {
                button.set_active(true);
                first = Some(button.clone());
            }
        }
        button.connect_toggled({
            let sender = sender.clone();
            move |button| {
                if button.is_active() {
                    sender.input(CollectionInput::SetLetter(letter));
                }
            }
        });
        row.append(&button);
        // Trennstriche nach „Alle“ und „#“: Sonderknöpfe vs. Buchstaben.
        if matches!(letter, None | Some('#')) {
            row.append(&group_separator());
        }
    }

    gtk::ScrolledWindow::builder()
        .css_classes(["letter-scroller"])
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .margin_bottom(10)
        .child(&row)
        .build()
}

fn new_media_page(
    api_client: &Arc<ApiClient>,
    collection: BaseItemDto,
    query: CollectionQuery,
    filter_bar: &gtk::Widget,
    letter_block: &gtk::Widget,
) -> Controller<MediaPage<CollectionItemsFetcher, EmptyComponent>> {
    let fetcher = CollectionItemsFetcher {
        api_client: api_client.clone(),
        collection,
        query,
    };
    MediaPage::builder()
        .launch(MediaPageInit {
            api_client: api_client.clone(),
            fetcher,
            empty_component: None,
            media_tile_display: Some(MediaTileDisplay::CoverLarge),
            header_center: Some(filter_bar.clone()),
            header_below: Some(letter_block.clone()),
        })
        .detach()
}

struct CollectionItemsFetcher {
    api_client: Arc<ApiClient>,
    collection: BaseItemDto,
    query: CollectionQuery,
}

impl Fetcher for CollectionItemsFetcher {
    async fn fetch(&self, start_index: usize, limit: usize) -> Result<(Vec<BaseItemDto>, usize)> {
        self.api_client
            .get_collection_items(&self.collection, &self.query, start_index, limit)
            .await
    }

    fn title(&self) -> String {
        self.collection
            .name
            .as_ref()
            .unwrap_or(tr!("library-unnamed-collection"))
            .clone()
    }
}

relm4::new_action_group!(CollectionActionGroup, "collection");
relm4::new_stateless_action!(RefreshAction, CollectionActionGroup, "refresh");
