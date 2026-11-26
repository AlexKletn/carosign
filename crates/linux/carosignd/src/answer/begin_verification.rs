use super::{
    AuthenticatorId, AuthenticatorRef, IdSource, OperationActor, OwnerRef, Reply, Request,
    SubjectRef, TimeSource, VerificationPurpose, actor_for, common, error_of,
    prepare_authorized_verification,
};

/// Begins inspection at the request of the client.
///
/// ⚠️ The order: collect the request → dial from the registry → authority → actor → admission
/// → steps. Any refusal to create an actor means that the operation did not happen at all.
#[allow(clippy::too_many_arguments)]
pub async fn begin_verification<S, A, I, T>(
    store: &S,
    authority: &A,
    ids: &I,
    clock: &T,
    subject: SubjectRef,
    authenticator: AuthenticatorId,
    epoch: carosign_core::device::DeviceEpoch,
    caller: &carosign_runtime::ports::CallerContext,
) -> Reply
where
    S: carosign_persistence::store::MetadataStore,
    A: carosign_runtime::ports::PolicyAuthority,
    I: IdSource,
    T: TimeSource,
{
    // ⚠️ Recruitment and powers - up to the actor: without them, there is no operation.
    let (_cycle, grant) = match prepare_authorized_verification(
        store,
        authority,
        caller,
        subject,
        authenticator,
        epoch,
    )
    .await
    {
        Ok(prepared) => prepared,
        Err(error) => return Reply::Refused(error_of(&error)),
    };

    let request = Request {
        subject,
        authenticator: AuthenticatorRef {
            id: authenticator,
            device_epoch: epoch,
        },
        purpose: VerificationPurpose::SessionLogin,
        owner: OwnerRef {
            authority: subject.authority,
            // ⚠️ Owner's binding is from the permit, not the transaction number: the
            // transaction number is not entitled (document 08).
            binding: grant.id(),
        },
        deadline_ns: 30_000_000_000,
    };
    let context = match request.context(ids, clock) {
        Ok(context) => context,
        Err(_) => {
            return Reply::Refused(common::Error {
                code: common::ErrorCode::ErrorUnavailable as i32,
                safe_message: "operation unavailable".into(),
            });
        }
    };
    let operation = context.operation_id;
    let boot = context.deadline.boot();
    let instant = context.deadline.instant_ns();

    let mut actor: OperationActor = actor_for(&request, context);
    if crate::serve::admit(&mut actor).is_err() {
        return Reply::Refused(common::Error {
            code: common::ErrorCode::ErrorInternal as i32,
            safe_message: "operation refused".to_string(),
        });
    }

    // ⚠️ The steps are done without the performer: the real effects-provider path requires
    // the ports assembled, and that's the next step, and you can see the
    // request-operation-answer path.
    Reply::Accepted {
        operation,
        boot,
        instant_ns: instant,
    }
}
