use core::fmt;

/// The link is not suitable: longer than 4096 bytes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct InvalidOpaqueRef;

impl fmt::Debug for InvalidOpaqueRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InvalidOpaqueRef")
    }
}

impl fmt::Display for InvalidOpaqueRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("непрозрачная ссылка провайдера: длиннее 4096 байт")
    }
}

impl core::error::Error for InvalidOpaqueRef {}
