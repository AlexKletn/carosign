use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use obscur_kit::{
    capability::SupportLevel,
    system::appearance::{
        AppearanceCallback, AppearanceProvider, AppearanceWatch, Contrast, SystemAppearance,
    },
    system_platform::provider::LinuxAppearanceProvider,
};

use super::system_defaults::with_fallbacks;
use crate::ui::widgets::EnrollmentColors;

pub(super) const STYLESHEET: &str = include_str!(concat!(env!("OUT_DIR"), "/window.css"));

#[derive(Clone)]
pub(super) struct WindowTheme {
    desktop: LinuxAppearanceProvider,
    reduced_motion: Rc<Cell<bool>>,
    appearance: Rc<RefCell<SystemAppearance>>,
    enrollment_colors: Rc<Cell<EnrollmentColors>>,
}

impl Default for WindowTheme {
    fn default() -> Self {
        Self {
            desktop: LinuxAppearanceProvider::default(),
            reduced_motion: Rc::new(Cell::new(false)),
            appearance: Rc::default(),
            enrollment_colors: Rc::new(Cell::new(enrollment_colors(&with_fallbacks(
                SystemAppearance::default(),
            )))),
        }
    }
}

impl WindowTheme {
    pub(super) fn snapshot(&self) -> std::cell::Ref<'_, SystemAppearance> {
        self.appearance.borrow()
    }

    pub(super) fn motion_enabled(&self) -> bool {
        !self.reduced_motion.get()
    }

    pub(super) fn enrollment_colors(&self) -> EnrollmentColors {
        self.enrollment_colors.get()
    }
}

impl AppearanceProvider for WindowTheme {
    fn appearance(&self) -> SystemAppearance {
        let appearance = with_fallbacks(self.desktop.appearance());
        self.appearance.replace(appearance.clone());
        self.reduced_motion
            .set(appearance.reduced_motion == Some(true));
        self.enrollment_colors.set(enrollment_colors(&appearance));
        appearance
    }

    fn support(&self) -> SupportLevel {
        self.desktop.support()
    }

    fn watch_support(&self) -> SupportLevel {
        self.desktop.watch_support()
    }

    fn watch(&self, callback: AppearanceCallback) -> Option<AppearanceWatch> {
        self.desktop.watch(callback)
    }
}

pub(super) fn enrollment_colors(appearance: &SystemAppearance) -> EnrollmentColors {
    let foreground = appearance
        .palette
        .window_foreground
        .expect("resolved window foreground");
    let high_contrast = appearance.contrast == Some(Contrast::High);
    EnrollmentColors {
        progress: if high_contrast {
            foreground
        } else {
            appearance.accent.expect("resolved accent")
        },
        track: foreground.with_alpha(if high_contrast { 0.48 } else { 0.28 }),
    }
}

#[cfg(test)]
pub(crate) fn apply(control: &obscur_kit::AppControl, appearance: SystemAppearance) {
    control.set_system_appearance(with_fallbacks(appearance));
    control.set_stylesheets(vec![STYLESHEET.to_owned()]);
}

#[cfg(test)]
#[path = "window_theme.tests.rs"]
mod tests;
