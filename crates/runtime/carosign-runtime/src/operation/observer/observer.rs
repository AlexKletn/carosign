/// One observer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Observer {
    /// Subscription number (unique inside the operation)
    pub id: u32,
    /// What consistency does he want to get?
    pub after_sequence: u64,
}
