//! Delete movies, series, seasons, and episodes.
//!
//! Jellyfin deletes the files on the server, not merely the entry. Always ask
//! for confirmation, and only when the account may delete
//! (Dashboard → Users → “Allow media deletion”).

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use adw::prelude::*;
use jellyfin_api::types::{BaseItemDto, BaseItemKind};
use relm4::{adw, gtk};
use serde_json::Value;
use tracing::{info, warn};

use crate::{
    app::{AppInput, APP_BROKER},
    jellyfin_api::api_client::ApiClient,
    tr,
    utils::main_window::get_main_window,
};

static CAN_DELETE: AtomicBool = AtomicBool::new(false);

/// May the signed-in account delete media?
pub fn can_delete() -> bool {
    CAN_DELETE.load(Ordering::SeqCst)
}

/// Fetch account permissions once after signing in.
pub fn load_permission(api_client: Arc<ApiClient>) {
    relm4::spawn(async move {
        let allowed = async {
            let url = api_client.root.join("Users/Me").ok()?;
            let user: Value = api_client
                .client
                .get(url)
                .send()
                .await
                .ok()?
                .json()
                .await
                .ok()?;
            let policy = &user["Policy"];
            Some(
                policy["EnableContentDeletion"].as_bool().unwrap_or(false)
                    || policy["EnableContentDeletionFromFolders"]
                        .as_array()
                        .is_some_and(|folders| !folders.is_empty()),
            )
        }
        .await
        .unwrap_or(false);
        info!("Content deletion allowed: {allowed}");
        CAN_DELETE.store(allowed, Ordering::SeqCst);
    });
}

impl ApiClient {
    pub async fn delete_item(&self, item_id: &uuid::Uuid) -> anyhow::Result<()> {
        let url = self.root.join(&format!("Items/{item_id}"))?;
        self.client.delete(url).send().await?.error_for_status()?;
        Ok(())
    }
}

fn describe(item: &BaseItemDto) -> String {
    let name = item.name.clone().unwrap_or_default();
    match (&item.type_, &item.series_name) {
        (Some(BaseItemKind::Episode | BaseItemKind::Season), Some(series)) => {
            format!("{series} – {name}")
        }
        _ => name,
    }
}

/// Show a confirmation; delete after approval and call `on_deleted`.
pub fn confirm_delete(
    item: &BaseItemDto,
    api_client: Arc<ApiClient>,
    on_deleted: impl Fn() + 'static,
) {
    let Some(item_id) = item.id else {
        return;
    };
    let body = match item.type_ {
        Some(BaseItemKind::Series) => tr!("delete-body-series").to_string(),
        Some(BaseItemKind::Season) => tr!("delete-body-season").to_string(),
        _ => tr!("delete-body").to_string(),
    };
    let dialog = adw::MessageDialog::builder()
        .heading(tr!("delete-heading", {"name" => describe(item)}).to_string())
        .body(body)
        .modal(true)
        .build();
    if let Some(window) = get_main_window() {
        dialog.set_transient_for(Some(&window));
    }
    dialog.add_responses(&[
        ("cancel", tr!("delete-cancel")),
        ("delete", tr!("delete-confirm")),
    ]);
    dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    let name = describe(item);
    let on_deleted: std::rc::Rc<dyn Fn()> = std::rc::Rc::new(on_deleted);
    dialog.connect_response(Some("delete"), move |_, _| {
        let api_client = api_client.clone();
        let (tx, rx) = relm4::tokio::sync::oneshot::channel();
        relm4::spawn(async move {
            let _ = tx.send(api_client.delete_item(&item_id).await);
        });
        let name = name.clone();
        let on_deleted = on_deleted.clone();
        gtk::glib::spawn_future_local(async move {
            match rx.await {
                Ok(Ok(())) => {
                    info!("Deleted item {item_id}");
                    on_deleted();
                    APP_BROKER.send(AppInput::Toast(
                        tr!("delete-done", {"name" => name}).to_string(),
                        Some(3),
                    ));
                }
                Ok(Err(err)) => {
                    warn!("Delete failed for {item_id}: {err:#}");
                    APP_BROKER.send(AppInput::Toast(
                        tr!("delete-failed", {"error" => format!("{err:#}")}).to_string(),
                        None,
                    ));
                }
                Err(_) => {}
            }
        });
    });
    dialog.present();
}
