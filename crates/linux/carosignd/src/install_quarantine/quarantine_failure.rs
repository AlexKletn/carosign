use super::PublicationFailure;

#[derive(Debug)]
pub(crate) enum QuarantineFailure {
    Preparation,
    InvalidFingerprint,
    Storage(PublicationFailure),
}
