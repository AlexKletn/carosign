use carosign_l10n::Localizer;
use obscur_kit::view::view::View;
use obscur_kit::{OkuiScreen, ScreenData, ScreenError};

use crate::{domain::Snapshot, ui::page::Page};

pub(in crate::ui) struct Sidebar(OkuiScreen);

impl Sidebar {
    pub(in crate::ui) fn new(localizer: &Localizer) -> Result<Self, ScreenError> {
        let data = ScreenData::new();
        data.set("restart_disabled", false);
        data.set("restart_label", localizer.text("ui-policy-restart-service"));
        data.set("restart_status_class", "footer-restart-status-hidden");
        data.set("app_name", localizer.text("ui-title"));
        data.set("title", localizer.text("ui-settings-title"));
        data.set("platform_label", localizer.text("monitor-platform"));
        data.set(
            "registration_label",
            localizer.text("ui-navigation-registration"),
        );
        data.set("settings_label", localizer.text("ui-navigation-settings"));
        data.set("face_label", localizer.text("ui-navigation-face"));
        data.set("iris_label", localizer.text("ui-navigation-iris"));
        data.set("voice_label", localizer.text("ui-navigation-voice"));
        data.set("fingerprints_label", localizer.text("ui-fingerprints"));
        data.set("fingerprints_class", "nav-item");
        data.set("policies_label", localizer.text("ui-policies"));
        data.set("policies_class", "nav-item");
        data.set("platform_class", "nav-item");
        Ok(Self(OkuiScreen::compile_with(
            include_str!("sidebar.okui"),
            data,
        )?))
    }

    pub(in crate::ui) fn view(
        &self,
        page: Page,
        snapshot: Option<&Snapshot>,
        localizer: &Localizer,
        restart: (bool, Option<&str>),
    ) -> View {
        let data = self.0.data();
        let status = match snapshot.map(|snapshot| &snapshot.health) {
            Some(Ok(health)) if health.service_ready => "ui-service-running",
            Some(Ok(_)) => "monitor-not-ready",
            Some(Err(error)) => error.message_key(),
            None => "monitor-loading",
        };
        data.set("service_state", localizer.text(status));
        data.set("restart_disabled", restart.0);
        data.set(
            "restart_status_class",
            match restart.1 {
                Some("ui-policy-restart-failed") => "footer-restart-status warning",
                Some(_) => "footer-restart-status",
                None => "footer-restart-status-hidden",
            },
        );
        data.set(
            "restart_status",
            restart
                .1
                .map_or_else(String::new, |key| localizer.text(key)),
        );
        data.set("platform_class", nav_class(page == Page::Platform));
        data.set("fingerprints_class", nav_class(page == Page::Fingerprints));
        data.set("policies_class", nav_class(page == Page::Policies));
        self.0.view()
    }
}

fn nav_class(selected: bool) -> &'static str {
    if selected {
        "nav-item selected"
    } else {
        "nav-item"
    }
}
