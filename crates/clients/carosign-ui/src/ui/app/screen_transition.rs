use super::{screen_motion::ScreenMotion, window_controls::WindowControls};

use obscur_kit::AppControl;
use obscur_kit::identity::InternedSymbol;
#[cfg(test)]
use obscur_kit::runtime::frame::FrameReason;
use obscur_kit::ui_ir::name_binding::{TagPosition, component_id_for};
use obscur_kit::view::view::{
    DISABLED, EVENT_POINTER_UP, EVENT_SCROLL_GESTURE, EVENT_TOUCH_GESTURE, EVENT_TOUCHPAD_GESTURE,
    TEST_ID, View, ViewEventBinding, ViewFragment, ViewNode, ViewProperty, ViewValue,
};

use super::screen_pose::{ScreenPose, WIDE_SIDEBAR_WIDTH};

#[derive(Default)]
pub(super) struct ScreenTransition {
    active: Option<(u64, View)>,
    outgoing: Option<(u64, View)>,
    backwards: bool,
    motion: Option<ScreenMotion>,
    generation: u64,
    tracking: Option<f32>,
    viewport: Option<(f32, bool)>,
    window_maximized: bool,
    window_controls: Option<WindowControls>,
}

impl ScreenTransition {
    pub(super) fn set_window_controls(&mut self, controls: WindowControls) {
        self.window_controls = Some(controls);
    }

    pub(super) fn set_window_maximized(&mut self, maximized: bool) {
        self.window_maximized = maximized;
    }

    pub(super) fn update(&mut self, key: u64, view: View, animate: bool, backwards: bool) {
        if !animate {
            self.clear();
        }
        let previous = self.active.replace((key, view));
        let Some(previous) = previous else { return };
        if !animate || previous.0 == key {
            return;
        }

        let phase = self.progress();
        let reversing = phase < 1.0 && self.outgoing.as_ref().is_some_and(|(id, _)| *id == key);
        let progress = if reversing { 1.0 - phase } else { 0.0 };
        self.tracking = None;
        self.outgoing = Some(previous);
        self.backwards = if reversing {
            !self.backwards
        } else {
            backwards
        };
        self.start_motion(progress);
    }

    pub(super) fn track_back(
        &mut self,
        offset: f32,
        width: f32,
        compact: bool,
        parent: impl FnOnce() -> (u64, View),
    ) {
        if self.tracking.is_none() {
            self.outgoing = Some(parent());
            self.backwards = false;
            if self.motion.take().is_some() {
                self.generation += 1;
            }
        }
        self.tracking =
            Some((1.0 - offset / Self::travel_width(width, compact).max(1.0)).clamp(0.0, 1.0));
    }

    pub(super) fn release_back(&mut self) {
        if let Some(phase) = self.tracking.take() {
            self.start_motion(phase);
        }
    }

    fn travel_width(width: f32, compact: bool) -> f32 {
        if compact {
            width
        } else {
            (width - WIDE_SIDEBAR_WIDTH).max(0.0)
        }
    }

    pub(super) fn view(&mut self, width: f32, compact: bool, control: &AppControl) -> View {
        let progress = self.progress();
        if let Some(previous) = self.viewport.replace((width, compact))
            && previous != (width, compact)
            && self.motion.is_some()
            && progress < 1.0
        {
            self.start_motion(progress);
        }
        if progress >= 1.0 && self.tracking.is_none() {
            self.clear();
        }
        let Some((active_key, active)) = self.active.as_ref() else {
            return View::empty();
        };
        let sidebar = if !compact
            && let Some(ViewNode::Element { children, .. }) = active.root().nodes().first()
        {
            children.nodes().first()
        } else {
            None
        };
        let mut layers = Vec::with_capacity(2);
        if let Some((outgoing_key, outgoing)) = self.outgoing.as_ref() {
            let phase = self.motion.as_ref().map_or(progress, |motion| motion.start);
            let pose = ScreenPose::at(
                phase,
                Self::travel_width(width, compact),
                compact,
                self.backwards,
            );
            let slide = pose.slide;
            let scale = pose.scale;
            let upper = format!("transform: translateX({slide:.3}px)");
            let lower = if compact {
                format!("transform: scale({scale:.5})")
            } else {
                format!("transform: scale({scale:.5}); opacity: {:.5}", pose.opacity)
            };
            let boundary = pose.boundary;
            let upper = ("screen-layer-upper", upper, None);
            let lower = ("screen-layer-lower", lower, Some(boundary.max(0.001)));
            let (active_plane, outgoing_plane) = if self.backwards {
                (lower, upper)
            } else {
                (upper, lower)
            };
            layers.push(Self::layer(
                *active_key,
                self.generation,
                active,
                Some(active_plane),
                true,
                compact,
            ));
            layers.push(Self::layer(
                *outgoing_key,
                self.generation,
                outgoing,
                Some(outgoing_plane),
                false,
                compact,
            ));
            if pose.shadow_width > 0.0 || self.motion.is_some() {
                layers.push(
                    ViewNode::element(component_id_for("div", TagPosition::Element))
                        .with_key(format!("screen-transition-shadow:{}", self.generation))
                        .class("screen-transition-shadow")
                        .with_prop(ViewProperty::new(
                            TEST_ID,
                            ViewValue::Text("screen-transition-shadow".into()),
                        ))
                        .style(format!(
                            "width: {:.3}px; transform: translateX({:.3}px); opacity: {:.5}",
                            pose.shadow_width, pose.shadow_left, pose.shadow_opacity,
                        )),
                );
            }
            if let Some(motion) = &mut self.motion {
                motion.schedule(
                    control,
                    Self::travel_width(width, compact),
                    compact,
                    self.backwards,
                );
            }
        } else {
            layers.push(Self::layer(
                *active_key,
                self.generation,
                active,
                None,
                true,
                compact,
            ));
        }

        let stage_class = if let Some(sidebar) = sidebar {
            layers.push(
                ViewNode::element(component_id_for("div", TagPosition::Element))
                    .with_key("stationary-sidebar")
                    .class("stationary-sidebar")
                    .with_children(ViewFragment::from_nodes(vec![sidebar.clone()])),
            );
            "screen-stage screen-stage-wide"
        } else {
            "screen-stage"
        };
        if !compact {
            layers.push(
                ViewNode::element(component_id_for("div", TagPosition::Element))
                    .with_key("content-divider")
                    .class("content-divider"),
            );
        }
        let mut stage_class = stage_class.to_owned();
        if compact {
            stage_class.push_str(" screen-stage-compact");
        }
        if let Some(controls) = &self.window_controls {
            layers.extend(controls.view.root().clone().into_nodes());
            if controls.left_width != 0 {
                stage_class.push_str(" screen-stage-with-left-controls");
            }
            stage_class.push_str(" screen-stage-with-window-controls");
        }
        if self.window_maximized {
            stage_class.push_str(" screen-stage-maximized");
        }
        let mut stage = ViewNode::element(component_id_for("div", TagPosition::Element))
            .class(stage_class)
            .with_children(ViewFragment::from_nodes(layers));
        if let Some(controls) = &self.window_controls {
            let left_occupied = if controls.left_width == 0 {
                0
            } else {
                controls.left_width + 8
            };
            let left_inset = (controls.left_width + 20).max(28);
            let right_inset = (controls.right_width + 20).max(28);
            stage = stage.style(format!(
                "--window-left-occupied: {left_occupied}px; --window-left-title-inset: {left_inset}px; --window-right-title-inset: {right_inset}px;"
            ));
        }
        let command = InternedSymbol::named("navigation_swipe");
        View::single(
            [
                EVENT_SCROLL_GESTURE,
                EVENT_TOUCHPAD_GESTURE,
                EVENT_TOUCH_GESTURE,
                EVENT_POINTER_UP,
            ]
            .into_iter()
            .fold(stage, |stage, event| {
                stage.with_event(ViewEventBinding::new(event, command))
            }),
        )
    }

    fn layer(
        key: u64,
        generation: u64,
        view: &View,
        plane: Option<(&str, String, Option<f32>)>,
        active: bool,
        compact: bool,
    ) -> ViewNode {
        let mut screens = view.root().clone().into_nodes();
        if !compact && let Some(ViewNode::Element { children, .. }) = screens.first_mut() {
            let mut parts = std::mem::take(children).into_nodes();
            if let Some(ViewNode::Element { children, .. }) = parts.first_mut() {
                *children = ViewFragment::new();
            }
            *children = ViewFragment::from_nodes(parts);
        }
        let children = ViewFragment::from_nodes(screens);
        let component = component_id_for("div", TagPosition::Element);
        let mut node = ViewNode::element(component);
        let mut viewport = ViewNode::element(component).with_key(format!("{key}:{generation}"));
        let mut viewport_classes = String::from("screen-layer-viewport");
        let mut classes = String::from("screen-layer");
        if !active {
            classes.push_str(" screen-layer-outgoing");
        }
        if let Some((plane_class, style, clip_width)) = plane {
            viewport_classes.push(' ');
            viewport_classes.push_str(plane_class);
            if let Some(clip_width) = clip_width {
                viewport = viewport.style(format!("width: {clip_width:.3}px; overflow: hidden"));
            }
            if clip_width.is_some() {
                viewport = viewport.with_prop(ViewProperty::new(
                    TEST_ID,
                    ViewValue::Text("screen-transition-clip".into()),
                ));
            }
            node = node.with_prop(ViewProperty::new(
                TEST_ID,
                ViewValue::Text(plane_class.into()),
            ));
            classes.push(' ');
            classes.push_str(plane_class);
            node = node.style(style);
        }
        node = node.class(classes).with_children(children);
        if !active {
            node = node.with_prop(ViewProperty::new(DISABLED, ViewValue::Bool(true)));
        }
        viewport
            .class(viewport_classes)
            .with_children(ViewFragment::from_nodes(vec![node]))
    }

    fn progress(&self) -> f32 {
        self.tracking
            .or_else(|| self.motion.as_ref().map(|motion| motion.progress.get()))
            .unwrap_or(1.0)
    }

    fn start_motion(&mut self, phase: f32) {
        if self.motion.replace(ScreenMotion::new(phase)).is_some() {
            self.generation += 1;
        }
    }

    fn clear(&mut self) {
        self.outgoing = None;
        if self.progress() < 1.0 && self.motion.is_some() {
            self.generation += 1;
        }
        self.motion = None;
        self.tracking = None;
    }
}

#[cfg(test)]
#[path = "screen_transition.tests.rs"]
mod tests;
