/// Image verification is not a migration authorization or backup confirmation; unsupported
/// data is not deleted or bypassed.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PreflightFailure {
    Corrupt,
    ActiveMutation,
    UnsupportedRecords,
}
