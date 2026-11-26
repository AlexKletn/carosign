use carosign_l10n::Localizer;
use obscur_kit::{
    OkuiScreen, ScreenData, ScreenError,
    view::view::{View, ViewNode},
};

use crate::ui::shared;

pub(super) struct RegisteredRow {
    row: OkuiScreen,
    name_editor: OkuiScreen,
    actions: OkuiScreen,
    delete_confirmation: OkuiScreen,
}

impl RegisteredRow {
    pub(super) fn new() -> Result<Self, ScreenError> {
        let row = ScreenData::new();
        row.set("row_id", "");
        row.set("expanded", false);
        row.set("row_class", "fingerprint-row fingerprint-row-trigger");
        row.set("item_class", "registered-fingerprint");
        row.set("chevron", "fingerprint-chevron-down.svg");
        row.set("chevron_class", "fingerprint-row-disclosure");
        let editor = ScreenData::new();
        editor.set("name", "");
        editor.set("name_label", "");
        let actions = ScreenData::new();
        actions.set("edit_action_class", "");
        actions.set("action_class", "");
        actions.set("delete_class", "");
        actions.set("save_class", "");
        actions.set("save_disabled", true);
        Ok(Self {
            row: OkuiScreen::compile_with(include_str!("registered_row.okui"), row)?,
            name_editor: OkuiScreen::compile_with(
                include_str!("registered_name_editor.okui"),
                editor,
            )?,
            actions: OkuiScreen::compile_with(include_str!("registered_actions.okui"), actions)?,
            delete_confirmation: OkuiScreen::compile(include_str!(
                "registered_delete_confirmation.okui"
            ))?,
        })
    }

    pub(super) fn view(
        &self,
        id: &str,
        label: &str,
        selected: bool,
        draft: Option<&str>,
        confirming_delete: bool,
        can_rename: bool,
        localizer: &Localizer,
    ) -> Option<ViewNode> {
        let data = self.row.data();
        let row_id = format!("fingerprint-row-{id}");
        data.set("row_id", row_id.as_str());
        data.set("expanded", selected);
        data.set(
            "chevron_class",
            if draft.is_some() {
                "fingerprint-action-hidden"
            } else {
                "fingerprint-row-disclosure"
            },
        );
        data.set(
            "chevron",
            if selected {
                "fingerprint-chevron-up.svg"
            } else {
                "fingerprint-chevron-down.svg"
            },
        );
        data.set(
            "item_class",
            if selected {
                "registered-fingerprint fingerprint-row-open"
            } else {
                "registered-fingerprint"
            },
        );
        data.set("label", label);
        data.set(
            "row_class",
            if draft.is_some() {
                "fingerprint-action-hidden"
            } else {
                "fingerprint-row fingerprint-row-trigger"
            },
        );
        let editor = if let Some(name) = draft {
            let data = self.name_editor.data();
            data.set("name", name);
            data.set("name_label", localizer.text("ui-enroll-name"));
            self.name_editor.view()
        } else {
            View::empty()
        };
        let actions = if confirming_delete {
            let data = self.delete_confirmation.data();
            data.set(
                "confirmation_prompt",
                localizer.text("ui-fingerprint-delete-question"),
            );
            data.set("cancel_label", localizer.text("ui-fingerprint-cancel"));
            data.set("delete_label", localizer.text("ui-fingerprint-delete"));
            self.delete_confirmation.view()
        } else if selected {
            self.actions(draft, can_rename, localizer)
        } else {
            View::empty()
        };
        shared::fill(
            self.row.view(),
            [
                ("name_editor", editor.into_fragment()),
                ("actions", actions.into_fragment()),
            ],
        )
        .into_single_node()
        .map(|row| row.with_key(row_id))
    }

    fn actions(&self, draft: Option<&str>, can_rename: bool, localizer: &Localizer) -> View {
        let data = self.actions.data();
        let hidden = "fingerprint-action-hidden";
        data.set(
            "edit_action_class",
            if draft.is_some() { "" } else { hidden },
        );
        let idle = draft.is_none();
        data.set("action_class", if idle && can_rename { "" } else { hidden });
        data.set(
            "delete_class",
            if idle {
                "fingerprint-row-delete"
            } else {
                hidden
            },
        );
        data.set(
            "save_class",
            if draft.is_some() {
                "fingerprint-row-save"
            } else {
                hidden
            },
        );
        data.set(
            "save_disabled",
            draft.is_none_or(|name| name.trim().is_empty()),
        );
        data.set("rename_label", localizer.text("ui-fingerprint-rename"));
        data.set("delete_label", localizer.text("ui-fingerprint-delete"));
        data.set("cancel_label", localizer.text("ui-fingerprint-cancel"));
        data.set("save_label", localizer.text("ui-fingerprint-save-name"));
        self.actions.view()
    }
}
