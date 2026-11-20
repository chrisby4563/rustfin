use std::{cell::RefCell, sync::Arc};

use gtk::prelude::*;
use relm4::{
    component::{AsyncComponent, AsyncComponentParts},
    prelude::*,
    AsyncComponentSender,
};
use tracing::warn;
use uuid::Uuid;

use crate::{
    config::video_player_config::{SegmentAction, SegmentActions},
    globals::CONFIG,
    jellyfin_api::{
        api::intro_skipper::{SegmentKind, SkipSegment},
        api_client::ApiClient,
    },
    tr,
};

use super::backends::VideoPlayerBackend;

#[derive(Debug)]
enum State {
    Hidden,
    /// Knopf für den Abschnitt mit diesem Index.
    Visible(usize),
    AutoSkipCountdown(usize, usize),
}

impl State {
    fn visibility(&self) -> bool {
        !matches!(self, State::Hidden)
    }

    fn label(&self, segments: &[SkipSegment]) -> String {
        match self {
            Self::Visible(index) => match segments.get(*index).map(|s| s.kind) {
                Some(SegmentKind::Recap) => tr!("vp-skip-intro.manual-recap").to_string(),
                Some(SegmentKind::Outro) => tr!("vp-skip-intro.manual-outro").to_string(),
                Some(SegmentKind::Preview) => tr!("vp-skip-intro.manual-preview").to_string(),
                _ => tr!("vp-skip-intro.manual").to_string(),
            },
            Self::AutoSkipCountdown(_, seconds) => {
                tr!("vp-skip-intro.auto", { "seconds" => seconds }).to_string()
            }
            State::Hidden => String::default(),
        }
    }

    fn segment_index(&self) -> Option<usize> {
        match self {
            Self::Visible(index) | Self::AutoSkipCountdown(index, _) => Some(*index),
            Self::Hidden => None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct SkipIntro {
    video_player: Arc<RefCell<dyn VideoPlayerBackend>>,
    segments: Vec<SkipSegment>,
    state: State,
    /// Intro dieser Folge schon automatisch übersprungen (danach nur noch Knopf,
    /// falls man zurückspult, um es doch zu sehen).
    already_skipped: Vec<usize>,
    enabled: bool,
    actions: SegmentActions,
}

#[derive(Debug)]
pub(crate) enum SkipIntroInput {
    Load(Uuid, Arc<ApiClient>),
    PositionUpdate(usize),
    SkipIntro,
    ConfigUpdated(bool, SegmentActions),
}

#[relm4::component(pub(crate) async)]
impl AsyncComponent for SkipIntro {
    type Init = Arc<RefCell<dyn VideoPlayerBackend>>;
    type Input = SkipIntroInput;
    type Output = ();
    type CommandOutput = ();

    view! {
        gtk::Button {
            #[watch]
            set_visible: model.state.visibility(),
            #[watch]
            set_label: model.state.label(&model.segments).as_ref(),
            add_css_class: "opaque",
            set_margin_bottom: 12,
            set_halign: gtk::Align::End,

            connect_clicked[sender] => move |_| {
                sender.input(SkipIntroInput::SkipIntro);
            },
        }
    }

    async fn init(
        video_player: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        video_player
            .borrow_mut()
            .connect_position_updated(Box::new({
                let sender = sender.clone();
                move |position| {
                    sender.input(SkipIntroInput::PositionUpdate(position));
                }
            }));

        let model = SkipIntro {
            video_player,
            segments: Vec::new(),
            state: State::Hidden,
            already_skipped: Vec::new(),
            enabled: CONFIG.read().video_player.intro_skipper,
            actions: CONFIG.read().video_player.segment_actions,
        };

        SkipIntro::subscribe_to_config(&sender);

        let widgets = view_output!();

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        message: Self::Input,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            SkipIntroInput::Load(id, api_client) => {
                self.segments = match api_client.get_skip_segments(&id).await {
                    Ok(segments) => {
                        tracing::info!(
                            "Media segments for {id}: {:?}",
                            segments
                                .iter()
                                .map(|s| (s.kind, s.start as u32, s.end as u32))
                                .collect::<Vec<_>>()
                        );
                        segments
                    }
                    Err(err) => {
                        warn!("Error getting media segments for {id}: {err:#?}");
                        Vec::new()
                    }
                };
                self.already_skipped.clear();
                self.state = State::Hidden;
            }
            SkipIntroInput::PositionUpdate(position) => {
                let position = position as f32;
                if !self.enabled {
                    self.state = State::Hidden;
                    return;
                }

                // Je Abschnittsart: automatisch (einmal je Folge), Knopf oder nichts.
                for (index, segment) in self.segments.iter().enumerate() {
                    if self.action(segment.kind) != SegmentAction::Skip
                        || self.already_skipped.contains(&index)
                    {
                        continue;
                    }
                    if segment.range().contains(&position) {
                        self.already_skipped.push(index);
                        if !crate::syncplay::intercept_seek(segment.end as usize) {
                            self.video_player.borrow().seek_to(segment.end as usize);
                        }
                        self.state = State::Hidden;
                        return;
                    }
                    if segment.range_auto_skip_show().contains(&position) {
                        let seconds = (segment.start - position).ceil().max(1.0) as usize;
                        self.state = State::AutoSkipCountdown(index, seconds);
                        return;
                    }
                }

                self.state = self
                    .segments
                    .iter()
                    .position(|s| {
                        self.action(s.kind) != SegmentAction::Nothing
                            && s.range_show().contains(&position)
                    })
                    .map(State::Visible)
                    .unwrap_or(State::Hidden);
            }
            SkipIntroInput::SkipIntro => {
                if let Some(segment) = self
                    .state
                    .segment_index()
                    .and_then(|index| self.segments.get(index))
                {
                    if let Some(index) = self.state.segment_index() {
                        self.already_skipped.push(index);
                    }
                    if !crate::syncplay::intercept_seek(segment.end as usize) {
                        self.video_player.borrow().seek_to(segment.end as usize);
                    }
                    self.state = State::Hidden;
                }
            }
            SkipIntroInput::ConfigUpdated(enabled, actions) => {
                self.enabled = enabled;
                self.actions = actions;
            }
        }
    }
}

impl SkipIntro {
    fn action(&self, kind: SegmentKind) -> SegmentAction {
        match kind {
            SegmentKind::Intro => self.actions.intro,
            SegmentKind::Recap => self.actions.recap,
            SegmentKind::Outro => self.actions.outro,
            SegmentKind::Preview => self.actions.preview,
        }
    }

    fn subscribe_to_config(sender: &AsyncComponentSender<Self>) {
        CONFIG.subscribe(sender.input_sender(), |config| {
            SkipIntroInput::ConfigUpdated(
                config.video_player.intro_skipper,
                config.video_player.segment_actions,
            )
        });
    }
}
