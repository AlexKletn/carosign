/// The result of checking only known types of records, the image can not be used after it is
/// closed as evidence of the immutability of the base.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PreflightSummary {
    pub(crate) packages: usize,
    pub(crate) records: usize,
}
