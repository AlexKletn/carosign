use std::cell::RefCell;

use carosign_l10n::Localizer;
use carosign_protocol::carosign::common::v1::{Modality, ProviderOperation};

use obscur_kit::view::view::{View, ViewFragment};
use obscur_kit::{OkuiScreen, ScreenData, ScreenError};

use super::{
    demo_enrollments::DemoEnrollments, live_fingerprints::LiveFingerprints,
    registered_row::RegisteredRow,
};

use crate::{
    domain::{QueryError, Snapshot},
    ipc::{fingerprint_job::FingerprintJob, fingerprint_update::FingerprintUpdate},
    ui::shared,
};

pub(in crate::ui) struct Fingerprints {
    layout: OkuiScreen,
    row: OkuiScreen,
    registered_row: RegisteredRow,
    message: OkuiScreen,
    demo: Option<RefCell<DemoEnrollments>>,
    live: RefCell<LiveFingerprints>,
    enrollment: super::enrollment_form::EnrollmentForm,
}

impl Fingerprints {
    pub(in crate::ui) fn new() -> Result<Self, ScreenError> {
        let layout = ScreenData::new();
        layout.set("add_disabled", true);
        layout.set("add_class", "fingerprint-action-hidden");
        layout.set("availability_class", "status fingerprint-list-note");
        layout.set("empty_class", "fingerprint-action-hidden");
        let message = ScreenData::new();
        message.set("detail_class", "value");
        Ok(Self {
            layout: OkuiScreen::compile_with(include_str!("fingerprints.okui"), layout)?,
            row: OkuiScreen::compile(include_str!("row.okui"))?,
            registered_row: RegisteredRow::new()?,
            message: OkuiScreen::compile_with(include_str!("message.okui"), message)?,
            demo: None,
            live: RefCell::default(),
            enrollment: super::enrollment_form::EnrollmentForm::new()?,
        })
    }

    pub(in crate::ui) fn enable_demo(&mut self, localizer: &Localizer) {
        self.demo = Some(RefCell::new(DemoEnrollments::new(localizer)));
    }

    pub(in crate::ui) fn is_enrolling(&self) -> bool {
        self.enrollment.is_open()
    }

    pub(in crate::ui) fn cancel_enrollment(&self) -> bool {
        if self.demo.is_none() && self.enrollment.is_open() {
            let mut live = self.live.borrow_mut();
            if live.busy {
                if live.captured.is_none() {
                    live.cancel();
                    self.enrollment.cancelling();
                }
                return true;
            }
            if live.captured.take().is_some() {
                live.status = Some("ui-enroll-unnamed-saved");
            }
        }
        self.enrollment.close()
    }

    pub(in crate::ui) fn live_command(&self, command: &str) -> (bool, Option<FingerprintJob>) {
        if self.demo.is_some() {
            return (false, None);
        }
        if command == "fingerprint_discard" && self.live.borrow().captured.is_none() {
            return (self.cancel_enrollment(), None);
        }
        let mut live = self.live.borrow_mut();
        if live.busy {
            return (false, None);
        }
        let job = match command {
            "fingerprint_rename" if live.rename.is_none() && !live.confirming_delete => {
                let Some(entry) = live
                    .rows
                    .iter()
                    .find(|entry| live.selected.is_some() && entry.enrollment == live.selected)
                else {
                    return (false, None);
                };
                let expected = entry.label.clone();
                live.rename = Some(super::rename_draft::RenameDraft {
                    value: expected.clone(),
                    expected,
                });
                live.status = None;
                return (true, None);
            }
            "fingerprint_rename_cancel" if live.rename.is_some() => {
                live.rename = None;
                live.status = None;
                return (true, None);
            }
            "fingerprint_rename_save" => {
                let Some(reference) = live.selected.clone() else {
                    return (false, None);
                };
                let Some(draft) = &live.rename else {
                    return (false, None);
                };
                let label = draft.value.trim();
                if label.is_empty() || label.len() > carosign_protocol::limits::LABEL_MAX {
                    return (false, None);
                }
                let expected = if live.status == Some("ui-fingerprint-rename-conflict") {
                    live.rows
                        .iter()
                        .find(|entry| entry.enrollment.as_ref() == Some(&reference))
                        .map_or_else(|| draft.expected.clone(), |entry| entry.label.clone())
                } else {
                    draft.expected.clone()
                };
                FingerprintJob::Rename {
                    reference,
                    expected,
                    label: label.to_owned(),
                }
            }
            "fingerprint_add" if !self.enrollment.is_open() => {
                let Some(device) = live.device.clone() else {
                    return (false, None);
                };
                self.enrollment.open_live();
                live.status = None;
                FingerprintJob::Enroll { device }
            }
            "fingerprint_next" if live.captured.is_some() => {
                let Some(label) = self.enrollment.captured_name() else {
                    return (false, None);
                };
                self.enrollment.finish_live("ui-enroll-saving-name", true);
                FingerprintJob::Rename {
                    reference: live.captured.clone().unwrap(),
                    expected: String::new(),
                    label,
                }
            }
            "fingerprint_discard" if live.captured.is_some() => {
                let reference = live.captured.clone().unwrap();
                self.enrollment.finish_live("ui-fingerprint-deleting", true);
                live.removing = Some(reference.clone());
                FingerprintJob::Remove(reference)
            }
            "fingerprint_delete" if live.selected.is_some() && !live.confirming_delete => {
                live.confirming_delete = true;
                live.status = None;
                return (true, None);
            }
            "fingerprint_delete_cancel" if live.confirming_delete => {
                live.confirming_delete = false;
                return (true, None);
            }
            "fingerprint_delete_confirm" if live.confirming_delete => {
                let Some(reference) = live.selected.take().filter(|reference| {
                    live.rows.iter().any(|entry| {
                        !entry.pending_reconciliation
                            && entry.enrollment.as_ref() == Some(reference)
                    })
                }) else {
                    return (false, None);
                };
                live.confirming_delete = false;
                live.status = Some("ui-fingerprint-deleting");
                live.removing = Some(reference.clone());
                FingerprintJob::Remove(reference)
            }
            _ => return (false, None),
        };
        live.busy = true;
        (true, Some(job))
    }

    pub(in crate::ui) fn apply_live_update(&self, update: FingerprintUpdate) {
        let mut live = self.live.borrow_mut();
        match update {
            FingerprintUpdate::Accepted(transport, operation_id) if self.enrollment.is_open() => {
                live.operation = Some((transport, operation_id));
                if live.cancelling {
                    live.send_cancel();
                }
            }
            FingerprintUpdate::Progress(event) => self.enrollment.apply(&event),
            FingerprintUpdate::Captured(result) => {
                live.busy = false;
                live.operation = None;
                match result {
                    Ok(reference) => {
                        live.captured = Some(reference);
                        self.enrollment.captured();
                    }
                    Err(error) => {
                        self.enrollment.close();
                        live.status =
                            Some(if live.cancelling && error == "ui-fingerprint-no-change" {
                                "ui-enroll-cancelled"
                            } else {
                                error
                            });
                    }
                }
                live.cancelling = false;
            }
            FingerprintUpdate::Finished(result) => {
                live.busy = false;
                if let Some(reference) = live.removing.take()
                    && result.is_ok()
                {
                    live.removed.push(reference);
                }
                if live.captured.is_some() {
                    if let Err(error) = result {
                        self.enrollment.finish_live(error, false);
                        return;
                    }
                    live.captured = None;
                    self.enrollment.close();
                }
                live.status = Some(result.err().unwrap_or("ui-fingerprint-deleted"));
            }
            FingerprintUpdate::Renamed {
                reference,
                expected,
                label,
                result,
            } => {
                live.busy = false;
                match result {
                    Ok(()) => {
                        live.rename = None;
                        let completing_enrollment = live.captured.as_ref() == Some(&reference);
                        if completing_enrollment {
                            live.captured = None;
                            self.enrollment.close();
                        }
                        if let Some(previous) = live
                            .renamed
                            .iter_mut()
                            .find(|(target, _, _)| *target == reference)
                        {
                            previous.2 = label;
                        } else {
                            live.renamed.push((reference, expected, label));
                        }
                        live.status = Some(if completing_enrollment {
                            "ui-fingerprint-enrolled"
                        } else {
                            "ui-fingerprint-renamed"
                        });
                    }
                    Err(error) if live.captured.as_ref() == Some(&reference) => {
                        self.enrollment.finish_live(error, false);
                    }
                    Err(error) => live.status = Some(error),
                }
            }
            FingerprintUpdate::Accepted(..) => {}
        }
    }

    pub(in crate::ui) fn edit_enrollment(&self, label: &str) -> bool {
        self.enrollment.edit(label)
    }

    pub(in crate::ui) fn set_enrollment_fit(&self, scale: f32, offset: f32) -> bool {
        self.enrollment.set_fit(scale, offset)
    }

    pub(in crate::ui) fn enrollment_view(&self, localizer: &Localizer, motion: bool) -> View {
        self.enrollment.view(localizer, motion)
    }

    pub(in crate::ui) fn enrollment_command(&self, command: &str, localizer: &Localizer) -> bool {
        let Some(demo) = &self.demo else { return false };
        match command {
            "fingerprint_add" if !self.enrollment.is_open() => {
                let number = demo.borrow().next_id();
                self.enrollment.open(format!(
                    "{} {number}",
                    localizer.text("ui-fingerprint-demo-entry")
                ));
                true
            }
            "fingerprint_next" if !self.enrollment.is_ready() => self.enrollment.scan(),
            "fingerprint_next" => {
                let Some(label) = self.enrollment.finish() else {
                    return false;
                };
                demo.borrow_mut().save(label);
                true
            }
            "fingerprint_discard" if self.enrollment.is_ready() => self.enrollment.close(),
            _ if !self.enrollment.is_open() => demo.borrow_mut().command(command),
            _ => false,
        }
    }

    pub(in crate::ui) fn select_registered(&self, id: &str) -> bool {
        !self.enrollment.is_open()
            && self.demo.as_ref().map_or_else(
                || self.live.borrow_mut().select(id),
                |demo| id.parse().is_ok_and(|id| demo.borrow_mut().select(id)),
            )
    }

    pub(in crate::ui) fn edit_registered_name(&self, name: &str) -> bool {
        if self.enrollment.is_open() {
            return false;
        }
        if let Some(demo) = &self.demo {
            return demo.borrow_mut().edit_name(name);
        }
        let mut live = self.live.borrow_mut();
        if live.busy {
            return false;
        }
        let Some(draft) = &mut live.rename else {
            return false;
        };
        draft.value = name.to_owned();
        true
    }

    pub(in crate::ui) fn selected_row_id(&self) -> Option<String> {
        if let Some(demo) = &self.demo {
            return demo.borrow().selected_id().map(|id| id.to_string());
        }
        self.live
            .borrow()
            .selected
            .as_ref()
            .map(LiveFingerprints::row_id)
    }

    pub(in crate::ui) fn has_entries(&self, snapshot: Option<&Snapshot>) -> bool {
        self.demo.is_some() || matches!(snapshot.map(|snapshot| &snapshot.enrollments), Some(Ok(_)))
    }

    pub(in crate::ui) fn render(&self, snapshot: Option<&Snapshot>, localizer: &Localizer) -> View {
        if let Some(demo) = &self.demo {
            let demo = demo.borrow();
            let rows = demo
                .rows()
                .filter_map(|(id, label, selected, draft, confirming_delete)| {
                    self.registered_row.view(
                        &id.to_string(),
                        label,
                        selected,
                        draft,
                        confirming_delete,
                        true,
                        localizer,
                    )
                })
                .collect();
            return self.render_list(rows, true, true, None, localizer);
        }
        let mut live = self.live.borrow_mut();
        let (title, detail) = match snapshot.map(|snapshot| &snapshot.enrollments) {
            Some(Ok(entries)) => {
                live.renamed.retain(|(reference, expected, _)| {
                    entries.iter().any(|entry| {
                        entry.enrollment.as_ref() == Some(reference) && entry.label == *expected
                    })
                });
                live.rows = entries
                    .iter()
                    .cloned()
                    .map(|mut entry| {
                        if let Some((_, _, label)) = live
                            .renamed
                            .iter()
                            .find(|(reference, _, _)| entry.enrollment.as_ref() == Some(reference))
                        {
                            entry.label.clone_from(label);
                        }
                        entry
                    })
                    .collect();
                live.removed.retain(|reference| {
                    entries
                        .iter()
                        .any(|entry| entry.enrollment.as_ref() == Some(reference))
                });
                live.device = snapshot
                    .and_then(|snapshot| snapshot.inventory.as_ref().ok())
                    .and_then(|inventory| {
                        let mut devices = inventory.entries.iter().filter(|device| {
                            device.available
                                && device.capabilities.as_ref().is_some_and(|caps| {
                                    caps.modality() == Modality::Fingerprint
                                        && caps
                                            .operations
                                            .contains(&(ProviderOperation::ProviderEnroll as i32))
                                })
                        });
                        let device = devices.next()?;
                        devices
                            .next()
                            .is_none()
                            .then(|| device.authenticator_id.clone())
                    });
                if !entries.iter().any(|entry| {
                    !entry.pending_reconciliation
                        && entry.enrollment.as_ref() == live.selected.as_ref()
                }) {
                    live.selected = None;
                    live.confirming_delete = false;
                    live.rename = None;
                }
                let rows = live
                    .rows
                    .iter()
                    .filter(|entry| {
                        entry
                            .enrollment
                            .as_ref()
                            .is_none_or(|reference| !live.removed.contains(reference))
                    })
                    .flat_map(|entry| {
                        let label = if entry.label.trim().is_empty() {
                            localizer.text("ui-unnamed-fingerprint")
                        } else {
                            entry.label.clone()
                        };
                        let Some(reference) = entry
                            .enrollment
                            .as_ref()
                            .filter(|_| !entry.pending_reconciliation)
                        else {
                            self.row.data().set("label", label);
                            return self.row.view().into_fragment().into_nodes();
                        };
                        let selected = !live.busy && live.selected == entry.enrollment;
                        let draft = if selected {
                            live.rename.as_ref().map(|draft| draft.value.as_str())
                        } else {
                            None
                        };
                        self.registered_row
                            .view(
                                &LiveFingerprints::row_id(reference),
                                &label,
                                selected,
                                draft,
                                selected && live.confirming_delete,
                                true,
                                localizer,
                            )
                            .into_iter()
                            .collect()
                    });
                let can_add = live.device.is_some() && !live.busy;
                let status = live.status.or_else(|| {
                    live.device
                        .is_none()
                        .then_some("ui-fingerprint-scanner-required")
                });
                return self.render_list(rows.collect(), false, can_add, status, localizer);
            }
            Some(Err(QueryError::Unauthorized)) => (
                "ui-fingerprint-access-title",
                Some("ui-fingerprint-access-detail"),
            ),
            Some(Err(QueryError::Unavailable))
                if snapshot.is_some_and(|snapshot| snapshot.health.is_ok()) =>
            {
                (
                    "ui-fingerprint-error-title",
                    Some("ui-fingerprint-query-detail"),
                )
            }
            Some(Err(QueryError::Unavailable)) => (
                "ui-fingerprint-service-title",
                Some("ui-fingerprint-service-detail"),
            ),
            Some(Err(QueryError::Unexpected)) => {
                ("ui-fingerprint-error-title", Some("monitor-unexpected"))
            }
            None => ("ui-fingerprint-loading", None),
        };
        live.rows.clear();
        live.selected = None;
        live.confirming_delete = false;
        live.rename = None;
        live.device = None;
        let data = self.message.data();
        data.set("title", localizer.text(title));
        data.set(
            "detail_class",
            if detail.is_some() {
                "value"
            } else {
                "fingerprint-message-detail-hidden"
            },
        );
        data.set(
            "detail",
            detail.map_or_else(String::new, |key| localizer.text(key)),
        );
        self.message.view()
    }

    fn render_list(
        &self,
        rows: Vec<obscur_kit::view::view::ViewNode>,
        demo: bool,
        can_add: bool,
        status: Option<&str>,
        localizer: &Localizer,
    ) -> View {
        let empty = rows.is_empty();
        let data = self.layout.data();
        data.set(
            "scope",
            localizer.text(if demo {
                "ui-fingerprint-demo-scope"
            } else {
                "ui-fingerprint-current-account"
            }),
        );
        data.set("add_class", "");
        data.set(
            "availability",
            status.map(|key| localizer.text(key)).unwrap_or_default(),
        );
        data.set(
            "availability_class",
            if status.is_some() {
                "status fingerprint-list-note"
            } else {
                "fingerprint-action-hidden"
            },
        );
        data.set("add_label", localizer.text("ui-fingerprint-add"));
        data.set("add_disabled", !can_add);
        data.set(
            "empty_label",
            if empty {
                localizer.text("ui-no-fingerprints")
            } else {
                String::new()
            },
        );
        data.set(
            "empty_class",
            if empty {
                "status"
            } else {
                "fingerprint-action-hidden"
            },
        );
        shared::fill(
            self.layout.view(),
            [("rows", ViewFragment::from_nodes(rows))],
        )
    }
}

#[cfg(test)]
#[path = "fingerprints.tests.rs"]
mod tests;
