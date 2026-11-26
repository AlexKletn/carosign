use carosign_fprintd_compat::dbus::{
    Backend, EnrollOutcome, EnrollProgress, VerifyOutcome, VerifyProgress,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct FixtureBackend {
    pub(super) enroll_stopped: Arc<AtomicBool>,
    pub(super) capture_pending: bool,
    pub(super) stop_ready: Arc<AtomicBool>,
}
impl Backend for FixtureBackend {
    fn available(&self) -> bool {
        true
    }
    fn claim(&mut self, _: &str, _: &str) -> bool {
        true
    }
    fn release(&mut self, _: &str) {}
    fn enrolled(&mut self, _: &str, _: &str) -> Option<Vec<String>> {
        Some(vec!["right-index-finger".to_owned()])
    }
    fn verify(&mut self, _: &str, _: &str) -> Option<VerifyOutcome> {
        Some(VerifyOutcome::NoMatch)
    }
    fn begin_verify(&mut self, caller: &str, finger: &str) -> Option<VerifyProgress> {
        if finger == "right-thumb" {
            self.capture_pending = true;
            Some(VerifyProgress::Pending)
        } else {
            self.verify(caller, finger).map(VerifyProgress::Completed)
        }
    }
    fn poll_verify_progress(&mut self, _: &str) -> Option<VerifyProgress> {
        std::mem::take(&mut self.capture_pending).then_some(VerifyProgress::Capture {
            present: Some(true),
            needed: Some(true),
        })
    }
    fn stop(&mut self, _: &str) {}
    fn idle_after_stop(&mut self) -> bool {
        self.stop_ready.load(Ordering::SeqCst)
    }
    fn enroll(&mut self, _: &str, _: &str) -> Option<EnrollOutcome> {
        Some(EnrollOutcome::Failed)
    }
    fn begin_enroll(&mut self, caller: &str, finger: &str) -> Option<EnrollProgress> {
        if finger == "left-thumb" {
            Some(EnrollProgress::Pending)
        } else {
            self.enroll(caller, finger).map(EnrollProgress::Completed)
        }
    }
    fn stop_enroll(&mut self, _: &str) {
        self.enroll_stopped.store(true, Ordering::SeqCst);
    }
}
