use super::HelperStage;

/// Refusal after attempting to pass the report on to the parent.
#[derive(Debug)]
pub(crate) struct HelperFailure {
    pub(crate) stage: HelperStage,
    pub(crate) errno: i32,
    pub(crate) report_failed: bool,
}
