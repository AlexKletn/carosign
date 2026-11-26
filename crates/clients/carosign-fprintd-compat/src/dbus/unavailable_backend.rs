use super::Backend;
use super::VerifyOutcome;

pub(super) struct UnavailableBackend;

impl Backend for UnavailableBackend {
    fn available(&self) -> bool {
        false
    }
    fn claim(&mut self, _: &str, _: &str) -> bool {
        false
    }
    fn release(&mut self, _: &str) {}
    fn enrolled(&mut self, _: &str, _: &str) -> Option<Vec<String>> {
        None
    }
    fn verify(&mut self, _: &str, _: &str) -> Option<VerifyOutcome> {
        None
    }
    fn stop(&mut self, _: &str) {}
}
