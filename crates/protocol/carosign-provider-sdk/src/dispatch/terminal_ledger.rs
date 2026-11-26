use super::DuplicateTerminal;

/// Accounting for the results.
#[derive(Debug, Default)]
pub struct TerminalLedger {
    terminals: u32,
}

impl TerminalLedger {
    /// Celebrates the outcome. The second outcome is rejection.
    pub fn record(&mut self) -> Result<(), DuplicateTerminal> {
        if self.terminals > 0 {
            return Err(DuplicateTerminal);
        }
        self.terminals += 1;
        Ok(())
    }

    /// How many results have been issued.
    pub fn count(&self) -> u32 {
        self.terminals
    }
}
