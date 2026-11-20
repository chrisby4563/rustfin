//! Einstellungen: Server & Konto (Konzept „Einstellungen“).

use std::{sync::Arc, time::Instant};

use adw::prelude::*;
use relm4::{prelude::*, gtk};
use serde_json::Value;

use crate::{
    app::{AppInput, APP_BROKER},
    borgar::sign_out_dialog::SignOutDialog,
    config::{Account, Server},
    jellyfin_api::api_client::ApiClient,
    tr,
};

type Auth = Option<(Arc<ApiClient>, Server, Account)>;

async fn run_bg<T: Send + 'static>(
    future: impl std::future::Future<Output = T> + Send + 'static,
) -> Option<T> {
    let (tx, rx) = relm4::tokio::sync::oneshot::channel();
    relm4::spawn(async move {
        let _ = tx.send(future.await);
    });
    rx.await.ok()
}

fn pill(label: &str, classes: &[&str]) -> gtk::Button {
    let button = gtk::Button::builder()
        .label(label)
        .valign(gtk::Align::Center)
        .build();
    button.add_css_class("pill");
    button.add_css_class("settings-pill");
    for class in classes {
        button.add_css_class(class);
    }
    button
}

pub fn page(auth: Auth, window: &adw::PreferencesWindow) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title(tr!("prefs-account-page"))
        .icon_name("system-users-symbolic")
        .build();
    let Some((api_client, server, account)) = auth else {
        page.set_visible(false);
        return page;
    };

    // ── Server ──
    let server_group = adw::PreferencesGroup::builder()
        .title(tr!("prefs-account-server"))
        .build();
    let status_dot = gtk::Box::builder()
        .width_request(10)
        .height_request(10)
        .valign(gtk::Align::Center)
        .css_classes(["status-dot"])
        .build();
    let server_row = adw::ActionRow::builder()
        .title(&server.name)
        .subtitle(format!("{} · {}", server.url, tr!("prefs-account-checking")))
        .build();
    server_row.add_prefix(&status_dot);
    let switch_server = pill(tr!("prefs-account-switch-server"), &[]);
    switch_server.connect_clicked({
        let window = window.clone();
        move |_| {
            window.close();
            APP_BROKER.send(AppInput::SignOut);
        }
    });
    server_row.add_suffix(&switch_server);
    server_group.add(&server_row);
    page.add(&server_group);

    // Erreichbarkeit + Version (öffentlicher Endpunkt, ohne Anmeldedaten).
    gtk::glib::spawn_future_local({
        let api_client = api_client.clone();
        let server_row = server_row.clone();
        let status_dot = status_dot.clone();
        let url = server.url.clone();
        async move {
            let result = run_bg(async move {
                let target = api_client.root.join("System/Info/Public").ok()?;
                let start = Instant::now();
                let info: Value = api_client.client.get(target).send().await.ok()?.json().await.ok()?;
                Some((info["Version"].as_str().unwrap_or("?").to_string(), start.elapsed().as_millis()))
            })
            .await
            .flatten();
            match result {
                Some((version, ms)) => {
                    server_row.set_subtitle(&format!(
                        "{url} · Jellyfin {version} · {} · {ms} ms",
                        tr!("prefs-account-connected")
                    ));
                    status_dot.add_css_class("ok");
                }
                None => {
                    server_row.set_subtitle(&format!("{url} · {}", tr!("prefs-account-unreachable")));
                    status_dot.add_css_class("bad");
                }
            }
        }
    });

    // ── Konto ──
    let account_group = adw::PreferencesGroup::builder()
        .title(tr!("prefs-account-account"))
        .build();
    let avatar = adw::Avatar::new(40, Some(&account.username), true);
    let account_row = adw::ActionRow::builder()
        .title(&account.username)
        .build();
    account_row.add_prefix(&avatar);
    let switch_account = pill(tr!("prefs-account-switch-account"), &[]);
    switch_account.connect_clicked({
        let window = window.clone();
        move |_| {
            window.close();
            APP_BROKER.send(AppInput::SignOut);
        }
    });
    let sign_out = pill(tr!("prefs-account-sign-out"), &["settings-danger"]);
    sign_out.connect_clicked({
        let window = window.clone();
        let api_client = api_client.clone();
        let server = server.clone();
        let account = account.clone();
        move |_| {
            window.close();
            let mut dialog = SignOutDialog::builder()
                .transient_for(&relm4::main_application().active_window().unwrap())
                .launch((api_client.clone(), server.clone(), account.clone()))
                .detach();
            dialog.detach_runtime();
        }
    });
    let account_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    account_buttons.set_valign(gtk::Align::Center);
    account_buttons.append(&switch_account);
    account_buttons.append(&sign_out);
    account_row.add_suffix(&account_buttons);
    account_group.add(&account_row);

    gtk::glib::spawn_future_local({
        let api_client = api_client.clone();
        let account_row = account_row.clone();
        async move {
            let user = run_bg(async move {
                let target = api_client.root.join("Users/Me").ok()?;
                let user: Value = api_client.client.get(target).send().await.ok()?.json().await.ok()?;
                Some(user)
            })
            .await
            .flatten();
            if let Some(user) = user {
                let policy = &user["Policy"];
                let mut parts = Vec::new();
                parts.push(if policy["IsAdministrator"].as_bool().unwrap_or(false) {
                    tr!("prefs-account-admin").to_string()
                } else {
                    tr!("prefs-account-user").to_string()
                });
                if crate::utils::delete_item::can_delete() {
                    parts.push(tr!("prefs-account-can-delete").to_string());
                }
                account_row.set_subtitle(&parts.join(" · "));
            }
        }
    });

    // Quick Connect: Code eines anderen Geräts freigeben.
    let qc_row = adw::ActionRow::builder()
        .title(tr!("prefs-account-qc.title"))
        .subtitle(tr!("prefs-account-qc.subtitle"))
        .build();
    let qc_entry = gtk::Entry::builder()
        .placeholder_text("123456")
        .max_length(6)
        .width_chars(8)
        .xalign(0.5)
        .input_purpose(gtk::InputPurpose::Digits)
        .valign(gtk::Align::Center)
        .css_classes(["qc-entry"])
        .build();
    let qc_button = pill(tr!("prefs-account-qc.button"), &["suggested-action"]);
    let authorize = {
        let api_client = api_client.clone();
        let qc_entry = qc_entry.clone();
        let window = window.clone();
        move || {
            let code: String = qc_entry.text().chars().filter(|c| c.is_ascii_digit()).collect();
            if code.len() != 6 {
                window.add_toast(adw::Toast::new(tr!("prefs-account-qc.invalid")));
                return;
            }
            let api_client = api_client.clone();
            let qc_entry = qc_entry.clone();
            let window = window.clone();
            gtk::glib::spawn_future_local(async move {
                let ok = run_bg(async move {
                    let mut target = api_client.root.join("QuickConnect/Authorize").ok()?;
                    target.query_pairs_mut().append_pair("code", &code);
                    let response = api_client.client.post(target).send().await.ok()?;
                    Some(response.status().is_success())
                })
                .await
                .flatten()
                .unwrap_or(false);
                if ok {
                    qc_entry.set_text("");
                    window.add_toast(adw::Toast::new(tr!("prefs-account-qc.done")));
                } else {
                    window.add_toast(adw::Toast::new(tr!("prefs-account-qc.failed")));
                }
            });
        }
    };
    qc_button.connect_clicked({
        let authorize = authorize.clone();
        move |_| authorize()
    });
    qc_entry.connect_activate(move |_| authorize());
    let qc_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    qc_box.set_valign(gtk::Align::Center);
    qc_box.append(&qc_entry);
    qc_box.append(&qc_button);
    qc_row.add_suffix(&qc_box);
    account_group.add(&qc_row);
    page.add(&account_group);

    page
}
