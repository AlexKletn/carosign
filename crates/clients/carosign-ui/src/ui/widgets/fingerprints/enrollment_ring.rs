use std::f32::consts::{FRAC_PI_2, TAU};

use obscur_kit::{
    geometry::Point,
    identity::InternedSymbol,
    scene::{
        command::CanvasRegion,
        paint::{Brush, LineCap, LineJoin, Stroke},
        path::Path,
    },
    ui_ir::name_binding::{TagPosition, component_id_for},
    view::view::{
        CanvasPaint, ViewFragment, ViewNode, ViewProperty, ViewValue, canvas_painter_prop,
    },
};

use super::EnrollmentColors;

pub(in crate::ui) const PAINTER: InternedSymbol = InternedSymbol::named("enrollment-ring");
const PERCENT: InternedSymbol = InternedSymbol::named("enrollment-ring-percent");

pub(super) fn view(percent: f32) -> ViewFragment {
    ViewFragment::from_nodes(vec![
        ViewNode::element(component_id_for("canvas", TagPosition::Element))
            .class("enrollment-ring")
            .with_prop(canvas_painter_prop(PAINTER))
            .with_prop(ViewProperty::new(
                PERCENT,
                ViewValue::Float(f64::from(percent)),
            )),
    ])
}

pub(in crate::ui) fn paint(
    region: &mut CanvasRegion<'_>,
    input: &CanvasPaint<'_>,
    colors: EnrollmentColors,
) -> bool {
    let percent = input
        .props()
        .iter()
        .find_map(|prop| {
            if prop.name() != PERCENT {
                return None;
            }
            match prop.value() {
                ViewValue::Float(value) => Some(*value as f32),
                _ => None,
            }
        })
        .unwrap_or(0.0)
        .clamp(0.0, 100.0);

    let center = region.at(0.5, 0.5);
    let size = region.size();
    let radius = size.width.min(size.height) / 2.0 - 5.0;
    if radius <= 0.0 {
        return false;
    }
    let stroke = |color| {
        Stroke::new(Brush::solid(color), 3.0)
            .with_line_cap(LineCap::Round)
            .with_line_join(LineJoin::Round)
    };
    let draw = |progress: f32| {
        let mut path = Path::new();
        let segments = (progress * 96.0).ceil().max(1.0) as u32;
        for step in 0..=segments {
            let angle = TAU * progress * step as f32 / segments as f32 - FRAC_PI_2;
            let point = Point::new(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            );
            if step == 0 {
                path.move_to(point);
            } else {
                path.line_to(point);
            }
        }
        path
    };
    region.painter().stroke(draw(1.0), stroke(colors.track));
    if percent > 0.0 {
        region
            .painter()
            .stroke(draw(percent / 100.0), stroke(colors.progress));
    }
    false
}
