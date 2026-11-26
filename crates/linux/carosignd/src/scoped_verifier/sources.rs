use super::{EvidenceStore, IdSource, ScopedChallenges, ScopedTrust, TimeSource};

/// It is only performed with explicitly transmitted independent sources, and the staff does
/// not connect this transition until they are implemented and verified.
pub(crate) struct Sources<'a> {
    pub(crate) trust: &'a dyn ScopedTrust,
    pub(crate) challenges: &'a ScopedChallenges,
    pub(crate) store: &'a EvidenceStore,
    pub(crate) ids: &'a (dyn IdSource + Send + Sync),
    pub(crate) clock: &'a dyn TimeSource,
}
