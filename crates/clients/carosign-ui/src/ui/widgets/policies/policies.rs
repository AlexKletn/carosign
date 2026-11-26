use std::cell::RefCell;

use carosign_l10n::Localizer;
use obscur_kit::view::view::View;
use obscur_kit::{OkuiScreen, ScreenData, ScreenError};

use super::{
    PolicyJob, PolicyResult,
    account_picker::AccountPicker,
    policy_access::PolicyAccess,
    policy_draft::PolicyDraft,
    rule_form::{ACTIONS, PURPOSES, RuleForm},
    rule_list::RuleList,
};

pub(in crate::ui) struct Policies {
    screen: OkuiScreen,
    access: PolicyAccess,
    draft: RefCell<PolicyDraft>,
    accounts: RefCell<AccountPicker>,
}

impl Policies {
    pub(in crate::ui) const FIELDS: [&str; 2] = ["lifetime", "select_search"];
    const COMMANDS: [&str; 19] = [
        "policy_unlock",
        "policy_restart_service",
        "policy_caller",
        "policy_subject",
        "policy_write",
        "policy_previous",
        "policy_next",
        "policy_add",
        "policy_delete",
        "policy_undo_delete",
        "policy_reload",
        "policy_close_editor",
        "policy_advanced",
        "verify",
        "enroll",
        "remove",
        "observe",
        "inventory",
        "cancel",
    ];

    pub(in crate::ui) fn commands() -> impl Iterator<Item = &'static str> {
        Self::COMMANDS
            .into_iter()
            .chain(RuleList::ROWS)
            .chain(PURPOSES.iter().map(|(_, name)| *name))
    }

    pub(in crate::ui) fn requires_unlock(&self) -> bool {
        self.draft.borrow().baseline.is_none()
    }

    pub(in crate::ui) fn service_restart(&self) -> (bool, Option<&'static str>) {
        let draft = self.draft.borrow();
        (draft.busy, draft.restart_status)
    }

    pub(in crate::ui) fn is_editing(&self) -> bool {
        self.draft.borrow().editing
    }

    fn unlock(&self) -> Option<PolicyJob> {
        let mut draft = self.draft.borrow_mut();
        if draft.busy || draft.baseline.is_some() {
            return None;
        }
        draft.busy = true;
        Some(PolicyJob::Read)
    }

    pub(in crate::ui) fn new(localizer: &Localizer) -> Result<Self, ScreenError> {
        let data = ScreenData::new();
        for field in [
            "save",
            "previous",
            "next",
            "add",
            "delete",
            "undo-delete",
            "uid",
            "subject",
            "lifetime",
            "actions",
            "purpose",
            "rules",
            "rule-summary",
            "caller-guide",
            "subject-guide",
            "purpose-guide",
            "advanced",
        ] {
            data.set(
                &format!("{}_label", field.replace('-', "_")),
                localizer.text(&format!("ui-policy-{field}")),
            );
        }
        for name in [
            "lifetime",
            "position",
            "result",
            "rules_status",
            "pagination_class",
            "editor_class",
            "list_class",
            "advanced_class",
            "purpose_class",
            "inventory_hint_class",
            "undo_class",
            "reload_class",
            "reload_label",
        ] {
            data.set(name, "");
        }
        for (_, name) in ACTIONS {
            data.set(name, localizer.text(&format!("ui-policy-{name}")));
            data.set(&format!("{name}_checked"), false);
        }
        for (index, (_, name)) in PURPOSES.iter().enumerate() {
            data.set(
                &format!("purpose_{index}"),
                localizer.text(&format!("ui-policy-purpose-{name}")),
            );
            data.set(&format!("purpose_{index}_checked"), false);
        }
        for name in [
            "save_disabled",
            "write_disabled",
            "rule_disabled",
            "previous_disabled",
            "next_disabled",
        ] {
            data.set(name, true);
        }
        data.set("result_class", "status");
        Ok(Self {
            screen: OkuiScreen::compile_with(include_str!("policies.okui"), data)?,
            access: PolicyAccess::new(localizer)?,
            draft: RefCell::default(),
            accounts: RefCell::new(AccountPicker::new()?),
        })
    }

    pub(in crate::ui) fn dismiss_account_menu(&self) -> bool {
        self.accounts.borrow_mut().field.take().is_some()
    }

    pub(in crate::ui) fn account_menu_trigger(&self) -> Option<&'static str> {
        match self.accounts.borrow().field {
            Some("uid") => Some("policy-caller-trigger"),
            Some("subject") => Some("policy-subject-trigger"),
            _ => None,
        }
    }

    pub(in crate::ui) fn move_account_cursor(&self, down: bool) -> usize {
        let mut accounts = self.accounts.borrow_mut();
        accounts.move_active(down);
        accounts.active
    }

    pub(in crate::ui) fn choose_active_account(&self) {
        let choice = self.accounts.borrow().active;
        self.choose_account(choice);
    }

    pub(in crate::ui) fn choose_account(&self, index: usize) {
        let mut draft = self.draft.borrow_mut();
        if draft.busy {
            return;
        }
        let selected = draft.selected;
        if let Some(rule) = draft.rules.get_mut(selected)
            && let Some((field, uid)) = self.accounts.borrow_mut().choose(index)
        {
            if field == "uid" {
                rule.uid = uid.to_string();
            } else {
                rule.subject = uid.to_string();
            }
            draft.status = None;
        }
    }

    pub(in crate::ui) fn edit(&self, field: &str, value: &str) {
        let mut draft = self.draft.borrow_mut();
        if draft.busy {
            return;
        }
        if field == "select_search" {
            let mut accounts = self.accounts.borrow_mut();
            if accounts.field.is_some() {
                accounts.query = value.to_owned();
                accounts.active = 0;
            }
            return;
        }
        let selected = draft.selected;
        let Some(rule) = draft.rules.get_mut(selected) else {
            return;
        };
        match field {
            "lifetime" => rule.lifetime = value.to_owned(),
            _ => return,
        }
        draft.status = None;
    }

    pub(in crate::ui) fn command(&self, command: &str) -> Option<PolicyJob> {
        if command == "policy_unlock" {
            return self.unlock();
        }
        let mut draft = self.draft.borrow_mut();
        if command == "policy_close_editor" {
            draft.editing = false;
            draft.advanced = false;
            self.dismiss_account_menu();
            return None;
        }
        if draft.busy {
            return None;
        }
        if command == "policy_reload" && (draft.baseline.is_some() && draft.stale) {
            draft.busy = true;
            self.dismiss_account_menu();
            return Some(PolicyJob::Read);
        }
        if command == "policy_restart_service" {
            draft.busy = true;
            draft.restarting = true;
            draft.restart_status = Some("ui-policy-service-restarting");
            return Some(PolicyJob::RestartService);
        }
        if draft.baseline.is_none() {
            return None;
        }
        let selected = draft.selected;
        let mut accounts = self.accounts.borrow_mut();
        if matches!(command, "policy_caller" | "policy_subject") {
            if draft.rules.get(selected).is_some() {
                accounts.toggle(if command == "policy_caller" {
                    "uid"
                } else {
                    "subject"
                });
            }
            return None;
        }
        if let Some(index) = command
            .strip_prefix("policy_account_")
            .and_then(|index| index.parse().ok())
        {
            drop(accounts);
            drop(draft);
            self.choose_account(index);
            return None;
        }
        accounts.field = None;
        if let Some(offset) = RuleList::ROWS
            .iter()
            .position(|candidate| *candidate == command)
        {
            let index = RuleList::page_start(selected) + offset;
            if index < draft.rules.len() {
                draft.selected = index;
                draft.editing = true;
                draft.advanced = false;
            }
            return None;
        }
        draft.status = None;
        match command {
            "policy_write" if draft.stale => {
                draft.status = Some(("ui-policy-conflict", true));
            }
            "policy_write" => {
                if !draft.has_changes() {
                    return None;
                }
                let Some(proposed) = draft.document() else {
                    draft.status = Some(("ui-policy-invalid", true));
                    return None;
                };
                let expected = draft.baseline.clone()?;
                draft.busy = true;
                return Some(PolicyJob::Write { expected, proposed });
            }
            "policy_previous" => {
                draft.selected =
                    RuleList::page_start(selected).saturating_sub(RuleList::ROWS.len());
            }
            "policy_next" => {
                let next = RuleList::page_start(selected) + RuleList::ROWS.len();
                if next < draft.rules.len() {
                    draft.selected = next;
                }
            }
            "policy_advanced" => draft.advanced = !draft.advanced,
            "policy_add" => {
                draft.selected = draft.rules.len();
                draft.rules.push(RuleForm::default());
                draft.editing = true;
                draft.advanced = false;
            }
            "policy_delete" if selected < draft.rules.len() => {
                draft.removed = Some((selected, draft.rules.remove(selected)));
                draft.editing = false;
                draft.advanced = false;
                draft.selected = selected.min(draft.rules.len().saturating_sub(1));
            }
            "policy_undo_delete" => {
                if let Some((index, rule)) = draft.removed.take() {
                    let index = index.min(draft.rules.len());
                    draft.rules.insert(index, rule);
                    draft.selected = index;
                    draft.editing = false;
                }
            }
            _ => {
                let rule = draft.rules.get_mut(selected)?;
                if let Some((purpose, _)) = PURPOSES.iter().find(|(_, name)| *name == command) {
                    rule.purpose = *purpose;
                } else if let Some((action, _)) = ACTIONS.iter().find(|(_, name)| *name == command)
                {
                    rule.toggle(*action);
                }
            }
        }
        None
    }

    pub(in crate::ui) fn finish(&self, result: Result<PolicyResult, &'static str>) {
        let mut draft = self.draft.borrow_mut();
        draft.busy = false;
        if std::mem::take(&mut draft.restarting) {
            draft.restart_status = Some(if matches!(result, Ok(PolicyResult::ServiceRestarted)) {
                draft.restart_required = false;
                if draft.status == Some(("ui-policy-restart-required", false)) {
                    draft.status = None;
                }
                "ui-policy-service-restarted"
            } else {
                "ui-policy-restart-failed"
            });
            return;
        }
        draft.status = Some(match result {
            Ok(PolicyResult::ServiceRestarted) => return,
            Ok(PolicyResult::Saved(document)) => {
                draft.baseline = Some(document);
                draft.restart_required = true;
                draft.removed = None;
                ("ui-policy-restart-required", false)
            }
            Ok(PolicyResult::Loaded { document, accounts }) => match draft.load(document) {
                Ok(()) => {
                    self.accounts.borrow_mut().load(accounts);
                    ("ui-policy-loaded", false)
                }
                Err(_) => ("ui-policy-invalid", true),
            },
            Err(key) => {
                draft.stale |= key == "ui-policy-conflict";
                (key, true)
            }
        });
    }

    pub(in crate::ui) fn view_with_editor(&self, localizer: &Localizer, editing: bool) -> View {
        let data = self.screen.data();
        let draft = self.draft.borrow();
        if draft.baseline.is_none() {
            let error = draft.status.filter(|(_, error)| *error).map(|(key, _)| key);
            return self
                .access
                .view(localizer, draft.busy && !draft.restarting, error);
        }
        let rule = draft.rules.get(draft.selected);
        let dirty = draft.has_changes();
        data.set("write_disabled", draft.busy);
        data.set("save_disabled", draft.busy || !dirty || draft.stale);
        data.set(
            "undo_class",
            if !draft.busy && draft.removed.is_some() {
                "policy-undo"
            } else {
                "policy-hidden"
            },
        );
        let retry = !draft.busy && draft.stale;
        data.set(
            "reload_class",
            if retry {
                "policy-reload"
            } else {
                "policy-hidden"
            },
        );
        data.set(
            "reload_label",
            localizer.text(if dirty {
                "ui-policy-reload-discard"
            } else {
                "ui-policy-reload"
            }),
        );
        data.set("rule_disabled", draft.busy || rule.is_none());
        let page_size = RuleList::ROWS.len();
        let page_start = RuleList::page_start(draft.selected);
        data.set("previous_disabled", draft.busy || page_start == 0);
        data.set(
            "next_disabled",
            draft.busy || page_start + page_size >= draft.rules.len(),
        );
        data.set(
            "position",
            format!(
                "{} / {}",
                page_start / page_size + 1,
                draft.rules.len().div_ceil(page_size)
            ),
        );
        data.set(
            "pagination_class",
            if draft.rules.len() > page_size {
                "policy-controls"
            } else {
                "policy-hidden"
            },
        );
        data.set(
            "editor_class",
            if editing && rule.is_some() {
                "panel"
            } else {
                "policy-hidden"
            },
        );
        data.set(
            "list_class",
            if editing { "policy-hidden" } else { "panel" },
        );
        data.set(
            "advanced_class",
            if draft.advanced {
                "policy-option-group"
            } else {
                "policy-hidden"
            },
        );
        data.set(
            "purpose_class",
            if rule.is_some_and(|rule| {
                rule.actions
                    .contains(&carosign_adapters::policy::Action::Verify)
            }) {
                "policy-option-group"
            } else {
                "policy-hidden"
            },
        );
        data.set(
            "rules_status",
            if draft.rules.is_empty() {
                localizer.text("ui-policy-empty")
            } else {
                String::new()
            },
        );
        let accounts = self.accounts.borrow();
        let inventory_mismatch = rule.is_some_and(|rule| {
            rule.actions
                .contains(&carosign_adapters::policy::Action::Inventory)
                && !rule.uid.is_empty()
                && !rule.subject.is_empty()
                && rule.uid != rule.subject
        });
        data.set(
            "inventory_hint_class",
            if inventory_mismatch {
                "status warning"
            } else {
                "policy-hidden"
            },
        );
        data.set(
            "inventory_hint",
            localizer.text("ui-policy-inventory-same-account"),
        );
        data.set("lifetime", rule.map_or("", |rule| rule.lifetime.as_str()));

        for (action, name) in ACTIONS {
            data.set(
                &format!("{name}_checked"),
                rule.is_some_and(|rule| rule.actions.contains(&action)),
            );
        }
        for (index, (purpose, _)) in PURPOSES.iter().enumerate() {
            data.set(
                &format!("purpose_{index}_checked"),
                rule.is_some_and(|rule| rule.purpose == *purpose),
            );
        }
        let status = if draft.busy && !draft.restarting {
            Some(("ui-policy-authorizing", false))
        } else if draft.status.is_some_and(|(_, error)| error) {
            draft.status
        } else if draft.stale {
            Some(("ui-policy-conflict", true))
        } else if dirty {
            Some(("ui-policy-dirty", false))
        } else if draft.restart_required {
            Some(("ui-policy-restart-required", false))
        } else {
            draft.status
        };
        data.set(
            "result",
            status.map_or_else(String::new, |(key, _)| localizer.text(key)),
        );
        data.set(
            "result_class",
            if status.is_some_and(|(_, error)| error) {
                "status warning"
            } else {
                "status"
            },
        );
        crate::ui::shared::fill(
            self.screen.view(),
            [
                ("rules", RuleList::view(&draft, &accounts, localizer)),
                (
                    "uid_trigger",
                    accounts.trigger(
                        rule.map_or("", |rule| rule.uid.as_str()),
                        "policy-caller-trigger",
                        "policy_caller",
                        "uid",
                        draft.busy || rule.is_none(),
                        localizer,
                    ),
                ),
                (
                    "subject_trigger",
                    accounts.trigger(
                        rule.map_or("", |rule| rule.subject.as_str()),
                        "policy-subject-trigger",
                        "policy_subject",
                        "subject",
                        draft.busy || rule.is_none(),
                        localizer,
                    ),
                ),
            ],
        )
    }
}
