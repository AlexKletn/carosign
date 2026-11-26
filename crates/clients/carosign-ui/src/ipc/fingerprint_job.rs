use std::{cell::RefCell, fs::File, io::Read, sync::Arc};

use carosign_linux_ipc::client::{
    mutation_events::observe_mutation_with_events, transport::Transport,
};
use carosign_protocol::carosign::{
    client::v1::{BeginEnrollment, BeginRemoval, MutationResult, client_frame::Payload},
    common::v1::{EnrollmentRef, SubjectSelector, subject_selector::Selector},
};

use super::{fingerprint_update::FingerprintUpdate, read_snapshot::CONTROL_SOCKET};

pub(crate) enum FingerprintJob {
    Enroll {
        device: Vec<u8>,
    },
    Remove(EnrollmentRef),
    Rename {
        reference: EnrollmentRef,
        expected: String,
        label: String,
    },
}

impl FingerprintJob {
    pub(crate) fn run(self, mut send: impl FnMut(FingerprintUpdate)) {
        if let Self::Rename {
            reference,
            expected,
            label,
        } = self
        {
            let result = super::rename_fingerprint::rename(&reference, &expected, &label);
            send(FingerprintUpdate::Renamed {
                reference,
                expected,
                label,
                result,
            });
            return;
        }
        let enrolling = matches!(self, Self::Enroll { .. });
        let result = self.execute(&mut send);
        send(if enrolling {
            FingerprintUpdate::Captured(result)
        } else {
            FingerprintUpdate::Finished(result.map(|_| ()))
        });
    }

    fn execute(
        self,
        send: &mut impl FnMut(FingerprintUpdate),
    ) -> Result<EnrollmentRef, &'static str> {
        let failure = match &self {
            Self::Enroll { .. } => "ui-fingerprint-enroll-failed",
            Self::Remove(_) => "ui-fingerprint-delete-failed",
            Self::Rename { .. } => unreachable!(),
        };
        let mut key = [0; 16];
        File::open("/dev/urandom")
            .and_then(|mut source| source.read_exact(&mut key))
            .map_err(|_| failure)?;
        if key == [0; 16] {
            return Err(failure);
        }
        let transport = Arc::new(Transport::connect_root(CONTROL_SOCKET).map_err(|_| failure)?);
        let subject = Some(SubjectSelector {
            selector: Some(Selector::CurrentSubject(true)),
        });
        let request = match &self {
            Self::Enroll { device } => Payload::BeginEnrollment(BeginEnrollment {
                idempotency_key: key.to_vec(),
                subject,
                authenticator_id: device.clone(),
                label: String::new(),
                ..BeginEnrollment::default()
            }),
            Self::Remove(reference) => Payload::BeginRemoval(BeginRemoval {
                idempotency_key: key.to_vec(),
                subject,
                enrollment: Some(reference.clone()),
                ..BeginRemoval::default()
            }),
            Self::Rename { .. } => unreachable!(),
        };
        let send = RefCell::new(send);
        let outcome = observe_mutation_with_events(
            &transport,
            request,
            |id| {
                send.borrow_mut()(FingerprintUpdate::Accepted(
                    Arc::clone(&transport),
                    id.to_vec(),
                ))
            },
            |event| send.borrow_mut()(FingerprintUpdate::Progress(event.clone())),
        )
        .map_err(|_| failure)?;
        match outcome.result() {
            MutationResult::MutationCommitted
                if outcome
                    .enrollment
                    .as_ref()
                    .is_some_and(|entry| match &self {
                        Self::Enroll { device, .. } => entry.authenticator_id == *device,
                        Self::Remove(reference) => entry == reference,
                        Self::Rename { .. } => unreachable!(),
                    }) =>
            {
                outcome.enrollment.ok_or(failure)
            }
            MutationResult::MutationNeedsReconciliation => Err("ui-fingerprint-pending"),
            MutationResult::MutationNoChange => Err("ui-fingerprint-no-change"),
            _ => Err(failure),
        }
    }
}
