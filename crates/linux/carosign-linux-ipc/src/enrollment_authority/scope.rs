use carosign_runtime::enrollment_authorization::EnrollmentAuthorizationScope;
use std::io;

pub(super) fn canonical_scope(scope: &EnrollmentAuthorizationScope) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(200);
    for value in [
        scope.operation.as_bytes(),
        scope.mutation.as_bytes(),
        scope.challenge_command.as_bytes(),
        scope.command.as_bytes(),
        scope.subject.authority.as_bytes(),
        scope.subject.subject.as_bytes(),
        scope.authenticator.id.as_bytes(),
        scope.recipient.as_bytes(),
        scope.worker.as_bytes(),
        scope.lease.as_bytes(),
        scope.boot.as_bytes(),
    ] {
        bytes.extend_from_slice(value);
    }
    bytes.extend_from_slice(&scope.uid.to_be_bytes());
    bytes.extend_from_slice(&scope.authenticator.device_epoch.value().to_be_bytes());
    bytes
}

pub(super) fn parse_scope(bytes: &[u8]) -> io::Result<EnrollmentAuthorizationScope> {
    use carosign_core::device::{AuthenticatorRef, DeviceEpoch};
    use carosign_core::identity::SubjectRef;
    use carosign_core::ids::{
        AuthenticatorId, AuthorityId, BootId, CommandId, LeaseId, MutationId, OperationId,
        SubjectId, WorkerInstanceId,
    };
    use carosign_runtime::enrollment_authorization::EnrollmentRecipientId;
    if bytes.len() != 188 {
        return Err(io::Error::other("неверная длина области"));
    }
    let mut pos = 0;
    let mut next = || {
        let value: [u8; 16] = bytes[pos..pos + 16].try_into().expect("длина проверена");
        pos += 16;
        value
    };
    macro_rules! id { ($ty:ty) => { <$ty>::from_bytes(next()).map_err(|_| io::Error::other("неверный идентификатор"))? }; }
    let operation = id!(OperationId);
    let mutation = id!(MutationId);
    let challenge_command = id!(CommandId);
    let command = id!(CommandId);
    let authority = id!(AuthorityId);
    let subject = id!(SubjectId);
    let authenticator = id!(AuthenticatorId);
    let recipient = EnrollmentRecipientId::from_bytes(next())
        .map_err(|_| io::Error::other("неверный получатель"))?;
    let worker = id!(WorkerInstanceId);
    let lease = id!(LeaseId);
    let boot = id!(BootId);
    let uid = u32::from_be_bytes(bytes[176..180].try_into().expect("длина проверена"));
    let epoch = u64::from_be_bytes(bytes[180..188].try_into().expect("длина проверена"));
    let device_epoch =
        DeviceEpoch::new(epoch).map_err(|_| io::Error::other("неверное поколение"))?;
    Ok(EnrollmentAuthorizationScope {
        operation,
        mutation,
        challenge_command,
        command,
        subject: SubjectRef { authority, subject },
        uid,
        authenticator: AuthenticatorRef {
            id: authenticator,
            device_epoch,
        },
        recipient,
        worker,
        lease,
        boot,
    })
}
