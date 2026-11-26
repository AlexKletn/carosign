use core::fmt;

/// The worker gave a second result on one command.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DuplicateTerminal;

impl fmt::Debug for DuplicateTerminal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DuplicateTerminal")
    }
}

impl fmt::Display for DuplicateTerminal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("второй итог по одной команде")
    }
}

impl core::error::Error for DuplicateTerminal {}
