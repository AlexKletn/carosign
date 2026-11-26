use prost::Message;

use super::PlanChange;

#[derive(Clone, PartialEq, Message)]
pub struct PlanBinding {
    #[prost(bytes = "vec", tag = "1")]
    pub planned_enrollment: Vec<u8>,
    #[prost(uint64, tag = "2")]
    pub generation: u64,
    #[prost(bytes = "vec", tag = "3")]
    pub plan_digest: Vec<u8>,
    #[prost(enumeration = "PlanChange", tag = "4")]
    pub change: i32,
    #[prost(bytes = "vec", tag = "5")]
    pub old_enrollment: Vec<u8>,
    #[prost(uint64, tag = "6")]
    pub old_generation: u64,
}
