pub(super) const UNDERLAY_SCALE: f32 = 0.95;
pub(super) const WIDE_SIDEBAR_WIDTH: f32 = 248.0;

pub(super) struct ScreenPose {
    pub(super) slide: f32,
    pub(super) scale: f32,
    pub(super) opacity: f32,
    pub(super) boundary: f32,
    pub(super) shadow_width: f32,
    pub(super) shadow_left: f32,
    pub(super) shadow_opacity: f32,
}

impl ScreenPose {
    pub(super) fn at(phase: f32, travel: f32, compact: bool, backwards: bool) -> Self {
        let reveal = if backwards { 1.0 - phase } else { phase };
        let slide = travel * (1.0 - reveal);
        let boundary = slide + if compact { 0.0 } else { WIDE_SIDEBAR_WIDTH };
        let shadow_width = slide.min(22.0);
        let fade = ((reveal - 0.35) / 0.65).clamp(0.0, 1.0);
        Self {
            slide,
            scale: 1.0 - (1.0 - UNDERLAY_SCALE) * reveal,
            opacity: 1.0 - fade * fade * (3.0 - 2.0 * fade),
            boundary,
            shadow_width,
            shadow_left: boundary - shadow_width,
            shadow_opacity: (reveal * 8.0).min(1.0),
        }
    }
}
