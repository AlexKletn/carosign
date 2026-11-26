use std::process::Command;

use crate::ipc::{policy_admin, system_accounts};

use super::PolicyResult;

pub(in crate::ui) enum PolicyJob {
    Read,
    RestartService,
    Write { expected: String, proposed: String },
}

impl PolicyJob {
    pub(in crate::ui) fn run(self) -> Result<PolicyResult, &'static str> {
        match self {
            Self::RestartService => {
                let status = Command::new("/usr/bin/systemctl")
                    .args(["restart", "carosignd.service"])
                    .output()
                    .map_err(|_| "ui-policy-restart-failed")?
                    .status;
                if status.success() {
                    Ok(PolicyResult::ServiceRestarted)
                } else {
                    Err("ui-policy-restart-failed")
                }
            }
            Self::Read => {
                let document = policy_admin::read()?;
                let accounts = system_accounts::read().ok();
                Ok(PolicyResult::Loaded { document, accounts })
            }
            Self::Write { expected, proposed } => {
                policy_admin::write(&expected, &proposed)?;
                Ok(PolicyResult::Saved(proposed))
            }
        }
    }
}
