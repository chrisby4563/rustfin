use std::sync::Arc;

use jellyfin_api::types::{BaseItemDto, BaseItemKind};
use uuid::Uuid;

use crate::jellyfin_api::{
    api::{latest::GetNextUpOptionsBuilder, shows::GetEpisodesOptionsBuilder},
    api_client::ApiClient,
};

// Gets the next playable media for the given media item.
// For episodes and movies, this just returns the passed in media.
// For TV shows, this looks for the first episode that the user can resume, continue the series
// with, or start from the beginning.
pub async fn get_next_playable_media(
    api_client: Arc<ApiClient>,
    media: BaseItemDto,
) -> Option<BaseItemDto> {
    let media_id = media.id.expect("Media missing id: {media:#?}");
    let media_type = media.type_.expect("Media missing type: {media:#?}");

    match media_type {
        BaseItemKind::Series => {
            let next = get_next_episode(api_client.clone(), media_id).await?;
            // Jellyfin 12 liefert unter „Weiterschauen“ teils die Staffel statt
            // der Folge (z. B. bei Disc-Abbildern). Eine Staffel ist nicht
            // abspielbar – dann deren nächste Folge nehmen.
            if next.type_ == Some(BaseItemKind::Season) {
                get_next_episode_in_season(api_client, &next).await
            } else {
                Some(next)
            }
        }
        // Eine Staffel ist nur ein Ordner ohne eigene Videodatei. Jellyfin 12
        // zeigt Staffeln z. B. unter „Neueste“; direkt abgespielt antwortet der
        // Server mit 400 („Season … has no media sources“) und der Player
        // bleibt schwarz bei 00:00.
        BaseItemKind::Season => get_next_episode_in_season(api_client, &media).await,
        _ => Some(media),
    }
}

/// Nächste Folge innerhalb einer Staffel: eine angefangene, sonst die erste
/// ungesehene, sonst (alles gesehen) die erste Folge der Staffel.
pub async fn get_next_episode_in_season(
    api_client: Arc<ApiClient>,
    season: &BaseItemDto,
) -> Option<BaseItemDto> {
    let season_id = season.id?;
    let Some(series_id) = season.series_id else {
        tracing::warn!("Season {season_id} has no series id");
        return None;
    };

    let episodes = api_client
        .get_episodes(
            &GetEpisodesOptionsBuilder::default()
                .series_id(series_id)
                .season_id(season_id)
                .is_missing(false)
                .build()
                .unwrap(),
        )
        .await
        .ok()?;

    let played = |episode: &BaseItemDto| {
        episode
            .user_data
            .as_ref()
            .and_then(|user_data| user_data.played)
            .unwrap_or(false)
    };
    let started = |episode: &BaseItemDto| {
        episode
            .user_data
            .as_ref()
            .and_then(|user_data| user_data.playback_position_ticks)
            .unwrap_or(0)
            > 0
    };

    episodes
        .iter()
        .find(|episode| started(episode) && !played(episode))
        .or_else(|| episodes.iter().find(|episode| !played(episode)))
        .or_else(|| episodes.first())
        .cloned()
}

pub async fn get_next_episode(api_client: Arc<ApiClient>, media_id: Uuid) -> Option<BaseItemDto> {
    if let Some(resume) = api_client
        .get_continue_watching(
            GetNextUpOptionsBuilder::default()
                .series_id(media_id)
                .limit(1)
                .build()
                .unwrap(),
        )
        .await
        .as_ref()
        .ok()
        .and_then(|resume| resume.first())
    {
        return Some(resume.to_owned());
    };

    if let Some(next_up) = api_client
        .get_next_up(
            GetNextUpOptionsBuilder::default()
                .series_id(media_id)
                .limit(1)
                .build()
                .unwrap(),
        )
        .await
        .ok()
        .as_ref()
        .and_then(|next_up| next_up.first())
    {
        return Some(next_up.to_owned());
    }

    if let Some(items) = api_client
        .get_episodes(
            &GetEpisodesOptionsBuilder::default()
                .series_id(media_id)
                .build()
                .unwrap(),
        )
        .await
        .ok()
        .as_ref()
        .and_then(|episodes| episodes.first())
    {
        return Some(items.to_owned());
    }

    None
}
