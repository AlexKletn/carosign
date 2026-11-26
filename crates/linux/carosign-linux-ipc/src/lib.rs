//! Linux seqpacket transport. Peer credentials identify the caller for policy checks.

pub mod client;
pub mod codec;
pub mod credentials;
pub mod enrollment_authority;
pub mod enrollment_authority_adapter;
pub mod fd_slots;
pub mod frame;
pub mod mapping;
pub mod provider;
pub mod provider_rendezvous;

pub use credentials::PeerCredentials;
pub use frame::FrameError;
