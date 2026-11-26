use prost::Oneof;

#[derive(Clone, PartialEq, Oneof)]
pub enum Action {
    #[prost(message, tag = "10")]
    Assess(super::Empty),
    #[prost(message, tag = "11")]
    Validate(super::Validate),
    #[prost(message, tag = "12")]
    Authorize(super::Authorization),
    #[prost(message, tag = "13")]
    Revalidate(super::Authorization),
    #[prost(message, tag = "14")]
    Revoke(super::Empty),
    /// The trustee is responsible after completion of the execution.
    #[prost(message, tag = "15")]
    Execute(super::Authorization),
    #[prost(message, tag = "16")]
    PrepareChallenge(super::Empty),
    #[prost(message, tag = "17")]
    BindOwner(super::OwnerBinding),
    #[prost(message, tag = "18")]
    BindPlan(super::PlanBinding),
}
