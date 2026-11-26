/// Limits of queues (document 22).
pub const CONTROL_CAPACITY: usize = 16;

/// It's a regular line.
pub const NORMAL_CAPACITY: usize = 64;

/// Between the portions of the control is disassembled ** one ** normal message.
pub const CONTROL_BURST: usize = 8;
