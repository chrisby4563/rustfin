//! “Watch together” button: list, join, create and
//! leave. It lives in the library header bar and the player.

use gtk::prelude::*;
use relm4::prelude::*;

use crate::tr;

use super::{GroupInfo, GROUP};

pub struct SyncPlayButton {
    group: Option<GroupInfo>,
}

#[derive(Debug)]
pub enum SyncPlayButtonInput {
    Refresh,
    GroupChanged(Option<GroupInfo>),
    Create(String),
    Join(String),
    Leave,
}

#[derive(Debug)]
pub enum SyncPlayButtonCommand {
    Groups(Result<Vec<GroupInfo>, String>),
    Done(Result<(), String>),
}

#[relm4::component(pub)]
impl Component for SyncPlayButton {
    type Init = ();
    type Input = SyncPlayButtonInput;
    type Output = ();
    type CommandOutput = SyncPlayButtonCommand;

    view! {
        gtk::MenuButton {
            set_tooltip_text: Some(tr!("syncplay-tooltip")),
            #[watch]
            set_css_classes: if model.group.is_some() { &["header-pill", "syncplay-active"] } else { &["header-pill"] },

            #[wrap(Some)]
            set_child = &gtk::Box {
                set_spacing: 6,
                gtk::Image::from_icon_name("system-users-symbolic"),
                gtk::Label {
                    #[watch]
                    set_label: &model.group.as_ref().map(status_text)
                        .unwrap_or_else(|| tr!("syncplay-short").to_string()),
                },
            },

            #[wrap(Some)]
            #[name = "popover"]
            set_popover = &gtk::Popover {
                connect_show[sender] => move |_| {
                    sender.input(SyncPlayButtonInput::Refresh);
                },

                #[name = "content"]
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 8,
                    set_margin_all: 8,
                    set_width_request: 280,
                },
            },
        }
    }

    fn init(_: Self::Init, root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        GROUP.subscribe(sender.input_sender(), |group| {
            SyncPlayButtonInput::GroupChanged(group.clone())
        });
        let model = SyncPlayButton {
            group: GROUP.read().clone(),
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
            SyncPlayButtonInput::GroupChanged(group) => {
                let membership_changed = self.group.is_some() != group.is_some()
                    || self.group.as_ref().map(|g| g.participants.len())
                        != group.as_ref().map(|g| g.participants.len());
                self.group = group;
                if membership_changed && widgets.popover.is_visible() {
                    sender.input(SyncPlayButtonInput::Refresh);
                }
            }
            SyncPlayButtonInput::Refresh => {
                set_content(&widgets.content, &[spinner()]);
                if let Some(group) = GROUP.read().clone() {
                    show_group(&widgets.content, &group, &sender);
                } else {
                    sender.oneshot_command(async {
                        let result = async {
                            super::connect().await?;
                            super::list_groups().await
                        }
                        .await
                        .map_err(|e| format!("{e:#}"));
                        SyncPlayButtonCommand::Groups(result)
                    });
                }
            }
            SyncPlayButtonInput::Create(name) => {
                set_content(&widgets.content, &[spinner()]);
                sender.oneshot_command(async move {
                    SyncPlayButtonCommand::Done(
                        super::create_group(&name).await.map_err(|e| format!("{e:#}")),
                    )
                });
            }
            SyncPlayButtonInput::Join(id) => {
                set_content(&widgets.content, &[spinner()]);
                sender.oneshot_command(async move {
                    SyncPlayButtonCommand::Done(
                        super::join_group(&id).await.map_err(|e| format!("{e:#}")),
                    )
                });
            }
            SyncPlayButtonInput::Leave => {
                sender.oneshot_command(async {
                    SyncPlayButtonCommand::Done(
                        super::leave_group().await.map_err(|e| format!("{e:#}")),
                    )
                });
                widgets.popover.popdown();
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
            SyncPlayButtonCommand::Groups(Ok(groups)) => {
                show_groups(&widgets.content, &groups, &sender);
            }
            SyncPlayButtonCommand::Groups(Err(err)) | SyncPlayButtonCommand::Done(Err(err)) => {
                set_content(
                    &widgets.content,
                    &[heading(tr!("syncplay-error")), wrap_label(&err)],
                );
            }
            SyncPlayButtonCommand::Done(Ok(())) => {
                // The server reports group changes; rebuild shortly afterward.
                let sender = sender.clone();
                relm4::gtk::glib::timeout_add_local_once(
                    std::time::Duration::from_millis(700),
                    move || sender.input(SyncPlayButtonInput::Refresh),
                );
            }
        }
        self.update_view(widgets, sender);
    }
}

/// “Together · 2 · Playing”
fn status_text(group: &GroupInfo) -> String {
    let state = match group.state.as_str() {
        "Waiting" => tr!("syncplay-state-waiting-short").to_string(),
        "Playing" => tr!("syncplay-state-playing").to_string(),
        "Paused" => tr!("syncplay-state-paused").to_string(),
        _ => String::new(),
    };
    let mut text = format!(
        "{} · {}",
        tr!("syncplay-short"),
        group.participants.len()
    );
    if !state.is_empty() {
        text = format!("{text} · {state}");
    }
    text
}

fn set_content(content: &gtk::Box, children: &[gtk::Widget]) {
    while let Some(child) = content.first_child() {
        content.remove(&child);
    }
    for child in children {
        content.append(child);
    }
}

fn spinner() -> gtk::Widget {
    let spinner = gtk::Spinner::new();
    spinner.start();
    spinner.set_margin_top(12);
    spinner.set_margin_bottom(12);
    spinner.upcast()
}

fn heading(text: &str) -> gtk::Widget {
    gtk::Label::builder()
        .label(text)
        .css_classes(["heading"])
        .halign(gtk::Align::Start)
        .build()
        .upcast()
}

fn wrap_label(text: &str) -> gtk::Widget {
    gtk::Label::builder()
        .label(text)
        .wrap(true)
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build()
        .upcast()
}

fn button(label: &str, suggested: bool) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    if suggested {
        button.add_css_class("suggested-action");
    }
    button
}

fn show_group(content: &gtk::Box, group: &GroupInfo, sender: &ComponentSender<SyncPlayButton>) {
    let mut children = vec![
        heading(&tr!("syncplay-in-group", {"name" => group.name.clone()})),
        wrap_label(&format!(
            "{}: {}",
            tr!("syncplay-participants", {"count" => group.participants.len()}),
            group.participants.join(", ")
        )),
        wrap_label(&status_text(group)),
        wrap_label(tr!("syncplay-hint")),
    ];
    // Alone in the group: leaving dissolves it (the server deletes empty groups).
    let leave = if group.participants.len() <= 1 {
        button(tr!("syncplay-dissolve"), false)
    } else {
        button(tr!("syncplay-leave"), false)
    };
    leave.add_css_class("destructive-action");
    leave.connect_clicked({
        let sender = sender.clone();
        move |_| sender.input(SyncPlayButtonInput::Leave)
    });
    children.push(leave.upcast());
    set_content(content, &children);
}

fn show_groups(content: &gtk::Box, groups: &[GroupInfo], sender: &ComponentSender<SyncPlayButton>) {
    let mut children = vec![heading(tr!("syncplay-tooltip"))];
    if groups.is_empty() {
        children.push(wrap_label(tr!("syncplay-no-groups")));
    }
    for group in groups {
        let label = format!(
            "{} · {}",
            group.name,
            tr!("syncplay-participants", {"count" => group.participants.len()})
        );
        let join = button(&label, false);
        join.set_tooltip_text(Some(&group.participants.join(", ")));
        join.connect_clicked({
            let sender = sender.clone();
            let id = group.id.clone();
            move |_| sender.input(SyncPlayButtonInput::Join(id.clone()))
        });
        children.push(join.upcast());
    }

    let entry = gtk::Entry::builder()
        .text(tr!("syncplay-default-name"))
        .build();
    let create = button(tr!("syncplay-create"), true);
    create.connect_clicked({
        let sender = sender.clone();
        let entry = entry.clone();
        move |_| sender.input(SyncPlayButtonInput::Create(entry.text().to_string()))
    });
    children.push(gtk::Separator::new(gtk::Orientation::Horizontal).upcast());
    children.push(entry.upcast());
    children.push(create.upcast());

    // Always offer leaving: the server retains a group per device, even when
    // Delfin itself currently knows nothing about it.
    let leave = button(tr!("syncplay-leave"), false);
    leave.add_css_class("flat");
    leave.set_tooltip_text(Some(tr!("syncplay-leave-any-tooltip")));
    leave.connect_clicked({
        let sender = sender.clone();
        move |_| sender.input(SyncPlayButtonInput::Leave)
    });
    children.push(leave.upcast());
    set_content(content, &children);
}
