//! Discussion `package.toml`.
//!
//! The TOML syntax is disassembled by the library; unfamiliar fields, repeat IDs, and
//! dangerous paths are rejected by schema verification.
//!
//! ⚠️ Executable file path - **relative**, no `..` And without the lead slash, otherwise, the
//! packet might have declared `/bin/sh`.

mod instance_model;
mod limits;
mod manifest;
mod manifest_error;
mod package_section;
mod profile_entry;
mod protocol_section;
mod relative_path;
mod worker_section;

pub use instance_model::InstanceModel;
pub use limits::PACKAGE_ID_MAX;
pub use limits::SCHEMA;
pub use manifest::Manifest;
pub use manifest::parse;
pub use manifest_error::ManifestError;
pub use package_section::PackageSection;
pub use profile_entry::ProfileEntry;
pub use protocol_section::ProtocolSection;
pub use relative_path::check_relative_path;
pub use worker_section::WorkerSection;
