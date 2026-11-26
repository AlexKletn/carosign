//! ISP test process, it doesn't work with devices, it doesn't give out trusted evidence.

mod enrollments;
mod packet_channel;
mod provider_frame;
mod scenario;
mod session;
#[cfg(feature = "ssh-mock")]
mod ssh_decision;

use std::os::fd::RawFd;

use scenario::Scenario;
use session::run;

#[cfg(test)]
use {
    carosign_protocol::carosign::common::v1 as common,
    enrollments::{MockEnrollments, load_mock_state, save_mock_state},
    packet_channel::{receive, validate_channel},
    session::valid_verification,
};

fn main() {
    let mut args = std::env::args().skip(1);
    let result = args
        .next()
        .ok_or("expected inherited socket descriptor".to_string())
        .and_then(|arg| {
            arg.parse::<RawFd>()
                .map_err(|_| "invalid socket descriptor".to_string())
        })
        .and_then(|fd| {
            let scenario = match args.next().as_deref() {
                #[cfg(feature = "ssh-mock")]
                None => Scenario::SshMock,
                #[cfg(not(feature = "ssh-mock"))]
                None => Scenario::NoSample,
                Some("no-sample") => Scenario::NoSample,
                Some("matched") => Scenario::Matched,
                Some("matched-scope") => Scenario::MatchedScope,
                Some("seeded-matched-scope") => Scenario::SeededMatchedScope,
                Some("matched-token") => Scenario::MatchedToken,
                Some("seeded-no-sample") => Scenario::SeededNoSample,
                Some("seeded-matched-token") => Scenario::SeededMatchedToken,
                Some("pending-cancel") => Scenario::PendingCancel,
                Some("seeded-pending-cancel") => Scenario::SeededPendingCancel,
                Some("linux-no-sample") => Scenario::LinuxNoSample,
                Some("linux-matched-scope") => Scenario::LinuxMatchedScope,
                Some("linux-pending-cancel") => Scenario::LinuxPendingCancel,
                Some("linux-persistent") => Scenario::LinuxPersistent,
                Some("linux-pending-mutation") => Scenario::LinuxPendingMutation,
                Some("linux-pending-rollback") => Scenario::LinuxPendingRollback,
                Some("linux-error-rollback") => Scenario::LinuxErrorRollback,
                Some("linux-lost-rollback") => Scenario::LinuxLostRollback,
                Some("linux-committed-rollback") => Scenario::LinuxCommittedRollback,
                _ => return Err("invalid scenario".into()),
            };
            if args.next().is_some() {
                return Err("unexpected arguments".into());
            }
            if fd >= 3 {
                run(fd, scenario)
            } else {
                Err("invalid socket descriptor".into())
            }
        });
    if let Err(error) = result {
        eprintln!("mock provider: {error}");
        std::process::exit(1);
    }
}

#[path = "main.tests.rs"]
#[cfg(test)]
mod tests;
