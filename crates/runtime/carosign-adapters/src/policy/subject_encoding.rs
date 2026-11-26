use carosign_core::{
    identity::SubjectRef,
    ids::{AuthorityId, SubjectId},
};

fn subject_bytes(subject: u64) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    bytes[8..].copy_from_slice(&subject.to_be_bytes());
    bytes[0] = 1;
    bytes
}

pub(super) fn local_subject(reference: SubjectRef, authority: AuthorityId) -> Option<u64> {
    if reference.authority != authority {
        return None;
    }
    let bytes = reference.subject.as_bytes();
    let number = u64::from_be_bytes(bytes[8..].try_into().ok()?);
    (subject_bytes(number) == *bytes).then_some(number)
}

/// The subject by number is out of sheer transformation, not out of hunch.
pub fn subject_of(number: u64) -> SubjectId {
    SubjectId::from_bytes(subject_bytes(number)).expect("номер не нулевой")
}
