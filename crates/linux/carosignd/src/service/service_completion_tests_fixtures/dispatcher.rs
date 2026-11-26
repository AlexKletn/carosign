use super::*;
use crate::talk::{Ended, VerificationDispatcher};
use crate::verification_owner::{FlowProgress, VerificationFlow, VerificationFlowTarget};
use carosign_linux_ipc::codec::ClientRequest;
use std::time::Duration;

pub(super) struct Dispatcher<'a> {
    pub(super) service: &'a crate::service::Service<
        'a,
        MemoryStore,
        ClientAuthority,
        Ids,
        carosign_adapters::clock::StepClock,
    >,
    pub(super) authority: &'a ClientAuthority,
    pub(super) registry: std::sync::Arc<crate::operations::OperationRegistry>,
    pub(super) clock: std::sync::Arc<carosign_adapters::clock::StepClock>,
    pub(super) scheduler: std::sync::Arc<std::sync::Mutex<carosign_runtime::scheduler::Scheduler>>,
    pub(super) subject: SubjectRef,
    pub(super) authenticator: AuthenticatorId,
    pub(super) expected_pid: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

impl VerificationDispatcher for Dispatcher<'_> {
    fn dispatch<'a>(
        &'a self,
        connection: &'a crate::server::Connection,
        handle: crate::operations::ConnectionHandle,
        request: &'a ClientRequest,
        request_id: &'a [u8],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Ended>> + 'a>> {
        Box::pin(async move {
            assert_eq!(
                connection.peer.pid,
                self.expected_pid.load(std::sync::atomic::Ordering::Acquire)
            );
            // SAFETY: Getuid/getgid has no preconditions.
            assert_eq!(connection.peer.uid, unsafe { libc::getuid() });
            assert_eq!(connection.peer.gid, unsafe { libc::getgid() });
            let caller = &self.authority.caller;
            assert_eq!(
                connection.session.connection(),
                *caller.transaction().as_bytes()
            );
            let domain = carosign_linux_ipc::mapping::client::to_domain(request)
                .unwrap()
                .unwrap();
            let prepared = self
                .service
                .prepare_client_verification(caller, &domain, self.authority, DeviceEpoch::FIRST)
                .await
                .unwrap();
            let GrantScope::Verify { purpose, .. } = *prepared.grant.scope() else {
                panic!("не область проверки")
            };
            let target = VerificationFlowTarget {
                worker: WorkerInstanceId::from_bytes([1; 16]).unwrap(),
                command: CommandId::from_bytes([2; 16]).unwrap(),
                authenticator: AuthenticatorRef {
                    id: self.authenticator,
                    device_epoch: DeviceEpoch::FIRST,
                },
                provider_subject: self.subject,
                uid: 1000,
                purpose,
                challenge: carosign_evidence::recipient::ChallengeRef::accepted_by_recipient(
                    [22; 16],
                )
                .unwrap(),
                recipient: carosign_evidence::recipient::RecipientRef::new([23; 16]).unwrap(),
            };
            let binding = crate::operations::TrustedCallerBinding::fixture([42; 32]);
            let mut flow = VerificationFlow::admit(
                self.service,
                &prepared,
                Some(&binding),
                handle,
                8,
                self.registry.clone(),
                self.clock.clone(),
                std::sync::Arc::new(crate::control_watch::ControlOwner::default()),
                self.scheduler.clone(),
                target,
                Duration::from_secs(5),
            )
            .unwrap();
            let accepted = crate::answer::encode(&flow.accepted_reply().unwrap(), request_id);
            crate::server::write_frame(connection, &accepted).map_err(Ended::Broken)?;
            let sockets = carosign_linux::sys::socketpair_seqpacket().unwrap();
            let (transport, worker, mut process) = prepare_flow_provider(sockets, false);
            let result = flow
                .run(
                    self.service,
                    &prepared,
                    Some(transport),
                    connection,
                    &Cleanup,
                    Duration::from_secs(2),
                    std::future::pending,
                )
                .await;
            assert!(matches!(result, Ok(FlowProgress::Sent)), "итог: {result:?}");
            if let Some(worker) = worker {
                worker.join().unwrap();
            }
            if let Some(child) = process.as_mut() {
                assert!(
                    child
                        .wait_bounded(Duration::from_secs(5))
                        .unwrap()
                        .success()
                );
            }
            Ok(())
        })
    }
}
