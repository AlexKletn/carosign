//! The Linux part of the platform: device inventory, policies, processes, resources.
//! Doesn't know the domain above `core`.

pub mod discovery;
pub mod identity;
pub mod ids;
pub mod inventory;
pub mod paths;
pub mod policy;
pub mod process;
pub mod resources;
pub mod sys;
pub mod time;
