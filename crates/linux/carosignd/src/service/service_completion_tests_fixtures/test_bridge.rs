use super::cancel_state::CancelState;
use carosign_fprintd_compat::dbus::{Backend, VerifyOutcome, VerifyProgress};
use pam_carosign::client::{Observation, observe_verification_with_acceptance};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};

pub(super) struct TestBridge {
    pub(super) transport: Arc<carosign_linux_ipc::client::transport::Transport>,
    pub(super) request: Option<carosign_protocol::carosign::client::v1::BeginVerification>,
    pub(super) saw_no_sample: Arc<AtomicBool>,
    pub(super) pending: Option<mpsc::Receiver<VerifyOutcome>>,
    pub(super) cancellation: Option<Arc<Mutex<CancelState>>>,
}

impl Backend for TestBridge {
    fn available(&self) -> bool {
        true
    }
    fn claim(&mut self, _: &str, _: &str) -> bool {
        true
    }
    fn release(&mut self, _: &str) {}
    fn enrolled(&mut self, _: &str, _: &str) -> Option<Vec<String>> {
        Some(vec!["right-index-finger".into()])
    }
    fn verify(&mut self, _: &str, _: &str) -> Option<VerifyOutcome> {
        None
    }
    fn begin_verify(&mut self, _: &str, _: &str) -> Option<VerifyProgress> {
        let request = self.request.take()?;
        let transport = Arc::clone(&self.transport);
        let observed = Arc::clone(&self.saw_no_sample);
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        let cancellation = Arc::new(Mutex::new(CancelState::default()));
        self.cancellation = Some(Arc::clone(&cancellation));
        std::thread::spawn(move || {
            let outcome = match observe_verification_with_acceptance(&transport, request, |id| {
                cancellation.lock().unwrap().operation = Some(id.to_vec());
                dispatch_cancel(&cancellation, &transport);
            }) {
                Ok(Observation::NoSample) => {
                    observed.store(true, Ordering::SeqCst);
                    VerifyOutcome::Unavailable
                }
                Ok(Observation::NotMatched) => VerifyOutcome::NoMatch,
                _ => VerifyOutcome::Unavailable,
            };
            cancellation.lock().unwrap().finished = true;
            let _ = tx.send(outcome);
        });
        Some(VerifyProgress::Pending)
    }
    fn poll_verify(&mut self, _: &str) -> Option<VerifyOutcome> {
        self.pending.as_ref()?.try_recv().ok()
    }
    fn stop(&mut self, _: &str) {
        if let Some(cancellation) = self.cancellation.take() {
            cancellation.lock().unwrap().requested = true;
            dispatch_cancel(&cancellation, &self.transport);
        }
        self.pending = None;
    }
}

fn dispatch_cancel(
    state: &Arc<Mutex<CancelState>>,
    transport: &Arc<carosign_linux_ipc::client::transport::Transport>,
) {
    let operation = {
        let mut state = state.lock().unwrap();
        if !state.requested || state.sent || state.finished {
            return;
        }
        let Some(operation) = state.operation.clone() else {
            return;
        };
        state.sent = true;
        operation
    };
    let transport = Arc::clone(transport);
    std::thread::spawn(move || {
        use carosign_protocol::carosign::client::v1::{CancelOperation, client_frame::Payload};
        let _ = transport.request(Payload::CancelOperation(CancelOperation {
            operation_id: operation,
        }));
    });
}
