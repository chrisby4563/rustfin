//! SyncPlay: gemeinsam schauen über den Jellyfin-Server.
//!
//! Ablauf: Delfin hält eine WebSocket-Verbindung zum Server (`/socket`), über
//! die Gruppen-Ereignisse und zeitgestempelte Befehle („Pause um …“) kommen.
//! Eigene Bedienung (Pause, Play, Spulen, neuer Titel) geht bei aktiver Gruppe
//! nicht direkt an den Player, sondern als Anfrage an den Server, der sie an
//! alle Teilnehmer verteilt – auch an Delfin selbst. Damit alle im selben
//! Moment handeln, misst Delfin den Abstand zur Server-Uhr (`/GetUtcTime`).

pub mod ui;

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use relm4::{once_cell::sync::Lazy, SharedState};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    app::{AppInput, APP_BROKER},
    jellyfin_api::api_client::ApiClient,
    tr,
    video_player::{VideoPlayerInput, VIDEO_PLAYER_BROKER},
};

const TICKS_PER_SECOND: f64 = 10_000_000.0;

/// Gruppe, in der Delfin gerade ist (für die Oberfläche).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GroupInfo {
    pub id: String,
    pub name: String,
    pub participants: Vec<String>,
    /// Zustand der Gruppe laut Server: Idle, Waiting, Paused, Playing.
    pub state: String,
}

pub static GROUP: Lazy<SharedState<Option<GroupInfo>>> = Lazy::new(SharedState::new);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    Unpause,
    Pause,
    Seek,
    Stop,
}

/// Befehl vom Server, auszuführen zur Server-Zeit `when_ms`.
#[derive(Debug, Clone, Copy)]
pub struct SyncCommand {
    pub kind: CommandKind,
    pub when_ms: i64,
    pub position_seconds: f64,
}

#[derive(Default)]
struct Inner {
    api_client: Option<Arc<ApiClient>>,
    ws_tx: Option<mpsc::UnboundedSender<Message>>,
    /// Server-Uhr minus eigene Uhr, in Millisekunden.
    offset_ms: i64,
    playlist_item_id: Option<String>,
    /// Gruppe selbst erstellt → laufenden Titel an die Gruppe geben.
    created_by_us: bool,
    /// Titel, der gerade im Player läuft.
    local_item: Option<Uuid>,
    /// Startstelle (Sekunden) für den nächsten vom Server gestarteten Titel.
    pending_start: Option<usize>,
}

static INNER: Lazy<Mutex<Inner>> = Lazy::new(|| Mutex::new(Inner::default()));
/// Nächstes `PlayVideo` kommt von SyncPlay selbst und darf direkt spielen.
static ALLOW_NEXT_PLAY: AtomicBool = AtomicBool::new(false);

fn inner() -> std::sync::MutexGuard<'static, Inner> {
    INNER.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn set_api_client(api_client: Arc<ApiClient>) {
    inner().api_client = Some(api_client);
}

pub fn in_group() -> bool {
    GROUP.read().is_some()
}

pub fn connected() -> bool {
    inner().ws_tx.as_ref().is_some_and(|tx| !tx.is_closed())
}

fn toast(message: String) {
    APP_BROKER.send(AppInput::Toast(message, Some(3)));
}

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

/// Aktuelle Server-Zeit in Millisekunden.
pub fn server_now_ms() -> i64 {
    now_ms() + inner().offset_ms
}

fn server_now_iso() -> String {
    DateTime::<Utc>::from_timestamp_millis(server_now_ms())
        .unwrap_or_else(Utc::now)
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Millisekunden bis zur Server-Zeit `when_ms` (negativ = schon vorbei).
pub fn delay_until(when_ms: i64) -> i64 {
    when_ms - server_now_ms()
}

pub fn set_local_item(item_id: Option<Uuid>) {
    inner().local_item = item_id;
}

/// Startstelle für einen gerade von SyncPlay gestarteten Titel abholen.
pub fn take_pending_start() -> Option<usize> {
    inner().pending_start.take()
}

/// Soll ein `PlayVideo` direkt spielen? Bei aktiver Gruppe nicht – dann geht
/// der Titel als neue Warteschlange an den Server.
pub fn intercept_play_video(item_id: Option<Uuid>, start_ticks: i64) -> bool {
    if ALLOW_NEXT_PLAY.swap(false, Ordering::SeqCst) || !in_group() {
        return false;
    }
    let Some(item_id) = item_id else {
        return false;
    };
    spawn_request(
        "SyncPlay/SetNewQueue",
        json!({
            "PlayingQueue": [item_id.to_string()],
            "PlayingItemPosition": 0,
            "StartPositionTicks": start_ticks,
        }),
    );
    true
}

/// Play/Pause vom Nutzer. `true` = von SyncPlay übernommen.
pub fn intercept_play(playing: bool) -> bool {
    if !in_group() {
        return false;
    }
    spawn_request(
        if playing { "SyncPlay/Unpause" } else { "SyncPlay/Pause" },
        json!({}),
    );
    true
}

/// Spulen vom Nutzer. `true` = von SyncPlay übernommen.
pub fn intercept_seek(seconds: usize) -> bool {
    if !in_group() {
        return false;
    }
    spawn_request(
        "SyncPlay/Seek",
        json!({ "PositionTicks": (seconds as f64 * TICKS_PER_SECOND) as i64 }),
    );
    true
}

pub fn send_buffering(position_seconds: usize, is_playing: bool) {
    send_state("SyncPlay/Buffering", position_seconds, is_playing);
}

pub fn send_ready(position_seconds: usize, is_playing: bool) {
    send_state("SyncPlay/Ready", position_seconds, is_playing);
}

fn send_state(path: &'static str, position_seconds: usize, is_playing: bool) {
    if !in_group() {
        return;
    }
    let playlist_item_id = inner().playlist_item_id.clone();
    spawn_request(
        path,
        json!({
            "When": server_now_iso(),
            "PositionTicks": (position_seconds as f64 * TICKS_PER_SECOND) as i64,
            "IsPlaying": is_playing,
            "PlaylistItemId": playlist_item_id,
        }),
    );
}

/// Eigenen laufenden Titel als Warteschlange an die (neue) Gruppe geben.
pub fn share_current(item_id: Uuid, position_seconds: usize) {
    spawn_request(
        "SyncPlay/SetNewQueue",
        json!({
            "PlayingQueue": [item_id.to_string()],
            "PlayingItemPosition": 0,
            "StartPositionTicks": (position_seconds as f64 * TICKS_PER_SECOND) as i64,
        }),
    );
}

fn spawn_request(path: &'static str, body: Value) {
    info!("SyncPlay → {path} {body}");
    relm4::spawn(async move {
        if let Err(err) = post(path, body).await {
            warn!("SyncPlay {path}: {err:#}");
        }
    });
}

async fn post(path: &str, body: Value) -> Result<()> {
    let api_client = inner().api_client.clone().context("not signed in")?;
    let url = api_client.root.join(path)?;
    api_client
        .client
        .post(url)
        .json(&body)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

/// Bestehende Gruppen auf dem Server.
pub async fn list_groups() -> Result<Vec<GroupInfo>> {
    let api_client = inner().api_client.clone().context("not signed in")?;
    let url = api_client.root.join("SyncPlay/List")?;
    let groups: Value = api_client
        .client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(groups
        .as_array()
        .map(|groups| groups.iter().filter_map(parse_group).collect())
        .unwrap_or_default())
}

fn parse_group(value: &Value) -> Option<GroupInfo> {
    Some(GroupInfo {
        id: value["GroupId"].as_str()?.to_string(),
        name: value["GroupName"].as_str().unwrap_or_default().to_string(),
        participants: value["Participants"]
            .as_array()
            .map(|p| {
                p.iter()
                    .filter_map(|n| n.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        state: value["State"].as_str().unwrap_or("Idle").to_string(),
    })
}

pub async fn create_group(name: &str) -> Result<()> {
    connect().await?;
    inner().created_by_us = true;
    post("SyncPlay/New", json!({ "GroupName": name })).await
}

pub async fn join_group(group_id: &str) -> Result<()> {
    connect().await?;
    inner().created_by_us = false;
    post("SyncPlay/Join", json!({ "GroupId": group_id })).await
}

pub async fn leave_group() -> Result<()> {
    let result = post("SyncPlay/Leave", json!({})).await;
    *GROUP.write() = None;
    result
}

/// WebSocket aufbauen (falls nicht schon offen) und Uhren abgleichen.
pub async fn connect() -> Result<()> {
    if connected() {
        return Ok(());
    }
    let api_client = inner().api_client.clone().context("not signed in")?;

    let mut url = api_client.root.join("socket")?;
    let scheme = if url.scheme() == "https" { "wss" } else { "ws" };
    if url.set_scheme(scheme).is_err() {
        bail!("cannot build WebSocket URL");
    }
    url.query_pairs_mut()
        .append_pair("ApiKey", &api_client.account.access_token)
        .append_pair("deviceId", &api_client.account.device_id.to_string());

    let (stream, _) = tokio_tungstenite::connect_async(url.as_str())
        .await
        .context("WebSocket connect")?;
    info!("SyncPlay: WebSocket connected");
    let (mut write, mut read) = stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    inner().ws_tx = Some(tx.clone());

    // Schreiben
    relm4::spawn(async move {
        while let Some(message) = rx.recv().await {
            if write.send(message).await.is_err() {
                break;
            }
        }
    });

    // Lesen
    relm4::spawn(async move {
        while let Some(message) = read.next().await {
            match message {
                Ok(Message::Text(text)) => handle_message(&text).await,
                Ok(Message::Close(_)) | Err(_) => break,
                _ => {}
            }
        }
        warn!("SyncPlay: WebSocket closed");
        inner().ws_tx = None;
        if GROUP.read().is_some() {
            *GROUP.write() = None;
            toast(tr!("syncplay-disconnected").to_string());
        }
    });

    // Lebenszeichen und regelmäßiger Uhrenabgleich
    relm4::spawn({
        let tx = tx.clone();
        async move {
            let mut tick: u64 = 0;
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                if tx.is_closed() {
                    break;
                }
                let _ = tx.send(Message::Text(
                    json!({ "MessageType": "KeepAlive" }).to_string(),
                ));
                tick += 1;
                if tick % 2 == 0 {
                    let _ = sync_time().await;
                }
            }
        }
    });

    sync_time().await?;
    Ok(())
}

/// Abstand zur Server-Uhr messen (NTP-artig, beste von 5 Messungen).
async fn sync_time() -> Result<()> {
    let api_client = inner().api_client.clone().context("not signed in")?;
    let url = api_client.root.join("GetUtcTime")?;
    let mut best: Option<(i64, i64)> = None; // (Laufzeit, Abstand)
    for _ in 0..5 {
        let t0 = now_ms();
        let response: Value = api_client
            .client
            .get(url.clone())
            .send()
            .await?
            .json()
            .await?;
        let t3 = now_ms();
        let parse = |key: &str| -> Option<i64> {
            DateTime::parse_from_rfc3339(response[key].as_str()?)
                .ok()
                .map(|t| t.timestamp_millis())
        };
        let (Some(t1), Some(t2)) = (
            parse("RequestReceptionTime"),
            parse("ResponseTransmissionTime"),
        ) else {
            bail!("unexpected /GetUtcTime answer");
        };
        let delay = (t3 - t0) - (t2 - t1);
        let offset = ((t1 - t0) + (t2 - t3)) / 2;
        if best.is_none_or(|(d, _)| delay < d) {
            best = Some((delay, offset));
        }
    }
    if let Some((delay, offset)) = best {
        inner().offset_ms = offset;
        info!("SyncPlay: clock offset {offset} ms (round trip {delay} ms)");
        let _ = post("SyncPlay/Ping", json!({ "Ping": delay.max(1) })).await;
    }
    Ok(())
}

async fn handle_message(text: &str) {
    let Ok(message) = serde_json::from_str::<Value>(text) else {
        return;
    };
    match message["MessageType"].as_str() {
        Some("SyncPlayGroupUpdate") => handle_group_update(&message["Data"]).await,
        Some("SyncPlayCommand") => {
            let data = &message["Data"];
            let kind = match data["Command"].as_str() {
                Some("Unpause") => CommandKind::Unpause,
                Some("Pause") => CommandKind::Pause,
                Some("Seek") => CommandKind::Seek,
                Some("Stop") => CommandKind::Stop,
                _ => return,
            };
            let when_ms = data["When"]
                .as_str()
                .and_then(|w| DateTime::parse_from_rfc3339(w).ok())
                .map(|t| t.timestamp_millis())
                .unwrap_or_else(server_now_ms);
            let position_seconds =
                data["PositionTicks"].as_i64().unwrap_or_default() as f64 / TICKS_PER_SECOND;
            info!("SyncPlay command {kind:?} at {position_seconds:.1}s");
            VIDEO_PLAYER_BROKER.send(VideoPlayerInput::SyncPlayCommand(SyncCommand {
                kind,
                when_ms,
                position_seconds,
            }));
        }
        _ => {}
    }
}

async fn handle_group_update(data: &Value) {
    let payload = &data["Data"];
    match data["Type"].as_str() {
        Some("GroupJoined") => {
            let Some(group) = parse_group(payload) else {
                return;
            };
            toast(tr!("syncplay-joined", {"name" => group.name.clone()}).to_string());
            *GROUP.write() = Some(group);
            let created = std::mem::take(&mut inner().created_by_us);
            if created {
                VIDEO_PLAYER_BROKER.send(VideoPlayerInput::SyncPlayShareCurrent);
            }
        }
        Some("UserJoined") => {
            let name = payload.as_str().unwrap_or_default().to_string();
            toast(tr!("syncplay-user-joined", {"name" => name.clone()}).to_string());
            if let Some(group) = GROUP.write().as_mut() {
                group.participants.push(name);
            }
        }
        Some("UserLeft") => {
            let name = payload.as_str().unwrap_or_default().to_string();
            toast(tr!("syncplay-user-left", {"name" => name.clone()}).to_string());
            if let Some(group) = GROUP.write().as_mut() {
                if let Some(pos) = group.participants.iter().position(|p| *p == name) {
                    group.participants.remove(pos);
                }
            }
        }
        Some("GroupLeft") | Some("NotInGroup") => {
            if GROUP.read().is_some() {
                *GROUP.write() = None;
                toast(tr!("syncplay-left").to_string());
            }
        }
        Some("GroupDoesNotExist") => toast(tr!("syncplay-group-missing").to_string()),
        Some("LibraryAccessDenied") => toast(tr!("syncplay-access-denied").to_string()),
        Some("PlayQueue") => handle_play_queue(payload).await,
        Some("StateUpdate") => {
            let state = payload["State"].as_str().unwrap_or_default().to_string();
            let reason = payload["Reason"].as_str().unwrap_or_default();
            info!("SyncPlay state {state} ({reason})");
            let previous = GROUP.read().as_ref().map(|g| g.state.clone());
            if previous.as_deref() == Some(state.as_str()) {
                return;
            }
            match (state.as_str(), reason) {
                ("Waiting", _) => toast(tr!("syncplay-state-waiting").to_string()),
                ("Playing", _) => toast(tr!("syncplay-state-playing-toast").to_string()),
                ("Paused", "Pause") => toast(tr!("syncplay-state-paused-toast").to_string()),
                _ => {}
            }
            if let Some(group) = GROUP.write().as_mut() {
                group.state = state;
            }
        }
        _ => {}
    }
}

async fn handle_play_queue(queue: &Value) {
    let reason = queue["Reason"].as_str().unwrap_or_default();
    let index = queue["PlayingItemIndex"].as_i64().unwrap_or(-1);
    let Some(entry) = queue["Playlist"]
        .as_array()
        .and_then(|playlist| playlist.get(usize::try_from(index).ok()?))
    else {
        return;
    };
    inner().playlist_item_id = entry["PlaylistItemId"].as_str().map(String::from);

    if !matches!(
        reason,
        "NewPlaylist" | "SetCurrentItem" | "NextItem" | "PreviousItem"
    ) {
        return;
    }
    let Some(item_id) = entry["ItemId"]
        .as_str()
        .and_then(|id| Uuid::parse_str(id).ok())
    else {
        return;
    };
    let start_seconds =
        (queue["StartPositionTicks"].as_i64().unwrap_or_default() as f64 / TICKS_PER_SECOND)
            as usize;
    info!("SyncPlay queue ({reason}): {item_id} from {start_seconds}s");

    // Läuft der Titel schon: nur an die Stelle und bereit melden.
    if inner().local_item == Some(item_id) {
        VIDEO_PLAYER_BROKER.send(VideoPlayerInput::SyncPlayPrepare(start_seconds));
        return;
    }

    let Some(api_client) = inner().api_client.clone() else {
        return;
    };
    match api_client.get_item(&item_id).await {
        Ok(item) => {
            inner().pending_start = Some(start_seconds);
            ALLOW_NEXT_PLAY.store(true, Ordering::SeqCst);
            APP_BROKER.send(AppInput::PlayVideo(item));
        }
        Err(err) => warn!("SyncPlay: cannot load item {item_id}: {err:#}"),
    }
}
