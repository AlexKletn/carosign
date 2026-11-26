/// The stage at which the kernel failed or the result was not confirmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrivilegeStage {
    ValidateIdentity,
    ClearGroups,
    SetGroup,
    SetUser,
    ClearAmbient,
    ClearCapabilities,
    VerifyIdentity,
    VerifyGroups,
    VerifyCapabilities,
    SetNoNewPrivileges,
    VerifyNoNewPrivileges,
}
