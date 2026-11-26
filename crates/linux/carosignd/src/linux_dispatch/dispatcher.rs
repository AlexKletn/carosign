use std::sync::Arc;

use carosign_core::device::DeviceEpoch;
use carosign_linux::ids::KernelIds;
use carosign_protocol::carosign::{client::v1::Authenticators, common::v1::UnavailabilityReason};
use carosign_runtime::ports::{AuthorityRefusal, TimeSource};

use super::{execute_mutation, verify};

/// Transfer of the system performer to the public protocol session.
pub(crate) struct LinuxDispatcher<'a, S, T> {
    pub(crate) service:
        &'a crate::service::Service<'a, S, crate::linux_policy::LinuxPolicy, KernelIds, T>,
    pub(crate) profile: &'a crate::provider_supervisor::ProviderProfile,
    pub(crate) registry: Arc<crate::operations::OperationRegistry>,
    pub(crate) resources: Arc<crate::resources::LocalResourceOwner>,
    pub(crate) clock: Arc<T>,
    pub(crate) control: Arc<crate::control_watch::ControlOwner>,
    pub(crate) challenges: &'a crate::scoped_challenge::ScopedChallenges,
    pub(crate) capabilities: &'a std::sync::Mutex<crate::linux_service::CapabilityState>,
    pub(crate) queue: &'a super::DeviceQueue,
    pub(crate) activity: &'a crate::linux_service::MutationActivity,
}

/// The declaration of capabilities does not in itself confirm the readiness of the device.
pub(crate) fn available_capabilities(
    capabilities: Option<&carosign_protocol::carosign::common::v1::CapabilityDescriptor>,
    verified_ready: bool,
) -> bool {
    use carosign_protocol::carosign::common::v1::{Modality, ProviderOperation};
    verified_ready
        && capabilities.is_some_and(|capabilities| {
            if capabilities.modality != Modality::Fingerprint as i32 {
                return false;
            }
            capabilities.operations.iter().any(|operation| {
                matches!(
                    ProviderOperation::from_i32(*operation),
                    Some(
                        ProviderOperation::ProviderVerify
                            | ProviderOperation::ProviderEnroll
                            | ProviderOperation::ProviderRemove
                            | ProviderOperation::ProviderEnumerate
                            | ProviderOperation::ProviderQueryMutation
                    )
                )
            })
        })
}

fn unavailability_reason(available: bool, verified: bool) -> i32 {
    (if available {
        UnavailabilityReason::Unspecified
    } else if verified {
        UnavailabilityReason::UnsupportedCapabilities
    } else {
        UnavailabilityReason::ReadinessCheckFailed
    }) as i32
}

#[path = "dispatcher.tests.rs"]
#[cfg(test)]
mod availability_tests;

impl<S: Sync, T: TimeSource + 'static> LinuxDispatcher<'_, S, T> {
    fn inventory(
        &self,
    ) -> Result<Vec<carosign_protocol::carosign::common::v1::AuthenticatorDescriptor>, ()> {
        if self.profile.requires_durable_binding() {
            // The declaration does not consume hardware and does not check the hardware.
            let state = self.capabilities.lock().map_err(|_| ())?;
            let mut descriptor = self.profile.identity().descriptor(DeviceEpoch::FIRST);
            descriptor.capabilities = state.descriptor.clone().or(descriptor.capabilities);
            descriptor.available =
                available_capabilities(descriptor.capabilities.as_ref(), state.verified_ready);
            descriptor.unavailability_reason =
                unavailability_reason(descriptor.available, state.verified_ready);
            return Ok(vec![descriptor]);
        }
        let mut capabilities = self.capabilities.lock().map_err(|_| ())?;
        if capabilities.descriptor.is_none() {
            // The adapter is called from the session performer; the probe itself awaits the
            // resource port, so we execute it outside the nested executor.
            capabilities.descriptor = std::thread::scope(|scope| {
                scope
                    .spawn(|| {
                        crate::linux_service::probe_capabilities(
                            self.profile,
                            self.resources.as_ref(),
                            self.clock.as_ref(),
                        )
                    })
                    .join()
                    .ok()
                    .and_then(Result::ok)
            });
            capabilities.verified_ready = capabilities.descriptor.is_some();
        }
        let mut descriptor = self.profile.identity().descriptor(DeviceEpoch::FIRST);
        descriptor.capabilities = capabilities.descriptor.clone().or(descriptor.capabilities);
        descriptor.available = available_capabilities(
            descriptor.capabilities.as_ref(),
            capabilities.verified_ready,
        );
        descriptor.unavailability_reason =
            unavailability_reason(descriptor.available, capabilities.verified_ready);
        Ok(vec![descriptor])
    }
}

impl<S, T> crate::talk::VerificationDispatcher for LinuxDispatcher<'_, S, T>
where
    S: carosign_persistence::store::MetadataStore,
    T: TimeSource + 'static,
{
    fn inventory_for(
        &self,
        connection: &crate::server::Connection,
    ) -> Result<Authenticators, AuthorityRefusal> {
        self.service.authority.read_inventory(connection, || {
            self.inventory().map(|entries| Authenticators {
                entries,
                providers: None,
            })
        })
    }

    fn enrollments<'a>(
        &'a self,
        connection: &'a crate::server::Connection,
        request: &'a carosign_linux_ipc::codec::ClientRequest,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<
                        Vec<carosign_protocol::carosign::common::v1::EnrollmentSummary>,
                        (),
                    >,
                > + 'a,
        >,
    > {
        Box::pin(async move {
            use carosign_core::authorization::AuthorizationBoundary;
            use carosign_persistence::enrollment_codec::EnrollmentStatus;
            use carosign_protocol::carosign::common::v1 as common;
            use carosign_runtime::authority::verification::{
                VerificationIntent, authorize_verification,
            };
            use carosign_runtime::ports::PolicyAuthority;

            let carosign_linux_ipc::codec::ClientRequest::ListEnrollments {
                authenticator,
                subject,
                authority_request_ref,
            } = request
            else {
                return Err(());
            };
            let authenticator =
                carosign_core::ids::AuthenticatorId::try_from(authenticator.as_slice())
                    .map_err(|_| ())?;
            if authenticator != self.profile.identity().authenticator {
                return Err(());
            }
            let caller = crate::cycle::caller_from_session(&connection.session).ok_or(())?;
            // The existing subject verification rule is used to read the list: it does not
            // give the right to register or delete.
            let intent = VerificationIntent::from_wire(
                subject,
                common::VerificationPurpose::PurposeSessionLogin as i32,
                authority_request_ref,
                authenticator,
            )
            .map_err(|_| ())?;
            let (subject, grant) = authorize_verification(
                self.service.authority,
                self.service.authority,
                &caller,
                &intent,
            )
            .await
            .map_err(|_| ())?;
            let scope = crate::accept::scope_of(subject, authenticator);
            let records = crate::accept::read_enrollment_records(self.service.store, &scope)
                .map_err(|_| ())?;
            self.service
                .authority
                .revalidate_caller(&caller, &grant, AuthorizationBoundary::BeforeDelivery)
                .await
                .map_err(|_| ())?;
            Ok(records
                .into_iter()
                .filter_map(|record| {
                    if record.status == EnrollmentStatus::Unavailable {
                        return None;
                    }
                    Some(common::EnrollmentSummary {
                        enrollment: Some(common::EnrollmentRef {
                            enrollment_id: record.enrollment.id.as_bytes().to_vec(),
                            authenticator_id: record.enrollment.authenticator.as_bytes().to_vec(),
                            generation: record.enrollment.generation.value(),
                        }),
                        label: record.display_label().to_owned(),
                        original_label: record.display_label.is_some().then_some(record.label),
                        pending_reconciliation: record.status
                            == EnrollmentStatus::PendingReconciliation,
                    })
                })
                .collect())
        })
    }

    fn rename_enrollment<'a>(
        &'a self,
        connection: &'a crate::server::Connection,
        request: &'a carosign_protocol::carosign::client::v1::RenameEnrollment,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<
        carosign_protocol::carosign::client::v1::EnrollmentRenamed,
        carosign_protocol::carosign::common::v1::ErrorCode,
    >> + 'a>> {
        Box::pin(crate::rename_enrollment::rename_enrollment(
            self.service.store, self.service.authority, connection, request,
        ))
    }

    fn dispatch<'a>(
        &'a self,
        connection: &'a crate::server::Connection,
        handle: crate::operations::ConnectionHandle,
        request: &'a carosign_linux_ipc::codec::ClientRequest,
        request_id: &'a [u8],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), crate::talk::Ended>> + 'a>>
    {
        Box::pin(async move {
            self.control.retire_completed(&self.registry);
            self.registry.retire_completed(self.clock.now());
            let domain = carosign_linux_ipc::mapping::client::to_domain(request)
                .map_err(|_| crate::talk::Ended::Refused("invalid request".into()))?
                .ok_or_else(|| crate::talk::Ended::Refused("unsupported request".into()))?;
            if self.profile.identity().modality
                != carosign_protocol::carosign::common::v1::Modality::Fingerprint
            {
                return send_queue_error(
                    connection,
                    request_id,
                    super::device_queue::QueueError::Unavailable,
                );
            }
            if matches!(
                domain,
                carosign_linux_ipc::mapping::client::DomainRequest::BeginEnrollment { .. }
                    | carosign_linux_ipc::mapping::client::DomainRequest::BeginRemoval { .. }
            ) {
                use carosign_protocol::carosign::common::v1::ErrorCode;
                use prost::Message;
                let caller = crate::cycle::caller_from_session(&connection.session)
                    .ok_or_else(|| crate::talk::Ended::Refused("invalid caller".into()))?;
                let authorized = crate::mutation_dispatch::authorize(
                    self.service.authority,
                    &caller,
                    &domain,
                    self.profile.identity().authenticator,
                )
                .await;
                let result = match authorized {
                    Ok(authorized) => {
                        let turn =
                            match self.queue.wait(connection, authorized.authenticator, || {
                                self.service
                                    .authority
                                    .revalidate_now(&authorized.caller, &authorized.grant)
                            }) {
                                Ok(turn) => turn,
                                Err(error) => {
                                    return send_queue_error(connection, request_id, error);
                                }
                            };
                        crate::mutation_dispatch::prepare_authorized(
                            self.service.store,
                            self.service.authority,
                            authorized,
                        )
                        .await
                        .map(|prepared| (prepared, turn))
                    }
                    Err(error) => Err(error),
                };
                let (code, message) = match result {
                    Ok((prepared, _turn)) => {
                        let _activity = self.activity.enter().map_err(|_| {
                            crate::talk::Ended::Refused("mutation tracking unavailable".into())
                        })?;
                        return execute_mutation(self, connection, handle, request_id, &prepared)
                            .await;
                    }
                    Err(crate::mutation_dispatch::MutationPrepareError::Denied) => {
                        (ErrorCode::ErrorUnauthorized, "mutation unauthorized")
                    }
                    Err(crate::mutation_dispatch::MutationPrepareError::Invalid) => {
                        (ErrorCode::ErrorInvalidMessage, "invalid mutation")
                    }
                    Err(crate::mutation_dispatch::MutationPrepareError::Conflict) => {
                        (ErrorCode::ErrorUnavailable, "registration changed")
                    }
                    Err(crate::mutation_dispatch::MutationPrepareError::Unavailable) => {
                        (ErrorCode::ErrorUnavailable, "mutation unavailable")
                    }
                };
                let mut frame = carosign_linux_ipc::codec::error_frame(request_id, code, message);
                let version = connection
                    .session
                    .negotiated()
                    .ok_or_else(|| crate::talk::Ended::Refused("version unavailable".into()))?;
                frame.major = version.major;
                frame.minor = version.minor;
                let frame = frame.encode_to_vec();
                return crate::server::write_frame(connection, &frame)
                    .map_err(crate::talk::Ended::Broken);
            }
            match verify(
                self.service,
                &domain,
                request_id,
                connection,
                handle,
                self.profile,
                self.registry.clone(),
                self.resources.clone(),
                self.clock.clone(),
                self.control.clone(),
                self.challenges,
                self.queue,
            )
            .await
            {
                Ok(()) => Ok(()),
                Err(super::error::DispatchError::Queue(error)) => {
                    send_queue_error(connection, request_id, error)
                }
                Err(error) => Err(crate::talk::Ended::Refused(format!(
                    "verification unavailable: {error}"
                ))),
            }
        })
    }
}

fn send_queue_error(
    connection: &crate::server::Connection,
    request_id: &[u8],
    error: super::device_queue::QueueError,
) -> Result<(), crate::talk::Ended> {
    use prost::Message;
    if error == super::device_queue::QueueError::Disconnected {
        return Err(crate::talk::Ended::Closed);
    }
    let version = connection
        .session
        .negotiated()
        .ok_or_else(|| crate::talk::Ended::Refused("version unavailable".into()))?;
    let (code, message) = error.response();
    let mut frame = carosign_linux_ipc::codec::error_frame(request_id, code, message);
    frame.major = version.major;
    frame.minor = version.minor;
    crate::server::write_frame(connection, &frame.encode_to_vec())
        .map_err(crate::talk::Ended::Broken)
}
