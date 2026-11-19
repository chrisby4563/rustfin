use anyhow::Result;
use jellyfin_api::types::{BaseItemDto, PlaybackInfoDto, PlaybackInfoResponse, VideoType};
use uuid::Uuid;

use crate::{
    globals::CONFIG, jellyfin_api::api_client::ApiClient,
    utils::device_profile::DEVICE_PROFILE_DIRECT_PLAY,
};

impl ApiClient {
    /// Stream-Adresse für ein Element. Disc-Abbilder (ISO, Blu-ray-/DVD-Ordner)
    /// kann der Server nicht als Datei durchreichen – „static“ liefert dann nur
    /// einen Schnipsel (falsche Dauer, Ruckeln). Für sie lässt Delfin den
    /// Server per HLS umwandeln, wie die Weboberfläche.
    pub fn get_stream_url_for(&self, item: &BaseItemDto) -> String {
        self.get_stream_url_with_audio(item, None)
    }

    /// Wird dieses Element vom Server umgewandelt (statt als Datei geliefert)?
    /// Dann enthält der Stream nur eine Tonspur; ein Wechsel braucht einen
    /// neuen Stream mit `AudioStreamIndex`.
    pub fn is_server_transcoded(item: &BaseItemDto) -> bool {
        let is_disc = matches!(
            item.video_type,
            Some(VideoType::Iso | VideoType::BluRay | VideoType::Dvd)
        );
        // 4K (meist HEVC 10 Bit HDR/Dolby Vision): Die Laptop-Grafik (UHD 620)
        // dekodiert das zwar, schafft aber Skalierung + HDR-Umrechnung nicht
        // ruckelfrei. Der Server (GTX 1080 Ti) wandelt auf 1080p um.
        let is_4k = item.width.unwrap_or(0) > 1920 || item.height.unwrap_or(0) > 1440;
        is_disc || is_4k
    }

    pub fn get_stream_url_with_audio(
        &self,
        item: &BaseItemDto,
        audio_stream_index: Option<i32>,
    ) -> String {
        let item_id = item.id.unwrap();
        if !Self::is_server_transcoded(item) {
            return self.get_stream_url(&item_id);
        }
        let mut url = self
            .root
            .join(&format!("Videos/{item_id}/master.m3u8"))
            .unwrap();
        url.query_pairs_mut()
            .append_pair("MediaSourceId", &item_id.simple().to_string())
            .append_pair("DeviceId", &self.account.device_id.to_string())
            .append_pair("VideoCodec", "h264,hevc")
            .append_pair("AudioCodec", "aac,mp3,ac3,eac3")
            .append_pair("SegmentContainer", "ts")
            // Nicht nur umpacken lassen: Disc-Video ist oft feldkodiert
            // (PAFF), damit ruckelt die Intel-Hardwaredekodierung. Der Server
            // kodiert neu (NVENC, mit Deinterlacing) wie für den Browser.
            .append_pair("AllowVideoStreamCopy", "false")
            .append_pair("VideoBitrate", "20000000")
            .append_pair("MaxWidth", "1920")
            .append_pair("MaxHeight", "1080")
            // Neue Sitzung je Anfrage, sonst liefert der Server beim
            // Spurwechsel die alten, schon umgewandelten Abschnitte.
            .append_pair("PlaySessionId", &Uuid::new_v4().simple().to_string())
            .append_pair("ApiKey", &self.account.access_token);
        if let Some(index) = audio_stream_index {
            url.query_pairs_mut()
                .append_pair("AudioStreamIndex", &index.to_string());
        }
        url.to_string()
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
