use carosign_l10n::Localizer;
use obscur_kit::identity::InternedSymbol;
use obscur_kit::view::view::{CLASS, ViewFragment, ViewProperty};
use obscur_kit::widgets::button::Button;

use super::{
    account_picker::AccountPicker,
    policy_draft::PolicyDraft,
    rule_form::{ACTIONS, PURPOSES},
};

pub(super) struct RuleList;

impl RuleList {
    pub(super) const ROWS: [&'static str; 6] = [
        "policy_rule_0",
        "policy_rule_1",
        "policy_rule_2",
        "policy_rule_3",
        "policy_rule_4",
        "policy_rule_5",
    ];

    pub(super) fn page_start(selected: usize) -> usize {
        selected / Self::ROWS.len() * Self::ROWS.len()
    }

    pub(super) fn view(
        draft: &PolicyDraft,
        accounts: &AccountPicker,
        localizer: &Localizer,
    ) -> ViewFragment {
        let start = Self::page_start(draft.selected);
        let nodes = draft
            .rules
            .iter()
            .skip(start)
            .zip(Self::ROWS)
            .enumerate()
            .flat_map(|(offset, (rule, command))| {
                let caller = accounts.button_label(&rule.uid, localizer);
                let subject = accounts.button_label(&rule.subject, localizer);
                let label = if rule.uid.is_empty() && rule.subject.is_empty() {
                    localizer.text("ui-policy-new-rule")
                } else {
                    let mut permissions = ACTIONS
                        .iter()
                        .filter(|(action, _)| rule.actions.contains(action))
                        .map(|(_, name)| {
                            if *name == "verify"
                                && let Some((_, purpose)) = PURPOSES.iter().find(|(purpose, _)| {
                                    purpose.is_some() && *purpose == rule.purpose
                                })
                            {
                                localizer.text(&format!("ui-policy-purpose-{purpose}"))
                            } else {
                                localizer.text(&format!("ui-policy-{name}"))
                            }
                        })
                        .collect::<Vec<_>>();
                    if permissions.is_empty() {
                        permissions.push(localizer.text("ui-policy-no-actions"));
                    }
                    format!("{caller} → {subject} · {}", permissions.join(", "))
                };
                let class = if start + offset == draft.selected {
                    "policy-rule selected"
                } else {
                    "policy-rule"
                };
                Button::new(label, InternedSymbol::named(command))
                    .disabled(draft.busy)
                    .view()
                    .into_fragment()
                    .into_nodes()
                    .into_iter()
                    .map(move |node| node.with_prop(ViewProperty::new(CLASS, class.into())))
            });
        ViewFragment::from_nodes(nodes.collect())
    }
}
