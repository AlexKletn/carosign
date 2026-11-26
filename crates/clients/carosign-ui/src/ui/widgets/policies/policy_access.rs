use carosign_l10n::Localizer;
use obscur_kit::view::view::View;
use obscur_kit::{OkuiScreen, ScreenData, ScreenError};

pub(super) struct PolicyAccess(OkuiScreen);

impl PolicyAccess {
    pub(super) fn new(localizer: &Localizer) -> Result<Self, ScreenError> {
        let data = ScreenData::new();
        data.set("title", localizer.text("ui-policy-access-title"));
        data.set(
            "description",
            localizer.text("ui-policy-access-description"),
        );
        data.set("unlock_label", localizer.text("ui-policy-unlock"));
        data.set("pending", false);
        data.set("status_class", "policy-hidden");
        data.set("status", "");
        Ok(Self(OkuiScreen::compile_with(
            include_str!("policy_access.okui"),
            data,
        )?))
    }

    pub(super) fn view(&self, localizer: &Localizer, pending: bool, error: Option<&str>) -> View {
        let data = self.0.data();
        let (class, key) = match (pending, error) {
            (true, _) => ("status", Some("ui-policy-access-pending")),
            (false, Some("ui-policy-denied")) => {
                ("status warning", Some("ui-policy-access-denied"))
            }
            (false, Some(key)) => ("status warning", Some(key)),
            (false, None) => ("policy-hidden", None),
        };
        data.set("pending", pending);
        data.set("status_class", class);
        data.set(
            "status",
            key.map_or_else(String::new, |key| localizer.text(key)),
        );
        self.0.view()
    }
}
