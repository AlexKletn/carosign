use std::sync::Arc;

use carosign_linux_ipc::client::transport::Transport;
use carosign_protocol::carosign::{
    client::v1::{CancelOperation, client_frame::Payload},
    common::v1::{EnrollmentRef, EnrollmentSummary},
};

use super::rename_draft::RenameDraft;

#[derive(Default)]
pub(super) struct LiveFingerprints {
    pub(super) device: Option<Vec<u8>>,
    pub(super) rows: Vec<EnrollmentSummary>,
    pub(super) selected: Option<EnrollmentRef>,
    pub(super) confirming_delete: bool,
    pub(super) rename: Option<RenameDraft>,
    pub(super) renamed: Vec<(EnrollmentRef, String, String)>,
    pub(super) captured: Option<EnrollmentRef>,
    pub(super) removing: Option<EnrollmentRef>,
    pub(super) removed: Vec<EnrollmentRef>,
    pub(super) busy: bool,
    pub(super) cancelling: bool,
    pub(super) operation: Option<(Arc<Transport>, Vec<u8>)>,
    pub(super) status: Option<&'static str>,
}

impl LiveFingerprints {
    pub(super) fn cancel(&mut self) {
        if !self.cancelling {
            self.cancelling = true;
            self.send_cancel();
        }
    }

    pub(super) fn send_cancel(&mut self) {
        if let Some((transport, operation_id)) = self.operation.take() {
            std::thread::spawn(move || {
                let _ =
                    transport.request(Payload::CancelOperation(CancelOperation { operation_id }));
            });
        }
    }

    pub(super) fn row_id(reference: &EnrollmentRef) -> String {
        use std::fmt::Write;
        let mut id = String::with_capacity(84);
        for byte in reference
            .authenticator_id
            .iter()
            .chain(&reference.enrollment_id)
        {
            write!(id, "{byte:02x}").expect("writing to a String cannot fail");
        }
        write!(id, "-{}", reference.generation).expect("writing to a String cannot fail");
        id
    }

    pub(super) fn select(&mut self, id: &str) -> bool {
        if self.busy {
            return false;
        }
        let Some(reference) = self
            .rows
            .iter()
            .filter(|entry| !entry.pending_reconciliation)
            .filter_map(|entry| entry.enrollment.as_ref())
            .find(|reference| !self.removed.contains(reference) && Self::row_id(reference) == id)
        else {
            return false;
        };
        self.selected = (self.selected.as_ref() != Some(reference)).then(|| reference.clone());
        self.confirming_delete = false;
        self.rename = None;
        true
    }
}
