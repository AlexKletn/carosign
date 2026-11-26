/// The largest set that is sent to the provider.
///
/// ⚠️ The limit of the scheme (`permitted_enrollments` 1..configured max. More of a
/// rejection, not a "default": a cropped set would take away a person's registration.
pub const BINDINGS_MAX: usize = 32;
