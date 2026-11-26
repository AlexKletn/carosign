//! Namespaces of a separate launch assistant.
//!
//! The creation of spaces itself does not limit the visible files, nor is it a sandbox. A
//! partial change error requires the completion of the assistant. Code cannot be invoked in a
//! multithreaded demon.

#![allow(dead_code)]

mod enter;
mod namespace_error;
mod namespace_stage;

pub(crate) use enter::enter;
pub(crate) use namespace_error::NamespaceError;
pub(crate) use namespace_stage::NamespaceStage;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
