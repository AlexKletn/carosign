//! Bridges the system D-Bus interface to the platform client protocol.

mod availability;
mod cancellation;
mod capture_feedback;
mod delete_records;
mod enrollment_progress;
mod fingerprint_inventory;
mod policy_kit;
pub(crate) use fingerprint_inventory::fingerprints;
mod system_account;

#[cfg(test)]
#[path = "system_backend.tests.rs"]
mod tests;

use crate::fingerprint_label::valid_label;
use availability::Availability;
use cancellation::{Cancellation, request_cancel};
use delete_records::delete_records;
use fingerprint_inventory::{authenticator, authenticator_for, listed_fingerprint};
use system_account::lookup_user;

use crate::dbus::{
    Backend, DeleteError, EnrollOutcome, EnrollProgress, StartError, VerifyOutcome, VerifyProgress,
};
use carosign_linux_ipc::client::transport::Transport;
use carosign_linux_ipc::client::verification_events::{
    Observation, observe_verification_with_result,
};
use carosign_protocol::carosign::{client::v1 as wire, common::v1 as common};
use dbus::blocking::Connection;
use std::cell::RefCell;
use std::sync::{
    Arc, Mutex,
    mpsc::{self, Receiver, TryRecvError},
};
use std::time::{Duration, Instant};

pub(crate) const SOCKET: &str = "/run/carosignd/control.sock";

fn enrollment_inventory(
    transport: &Transport,
    user: &str,
    authenticator_id: Vec<u8>,
) -> Option<Vec<common::EnrollmentSummary>> {
    let response = transport
        .request(wire::client_frame::Payload::ListEnrollments(
            wire::ListEnrollments {
                subject: Some(common::SubjectSelector {
                    selector: Some(common::subject_selector::Selector::AccountHint(
                        user.to_owned(),
                    )),
                }),
                authenticator_id,
                authority_request_ref: Vec::new(),
            },
        ))
        .ok()?;
    match response.payload {
        Some(wire::client_frame::Payload::Enrollments(list)) => Some(
            list.entries.into_iter().map(|mut entry| {
                if let Some(label) = entry.original_label.take() {
                    entry.label = label;
                }
                entry
            }).collect(),
        ),
        _ => None,
    }
}

fn named_enrollment(
    entries: &[common::EnrollmentSummary],
    label: &str,
) -> Option<common::EnrollmentRef> {
    let mut matching = entries
        .iter()
        .filter(|entry| entry.label == label && !entry.pending_reconciliation);
    let reference = matching.next()?.enrollment.clone()?;
    matching.next().is_none().then_some(reference)
}

pub struct SystemBackend<'a> {
    bus: &'a Connection,
    availability: RefCell<Availability>,
    selected: Option<Vec<u8>>,
    swipe: bool,
    device_name: Option<String>,
    authorized_caller: Option<(String, u32)>,
    owner: Option<(String, String)>,
    pending: Option<Receiver<VerifyProgress>>,
    verification_ready: Option<Receiver<()>>,
    pending_enrollment: Option<Receiver<EnrollProgress>>,
    enrollment_ready: Option<Receiver<()>>,
    pending_deletion: Option<Receiver<Result<(), DeleteError>>>,
    cancellation: Option<Arc<Mutex<Cancellation>>>,
    cancelled: bool,
}

impl<'a> SystemBackend<'a> {
    pub fn new(bus: &'a Connection) -> Self {
        Self {
            bus,
            availability: RefCell::new(Availability::default()),
            selected: None,
            swipe: false,
            device_name: None,
            authorized_caller: None,
            owner: None,
            pending: None,
            verification_ready: None,
            pending_enrollment: None,
            enrollment_ready: None,
            pending_deletion: None,
            cancellation: None,
            cancelled: false,
        }
    }

    pub(crate) fn select_device(&mut self, id: Vec<u8>, swipe: bool, name: Option<String>) {
        self.selected = Some(id);
        self.swipe = swipe;
        self.device_name = name;
    }

    fn caller_uid(&self, caller: &str) -> Option<u32> {
        if !caller.starts_with(':') {
            return None;
        }
        let proxy = self.bus.with_proxy(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            Duration::from_secs(2),
        );
        let (uid,): (u32,) = proxy
            .method_call("org.freedesktop.DBus", "GetConnectionUnixUser", (caller,))
            .ok()?;
        Some(uid)
    }

    fn username_for(&self, caller: &str, requested: &str) -> Option<String> {
        let uid = self.caller_uid(caller)?;
        let name = if requested.is_empty() {
            lookup_user(None, uid)?.0
        } else {
            requested.to_owned()
        };
        let (_, target_uid) = lookup_user(Some(&name), 0)?;
        if let Some((authorized, approved_uid)) = &self.authorized_caller {
            return (authorized == caller && *approved_uid == target_uid).then_some(name);
        }
        (uid == target_uid || policy_kit::authorize(self.bus, caller, uid, "setusername"))
            .then_some(name)
    }

    fn authorized(&self, caller: &str, action: &str) -> bool {
        self.authorized_caller
            .as_ref()
            .is_some_and(|(name, _)| name == caller)
            || self
                .caller_uid(caller)
                .is_some_and(|uid| policy_kit::authorize(self.bus, caller, uid, action))
    }

    fn set_claim(&mut self, caller: &str, user: String) -> Result<(), StartError> {
        self.reap_cancelled();
        if self.owner.is_some() || !self.idle_after_stop() {
            return Err(StartError::AlreadyInUse);
        }
        self.owner = Some((caller.to_owned(), user));
        Ok(())
    }

    fn delete_for(
        &mut self,
        caller: &str,
        finger: Option<&str>,
        requested: Option<String>,
    ) -> Result<(), StartError> {
        self.reap_cancelled();
        if (self.pending.is_some() && !self.cancelled)
            || self.pending_enrollment.is_some()
            || self.pending_deletion.is_some()
            || finger.is_some_and(|name| !valid_label(name))
        {
            return Err(StartError::AlreadyInUse);
        }
        let Some((owner, user)) = self.owner.as_ref() else {
            return Err(StartError::PermissionDenied);
        };
        if owner != caller || !self.authorized(caller, "enroll") {
            return Err(StartError::PermissionDenied);
        }
        let user = requested.unwrap_or_else(|| user.clone());
        let finger = finger.map(str::to_owned);
        let selected_device = self.selected.clone();
        let stopping_verification = self.pending.take();
        let cancellation = Arc::new(Mutex::new(Cancellation::default()));
        let worker_cancel = cancellation.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let can_delete = stopping_verification.is_none_or(|receiver| {
                let deadline = Instant::now() + std::time::Duration::from_secs(6);
                loop {
                    match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    {
                        Ok(VerifyProgress::Completed(_)) => break true,
                        Ok(_) => {}
                        Err(_) => break false,
                    }
                }
            });
            let result = if can_delete {
                delete_records(
                    &user,
                    selected_device.as_deref(),
                    finger.as_deref(),
                    &worker_cancel,
                )
            } else {
                Err(DeleteError::PrintsNotDeleted)
            };
            let _ = tx.send(result);
        });
        self.pending_deletion = Some(rx);
        self.cancellation = Some(cancellation);
        self.cancelled = false;
        Ok(())
    }

    fn reap_cancelled(&mut self) {
        if self.cancelled
            && self.pending.as_ref().is_some_and(|rx| {
                Self::worker_finished(rx, |event| matches!(event, VerifyProgress::Completed(_)))
            })
        {
            self.pending = None;
            self.cancellation = None;
        }
        if self.cancelled
            && self.pending_enrollment.as_ref().is_some_and(|rx| {
                Self::worker_finished(rx, |event| matches!(event, EnrollProgress::Completed(_)))
            })
        {
            self.pending_enrollment = None;
            self.enrollment_ready = None;
            self.cancellation = None;
        }
        if self.cancelled
            && self
                .pending_deletion
                .as_ref()
                .is_some_and(|rx| !matches!(rx.try_recv(), Err(TryRecvError::Empty)))
        {
            self.pending_deletion = None;
            self.cancellation = None;
        }
    }

    fn worker_finished<T>(receiver: &Receiver<T>, completed: impl Fn(T) -> bool) -> bool {
        loop {
            match receiver.try_recv() {
                Ok(event) => {
                    if completed(event) {
                        return true;
                    }
                }
                Err(TryRecvError::Empty) => return false,
                Err(TryRecvError::Disconnected) => return true,
            }
        }
    }

    fn cancel(&mut self) {
        self.cancelled = true;
        let operation = self.cancellation.as_ref().and_then(|state| {
            let mut state = state.lock().ok()?;
            state.cancelled = true;
            state.operation.take()
        });
        if let Some((transport, operation_id)) = operation {
            request_cancel(transport, operation_id);
        }
    }
}

impl Backend for SystemBackend<'_> {
    fn authorize_call(
        &self,
        caller: &str,
        method: &str,
        username: Option<&str>,
    ) -> Option<Result<Receiver<Option<u32>>, StartError>> {
        let actions: &[_] = match method {
            "Claim" => &["verify", "enroll"],
            "VerifyStart" | "ListEnrolledFingers" => &["verify"],
            "EnrollStart"
            | "DeleteEnrolledFingers"
            | "DeleteEnrolledFingers2"
            | "DeleteEnrolledFinger" => &["enroll"],
            _ => return None,
        };
        Some((|| {
            let uid = self
                .caller_uid(caller)
                .ok_or(StartError::PermissionDenied)?;
            let target_uid = if let Some(name) = username.filter(|name| !name.is_empty()) {
                lookup_user(Some(name), 0)
                    .ok_or(StartError::PermissionDenied)?
                    .1
            } else {
                uid
            };
            policy_kit::authorize_async(caller, uid, target_uid, actions)
                .ok_or(StartError::AlreadyInUse)
        })())
    }

    fn authorized_call(&mut self, caller: Option<(&str, u32)>) {
        self.authorized_caller = caller.map(|(name, uid)| (name.to_owned(), uid));
    }

    fn name(&self) -> &str {
        self.device_name.as_deref().unwrap_or("CaroSign")
    }

    fn scan_type(&self) -> &str {
        if self.swipe { "swipe" } else { "press" }
    }

    fn available(&self) -> bool {
        if self.selected.is_some() {
            return true;
        }
        let now = Instant::now();
        let mut cached = self.availability.borrow_mut();
        if cached.recently_checked(now) {
            crate::diagnostics::event(if cached.visible {
                "availability_cached_present"
            } else {
                "availability_cached_absent"
            });
            return cached.visible;
        }
        let current = match Transport::connect_root(SOCKET) {
            Ok(transport) => listed_fingerprint(&transport, self.selected.as_deref()).is_some(),
            Err(_) => {
                crate::diagnostics::event("ipc_connect_error");
                false
            }
        };
        crate::diagnostics::event(if current {
            "availability_observed_present"
        } else {
            "availability_observed_absent"
        });
        let previous = cached.visible;
        let visible = cached.record(Instant::now(), current);
        if previous != visible {
            crate::diagnostics::event(if visible {
                "availability_became_present"
            } else {
                "availability_became_absent"
            });
        } else if visible && !current {
            crate::diagnostics::event("availability_grace_retained");
        }
        visible
    }

    fn claim(&mut self, caller: &str, user: &str) -> bool {
        self.claim_checked(caller, user).is_ok()
    }

    fn claim_checked(&mut self, caller: &str, user: &str) -> Result<(), StartError> {
        let user = self
            .username_for(caller, user)
            .ok_or(StartError::PermissionDenied)?;
        if !self.authorized(caller, "verify") && !self.authorized(caller, "enroll") {
            return Err(StartError::PermissionDenied);
        }
        self.set_claim(caller, user)
    }

    fn claim_for_delete(&mut self, caller: &str, user: &str) -> Result<(), StartError> {
        let user = self
            .username_for(caller, user)
            .ok_or(StartError::PermissionDenied)?;
        self.set_claim(caller, user)
    }

    fn release(&mut self, caller: &str) {
        if self
            .owner
            .as_ref()
            .is_some_and(|(owner, _)| owner == caller)
        {
            self.cancel();
            self.owner = None;
        }
    }

    fn idle_after_stop(&mut self) -> bool {
        self.reap_cancelled();
        self.pending.is_none()
            && self.pending_enrollment.is_none()
            && self.pending_deletion.is_none()
    }

    fn enrolled(&mut self, caller: &str, user: &str) -> Option<Vec<String>> {
        let claimed = self.username_for(caller, user)?;
        if !self.authorized(caller, "verify") {
            return None;
        }
        let transport = Transport::connect_root(SOCKET).ok()?;
        let authenticator_id = listed_fingerprint(&transport, self.selected.as_deref())?;
        Some(
            enrollment_inventory(&transport, &claimed, authenticator_id)?
                .into_iter()
                .filter(|entry| !entry.pending_reconciliation)
                .map(|entry| entry.label)
                .filter(|label| valid_label(label))
                .collect(),
        )
    }

    fn verify(&mut self, _: &str, _: &str) -> Option<VerifyOutcome> {
        None
    }

    fn start_verify(&mut self, caller: &str, finger: &str) -> Result<VerifyProgress, StartError> {
        self.reap_cancelled();
        if (finger != "any" && !valid_label(finger))
            || self.pending.is_some()
            || self.pending_enrollment.is_some()
            || self.pending_deletion.is_some()
        {
            return Err(StartError::AlreadyInUse);
        }
        let (owner, user) = self.owner.as_ref().ok_or(StartError::PermissionDenied)?;
        if owner != caller || !self.authorized(caller, "verify") {
            return Err(StartError::PermissionDenied);
        }
        let user = user.clone();
        let selected_device = self.selected.clone();
        let finger = (finger != "any").then(|| finger.to_owned());
        let mut key = [0u8; 16];
        // SAFETY: The buffer is writable and remains valid for the getrandom call.
        if unsafe { libc::getrandom(key.as_mut_ptr().cast(), key.len(), 0) } != key.len() as isize
            || key.iter().all(|byte| *byte == 0)
        {
            return Err(StartError::Internal);
        }
        let transport =
            Arc::new(Transport::connect_root(SOCKET).map_err(|_| StartError::Internal)?);
        let auth =
            authenticator(&transport, selected_device.as_deref()).ok_or(StartError::Internal)?;
        let supports_selector = transport.supports_verification_enrollment_selector();
        if finger.is_some() && !supports_selector {
            return Err(StartError::Internal);
        }
        let entries = if supports_selector {
            let entries = enrollment_inventory(&transport, &user, auth.clone())
                .ok_or(StartError::Internal)?;
            if !entries
                .iter()
                .any(|entry| !entry.pending_reconciliation && entry.enrollment.is_some())
            {
                return Err(StartError::NoEnrolledPrints);
            }
            Some(entries)
        } else {
            None
        };
        let selected = if let Some(finger) = &finger {
            let entries = entries.as_deref().ok_or(StartError::Internal)?;
            Some(named_enrollment(entries, finger).ok_or(StartError::NoEnrolledPrints)?)
        } else if supports_selector {
            entries.as_deref().and_then(|entries| {
                let mut eligible = entries
                    .iter()
                    .filter(|entry| !entry.pending_reconciliation && entry.enrollment.is_some());
                let entry = eligible.next()?;
                (eligible.next().is_none() && valid_label(&entry.label))
                    .then(|| entry.enrollment.clone())
                    .flatten()
            })
        } else {
            None
        };
        let selected_label = finger
            .clone()
            .or_else(|| {
                let selected = selected.as_ref()?;
                entries
                    .as_ref()?
                    .iter()
                    .find(|entry| entry.enrollment.as_ref() == Some(selected))
                    .map(|entry| entry.label.clone())
            })
            .unwrap_or_else(|| "any".into());
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let cancellation = Arc::new(Mutex::new(Cancellation::default()));
        let worker_cancellation = cancellation.clone();
        std::thread::spawn(move || {
            let run = || {
                let request = wire::BeginVerification {
                    report_matched_enrollment: supports_selector,
                    enrollment: selected.clone().filter(|_| supports_selector),
                    idempotency_key: key.to_vec(),
                    subject: Some(common::SubjectSelector {
                        selector: Some(common::subject_selector::Selector::AccountHint(
                            user.clone(),
                        )),
                    }),
                    authenticator_id: auth.clone(),
                    purpose_hint: common::VerificationPurpose::PurposeSessionLogin as i32,
                    authority_request_ref: Vec::new(),
                };
                let accepted_transport = transport.clone();
                let (observation, matched_enrollment) = observe_verification_with_result(
                    &transport,
                    request,
                    |accepted| {
                        let _ = ready_tx.send(());
                        let Ok(mut state) = worker_cancellation.lock() else {
                            return;
                        };
                        let operation = (accepted_transport, accepted.operation_id.clone());
                        if state.cancelled {
                            drop(state);
                            request_cancel(operation.0, operation.1);
                        } else {
                            state.operation = Some(operation);
                            if supports_selector || selected_label == "any" {
                                let _ = tx.send(VerifyProgress::Selected(selected_label.clone()));
                            }
                        }
                    },
                    |event| {
                        if let Some(state) = capture_feedback::finger_state(event) {
                            let _ = tx.send(VerifyProgress::Capture {
                                present: state.present,
                                needed: state.needed,
                            });
                        }
                        if let Some(status) = capture_feedback::retry_status(event) {
                            let _ = tx.send(VerifyProgress::Retry(status));
                        }
                    },
                )
                .ok()?;
                Some(match observation {
                    Observation::FactorAccepted => {
                        if let Some(reference) = matched_enrollment {
                            let matched = entries.into_iter().flatten().find(|entry| {
                                !entry.pending_reconciliation
                                    && entry.enrollment.as_ref() == Some(&reference)
                                    && valid_label(&entry.label)
                            });
                            matched.map_or(VerifyOutcome::Matched, |entry| {
                                VerifyOutcome::MatchedFinger(entry.label)
                            })
                        } else {
                            VerifyOutcome::Matched
                        }
                    }
                    Observation::NotMatched => VerifyOutcome::NoMatch,
                    _ => VerifyOutcome::Unavailable,
                })
            };
            let outcome = run().unwrap_or(VerifyOutcome::Unavailable);
            let _ = tx.send(VerifyProgress::Completed(outcome));
        });
        self.pending = Some(rx);
        self.verification_ready = Some(ready_rx);
        self.cancellation = Some(cancellation);
        self.cancelled = false;
        Ok(VerifyProgress::Pending)
    }

    fn poll_verify_ready(&mut self, caller: &str) -> Option<bool> {
        if self.owner.as_ref().is_none_or(|(owner, _)| owner != caller) {
            return Some(false);
        }
        match self.verification_ready.as_ref()?.try_recv() {
            Ok(()) => {
                self.verification_ready = None;
                Some(true)
            }
            Err(TryRecvError::Disconnected) => {
                self.verification_ready = None;
                Some(false)
            }
            Err(TryRecvError::Empty) => None,
        }
    }

    fn poll_verify_progress(&mut self, caller: &str) -> Option<VerifyProgress> {
        if self.owner.as_ref().is_none_or(|(owner, _)| owner != caller) {
            return None;
        }
        let result = match self.pending.as_ref()?.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Disconnected) => {
                VerifyProgress::Completed(VerifyOutcome::Unavailable)
            }
            Err(TryRecvError::Empty) => return None,
        };
        if matches!(result, VerifyProgress::Completed(_)) {
            self.pending = None;
            self.cancellation = None;
        }
        (!self.cancelled).then_some(result)
    }

    fn stop(&mut self, caller: &str) {
        if self
            .owner
            .as_ref()
            .is_some_and(|(owner, _)| owner == caller)
        {
            self.verification_ready = None;
            self.cancel();
        }
    }

    fn start_enroll(&mut self, caller: &str, finger: &str) -> Result<EnrollProgress, StartError> {
        self.reap_cancelled();
        if !valid_label(finger)
            || self.pending.is_some()
            || self.pending_enrollment.is_some()
            || self.pending_deletion.is_some()
        {
            return Err(StartError::AlreadyInUse);
        }
        let (owner, user) = self.owner.as_ref().ok_or(StartError::PermissionDenied)?;
        if owner != caller || !self.authorized(caller, "enroll") {
            return Err(StartError::PermissionDenied);
        }
        let user = user.clone();
        let transport =
            Arc::new(Transport::connect_root(SOCKET).map_err(|_| StartError::Internal)?);
        let auth = authenticator_for(
            &transport,
            self.selected.as_deref(),
            common::ProviderOperation::ProviderEnroll,
        )
        .ok_or(StartError::Internal)?;
        if transport.supports_verification_enrollment_selector() {
            let entries = enrollment_inventory(&transport, &user, auth.clone())
                .ok_or(StartError::Internal)?;
            if entries
                .iter()
                .any(|entry| entry.label == finger && !entry.pending_reconciliation)
            {
                return Err(StartError::FingerAlreadyEnrolled);
            }
        }
        let mut key = [0u8; 16];
        if unsafe { libc::getrandom(key.as_mut_ptr().cast(), key.len(), 0) } != key.len() as isize
            || key.iter().all(|byte| *byte == 0)
        {
            return Err(StartError::Internal);
        }
        let finger = finger.to_owned();
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let cancellation = Arc::new(Mutex::new(Cancellation::default()));
        let worker_cancellation = cancellation.clone();
        std::thread::spawn(move || {
            use carosign_linux_ipc::client::mutation_events::observe_mutation_with_events;
            let request = wire::client_frame::Payload::BeginEnrollment(wire::BeginEnrollment {
                idempotency_key: key.to_vec(),
                subject: Some(common::SubjectSelector {
                    selector: Some(common::subject_selector::Selector::AccountHint(user)),
                }),
                authenticator_id: auth.clone(),
                label: finger,
                replace: None,
                authority_request_ref: Vec::new(),
            });
            let accepted_transport = transport.clone();
            let mut progress = enrollment_progress::EnrollmentProgress::default();
            let result = observe_mutation_with_events(
                &transport,
                request,
                move |id| {
                    let _ = ready_tx.send(());
                    let Ok(mut state) = worker_cancellation.lock() else {
                        return;
                    };
                    let operation = (accepted_transport, id.to_vec());
                    if state.cancelled {
                        drop(state);
                        request_cancel(operation.0, operation.1);
                    } else {
                        state.operation = Some(operation);
                    }
                },
                |event| {
                    if let Some(state) = capture_feedback::finger_state(event) {
                        let _ = tx.send(EnrollProgress::Capture {
                            present: state.present,
                            needed: state.needed,
                        });
                    }
                    progress.observe(event, |status| {
                        let _ = tx.send(status);
                    });
                },
            );
            let outcome = match result {
                Ok(result)
                    if result.result == wire::MutationResult::MutationCommitted as i32
                        && result
                            .enrollment
                            .as_ref()
                            .is_some_and(|enrollment| enrollment.authenticator_id == auth) =>
                {
                    EnrollOutcome::Succeeded
                }
                Ok(result) if result.result() == wire::MutationResult::MutationNoChange => {
                    match result.error.map(|error| error.code()) {
                        Some(common::ErrorCode::ErrorDuplicateSample) => EnrollOutcome::Duplicate,
                        Some(common::ErrorCode::ErrorCaptureStorageFull) => EnrollOutcome::DataFull,
                        Some(common::ErrorCode::ErrorDeviceRemoved) => EnrollOutcome::Disconnected,
                        _ => EnrollOutcome::Failed,
                    }
                }
                Ok(_) => EnrollOutcome::Failed,
                Err(_) => EnrollOutcome::UnknownError,
            };
            let _ = tx.send(EnrollProgress::Completed(outcome));
        });
        self.pending_enrollment = Some(rx);
        self.enrollment_ready = Some(ready_rx);
        self.cancellation = Some(cancellation);
        self.cancelled = false;
        Ok(EnrollProgress::Pending)
    }

    fn enrollment_stages(&self) -> Option<u32> {
        fingerprint_inventory::enrollment_stages(
            &Transport::connect_root(SOCKET).ok()?,
            self.selected.as_deref(),
        )
    }

    fn poll_enroll_ready(&mut self, caller: &str) -> Option<bool> {
        if self.owner.as_ref().is_none_or(|(owner, _)| owner != caller) {
            return Some(false);
        }
        match self.enrollment_ready.as_ref()?.try_recv() {
            Ok(()) => {
                self.enrollment_ready = None;
                Some(true)
            }
            Err(TryRecvError::Disconnected) => {
                self.enrollment_ready = None;
                Some(false)
            }
            Err(TryRecvError::Empty) => None,
        }
    }

    fn poll_enroll_progress(&mut self, caller: &str) -> Option<EnrollProgress> {
        if self.owner.as_ref().is_none_or(|(owner, _)| owner != caller) {
            return None;
        }
        let result = match self.pending_enrollment.as_ref()?.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Disconnected) => {
                EnrollProgress::Completed(EnrollOutcome::UnknownError)
            }
            Err(TryRecvError::Empty) => return None,
        };
        if matches!(result, EnrollProgress::Completed(_)) {
            self.pending_enrollment = None;
            self.cancellation = None;
        }
        (!self.cancelled).then_some(result)
    }

    fn stop_enroll(&mut self, caller: &str) {
        if self
            .owner
            .as_ref()
            .is_some_and(|(owner, _)| owner == caller)
        {
            self.enrollment_ready = None;
            self.cancel();
        }
    }

    fn start_delete(&mut self, caller: &str, finger: Option<&str>) -> Result<(), StartError> {
        self.delete_for(caller, finger, None)
    }

    fn start_delete_for(&mut self, caller: &str, user: &str) -> Result<(), StartError> {
        let user = self
            .username_for(caller, user)
            .ok_or(StartError::PermissionDenied)?;
        self.delete_for(caller, None, Some(user))
    }

    fn poll_delete_result(&mut self, caller: &str) -> Option<Result<(), DeleteError>> {
        if self.owner.as_ref().is_none_or(|(owner, _)| owner != caller) {
            return None;
        }
        let result = match self.pending_deletion.as_ref()?.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Disconnected) => Err(DeleteError::PrintsNotDeleted),
            Err(TryRecvError::Empty) => return None,
        };
        self.pending_deletion = None;
        self.cancellation = None;
        (!self.cancelled).then_some(result)
    }

    fn stop_delete(&mut self, caller: &str) {
        if self
            .owner
            .as_ref()
            .is_some_and(|(owner, _)| owner == caller)
        {
            self.cancel();
        }
    }
}
