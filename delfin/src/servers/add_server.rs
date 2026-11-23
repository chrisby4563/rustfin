use adw::prelude::*;
use relm4::prelude::*;
use tracing::error;

use crate::{
    config,
    jellyfin_api::{api::system::get_public_server_info, util::url::httpify},
    tr,
};

#[derive(Clone, Debug)]
pub enum ValidationState {
    Invalid,
    Loading,
    Valid(config::Server),
    Error,
}

#[derive(Clone)]
pub struct AddServerDialog {
    valid: ValidationState,
}

#[derive(Debug)]
pub enum AddServerInput {
    Validate,
    Invalidate,
    AddServer,
}

#[derive(Debug)]
pub enum AddServerOutput {
    ServerAdded(config::Server),
}

#[derive(Debug)]
pub enum AddServerCommandOutput {
    ServerValidated(ValidationState),
}

#[relm4::component(pub)]
impl Component for AddServerDialog {
    type Init = ();
    type Input = AddServerInput;
    type Output = AddServerOutput;
    type CommandOutput = AddServerCommandOutput;

    view! {
        adw::Window {
            set_application: Some(&relm4::main_application()),
            set_title: Some(tr!("server-list-add-server-title")),
            set_modal: true,
            // Wide enough for error toasts and the Quick Connect text.
            set_default_width: 440,
            set_visible: true,

            #[wrap(Some)]
            set_content = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {},

                #[wrap(Some)]
                set_content = &adw::Clamp {
                    set_margin_all: 20,
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 20,

                        adw::PreferencesGroup {
                            #[name = "url_entry"]
                            adw::EntryRow {
                                set_title: tr!("server-list-add-server-url"),
                                set_show_apply_button: true,
                                #[watch]
                                set_editable: !matches!(model.valid, ValidationState::Loading),
                                set_input_purpose: gtk::InputPurpose::Url,

                                connect_changed[sender] => move |_| {
                                    sender.input(AddServerInput::Invalidate);
                                },
                                connect_apply[sender] => move |_| {
                                    sender.input(AddServerInput::Validate);
                                },
                            },

                            adw::ActionRow {
                                set_title: tr!("server-list-add-server-name"),
                                #[watch]
                                set_subtitle: &match &model.valid {
                                    ValidationState::Valid(server) => server.name.clone(),
                                    ValidationState::Error => tr!("server-list-add-server-error").to_string(),
                                    _ => "...".to_string(),
                                },
                                add_suffix = if matches!(model.valid, ValidationState::Loading) {
                                    gtk::Spinner {
                                        set_spinning: true,
                                    }
                                } else if matches!(model.valid, ValidationState::Valid(_)) {
                                    gtk::Image {
                                        set_icon_name: Some("check-round-outline-symbolic")
                                    }
                                } else { gtk::Label {} },
                            },
                        },

                        #[name = "submit_btn"]
                        gtk::Button {
                            set_halign: gtk::Align::Center,
                            set_label: tr!("server-list-add-server-submit-button"),
                            add_css_class: "pill",
                            add_css_class: "suggested-action",
                            #[watch]
                            set_sensitive: matches!(model.valid, ValidationState::Valid(_)),
                            connect_clicked[sender] => move |_| {
                                sender.input(AddServerInput::AddServer);
                            },
                        },
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = AddServerDialog {
            valid: ValidationState::Invalid,
        };
        let widgets = view_output!();
        widgets.url_entry.grab_focus();
        root.set_default_widget(Some(&widgets.submit_btn));
        ComponentParts { model, widgets }
    }
    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: ComponentSender<Self>,
        root: &Self::Root,
    ) {
        match message {
            AddServerInput::Validate => {
                let url_entry = &widgets.url_entry;

                let input = url_entry.text().trim().to_string();
                // Like the official Jellyfin apps: without an explicit scheme
                // try HTTPS first, then plain HTTP (e.g. "server:8096").
                let candidates = if input.contains("://") {
                    vec![httpify(&input)]
                } else {
                    vec![
                        httpify(&format!("https://{input}")),
                        httpify(&format!("http://{input}")),
                    ]
                };

                self.valid = ValidationState::Loading;
                sender.oneshot_command(async move {
                    for url in candidates {
                        match get_public_server_info(&url).await {
                            Ok(info) => {
                                return AddServerCommandOutput::ServerValidated(
                                    ValidationState::Valid(config::Server {
                                        id: info.id,
                                        url,
                                        name: info.server_name,
                                        accounts: Vec::new(),
                                    }),
                                );
                            }
                            Err(err) => error!("Error getting server info from {url}: {err:#}"),
                        }
                    }
                    AddServerCommandOutput::ServerValidated(ValidationState::Error)
                });
            }
            AddServerInput::Invalidate => {
                // Showing the address that answered changes the entry too;
                // that must not throw away the result.
                let unchanged = matches!(&self.valid,
                    ValidationState::Valid(server) if widgets.url_entry.text() == server.url.as_str());
                if !unchanged {
                    self.valid = ValidationState::Invalid;
                }
            }
            AddServerInput::AddServer => {
                if let ValidationState::Valid(server) = &self.valid {
                    sender
                        .output(AddServerOutput::ServerAdded(server.clone()))
                        .unwrap();
                    root.close();
                }
            }
        }

        self.update_view(widgets, sender);
    }

    fn update_cmd_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::CommandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            AddServerCommandOutput::ServerValidated(valid) => {
                self.valid = valid;
                // Show the address that actually answered.
                if let ValidationState::Valid(server) = &self.valid {
                    if widgets.url_entry.text() != server.url.as_str() {
                        widgets.url_entry.set_text(&server.url);
                        widgets.url_entry.set_position(-1);
                    }
                }
            }
        }
        self.update_view(widgets, sender);
    }
}
