use super::{
    Action, Backend, DEVICE, DEVICE_PATH, DEVICE_XML, EnrollProgress, VerifyOutcome,
    VerifyProgress,
    method_error::{error, method_error},
    stop_action, verify_status,
};
use super::{finger_presence::FingerPresence, pending_authorization::PendingAuthorization};
use crate::fingerprint_label::valid_label;
use dbus::{Message, blocking::Connection, channel::Sender};
use std::error::Error;

pub(super) struct DeviceState {
    owner: Option<String>,
    enroll_stages: i32,
    declared_stages: i32,
    presence: FingerPresence,
    action: Option<Action>,
    verify_pending: bool,
    enroll_pending: bool,
    verify_start_pending: Option<Message>,
    enroll_start_pending: Option<Message>,
    delete_pending: Option<Message>,
    stop_pending: Option<Message>,
    after_stop: Option<Message>,
    transient_claim: bool,
    authorization: Option<PendingAuthorization>,
}

impl DeviceState {
    pub(super) fn new() -> Self {
        Self {
            owner: None,
            enroll_stages: -1,
            declared_stages: -1,
            presence: FingerPresence::default(),
            action: None,
            verify_pending: false,
            enroll_pending: false,
            verify_start_pending: None,
            enroll_start_pending: None,
            delete_pending: None,
            stop_pending: None,
            after_stop: None,
            transient_claim: false,
            authorization: None,
        }
    }

    pub(super) fn with_declared_stages(mut self, stages: Option<u32>) -> Self {
        self.declared_stages = stages
            .filter(|stages| (1..=128).contains(stages))
            .map_or(-1, |stages| stages as i32);
        self.enroll_stages = self.declared_stages;
        self
    }

    pub(super) fn path(index: usize) -> String {
        format!("/net/reactivated/Fprint/Device/{index}")
    }

    pub(super) fn index(path: &str) -> Option<usize> {
        let index = path
            .strip_prefix("/net/reactivated/Fprint/Device/")?
            .parse()
            .ok()?;
        (Self::path(index) == path).then_some(index)
    }

    pub(super) fn unpublish(
        &mut self,
        connection: &Connection,
        backend: &mut dyn Backend,
        path: &str,
    ) -> Result<(), Box<dyn Error>> {
        for request in [
            self.authorization.take().map(|pending| pending.request),
            self.verify_start_pending.take(),
            self.enroll_start_pending.take(),
            self.delete_pending.take(),
            self.stop_pending.take(),
            self.after_stop.take(),
        ]
        .into_iter()
        .flatten()
        {
            connection
                .send(error(
                    &request,
                    "net.reactivated.Fprint.Error.NoSuchDevice",
                    c"Device disconnected",
                ))
                .map_err(|_| "D-Bus reply failed")?;
        }
        if let Some(caller) = self.owner.as_deref() {
            for (pending, member, status) in [
                (self.verify_pending, "VerifyStatus", "verify-disconnected"),
                (self.enroll_pending, "EnrollStatus", "enroll-disconnected"),
            ] {
                if pending {
                    let mut signal =
                        Message::new_signal(path, DEVICE, member)?.append2(status, true);
                    signal.set_destination(Some(caller.into()));
                    connection.send(signal).map_err(|_| "D-Bus signal failed")?;
                }
            }
        }
        self.presence.reset(connection, path)?;
        self.stop(backend);
        Ok(())
    }

    pub(super) fn claimed(&self) -> bool {
        self.owner.is_some()
    }

    pub(super) fn disconnect(
        &mut self,
        connection: &Connection,
        backend: &mut dyn Backend,
        path: &str,
        name: &str,
    ) -> Result<(), Box<dyn Error>> {
        if self
            .authorization
            .as_ref()
            .is_some_and(|pending| pending.caller == name)
        {
            self.authorization = None;
        }
        if self.owner.as_deref() == Some(name) {
            self.stop(backend);
            self.presence.reset(connection, path)?;
            update_enroll_stages(
                connection,
                path,
                &mut self.enroll_stages,
                self.declared_stages,
            )?;
        }
        Ok(())
    }

    pub(super) fn stop(&mut self, backend: &mut dyn Backend) {
        self.authorization = None;
        self.verify_pending = false;
        self.enroll_pending = false;
        self.verify_start_pending = None;
        self.enroll_start_pending = None;
        self.delete_pending = None;
        self.stop_pending = None;
        self.after_stop = None;
        self.transient_claim = false;
        if let Some(owner) = self.owner.take() {
            stop_action(backend, &owner, self.action.take());
            backend.release(&owner);
        }
    }

    pub(super) fn poll(
        &mut self,
        connection: &Connection,
        backend: &mut dyn Backend,
        path: &str,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(allowed) = self
            .authorization
            .as_ref()
            .and_then(PendingAuthorization::decision)
            && let Some(pending) = self.authorization.take()
        {
            if let Some(uid) = allowed {
                let interface = pending
                    .request
                    .interface()
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                let member = pending
                    .request
                    .member()
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                backend.authorized_call(Some((&pending.caller, uid)));
                let result = self.dispatch(
                    connection,
                    backend,
                    path,
                    &interface,
                    &member,
                    &pending.caller,
                    pending.request,
                );
                backend.authorized_call(None);
                result?;
            } else {
                connection
                    .send(super::StartError::PermissionDenied.reply(&pending.request))
                    .map_err(|_| "D-Bus reply failed")?;
            }
        }
        if self.stop_pending.is_some() && backend.idle_after_stop() {
            if let Some(request) = self.stop_pending.take() {
                connection
                    .send(request.method_return())
                    .map_err(|_| "D-Bus reply failed")?;
            }
            self.action = None;
        }
        if self.stop_pending.is_none()
            && (self.authorization.is_none() || self.release_queued())
            && let Some(request) = self.after_stop.take()
            && let Some(caller) = self.owner.clone()
        {
            let member = request
                .member()
                .map(|member| member.to_string())
                .unwrap_or_default();
            self.dispatch_with_authorization(
                connection, backend, path, DEVICE, &member, &caller, request,
            )?;
        }
        if let Some(caller) = self.owner.as_deref()
            && self.verify_start_pending.is_some()
            && let Some(ready) = backend.poll_verify_ready(caller)
            && let Some(request) = self.verify_start_pending.take()
        {
            let reply = if ready {
                self.verify_pending = true;
                request.method_return()
            } else {
                backend.stop(caller);
                self.action = None;
                super::StartError::Internal.reply(&request)
            };
            connection.send(reply).map_err(|_| "D-Bus reply failed")?;
        }
        if let Some(caller) = self.owner.as_deref()
            && self.enroll_start_pending.is_some()
            && let Some(ready) = backend.poll_enroll_ready(caller)
        {
            let request = self
                .enroll_start_pending
                .take()
                .expect("запрос удерживается");
            let reply = if ready {
                self.enroll_pending = true;
                request.method_return()
            } else {
                backend.stop_enroll(caller);
                self.action = None;
                request.error(
                    &"net.reactivated.Fprint.Error.Internal".into(),
                    c"Не удалось подготовить регистрацию",
                )
            };
            connection
                .send(reply)
                .map_err(|_| "Не удалось отправить ответ D-Bus")?;
        }
        if let Some(caller) = self.owner.as_deref()
            && self.delete_pending.is_some()
            && let Some(result) = backend.poll_delete_result(caller)
        {
            let request = self.delete_pending.take().expect("запрос удерживается");
            let reply = match result {
                Ok(()) => request.method_return(),
                Err(error) => error.reply(&request),
            };
            connection
                .send(reply)
                .map_err(|_| "Не удалось отправить ответ D-Bus")?;
            self.action = None;
            if self.transient_claim {
                backend.release(caller);
                self.owner = None;
                self.transient_claim = false;
            }
        }
        if self.verify_pending
            && let Some(caller) = self.owner.as_deref()
            && let Some(result) = backend.poll_verify_progress(caller)
        {
            let (status, done) = match result {
                VerifyProgress::Selected(finger) => {
                    let mut signal =
                        Message::new_signal(path, DEVICE, "VerifyFingerSelected")?.append1(finger);
                    signal.set_destination(Some(caller.into()));
                    connection.send(signal).map_err(|_| "D-Bus signal failed")?;
                    return Ok(());
                }
                VerifyProgress::Capture { present, needed } => {
                    self.presence.update(connection, path, present, needed)?;
                    return Ok(());
                }
                VerifyProgress::Retry(status) => (status.verify(), false),
                VerifyProgress::Completed(outcome) => {
                    if let VerifyOutcome::MatchedFinger(finger) = &outcome {
                        let mut signal =
                            dbus::Message::new_signal(path, DEVICE, "VerifyFingerMatched")?
                                .append1(finger.as_str());
                        signal.set_destination(Some(caller.into()));
                        connection
                            .send(signal)
                            .map_err(|_| "Не удалось отправить сигнал D-Bus")?;
                    }
                    (verify_status(outcome), true)
                }
                VerifyProgress::Pending => return Ok(()),
            };
            let mut signal =
                dbus::Message::new_signal(path, DEVICE, "VerifyStatus")?.append2(status, done);
            signal.set_destination(Some(caller.into()));
            connection
                .send(signal)
                .map_err(|_| "Не удалось отправить сигнал D-Bus")?;
            self.verify_pending = !done;
            if done {
                self.presence.reset(connection, path)?;
            }
        }
        if self.enroll_pending
            && let Some(caller) = self.owner.as_deref()
            && let Some(outcome) = backend.poll_enroll_progress(caller)
        {
            let (status, done) = match outcome {
                EnrollProgress::Capture { present, needed } => {
                    self.presence.update(connection, path, present, needed)?;
                    return Ok(());
                }
                EnrollProgress::Stages(stages) => {
                    let stages = i32::try_from(stages).unwrap_or(1);
                    update_enroll_stages(connection, path, &mut self.enroll_stages, stages)?;
                    return Ok(());
                }
                EnrollProgress::Retry(status) => (status.enroll(), false),
                EnrollProgress::StagePassed => ("enroll-stage-passed", false),
                EnrollProgress::Completed(outcome) => (outcome.status(), true),
                EnrollProgress::Pending => return Ok(()),
            };
            let mut signal =
                dbus::Message::new_signal(path, DEVICE, "EnrollStatus")?.append2(status, done);
            signal.set_destination(Some(caller.into()));
            connection
                .send(signal)
                .map_err(|_| "Не удалось отправить сигнал D-Bus")?;
            self.enroll_pending = !done;
            if done {
                self.presence.reset(connection, path)?;
                self.delete_pending = None;
            }
        }
        Ok(())
    }

    pub(super) fn dispatch_with_authorization(
        &mut self,
        connection: &Connection,
        backend: &mut dyn Backend,
        path: &str,
        interface: &str,
        member: &str,
        caller: &str,
        message: Message,
    ) -> Result<(), Box<dyn Error>> {
        let owns = self.owner.as_deref() == Some(caller);
        if interface == DEVICE && owns && (self.stop_pending.is_some() || self.after_stop.is_some())
        {
            match member {
                "Release" if !self.transient_claim => {
                    if self.release_queued() {
                        connection
                            .send(super::StartError::AlreadyInUse.reply(&message))
                            .map_err(|_| "D-Bus reply failed")?;
                        return Ok(());
                    }
                    if let Some(request) = self.after_stop.replace(message) {
                        connection
                            .send(error(
                                &request,
                                "net.reactivated.Fprint.Error.ClaimDevice",
                                c"Device released before the operation started",
                            ))
                            .map_err(|_| "D-Bus reply failed")?;
                    }
                    return Ok(());
                }
                "EnrollStart" | "DeleteEnrolledFinger" | "DeleteEnrolledFingers2"
                    if self.action == Some(Action::Verify) && self.after_stop.is_none() =>
                {
                    self.after_stop = Some(message);
                    return Ok(());
                }
                "VerifyStart"
                | "EnrollStart"
                | "DeleteEnrolledFinger"
                | "DeleteEnrolledFingers2"
                    if self.after_stop.is_some() =>
                {
                    connection
                        .send(super::StartError::AlreadyInUse.reply(&message))
                        .map_err(|_| "D-Bus reply failed")?;
                    return Ok(());
                }
                _ => {}
            }
        }
        let available = self.action.is_none() && !self.transient_claim;
        let needs_policy = interface == DEVICE
            && match member {
                "Claim" => self.owner.is_none(),
                "ListEnrolledFingers" => true,
                "DeleteEnrolledFingers" => available && (self.owner.is_none() || owns),
                "VerifyStart"
                | "EnrollStart"
                | "DeleteEnrolledFingers2"
                | "DeleteEnrolledFinger" => available && owns,
                _ => false,
            };
        if needs_policy {
            let username = matches!(
                member,
                "Claim" | "ListEnrolledFingers" | "DeleteEnrolledFingers"
            )
            .then(|| message.read1::<&str>().ok())
            .flatten();
            if self.authorization.is_some() {
                connection
                    .send(super::StartError::AlreadyInUse.reply(&message))
                    .map_err(|_| "D-Bus reply failed")?;
                return Ok(());
            }
            match backend.authorize_call(caller, member, username) {
                Some(Ok(result)) => {
                    self.authorization = Some(PendingAuthorization {
                        request: message,
                        caller: caller.to_owned(),
                        result,
                    });
                    return Ok(());
                }
                Some(Err(error)) => {
                    connection
                        .send(error.reply(&message))
                        .map_err(|_| "D-Bus reply failed")?;
                    return Ok(());
                }
                None => {}
            }
        }
        self.dispatch(
            connection, backend, path, interface, member, caller, message,
        )
    }

    fn release_queued(&self) -> bool {
        self.after_stop
            .as_ref()
            .and_then(Message::member)
            .is_some_and(|member| &*member == "Release")
    }

    fn dispatch(
        &mut self,
        connection: &Connection,
        backend: &mut dyn Backend,
        path: &str,
        interface: &str,
        member: &str,
        caller: &str,
        message: Message,
    ) -> Result<(), Box<dyn Error>> {
        let mut status = None;
        if interface == DEVICE
            && member == "DeleteEnrolledFingers"
            && self.action.is_none()
            && self.owner.as_deref().is_none_or(|owner| owner == caller)
        {
            let user = message.read1::<&str>()?;
            let transient = self.owner.is_none();
            let start = if transient {
                backend.claim_for_delete(caller, user).and_then(|()| {
                    let result = backend.start_delete(caller, None);
                    if result.is_err() {
                        backend.release(caller);
                    }
                    result
                })
            } else {
                backend.start_delete_for(caller, user)
            };
            match start {
                Ok(()) => {
                    self.owner = Some(caller.to_owned());
                    self.transient_claim = transient;
                    self.action = Some(Action::Delete);
                    self.delete_pending = Some(message);
                }
                Err(error) => {
                    connection
                        .send(error.reply(&message))
                        .map_err(|_| "D-Bus reply failed")?;
                }
            }
            return Ok(());
        }

        if interface == DEVICE
            && matches!(member, "DeleteEnrolledFingers2" | "DeleteEnrolledFinger")
            && self.owner.as_deref() == Some(caller)
            && self.action.is_none()
        {
            let finger = if member == "DeleteEnrolledFinger" {
                message.read1::<&str>().ok()
            } else {
                None
            };
            let valid = member == "DeleteEnrolledFingers2" || finger.is_some_and(valid_label);
            if valid {
                match backend.start_delete(caller, finger) {
                    Ok(()) => {
                        self.action = Some(Action::Delete);
                        self.delete_pending = Some(message);
                    }
                    Err(error) => {
                        connection
                            .send(error.reply(&message))
                            .map_err(|_| "D-Bus reply failed")?;
                    }
                }
                return Ok(());
            }
        }
        let reply = match (DEVICE_PATH, interface, member) {
            (DEVICE_PATH, "org.freedesktop.DBus.Introspectable", "Introspect")
                if backend.available() =>
            {
                message.return_with_args((DEVICE_XML,))
            }
            (DEVICE_PATH, DEVICE, "Claim") if backend.available() => {
                if self.owner.is_some() {
                    let reply = message.error(
                        &"net.reactivated.Fprint.Error.AlreadyInUse".into(),
                        c"Устройство занято",
                    );
                    connection
                        .send(reply)
                        .map_err(|_| "Не удалось отправить ответ D-Bus")?;
                    return Ok(());
                }
                match backend.claim_checked(caller, message.read1::<&str>()?) {
                    Ok(()) => {
                        self.owner = Some(caller.to_owned());
                        let stages = backend
                            .enrollment_stages()
                            .and_then(|stages| i32::try_from(stages).ok())
                            .filter(|stages| *stages > 0)
                            .unwrap_or(1);
                        update_enroll_stages(connection, path, &mut self.enroll_stages, stages)?;
                        message.method_return()
                    }
                    Err(error) => error.reply(&message),
                }
            }
            (DEVICE_PATH, DEVICE, "Release")
                if self.owner.as_deref() == Some(caller)
                    && !self.transient_claim
                    && self.stop_pending.is_none() =>
            {
                self.verify_pending = false;
                if let Some(request) = self.verify_start_pending.take() {
                    connection
                        .send(super::StartError::Internal.reply(&request))
                        .map_err(|_| "D-Bus reply failed")?;
                }
                self.enroll_pending = false;
                self.presence.reset(connection, path)?;
                if let Some(request) = self.enroll_start_pending.take() {
                    connection
                        .send(request.error(
                            &"net.reactivated.Fprint.Error.Internal".into(),
                            c"Регистрация отменена",
                        ))
                        .map_err(|_| "Не удалось отправить ответ D-Bus")?;
                }
                if let Some(request) = self.delete_pending.take() {
                    connection
                        .send(error(
                            &request,
                            "net.reactivated.Fprint.Error.PrintsNotDeleted",
                            c"Deletion cancelled",
                        ))
                        .map_err(|_| "D-Bus reply failed")?;
                }
                stop_action(backend, caller, self.action.take());
                backend.release(caller);
                self.owner = None;
                update_enroll_stages(
                    connection,
                    path,
                    &mut self.enroll_stages,
                    self.declared_stages,
                )?;
                message.method_return()
            }
            (DEVICE_PATH, DEVICE, "ListEnrolledFingers") if backend.available() => {
                match message
                    .read1::<&str>()
                    .ok()
                    .and_then(|user| backend.enrolled(caller, user))
                {
                    Some(fingers) if fingers.is_empty() => message.error(
                        &"net.reactivated.Fprint.Error.NoEnrolledPrints".into(),
                        c"Нет зарегистрированных отпечатков",
                    ),
                    Some(fingers) => message.return_with_args((fingers,)),
                    None => message.error(
                        &"net.reactivated.Fprint.Error.PermissionDenied".into(),
                        c"Нет полномочий",
                    ),
                }
            }
            (DEVICE_PATH, DEVICE, "EnrollStart" | "DeleteEnrolledFinger" | "VerifyStart")
                if self.owner.as_deref() == Some(caller)
                    && self.action.is_none()
                    && message.read1::<&str>().is_ok_and(|finger| {
                        !valid_label(finger) && !(member == "VerifyStart" && finger == "any")
                    }) =>
            {
                error(
                    &message,
                    "net.reactivated.Fprint.Error.InvalidFingername",
                    c"Invalid finger name",
                )
            }
            (DEVICE_PATH, DEVICE, "VerifyStart")
                if self.owner.as_deref() == Some(caller) && self.action.is_none() =>
            {
                match backend.start_verify(caller, message.read1::<&str>()?) {
                    Ok(progress) => {
                        self.action = Some(Action::Verify);
                        match progress {
                            VerifyProgress::Pending
                            | VerifyProgress::Selected(_)
                            | VerifyProgress::Capture { .. }
                            | VerifyProgress::Retry(_) => {
                                self.verify_start_pending = Some(message);
                                return Ok(());
                            }
                            VerifyProgress::Completed(result) => {
                                status = Some(("VerifyStatus", verify_status(result)))
                            }
                        }
                        message.method_return()
                    }
                    Err(error) => error.reply(&message),
                }
            }
            (DEVICE_PATH, DEVICE, "VerifyStop")
                if self.owner.as_deref() == Some(caller)
                    && self.action == Some(Action::Verify)
                    && self.stop_pending.is_none() =>
            {
                self.verify_pending = false;
                if let Some(request) = self.verify_start_pending.take() {
                    connection
                        .send(super::StartError::Internal.reply(&request))
                        .map_err(|_| "D-Bus reply failed")?;
                }
                self.presence.reset(connection, path)?;
                backend.stop(caller);
                if !backend.idle_after_stop() {
                    self.stop_pending = Some(message);
                    return Ok(());
                }
                self.action = None;
                message.method_return()
            }
            (DEVICE_PATH, DEVICE, "EnrollStart")
                if self.owner.as_deref() == Some(caller) && self.action.is_none() =>
            {
                match backend.start_enroll(caller, message.read1::<&str>()?) {
                    Ok(progress) => {
                        self.action = Some(Action::Enroll);
                        match progress {
                            EnrollProgress::Pending
                            | EnrollProgress::Retry(_)
                            | EnrollProgress::StagePassed
                            | EnrollProgress::Stages(_)
                            | EnrollProgress::Capture { .. } => {
                                self.enroll_start_pending = Some(message);
                                return Ok(());
                            }
                            EnrollProgress::Completed(outcome) => {
                                status = Some(("EnrollStatus", outcome.status()));
                            }
                        }
                        message.method_return()
                    }
                    Err(error) => error.reply(&message),
                }
            }
            (DEVICE_PATH, DEVICE, "EnrollStop")
                if self.owner.as_deref() == Some(caller)
                    && self.action == Some(Action::Enroll)
                    && self.stop_pending.is_none() =>
            {
                self.enroll_pending = false;
                self.presence.reset(connection, path)?;
                if let Some(request) = self.enroll_start_pending.take() {
                    connection
                        .send(request.error(
                            &"net.reactivated.Fprint.Error.Internal".into(),
                            c"Регистрация отменена",
                        ))
                        .map_err(|_| "Не удалось отправить ответ D-Bus")?;
                }
                backend.stop_enroll(caller);
                if !backend.idle_after_stop() {
                    self.stop_pending = Some(message);
                    return Ok(());
                }
                self.action = None;
                message.method_return()
            }
            (DEVICE_PATH, "org.freedesktop.DBus.Properties", "Get") if backend.available() => {
                match message.read2::<&str, &str>() {
                    Ok((DEVICE, "name")) => {
                        message.return_with_args((dbus::arg::Variant(backend.name()),))
                    }
                    Ok((DEVICE, "scan-type")) => {
                        message.return_with_args((dbus::arg::Variant(backend.scan_type()),))
                    }
                    Ok((DEVICE, "num-enroll-stages")) => {
                        message.return_with_args((dbus::arg::Variant(self.enroll_stages),))
                    }
                    Ok((DEVICE, "finger-present")) => {
                        message.return_with_args((dbus::arg::Variant(self.presence.present),))
                    }
                    Ok((DEVICE, "finger-needed")) => {
                        message.return_with_args((dbus::arg::Variant(self.presence.needed),))
                    }
                    _ => method_error(
                        &message,
                        DEVICE_PATH,
                        interface,
                        member,
                        self.owner.is_some(),
                        self.owner.as_deref() == Some(caller),
                        self.action,
                    ),
                }
            }
            (DEVICE_PATH, "org.freedesktop.DBus.Properties", "GetAll") if backend.available() => {
                crate::diagnostics::event("get_all_request");
                use dbus::arg::{RefArg, Variant};
                match message.read1::<&str>() {
                    Ok(DEVICE) => {
                        let mut properties: std::collections::HashMap<
                            &str,
                            Variant<Box<dyn RefArg>>,
                        > = std::collections::HashMap::new();
                        properties.insert("name", Variant(Box::new(backend.name().to_owned())));
                        properties.insert(
                            "scan-type",
                            Variant(Box::new(backend.scan_type().to_owned())),
                        );
                        properties
                            .insert("num-enroll-stages", Variant(Box::new(self.enroll_stages)));
                        properties
                            .insert("finger-present", Variant(Box::new(self.presence.present)));
                        properties.insert("finger-needed", Variant(Box::new(self.presence.needed)));
                        crate::diagnostics::event("get_all_reply");
                        message.return_with_args((properties,))
                    }
                    _ => message.error(
                        &"org.freedesktop.DBus.Error.UnknownInterface".into(),
                        c"Нет интерфейса",
                    ),
                }
            }
            _ => method_error(
                &message,
                DEVICE_PATH,
                interface,
                member,
                self.owner.is_some(),
                self.owner.as_deref() == Some(caller) && !self.transient_claim,
                self.action,
            ),
        };
        connection
            .send(reply)
            .map_err(|_| "Не удалось отправить ответ D-Bus")?;
        if let Some((member, status)) = status {
            let mut signal = dbus::Message::new_signal(path, DEVICE, member)?.append2(status, true);
            signal.set_destination(Some(caller.into()));
            connection
                .send(signal)
                .map_err(|_| "Не удалось отправить сигнал D-Bus")?;
        }
        Ok(())
    }
}

fn update_enroll_stages(
    connection: &dbus::blocking::Connection,
    path: &str,
    current: &mut i32,
    stages: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    if *current == stages {
        return Ok(());
    }
    let mut changed = dbus::arg::PropMap::new();
    changed.insert(
        "num-enroll-stages".into(),
        dbus::arg::Variant(Box::new(stages)),
    );
    let signal =
        dbus::Message::new_signal(path, "org.freedesktop.DBus.Properties", "PropertiesChanged")?
            .append3(DEVICE, changed, Vec::<String>::new());
    connection
        .send(signal)
        .map_err(|_| "Не удалось отправить сигнал D-Bus")?;
    *current = stages;
    Ok(())
}
