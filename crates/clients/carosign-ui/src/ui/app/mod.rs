mod account_menu_scroll;
#[cfg(test)]
#[path = "back_swipe.tests.rs"]
mod back_swipe_tests;
mod dashboard;
mod run;
mod screen_motion;
mod screen_pose;
mod screen_transition;
mod system_defaults;
mod window_controls;
mod window_theme;

pub(crate) use dashboard::Dashboard;
pub(crate) use run::run;
