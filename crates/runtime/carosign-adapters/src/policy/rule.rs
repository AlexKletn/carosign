use super::action::Action;
use carosign_core::operation::context::VerificationPurpose;

/// The line of politics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    /// UID defiant.
    pub uid: u32,
    /// Who is the subject (the number that the source recognizes in the entry system).
    pub subject: u64,
    /// What can I do?
    pub actions: Vec<Action>,
    /// How long is the permit, seconds.
    pub lifetime_s: u64,
    /// The purpose of the audit is set by the policy and not by the customer.
    pub verification_purpose: Option<VerificationPurpose>,
}
