use super::CapabilityData;
use super::CapabilityHeader;
use super::PrivilegeError;
use super::PrivilegeStage;
use std::io;

const LINUX_CAPABILITY_VERSION_3: u32 = 0x2008_0522;

fn failure(stage: PrivilegeStage) -> PrivilegeError {
    PrivilegeError {
        stage,
        source: io::Error::last_os_error(),
    }
}

/// Resets additional groups and current powers of an individual assistant.
/// The caller must complete the process if any failure occurs: part of the steps may have
/// already been executed. This does not clear the bounding set or confirm the full sandbox.
pub(crate) fn drop_identity(uid: libc::uid_t, gid: libc::gid_t) -> Result<(), PrivilegeError> {
    if uid == 0 || gid == 0 || uid == libc::uid_t::MAX || gid == libc::gid_t::MAX {
        return Err(PrivilegeError {
            stage: PrivilegeStage::ValidateIdentity,
            source: io::Error::from(io::ErrorKind::InvalidInput),
        });
    }
    // SAFETY: Zero length does not require an array of groups.
    if unsafe { libc::setgroups(0, std::ptr::null()) } != 0 {
        return Err(failure(PrivilegeStage::ClearGroups));
    }
    // SAFETY: All three values define the chosen identity of the process.
    if unsafe { libc::setresgid(gid, gid, gid) } != 0 {
        return Err(failure(PrivilegeStage::SetGroup));
    }
    // SAFETY: All three values define the chosen identity of the process.
    if unsafe { libc::setresuid(uid, uid, uid) } != 0 {
        return Err(failure(PrivilegeStage::SetUser));
    }
    // SAFETY: Clear the ambient-set of the current process without pointers.
    if unsafe {
        libc::prctl(
            libc::PR_CAP_AMBIENT,
            libc::PR_CAP_AMBIENT_CLEAR_ALL,
            0,
            0,
            0,
        )
    } != 0
    {
        return Err(failure(PrivilegeStage::ClearAmbient));
    }
    let mut header = CapabilityHeader {
        version: LINUX_CAPABILITY_VERSION_3,
        pid: 0,
    };
    let mut data = [CapabilityData::default(); 2];
    // SAFETY: The layouts and length correspond to Linux capability v3, pid=0 is the current
    // stream.
    if unsafe { libc::syscall(libc::SYS_capset, &mut header, data.as_mut_ptr()) } != 0 {
        return Err(failure(PrivilegeStage::ClearCapabilities));
    }
    let mut user = 0;
    let mut effective_user = 0;
    let mut saved_user = 0;
    let mut group = 0;
    let mut effective_group = 0;
    let mut saved_group = 0;
    // SAFETY: All addresses indicate independent values of the corresponding type.
    if unsafe { libc::getresuid(&mut user, &mut effective_user, &mut saved_user) } != 0
        || unsafe { libc::getresgid(&mut group, &mut effective_group, &mut saved_group) } != 0
        || [user, effective_user, saved_user] != [uid; 3]
        || [group, effective_group, saved_group] != [gid; 3]
    {
        return Err(PrivilegeError {
            stage: PrivilegeStage::VerifyIdentity,
            source: io::Error::other("идентичность не подтверждена"),
        });
    }
    // SAFETY: The call requests only the list size and does not write an array.
    if unsafe { libc::getgroups(0, std::ptr::null_mut()) } != 0 {
        return Err(PrivilegeError {
            stage: PrivilegeStage::VerifyGroups,
            source: io::Error::other("дополнительные группы остались"),
        });
    }
    // SAFETY: The kernel fills in two elements of capability v3.
    if unsafe { libc::syscall(libc::SYS_capget, &mut header, data.as_mut_ptr()) } != 0 {
        return Err(failure(PrivilegeStage::VerifyCapabilities));
    }
    if data
        .iter()
        .any(|c| c.effective != 0 || c.permitted != 0 || c.inheritable != 0)
    {
        return Err(PrivilegeError {
            stage: PrivilegeStage::VerifyCapabilities,
            source: io::Error::other("полномочия остались"),
        });
    }
    Ok(())
}
