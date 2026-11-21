//! Skippable segments (intro, recap, credits, preview).
//!
//! The times formerly came from the IntroSkipper plugin at
//! `Episode/{id}/IntroTimestamps`. Since Jellyfin 10.10, the server provides
//! them as “Media Segments” (`/MediaSegments/{itemId}`); the old endpoint is
//! gone (404).

use std::ops::Range;

use anyhow::Result;
use jellyfin_api::types::{MediaSegmentDtoQueryResult, MediaSegmentType};
use uuid::Uuid;

use crate::jellyfin_api::api_client::ApiClient;

const TICKS_PER_SECOND: f32 = 10_000_000.0;
/// Seconds before an intro when the automatic-skip countdown starts.
const AUTO_SKIP_LEAD_SECONDS: f32 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentKind {
    Intro,
    Recap,
    Outro,
    Preview,
}

/// A skippable segment; times are in seconds.
#[derive(Debug, Clone)]
pub struct SkipSegment {
    pub kind: SegmentKind,
    pub start: f32,
    pub end: f32,
}

impl SkipSegment {
    /// The segment itself.
    pub fn range(&self) -> Range<f32> {
        self.start..self.end
    }

    /// The button remains visible until shortly before the segment ends.
    pub fn range_show(&self) -> Range<f32> {
        self.start..(self.end - 1.0).max(self.start)
    }

    /// Countdown before automatic skipping (intros only).
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
                // Segments shorter than 2 s are not worth a button.
                (end - start >= 2.0).then_some(SkipSegment { kind, start, end })
            })
            .collect();
        segments.sort_by(|a, b| a.start.total_cmp(&b.start));
        Ok(segments)
    }
}
