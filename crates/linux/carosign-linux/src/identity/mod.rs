//! Peer preview. It's not an authority test.
//!
//! This internal workpiece retains the socket and the kernel link to peer, but does not
//! confirm the systemd unit, installation, D-Bus, or the right of the client. SO_PEERPIDFD
//! Failure; you can not open pidfd by numerical PID as a backup option.

mod observed_peer;
mod systemd_observation;

#[cfg(test)]
pub(crate) use observed_peer::ObservedPeer;
#[cfg(test)]
use observed_peer::matching_invocation;
use systemd_observation::SystemdObservation;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
