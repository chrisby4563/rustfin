//! Medieninfo auf der Detailseite: Bild, Ton, Untertitel und Datei, als
//! aufklappbare Zeile (zu: Kurzfassung, auf: alle Spuren).

use adw::prelude::*;
use jellyfin_api::types::{BaseItemDto, MediaSourceInfo, MediaStream, MediaStreamType};
use relm4::{adw, gtk};

use crate::tr;

pub(crate) fn media_info_widget(item: &BaseItemDto, expanded: bool) -> Option<gtk::Widget> {
    let source = item.media_sources.as_ref()?.first()?;
    let streams = source.media_streams.clone().unwrap_or_default();
    let of_type = |kind: MediaStreamType| -> Vec<&MediaStream> {
        streams.iter().filter(|s| s.type_ == Some(kind)).collect()
    };
    let video = of_type(MediaStreamType::Video);
    let audio = of_type(MediaStreamType::Audio);
    let subtitles = of_type(MediaStreamType::Subtitle);
    if video.is_empty() && audio.is_empty() {
        return None;
    }

    let expander = adw::ExpanderRow::builder()
        .title(tr!("media-details-media-info"))
        .subtitle(summary(video.first().copied(), audio.len(), subtitles.len()))
        .expanded(expanded)
        .build();

    for stream in &video {
        add_row(&expander, tr!("media-details-media-info-video"), &video_text(stream));
    }
    for stream in &audio {
        add_row(&expander, tr!("media-details-media-info-audio"), &stream_text(stream));
    }
    for stream in &subtitles {
        add_row(
            &expander,
            tr!("media-details-media-info-subtitle"),
            &stream_text(stream),
        );
    }
    if let Some(file) = file_text(source) {
        add_row(&expander, tr!("media-details-media-info-file"), &file);
    }

    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    list.append(&expander);
    Some(list.upcast())
}

fn add_row(expander: &adw::ExpanderRow, title: &str, subtitle: &str) {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .subtitle_selectable(true)
        .css_classes(["property"])
        .build();
    expander.add_row(&row);
}

/// Zugeklappt: „4K HEVC Dolby Vision · 3 Tonspuren · 12 Untertitel“.
fn summary(video: Option<&MediaStream>, audio: usize, subtitles: usize) -> String {
    let mut parts = vec![];
    if let Some(title) = video.and_then(|v| v.display_title.clone()) {
        parts.push(title);
    }
    if audio > 0 {
        parts.push(tr!("media-details-media-info-audio-count", {"count" => audio}).to_string());
    }
    if subtitles > 0 {
        parts.push(
            tr!("media-details-media-info-subtitle-count", {"count" => subtitles}).to_string(),
        );
    }
    parts.join(" · ")
}

fn video_text(stream: &MediaStream) -> String {
    let mut parts = vec![];
    if let Some(title) = &stream.display_title {
        parts.push(title.clone());
    }
    if let (Some(w), Some(h)) = (stream.width, stream.height) {
        parts.push(format!("{w}×{h}"));
    }
    if let Some(codec) = &stream.codec {
        let mut codec = codec.to_uppercase();
        if let Some(profile) = &stream.profile {
            codec = format!("{codec} {profile}");
        }
        parts.push(codec);
    }
    if let Some(depth) = stream.bit_depth {
        parts.push(format!("{depth} Bit"));
    }
    if let Some(fps) = stream.real_frame_rate.or(stream.reference_frame_rate) {
        parts.push(format!("{fps:.3} fps").replace(".000", ""));
    }
    if let Some(rate) = stream.bit_rate {
        parts.push(format!("{:.1} Mbit/s", rate as f64 / 1_000_000.0));
    }
    parts.join(" · ")
}

fn stream_text(stream: &MediaStream) -> String {
    let mut text = stream
        .display_title
        .clone()
        .or_else(|| stream.language.clone())
        .unwrap_or_else(|| "?".to_string());
    if let Some(title) = stream.title.as_ref().filter(|t| !text.contains(t.as_str())) {
        text = format!("{text} – {title}");
    }
    if stream.is_external == Some(true) {
        text = format!("{text} ({})", tr!("media-details-media-info-external"));
    }
    text
}

fn file_text(source: &MediaSourceInfo) -> Option<String> {
    let mut parts = vec![];
    if let Some(container) = &source.container {
        parts.push(container.to_uppercase());
    }
    if let Some(size) = source.size {
        parts.push(format!("{:.1} GB", size as f64 / 1_000_000_000.0));
    }
    if let Some(rate) = source.bitrate {
        parts.push(format!("{:.1} Mbit/s", rate as f64 / 1_000_000.0));
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}
