use carosign_fprintd_compat::dbus::{Backend, CaptureStatus, EnrollProgress, VerifyOutcome};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct BackendFixture(pub(super) Arc<AtomicBool>, pub(super) u8);
impl Backend for BackendFixture {
    fn available(&self) -> bool {
        true
    }
    fn claim(&mut self, _: &str, _: &str) -> bool {
        true
    }
    fn release(&mut self, _: &str) {}
    fn enrolled(&mut self, _: &str, _: &str) -> Option<Vec<String>> {
        Some(Vec::new())
    }
    fn verify(&mut self, _: &str, _: &str) -> Option<VerifyOutcome> {
        None
    }
    fn stop(&mut self, _: &str) {}
    fn begin_enroll(&mut self, _: &str, _: &str) -> Option<EnrollProgress> {
        Some(EnrollProgress::Pending)
    }
    fn enrollment_stages(&self) -> Option<u32> {
        Some(3)
    }
    fn poll_enroll_progress(&mut self, _: &str) -> Option<EnrollProgress> {
        let event = match self.1 {
            0 => EnrollProgress::Stages(4),
            1 => EnrollProgress::Retry(CaptureStatus::FingerNotCentered),
            _ => return None,
        };
        self.1 += 1;
        Some(event)
    }
    fn poll_enroll_ready(&mut self, _: &str) -> Option<bool> {
        self.0.load(Ordering::SeqCst).then_some(true)
    }
}
