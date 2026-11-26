use std::{cell::Cell, rc::Rc};

use obscur_kit::AppControl;
use obscur_kit::headless::input::{SelectorResolver, SelectorTarget};
use obscur_kit::motion::{
    track::{Easing, Keyframe, KeyframeTrack},
    transition::TransitionInterruption,
    value::MotionValue,
};
use obscur_kit::runtime::orchestration::AnimatedKind;
use obscur_kit::style::property::PropertyId;

use super::screen_pose::ScreenPose;

pub(super) const DURATION: f32 = 0.25;

pub(super) struct ScreenMotion {
    pub(super) start: f32,
    pub(super) progress: Rc<Cell<f32>>,
    scheduled: bool,
}

impl ScreenMotion {
    pub(super) fn new(start: f32) -> Self {
        Self {
            start,
            progress: Rc::new(Cell::new(start)),
            scheduled: false,
        }
    }

    pub(super) fn schedule(
        &mut self,
        control: &AppControl,
        travel: f32,
        compact: bool,
        backwards: bool,
    ) {
        if std::mem::replace(&mut self.scheduled, true) {
            return;
        }
        let start = self.start;
        let progress = Rc::clone(&self.progress);
        let completion_control = control.clone();
        control.with_runtime(move |runtime| {
            let resolver = SelectorResolver::new();
            let mut timelines = Vec::new();
            let mut animate = |selector, property, kind, value: fn(ScreenPose) -> MotionValue| {
                let target = resolver.resolve(runtime, &SelectorTarget::test_id(selector))?;
                let track = KeyframeTrack::from_keyframes(
                    (0_u16..=32)
                        .map(|index| {
                            let time = f32::from(index) / 32.0;
                            let phase = start + (1.0 - start) * Easing::EaseInOut.apply(time);
                            Keyframe::new(
                                time,
                                value(ScreenPose::at(phase, travel, compact, backwards)),
                            )
                        })
                        .collect(),
                );
                let timeline = runtime.animate(
                    target,
                    property,
                    track,
                    DURATION * (1.0 - start).max(0.35),
                    kind,
                    TransitionInterruption::Restart,
                );
                timelines.push(timeline);
                Some(timeline)
            };
            let upper = animate(
                "screen-layer-upper",
                PropertyId::Transform,
                AnimatedKind::Transform,
                |pose| translate(pose.slide),
            );
            animate(
                "screen-layer-lower",
                PropertyId::Transform,
                AnimatedKind::Transform,
                |pose| MotionValue::transform([pose.scale, 0.0, 0.0, pose.scale, 0.0, 0.0]),
            );
            animate(
                "screen-transition-clip",
                PropertyId::Width,
                AnimatedKind::LengthPx,
                |pose| MotionValue::scalar(pose.boundary.max(0.001)),
            );
            if !compact {
                animate(
                    "screen-layer-lower",
                    PropertyId::Opacity,
                    AnimatedKind::Number,
                    |pose| MotionValue::scalar(pose.opacity),
                );
            }
            animate(
                "screen-transition-shadow",
                PropertyId::Transform,
                AnimatedKind::Transform,
                |pose| translate(pose.shadow_left),
            );
            animate(
                "screen-transition-shadow",
                PropertyId::Width,
                AnimatedKind::LengthPx,
                |pose| MotionValue::scalar(pose.shadow_width),
            );
            animate(
                "screen-transition-shadow",
                PropertyId::Opacity,
                AnimatedKind::Number,
                |pose| MotionValue::scalar(pose.shadow_opacity),
            );
            if let Some(upper) = upper {
                runtime.observe_animation_progress(upper, move |time| {
                    progress.set(start + (1.0 - start) * Easing::EaseInOut.apply(time));
                    if time >= 1.0 {
                        let timelines = std::mem::take(&mut timelines);
                        completion_control.with_runtime(move |runtime| {
                            for timeline in timelines {
                                runtime.cancel_animation(timeline);
                            }
                        });
                    }
                });
            }
        });
    }
}

fn translate(x: f32) -> MotionValue {
    MotionValue::transform([1.0, 0.0, 0.0, 1.0, x, 0.0])
}
