/// Limit of simultaneously held operations of this instance of service.
pub const OPERATIONS_MAX: usize = 256;
/// The time limit for the caller transaction; it's not counting connections.
pub const OPERATIONS_PER_OWNER: usize = 4;
/// The default connection limit; increasing requires a separate solution.
pub(crate) const DEFAULT_CONNECTIONS: usize = 32;
