/// Armament failure: Indefinite fixation requires reading a journal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MutationArmFailure {
    /// Verified powers, effects, or resources are not available.
    Unavailable,
    /// The enduring outcome of the fixation is unknown; re-arming is prohibited.
    CommitUnknown,
}
