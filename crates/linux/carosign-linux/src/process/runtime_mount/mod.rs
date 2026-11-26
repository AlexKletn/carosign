//! Attach one runtime file to an unpublished root.
//!
//! The source transmits the trusted composition; this mechanism does not certify the package,
//! does not check its contents, and does not translate the process into a new root.

#![allow(dead_code)]

mod attach_file;
mod attach_tree_inner;
mod clone_source;
mod mount_attr;
mod mount_error;
mod mount_input;
mod mount_stage;

pub(crate) use attach_file::attach_file;
#[cfg(test)]
pub(crate) use attach_file::attach_file_for_identity;
pub(crate) use attach_tree_inner::attach_tree_for_identity;
use attach_tree_inner::{
    MOUNT_ATTR_NODEV, MOUNT_ATTR_NOSUID, MOUNT_ATTR_RDONLY, attach_tree_inner, error,
};
pub(crate) use clone_source::clone_source;
use mount_attr::MountAttr;
pub(crate) use mount_error::MountError;
use mount_input::MountInput;
pub(crate) use mount_stage::MountStage;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
