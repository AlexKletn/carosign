/// The plan is not consistent with the area or form of transition.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlanError {
    /// Pictures are above the regulatory limit.
    TooManyBindings,
    /// The order is not canonical or the pictures are repeated.
    NonCanonicalBindings,
    /// Registration or entity outside the field.
    OutOfScope,
    /// The format does not conform to the limitation.
    InvalidFormat,
    /// The old target is missing or the new number is already taken.
    InvalidTransition,
}
