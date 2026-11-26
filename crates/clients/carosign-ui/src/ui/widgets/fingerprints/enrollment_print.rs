use obscur_kit::{
    geometry::Point,
    identity::InternedSymbol,
    scene::{
        command::CanvasRegion,
        paint::{Brush, LineCap, LineJoin, Stroke},
        path::{Path, PathTrim},
    },
    ui_ir::name_binding::{TagPosition, component_id_for},
    view::view::{
        CanvasPaint, ViewFragment, ViewNode, ViewProperty, ViewValue, canvas_painter_prop,
    },
};

use super::EnrollmentColors;

pub(in crate::ui) const PAINTER: InternedSymbol = InternedSymbol::named("enrollment-print");
const PROGRESS: InternedSymbol = InternedSymbol::named("enrollment-print-progress");
const MOTION: InternedSymbol = InternedSymbol::named("enrollment-print-motion");
const DRAW_SECONDS: f32 = 0.15;

pub(super) fn view(percent: f32, motion: bool) -> ViewFragment {
    ViewFragment::from_nodes(vec![
        ViewNode::element(component_id_for("canvas", TagPosition::Element))
            .class("enrollment-print")
            .with_prop(canvas_painter_prop(PAINTER))
            .with_prop(ViewProperty::new(
                PROGRESS,
                ViewValue::Float(f64::from(percent) / 100.0),
            ))
            .with_prop(ViewProperty::new(MOTION, ViewValue::Bool(motion))),
    ])
}

#[derive(Default)]
pub(in crate::ui) struct EnrollmentPrint {
    from: f32,
    target: f32,
    started: f32,
}

impl EnrollmentPrint {
    pub(in crate::ui) fn paint(
        &mut self,
        region: &mut CanvasRegion<'_>,
        input: &CanvasPaint<'_>,
        colors: EnrollmentColors,
    ) -> bool {
        let mut target = 0.0;
        let mut motion = false;
        for prop in input.props() {
            match (prop.name(), prop.value()) {
                (PROGRESS, ViewValue::Float(value)) => target = (*value as f32).clamp(0.0, 1.0),
                (MOTION, ViewValue::Bool(value)) => motion = *value,
                _ => {}
            }
        }
        let now = input.elapsed();
        if !motion || target < self.target {
            self.from = target;
            self.target = target;
        } else if target != self.target {
            self.from = self.progress(now);
            self.target = target;
            self.started = now;
        }
        let progress = self.progress(now);
        let width = region.size().width.min(region.size().height);
        let stroke = |color| {
            Stroke::new(Brush::solid(color), width / 16.0)
                .with_line_cap(LineCap::Round)
                .with_line_join(LineJoin::Round)
        };
        let ridges = Self::ridges(width / 256.0);
        let total: f32 = ridges.iter().map(PathTrim::length).sum();
        let mut remaining = progress * total;
        for ridge in ridges {
            let length = ridge.length();
            let fraction = (remaining / length).clamp(0.0, 1.0);
            let painter = region.painter();
            if fraction < 1.0 {
                painter.stroke(ridge.slice(fraction, 1.0), stroke(colors.track));
            }
            if fraction > 0.0 {
                painter.stroke(ridge.slice(0.0, fraction), stroke(colors.progress));
            }
            remaining -= length;
        }
        progress < self.target
    }

    fn progress(&self, now: f32) -> f32 {
        let time = ((now - self.started) / DRAW_SECONDS).clamp(0.0, 1.0);
        self.target - (self.target - self.from) * (1.0 - time).powi(2)
    }

    fn ridges(scale: f32) -> [PathTrim; 7] {
        let point = |(x, y)| Point::new(x * scale, y * scale);
        let curve = |start, segments: &[[(f32, f32); 3]]| {
            let mut path = Path::new();
            path.move_to(point(start));
            for &[first, second, end] in segments {
                path.cubic_to(point(first), point(second), point(end));
            }
            PathTrim::of(&path)
        };
        [
            curve(
                (104.0, 221.0),
                &[[(120.0, 192.0), (128.0, 160.0), (128.0, 128.0)]],
            ),
            curve(
                (96.0, 128.0),
                &[
                    [(96.0, 110.3), (110.3, 96.0), (128.0, 96.0)],
                    [(145.7, 96.0), (160.0, 110.3), (160.0, 128.0)],
                    [(160.0, 163.0), (153.0, 194.0), (139.0, 223.0)],
                ],
            ),
            curve(
                (75.0, 208.0),
                &[[(83.0, 194.0), (89.0, 177.0), (93.0, 160.0)]],
            ),
            curve(
                (51.0, 184.0),
                &[
                    [(60.0, 165.0), (64.0, 146.0), (64.0, 128.0)],
                    [(64.0, 108.0), (73.0, 90.0), (88.0, 78.0)],
                ],
            ),
            curve(
                (120.0, 64.5),
                &[
                    [(158.0, 60.0), (192.0, 90.0), (192.0, 128.0)],
                    [(192.0, 138.5), (191.0, 149.0), (190.0, 160.0)],
                ],
            ),
            curve(
                (179.0, 209.5),
                &[[(181.0, 203.0), (183.0, 197.0), (184.0, 192.0)]],
            ),
            curve(
                (26.5, 160.0),
                &[
                    [(30.0, 150.0), (32.0, 139.0), (32.0, 128.0)],
                    [(32.0, 75.0), (75.0, 32.0), (128.0, 32.0)],
                    [(181.0, 32.0), (224.0, 75.0), (224.0, 128.0)],
                    [(224.0, 147.0), (222.0, 165.5), (218.5, 184.0)],
                ],
            ),
        ]
    }
}
