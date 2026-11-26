use core::fmt;

/// The set is not suitable: empty, longer than 8 or with a repeated resource.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct InvalidResourceSet;

impl fmt::Debug for InvalidResourceSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InvalidResourceSet")
    }
}

impl fmt::Display for InvalidResourceSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("набор ресурсов: пуст, больше 8 или ресурс повторён")
    }
}

impl core::error::Error for InvalidResourceSet {}
