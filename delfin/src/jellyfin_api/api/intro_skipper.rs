//! Überspringbare Abschnitte (Intro, Rückblick, Abspann, Vorschau).
//!
//! Früher kamen die Zeiten vom IntroSkipper-Plugin unter
//! `Episode/{id}/IntroTimestamps`. Seit Jellyfin 10.10 liefert der Server sie
//! selbst als „Media Segments“ (`/MediaSegments/{itemId}`); die alte Adresse
//! gibt es nicht mehr (404).

use std::ops::Range;

use anyhow::Result;
use jellyfin_api::types::{MediaSegmentDtoQueryResult, MediaSegmentType};
use uuid::Uuid;

use crate::jellyfin_api::api_client::ApiClient;

const TICKS_PER_SECOND: f32 = 10_000_000.0;
/// So viele Sekunden vor dem Intro läuft der Countdown fürs automatische Überspringen.
const AUTO_SKIP_LEAD_SECONDS: f32 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentKind {
    Intro,
    Recap,
    Outro,
    Preview,
}

/// Ein überspringbarer Abschnitt, Zeiten in Sekunden.
#[derive(Debug, Clone)]
pub struct SkipSegment {
    pub kind: SegmentKind,
    pub start: f32,
    pub end: f32,
}

impl SkipSegment {
    /// Abschnitt selbst.
    pub fn range(&self) -> Range<f32> {
        self.start..self.end
    }

    /// Solange ist der Knopf sichtbar (bis kurz vor Ende des Abschnitts).
    pub fn range_show(&self) -> Range<f32> {
        self.start..(self.end - 1.0).max(self.start)
    }

    /// Countdown vor dem automatischen Überspringen (nur Intro).
    pub fn range_auto_skip_show(&self) -> Range<f32> {
        (self.start - AUTO_SKIP_LEAD_SECONDS).max(0.0)..self.start
    }
}

impl ApiClient {
    pub async fn get_skip_segments(&self, item_id: &Uuid) -> Result<Vec<SkipSegment>> {
        let url = self.root.join(&format!("MediaSegments/{item_id}"))?;
        let res: MediaSegmentDtoQueryResult = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let mut segments: Vec<SkipSegment> = res
            .items
            .into_iter()
            .filter_map(|segment| {
                let kind = match segment.type_ {
                    MediaSegmentType::Intro => SegmentKind::Intro,
                    MediaSegmentType::Recap => SegmentKind::Recap,
                    MediaSegmentType::Outro => SegmentKind::Outro,
                    MediaSegmentType::Preview => SegmentKind::Preview,
                    _ => return None,
                };
                let start = segment.start_ticks? as f32 / TICKS_PER_SECOND;
                let end = segment.end_ticks? as f32 / TICKS_PER_SECOND;
                // Abschnitte unter 2 s lohnen keinen Knopf.
                (end - start >= 2.0).then_some(SkipSegment { kind, start, end })
            })
            .collect();
        segments.sort_by(|a, b| a.start.total_cmp(&b.start));
        Ok(segments)
    }
}
