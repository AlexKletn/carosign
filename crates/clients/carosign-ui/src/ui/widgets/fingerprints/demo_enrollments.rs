use std::collections::BTreeMap;

use carosign_l10n::Localizer;

pub(super) struct DemoEnrollments {
    entries: BTreeMap<u64, String>,
    next_id: u64,
    selection: Option<(u64, Option<String>)>,
    confirming_delete: bool,
}

impl DemoEnrollments {
    pub(super) fn new(localizer: &Localizer) -> Self {
        let entries = [
            "ui-demo-fingerprint-right-index",
            "ui-demo-fingerprint-left-thumb",
            "ui-demo-fingerprint-right-thumb",
        ]
        .into_iter()
        .zip(1..)
        .map(|(key, id)| (id, localizer.text(key)))
        .collect::<BTreeMap<_, _>>();
        Self {
            next_id: entries.len() as u64 + 1,
            entries,
            selection: None,
            confirming_delete: false,
        }
    }

    pub(super) fn rows(&self) -> impl Iterator<Item = (u64, &str, bool, Option<&str>, bool)> {
        self.entries.iter().map(|(&id, label)| {
            let selection = self
                .selection
                .as_ref()
                .filter(|(selected, _)| *selected == id);
            (
                id,
                label.as_str(),
                selection.is_some(),
                selection.and_then(|(_, name)| name.as_deref()),
                selection.is_some() && self.confirming_delete,
            )
        })
    }

    pub(super) fn next_id(&self) -> u64 {
        self.next_id
    }

    pub(super) fn selected_id(&self) -> Option<u64> {
        self.selection.as_ref().map(|(id, _)| *id)
    }

    pub(super) fn save(&mut self, label: String) {
        self.entries.insert(self.next_id, label);
        self.next_id += 1;
    }

    pub(super) fn select(&mut self, id: u64) -> bool {
        if !self.entries.contains_key(&id) {
            return false;
        }
        self.confirming_delete = false;
        self.selection = if self
            .selection
            .as_ref()
            .is_some_and(|(selected, _)| *selected == id)
        {
            None
        } else {
            Some((id, None))
        };
        true
    }

    pub(super) fn edit_name(&mut self, name: &str) -> bool {
        let Some((_, Some(draft))) = self.selection.as_mut() else {
            return false;
        };
        *draft = name.to_owned();
        true
    }

    pub(super) fn command(&mut self, command: &str) -> bool {
        let Some((id, draft)) = self.selection.as_mut() else {
            return false;
        };
        let Some(label) = self.entries.get_mut(id) else {
            return false;
        };
        match command {
            "fingerprint_rename" if draft.is_none() && !self.confirming_delete => {
                *draft = Some(label.clone())
            }
            "fingerprint_rename_cancel" if draft.is_some() => *draft = None,
            "fingerprint_rename_save"
                if draft.as_ref().is_some_and(|name| !name.trim().is_empty()) =>
            {
                *label = draft.take().unwrap_or_default().trim().to_owned();
            }
            "fingerprint_delete" if draft.is_none() && !self.confirming_delete => {
                self.confirming_delete = true;
            }
            "fingerprint_delete_cancel" if self.confirming_delete => {
                self.confirming_delete = false;
            }
            "fingerprint_delete_confirm" if self.confirming_delete => {
                self.confirming_delete = false;
                self.entries.remove(id);
                self.selection = None;
            }
            _ => return false,
        }
        true
    }
}
