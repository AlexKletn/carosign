/// Registry registration status (coinciding with `carosign.storage.v1.EnrollmentStatus`).
///
/// ⚠️ The sign “waiting for reconciliation” is **a separate status**, not the inference that
/// the record is turned off: the mixing of these two has already once led to a failure where
/// it was necessary to refuse for another reason.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Status {
    /// It works.
    Active,
    /// Waiting for reconciliation: you can not check until the reconciliation has not proved
    /// otherwise.
    PendingReconciliation,
    /// Unavailable (provider removed and similar).
    Unavailable,
}
