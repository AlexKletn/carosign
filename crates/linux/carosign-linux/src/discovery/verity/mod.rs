//! Verification of immutable content through fs-verity.
//!
//! Measurement by itself does not prove origin: the expected value must come from a
//! system-verified installer. The source of that value and the launch resolution are not
//! created here.
#![allow(dead_code)]

mod digest;
mod enable_and_measure;
mod enable_arg;
mod measure;

use digest::Digest;
pub(super) use enable_and_measure::enable_and_measure;
use enable_arg::EnableArg;
#[cfg(test)]
use measure::MEASURE;
pub(crate) use measure::matches_approved_digest;
use measure::{SHA256, measure};

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
