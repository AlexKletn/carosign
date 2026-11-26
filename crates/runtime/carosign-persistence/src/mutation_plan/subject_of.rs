use carosign_core::identity::SubjectRef;
use carosign_core::ids::SubjectId;

/// Subject to check (so as not to pull extra types).
pub fn subject_of(authority: [u8; 16], subject: [u8; 16]) -> SubjectRef {
    SubjectRef {
        authority: carosign_core::ids::AuthorityId::from_bytes(authority).expect("ненулевой"),
        subject: SubjectId::from_bytes(subject).expect("ненулевой"),
    }
}
