use crate::{ipc::SystemAccount, ui::shared::Select};
use carosign_l10n::Localizer;
use obscur_kit::view::view::{ID, ViewFragment, ViewProperty};
use obscur_kit::{OkuiScreen, ScreenData, ScreenError, identity::InternedSymbol};

pub(super) struct AccountPicker {
    select: Select,
    avatar: OkuiScreen,
    accounts: Option<Vec<SystemAccount>>,
    pub(super) field: Option<&'static str>,
    pub(super) query: String,
    pub(super) active: usize,
}

impl AccountPicker {
    pub(super) fn new() -> Result<Self, ScreenError> {
        let avatar = ScreenData::new();
        avatar.set("source", "");
        avatar.set("image_class", "policy-hidden");
        Ok(Self {
            avatar: OkuiScreen::compile_with(include_str!("account_avatar.okui"), avatar)?,
            select: Select::new()?,
            accounts: None,
            field: None,
            query: String::new(),
            active: 0,
        })
    }

    pub(super) fn load(&mut self, accounts: Option<Vec<SystemAccount>>) {
        self.accounts = accounts;
        self.field = None;
        self.query.clear();
        self.active = 0;
    }

    pub(super) fn button_label(&self, value: &str, localizer: &Localizer) -> String {
        let Ok(uid) = value.parse::<u32>() else {
            return if value.is_empty() {
                localizer.text("ui-policy-account-choose")
            } else {
                format!("UID {value}")
            };
        };
        self.account(uid).map_or_else(
            || format!("UID {uid}"),
            |account| format!("{} ({uid})", account.name),
        )
    }

    pub(super) fn trigger(
        &self,
        value: &str,
        id: &str,
        command: &str,
        field: &str,
        disabled: bool,
        localizer: &Localizer,
    ) -> ViewFragment {
        let account = value.parse().ok().and_then(|uid| self.account(uid));
        if self.field == Some(field) {
            let search = self.select.search(
                id,
                &self.query,
                localizer.text("ui-policy-account-search"),
                self.avatar_view(account),
            );
            return self.open(search, localizer);
        }
        self.select.trigger(
            id,
            self.button_label(value, localizer),
            self.avatar_view(account),
            disabled,
            InternedSymbol::named(command),
        )
    }

    fn account(&self, uid: u32) -> Option<&SystemAccount> {
        self.accounts
            .iter()
            .flatten()
            .find(|account| account.uid == uid)
    }

    fn avatar_view(&self, account: Option<&SystemAccount>) -> ViewFragment {
        let data = self.avatar.data();
        let initial = account
            .and_then(|account| account.name.chars().next())
            .map_or_else(
                || "?".to_owned(),
                |initial| initial.to_uppercase().collect(),
            );
        let source = account
            .and_then(|account| account.avatar.as_ref())
            .and_then(|path| path.to_str())
            .map(|path| format!("account-avatar:{path}"));
        data.set("initial", initial);
        data.set(
            "image_class",
            if source.is_some() {
                "policy-avatar-image"
            } else {
                "policy-hidden"
            },
        );
        data.set("source", source.unwrap_or_default());
        self.avatar.view().into_fragment()
    }

    pub(super) fn toggle(&mut self, field: &'static str) {
        self.field = (self.field != Some(field)).then_some(field);
        self.query.clear();
        self.active = 0;
    }

    pub(super) fn move_active(&mut self, down: bool) {
        let last = self.matches().count().saturating_sub(1);
        self.active = if down {
            (self.active + 1).min(last)
        } else {
            self.active.saturating_sub(1)
        };
    }

    pub(super) fn choose(&mut self, index: usize) -> Option<(&'static str, u32)> {
        let field = self.field?;
        let uid = self.matches().nth(index)?.uid;
        self.field = None;
        Some((field, uid))
    }

    fn open(&self, search: ViewFragment, localizer: &Localizer) -> ViewFragment {
        if self.field.is_none() {
            return ViewFragment::from_nodes(Vec::new());
        }
        let mut rows = Vec::new();
        for (index, account) in self.matches().enumerate() {
            let option = self.select.option(
                format!("{} ({})", account.name, account.uid),
                self.avatar_view(Some(account)),
                index == self.active,
                InternedSymbol::named(&format!("policy_account_{index}")),
            );
            rows.extend(option.into_iter().map(|node| {
                node.with_prop(ViewProperty::new(
                    ID,
                    format!("select-result-{index}").into(),
                ))
            }));
        }
        let hint = if self.accounts.is_none() {
            "ui-policy-accounts-unavailable"
        } else if rows.is_empty() {
            "ui-policy-accounts-empty"
        } else {
            ""
        };
        self.select.open(
            search,
            if hint.is_empty() {
                String::new()
            } else {
                localizer.text(hint)
            },
            rows,
        )
    }

    fn matches(&self) -> impl Iterator<Item = &SystemAccount> {
        let query = self.query.trim().to_lowercase();
        self.accounts.iter().flatten().filter(move |account| {
            account.name.to_lowercase().contains(&query) || account.uid.to_string() == query
        })
    }
}
