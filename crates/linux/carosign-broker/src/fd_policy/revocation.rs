/// How can you cut off access?
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Revocation {
    /// Logical withdrawal of leasing access **not ** terminates.
    ///
    /// ⚠️ The only honest answer is that only the completion of the process stops, and the
    /// security of the device is a separate evidence.
    OnlyByTermination,
    /// There is no access at all (nothing issued).
    NotApplicable,
}
