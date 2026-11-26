#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub enum PlanChange {
    Unspecified = 0,
    Create = 1,
    Replace = 2,
}
