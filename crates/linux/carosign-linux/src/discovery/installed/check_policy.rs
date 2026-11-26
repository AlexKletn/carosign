use super::InstallationError;
use super::InstallationKey;
use super::PolicyAuthority;
use std::os::fd::OwnedFd;

/// Verifies the policy with an independently approved record and a fixed file.
/// Runtime files are not supported yet: you should not pass them off as verified.
fn check_policy(
    authority: &dyn PolicyAuthority,
    key: InstallationKey,
    policy: &super::super::supervisor_policy::SupervisorPolicy,
    executable: &OwnedFd,
) -> Result<(), InstallationError> {
    use std::os::fd::AsFd;
    let approved = authority.approved_policy(key)?;
    if approved.package != policy.package
        || approved.version != policy.version
        || approved.profile != policy.profile
        || approved.executable != policy.executable
        || !policy.runtime.is_empty()
    {
        return Err(InstallationError::MismatchedRecord);
    }
    match super::super::verity::matches_approved_digest(
        executable.as_fd(),
        &approved.executable_verity,
    ) {
        Ok(true) => Ok(()),
        Ok(false) => Err(InstallationError::MismatchedRecord),
        Err(_) => Err(InstallationError::Unavailable),
    }
}
