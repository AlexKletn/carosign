//! Domain: Values, identifiers and invariants. Types only and clean checks.
//!
//! ⚠️ This crate doesn't know anything about files, time, sockets and Linux, everything that
//! requires a system comes from the outside through ports, and it's done on purpose: the
//! domain is checked on the host, without a tablet, and without trust in the environment.
//!
//! Device and justification. `docs/specs/engineering/implementation-v06/03-domain-types.md`.

#![forbid(unsafe_code)]

pub mod assurance;
pub mod authorization;
pub mod capabilities;
pub mod device;
pub mod enrollment;
pub mod error;
pub mod evidence;
pub mod identity;
pub mod ids;
pub mod modality;
pub mod operation;
pub mod provider;
pub mod resource;
