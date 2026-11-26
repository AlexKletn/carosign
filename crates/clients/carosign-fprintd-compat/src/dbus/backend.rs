use super::EnrollProgress;
use super::VerifyOutcome;
use super::VerifyProgress;
use super::{DeleteError, EnrollOutcome, StartError};

/// Operation backend for the fprintd D-Bus interface.
///
/// The caller name identifies a D-Bus connection. Implementations must validate
/// its identity and authority independently.
pub trait Backend {
    fn authorize_call(
        &self,
        _caller: &str,
        _method: &str,
        _username: Option<&str>,
    ) -> Option<Result<std::sync::mpsc::Receiver<Option<u32>>, StartError>> {
        None
    }

    fn authorized_call(&mut self, _caller: Option<(&str, u32)>) {}

    fn available(&self) -> bool;
    fn name(&self) -> &str {
        "CaroSign"
    }
    fn scan_type(&self) -> &str {
        "press"
    }
    fn claim(&mut self, caller: &str, user: &str) -> bool;
    fn claim_checked(&mut self, caller: &str, user: &str) -> Result<(), StartError> {
        self.claim(caller, user)
            .then_some(())
            .ok_or(StartError::PermissionDenied)
    }
    fn claim_for_delete(&mut self, caller: &str, user: &str) -> Result<(), StartError> {
        self.claim_checked(caller, user)
    }
    fn release(&mut self, caller: &str);
    fn enrolled(&mut self, caller: &str, user: &str) -> Option<Vec<String>>;
    /// Successful verification requires an authorized caller and proof bound to this operation.
    fn verify(&mut self, caller: &str, finger: &str) -> Option<VerifyOutcome>;
    /// Asynchronous backends return `Pending` to keep the bus responsive.
    fn begin_verify(&mut self, caller: &str, finger: &str) -> Option<VerifyProgress> {
        self.verify(caller, finger).map(VerifyProgress::Completed)
    }
    fn start_verify(&mut self, caller: &str, finger: &str) -> Result<VerifyProgress, StartError> {
        self.begin_verify(caller, finger)
            .ok_or(StartError::PermissionDenied)
    }
    fn poll_verify_ready(&mut self, _caller: &str) -> Option<bool> {
        Some(true)
    }
    fn poll_verify(&mut self, _caller: &str) -> Option<VerifyOutcome> {
        None
    }
    fn poll_verify_progress(&mut self, caller: &str) -> Option<VerifyProgress> {
        self.poll_verify(caller).map(VerifyProgress::Completed)
    }
    fn stop(&mut self, caller: &str);
    fn idle_after_stop(&mut self) -> bool {
        true
    }
    /// Success requires a confirmed platform enrollment result.
    fn enroll(&mut self, _caller: &str, _finger: &str) -> Option<EnrollOutcome> {
        None
    }
    fn begin_enroll(&mut self, caller: &str, finger: &str) -> Option<EnrollProgress> {
        self.enroll(caller, finger).map(EnrollProgress::Completed)
    }
    fn start_enroll(&mut self, caller: &str, finger: &str) -> Result<EnrollProgress, StartError> {
        self.begin_enroll(caller, finger)
            .ok_or(StartError::PermissionDenied)
    }
    fn enrollment_stages(&self) -> Option<u32> {
        None
    }
    /// Reports whether authorization and preparation are complete.
    fn poll_enroll_ready(&mut self, _caller: &str) -> Option<bool> {
        Some(true)
    }
    fn poll_enroll(&mut self, _caller: &str) -> Option<EnrollOutcome> {
        None
    }
    fn poll_enroll_progress(&mut self, caller: &str) -> Option<EnrollProgress> {
        self.poll_enroll(caller).map(EnrollProgress::Completed)
    }
    fn stop_enroll(&mut self, _caller: &str) {}
    fn begin_delete(&mut self, _caller: &str, _finger: Option<&str>) -> bool {
        false
    }
    fn start_delete(&mut self, caller: &str, finger: Option<&str>) -> Result<(), StartError> {
        self.begin_delete(caller, finger)
            .then_some(())
            .ok_or(StartError::Internal)
    }
    fn start_delete_for(&mut self, caller: &str, _user: &str) -> Result<(), StartError> {
        self.start_delete(caller, None)
    }
    fn poll_delete(&mut self, _caller: &str) -> Option<bool> {
        None
    }
    fn poll_delete_result(&mut self, caller: &str) -> Option<Result<(), DeleteError>> {
        self.poll_delete(caller).map(|success| {
            if success {
                Ok(())
            } else {
                Err(DeleteError::PrintsNotDeleted)
            }
        })
    }
    fn stop_delete(&mut self, _caller: &str) {}
}
