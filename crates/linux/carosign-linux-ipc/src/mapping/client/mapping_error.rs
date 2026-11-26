/// Display failed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MappingError {
    /// Identifier is not 16 bytes or zero.
    BadId,
}

impl core::fmt::Display for MappingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("идентификатор не 16 байт или нулевой")
    }
}

impl core::error::Error for MappingError {}
