use anyhow::Result;
use jellyfin_api::types::{BaseItemDto, PlaybackInfoDto, PlaybackInfoResponse, VideoType};
use uuid::Uuid;

use crate::{
    globals::CONFIG, jellyfin_api::api_client::ApiClient,
    utils::device_profile::DEVICE_PROFILE_DIRECT_PLAY,
};

/// Player quality level.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StreamQuality {
    /// Play directly; the server converts disc images and 4K to 1080p.
    #[default]
    Auto,
    /// Always use the original file.
    Original,
    P1080,
    P720,
    P480,
}

impl StreamQuality {
    pub const ALL: [StreamQuality; 5] = [
        StreamQuality::Auto,
        StreamQuality::Original,
        StreamQuality::P1080,
        StreamQuality::P720,
        StreamQuality::P480,
    ];

    /// (Bitrate, width, height) for server conversion.
    fn transcode_limits(&self) -> (u32, u32, u32) {
        match self {
            StreamQuality::P720 => (8_000_000, 1280, 720),
            StreamQuality::P480 => (3_000_000, 854, 480),
            _ => (20_000_000, 1920, 1080),
        }
    }
}

/// How the current stream is requested.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StreamSettings {
    pub quality: StreamQuality,
    /// Selected version (media source ID); `None` = default.
    pub media_source_id: Option<String>,
    /// Audio track for server-side transcoding; `None` = server default.
    pub audio_stream_index: Option<i32>,
}

/// Is this a disc image? It cannot be played as a file.
pub fn is_disc(item: &BaseItemDto) -> bool {
    matches!(
        item.video_type,
        Some(VideoType::Iso | VideoType::BluRay | VideoType::Dvd)
    )
}

impl ApiClient {
    /// Does the server convert using these settings?
    pub fn is_transcoded(item: &BaseItemDto, settings: &StreamSettings) -> bool {
        match settings.quality {
            StreamQuality::Auto => Self::is_server_transcoded_source(item, settings),
            StreamQuality::Original => is_disc(item),
            _ => true,
        }
    }

    /// Automatic rule, using the selected version's image size when known.
    fn is_server_transcoded_source(item: &BaseItemDto, settings: &StreamSettings) -> bool {
        let source_width = settings.media_source_id.as_ref().and_then(|id| {
            item.media_sources
                .as_ref()?
                .iter()
                .find(|source| source.id.as_ref() == Some(id))?
                .media_streams
                .as_ref()?
                .iter()
                .find(|stream| stream.type_ == Some(jellyfin_api::types::MediaStreamType::Video))?
                .width
        });
        match source_width {
            Some(width) => is_disc(item) || width > 1920,
            None => Self::is_server_transcoded(item),
        }
    }

    /// Stream URL by quality, version, and audio track.
    pub fn stream_url(&self, item: &BaseItemDto, settings: &StreamSettings) -> String {
        let item_id = item.id.unwrap();
        let source_id = settings
            .media_source_id
            .clone()
            .unwrap_or_else(|| item_id.simple().to_string());
        if !Self::is_transcoded(item, settings) {
            let mut url = reqwest::Url::parse(&self.get_stream_url(&item_id)).unwrap();
            if settings.media_source_id.is_some() {
                url.query_pairs_mut().append_pair("MediaSourceId", &source_id);
            }
            return url.to_string();
        }
        let (bitrate, max_width, max_height) = settings.quality.transcode_limits();
        let mut url = self
            .root
            .join(&format!("Videos/{item_id}/master.m3u8"))
            .unwrap();
        url.query_pairs_mut()
            .append_pair("MediaSourceId", &source_id)
            .append_pair("DeviceId", &self.account.device_id.to_string())
            .append_pair("VideoCodec", "h264,hevc")
            .append_pair("AudioCodec", "aac,mp3,ac3,eac3")
            .append_pair("SegmentContainer", "ts")
            // Do not merely remux: disc video is often field-coded (PAFF),
            // which stutters with Intel hardware decoding. The server
            // re-encodes it (NVENC, with deinterlacing) as it does for browsers.
            .append_pair("AllowVideoStreamCopy", "false")
            .append_pair("VideoBitrate", &bitrate.to_string())
            .append_pair("MaxWidth", &max_width.to_string())
            .append_pair("MaxHeight", &max_height.to_string())
            // Use a new session per request; otherwise, switching tracks makes
            // the server return previously converted segments.
            .append_pair("PlaySessionId", &Uuid::new_v4().simple().to_string())
            .append_pair("ApiKey", &self.account.access_token);
        if let Some(index) = settings.audio_stream_index {
            url.query_pairs_mut()
                .append_pair("AudioStreamIndex", &index.to_string());
        }
        url.to_string()
    }

    /// Is this item transcoded by the server (instead of served as a file)?
    /// The stream then contains only one audio track; switching requires a
    /// new stream with `AudioStreamIndex`.
    pub fn is_server_transcoded(item: &BaseItemDto) -> bool {
        let is_disc = is_disc(item);
        // 4K (mostly HEVC 10-bit HDR/Dolby Vision): integrated laptop GPUs can
        // decode it, but cannot scale and tone-map HDR smoothly. Let the server
        // transcode it to 1080p instead.
        let is_4k = item.width.unwrap_or(0) > 1920 || item.height.unwrap_or(0) > 1440;
        is_disc || is_4k
    }

    pub fn get_stream_url(&self, item_id: &Uuid) -> String {
        if CONFIG.read().video_player.hls_playback {
            self.root
                .join(&format!(
                    "Videos/{}/main.m3u8?static=true&ApiKey={}",
                    item_id, self.account.access_token
                ))
                .unwrap()
                .to_string()
        } else {
            self.root
                .join(&format!("Videos/{}/stream?static=true", item_id))
                .unwrap()
                .to_string()
        }
    }

    pub async fn get_item(&self, item_id: &Uuid) -> Result<BaseItemDto> {
        let mut url = self
            .root
            .join(&format!("Users/{}/Items/{item_id}", self.account.id))?;
        url.query_pairs_mut()
            .append_pair("fields", "Trickplay,People,Chapters,MediaSources");

        let res = self.client.get(url).send().await?.json().await?;
        Ok(res)
    }

    pub async fn get_playback_info(&self, item_id: &Uuid) -> Result<PlaybackInfoResponse> {
        let mut url = self
            .root
            .join(&format!("Items/{item_id}/PlaybackInfo"))
            .unwrap();
        url.query_pairs_mut()
            .append_pair("userId", &self.account.id.to_string());

        let body: PlaybackInfoDto = PlaybackInfoDto::builder()
            .device_profile(Some(DEVICE_PROFILE_DIRECT_PLAY.clone()))
            .try_into()
            .unwrap();

        Ok(self
            .client
            .post(url)
            .json(&body)
            .send()
            .await?
            .json()
            .await?)
    }
}
