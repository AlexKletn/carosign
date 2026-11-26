/// How can you cut off access?
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Revocation {
    /// Only by completing the process.
    ///
    /// ⚠️ The only honest answer for an already given writer.
    OnlyByTermination,
    /// The writer did not give up - nothing remained.
    NothingDelegated,
}

/// What's left after we close our copy?
pub fn revocation_after_handoff(delegated: bool) -> Revocation {
    if delegated {
        Revocation::OnlyByTermination
    } else {
        Revocation::NothingDelegated
    }
}
