/// Departure from silence permitted by man.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Relaxation {
    /// That's weakened.
    pub field: &'static str,
    pub reason: String,
}
