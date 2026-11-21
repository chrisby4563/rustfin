//! Media information on the detail page: video, audio, subtitles, and file as
//! an expandable row (closed: summary, open: all tracks).

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

/// Collapsed: “4K HEVC Dolby Vision · 3 audio tracks · 12 subtitles”.
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

/// Movie design: media info as a box beside chapters and cast.
pub(crate) fn media_info_box(item: &BaseItemDto) -> Option<gtk::Widget> {
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

    let card = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .valign(gtk::Align::Start)
        .width_request(300)
        .css_classes(["media-info-card"])
        .build();
    card.append(
        &gtk::Label::builder()
            .label(tr!("media-details-media-info"))
            .xalign(0.0)
            .css_classes(["heading", "settings-heading"])
            .build(),
    );

    let short = |list: &[&MediaStream]| -> String {
        let names: Vec<String> = list.iter().map(|s| stream_text(s)).collect();
        if names.len() > 3 {
            format!("{} · +{}", names[..3].join(" · "), names.len() - 3)
        } else {
            names.join(" · ")
        }
    };
    let mut rows: Vec<(String, String)> = Vec::new();
    if let Some(v) = video.first() {
        rows.push((tr!("media-details-media-info-video").to_string(), video_text(v)));
    }
    if !audio.is_empty() {
        rows.push((tr!("media-details-media-info-audio").to_string(), short(&audio)));
    }
    if !subtitles.is_empty() {
        let mut langs: Vec<String> = Vec::new();
        for s in &subtitles {
            let lang = s
                .language
                .clone()
                .or_else(|| s.display_title.clone())
                .unwrap_or_else(|| "?".into());
            if !langs.contains(&lang) {
                langs.push(lang);
            }
        }
        rows.push((tr!("media-details-media-info-subtitle").to_string(), langs.join(", ")));
    }
    if let Some(file) = file_text(source) {
        rows.push((tr!("media-details-media-info-file").to_string(), file));
    }
    rows.push((
        tr!("media-details-media-info-playback").to_string(),
        if crate::jellyfin_api::api_client::ApiClient::is_server_transcoded(item) {
            tr!("media-details-media-info-playback-transcode").to_string()
        } else {
            tr!("media-details-media-info-playback-direct").to_string()
        },
    ));

    for (key, value) in rows {
        let row = gtk::Box::builder()
            .spacing(12)
            .css_classes(["media-info-line"])
            .build();
        row.append(
            &gtk::Label::builder()
                .label(&key)
                .xalign(0.0)
                .valign(gtk::Align::Start)
                .css_classes(["dim-label"])
                .build(),
        );
        row.append(
            &gtk::Label::builder()
                .label(&value)
                .xalign(1.0)
                .hexpand(true)
                .wrap(true)
                .selectable(true)
                .justify(gtk::Justification::Right)
                .build(),
        );
        card.append(&row);
    }
    Some(card.upcast())
}
