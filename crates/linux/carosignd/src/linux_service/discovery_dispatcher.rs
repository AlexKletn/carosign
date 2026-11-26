use std::{
    future::Future,
    path::Path,
    pin::Pin,
    sync::{Arc, Mutex},
};

use carosign_core::ids::AuthenticatorId;
use carosign_linux::ids::KernelIds;
use carosign_linux_ipc::codec::ClientRequest;
use carosign_protocol::carosign::{client::v1::Authenticators, common::v1::EnrollmentSummary};
use carosign_runtime::ports::{AuthorityRefusal, TimeSource};
use prost::Message;

use crate::{
    control_watch::ControlOwner,
    linux_dispatch::LinuxDispatcher,
    linux_policy::LinuxPolicy,
    operations::{ConnectionHandle, OperationRegistry},
    provider_catalog::ProviderCatalog,
    resources::LocalResourceOwner,
    scoped_challenge::ScopedChallenges,
    server::Connection,
    service::Service,
    talk::{Ended, VerificationDispatcher},
};

use super::{CapabilityState, MutationActivity};

pub(super) struct DiscoveryDispatcher<'a, S, T> {
    pub(super) catalog: &'a ProviderCatalog,
    pub(super) runtime: &'a Path,
    pub(super) service: &'a Service<'a, S, LinuxPolicy, KernelIds, T>,
    pub(super) registry: &'a Arc<OperationRegistry>,
    pub(super) resources: &'a Arc<LocalResourceOwner>,
    pub(super) clock: &'a Arc<T>,
    pub(super) control: &'a Arc<ControlOwner>,
    pub(super) challenges: &'a ScopedChallenges,
    pub(super) capabilities: &'a Mutex<CapabilityState>,
    pub(super) queue: &'a crate::linux_dispatch::DeviceQueue,
    pub(super) activity: &'a MutationActivity,
}

impl<S, T> VerificationDispatcher for DiscoveryDispatcher<'_, S, T>
where
    S: carosign_persistence::store::MetadataStore,
    T: TimeSource + 'static,
{
    fn inventory_for(&self, connection: &Connection) -> Result<Authenticators, AuthorityRefusal> {
        self.service.authority.read_inventory(connection, || {
            self.catalog.public_inventory().map_err(|_| ())
        })
    }

    fn enrollments<'a>(
        &'a self,
        connection: &'a Connection,
        request: &'a ClientRequest,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<EnrollmentSummary>, ()>> + 'a>> {
        Box::pin(async move {
            let ClientRequest::ListEnrollments { authenticator, .. } = request else {
                return Err(());
            };
            let id = AuthenticatorId::try_from(authenticator.as_slice()).map_err(|_| ())?;
            let profile = self.catalog.execution_profile(id).map_err(|_| ())?;
            let service = self.service_for(&profile);
            self.dispatcher(&profile, &service)
                .enrollments(connection, request)
                .await
        })
    }

    fn rename_enrollment<'a>(
        &'a self,
        connection: &'a Connection,
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
        connection: &'a Connection,
        handle: ConnectionHandle,
        request: &'a ClientRequest,
        request_id: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<(), Ended>> + 'a>> {
        Box::pin(async move {
            let authenticator = match request {
                ClientRequest::BeginVerification { authenticator, .. }
                | ClientRequest::BeginEnrollment { authenticator, .. }
                | ClientRequest::BeginRemoval { authenticator, .. } => authenticator,
                _ => {
                    return session_unavailable(connection, request_id);
                }
            };
            let profile = AuthenticatorId::try_from(authenticator.as_slice())
                .ok()
                .and_then(|id| self.catalog.resolve(id, self.runtime).ok());
            let Some(profile) = profile else {
                return session_unavailable(connection, request_id);
            };
            let service = self.service_for(&profile);
            self.dispatcher(&profile, &service)
                .dispatch(connection, handle, request, request_id)
                .await
        })
    }
}

type LinuxService<'a, S, T> = Service<'a, S, LinuxPolicy, KernelIds, T>;
type ProviderProfile = crate::provider_supervisor::ProviderProfile;

impl<S, T> DiscoveryDispatcher<'_, S, T> {
    fn service_for<'a>(&'a self, profile: &'a ProviderProfile) -> LinuxService<'a, S, T> {
        Service {
            resource_sets: profile.identity(),
            ..*self.service
        }
    }

    fn dispatcher<'a>(
        &'a self,
        profile: &'a ProviderProfile,
        service: &'a LinuxService<'a, S, T>,
    ) -> LinuxDispatcher<'a, S, T> {
        LinuxDispatcher {
            service,
            profile,
            registry: self.registry.clone(),
            resources: self.resources.clone(),
            clock: self.clock.clone(),
            control: self.control.clone(),
            challenges: self.challenges,
            capabilities: self.capabilities,
            activity: self.activity,
            queue: self.queue,
        }
    }
}

fn session_unavailable(connection: &Connection, request_id: &[u8]) -> Result<(), Ended> {
    let version = connection
        .session
        .negotiated()
        .ok_or_else(|| Ended::Refused("version unavailable".into()))?;
    let mut response = carosign_linux_ipc::codec::error_frame(
        request_id,
        carosign_protocol::carosign::common::v1::ErrorCode::ErrorUnavailable,
        "provider session unavailable",
    );
    response.major = version.major;
    response.minor = version.minor;
    crate::server::write_frame(connection, &response.encode_to_vec()).map_err(Ended::Broken)
}
