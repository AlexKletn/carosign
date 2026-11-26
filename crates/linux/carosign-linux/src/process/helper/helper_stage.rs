/// A stage where the assistant could not continue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HelperStage {
    /// It was not possible to place the current process in the group.
    Cgroup,
    /// Parental permission has not been obtained.
    Gate,
    /// The ban on increasing privileges has not been established or confirmed.
    NoNewPrivileges,
    /// The space of names or their distribution is not limited.
    Namespaces,
    /// A limited temporal root has not been created.
    TemporaryRoot,
    /// It was not possible to attach the allowed file to the prepared root.
    RuntimeFiles,
    /// Changing the root or closing the descriptors.
    RootSwitch,
    /// Resetting identity and authority.
    Identity,
    /// Installation of the system filter.
    Seccomp,
    /// Launch of the fixed object.
    Exec,
    /// The remaining mandatory restrictions have not yet been established.
    Isolation,
}

impl HelperStage {
    /// The numbers belong only to the assistant's internal reporting channel.
    pub(super) fn wire(self) -> u8 {
        match self {
            Self::Cgroup => 1,
            Self::Gate => 2,
            Self::Isolation => 3,
            Self::NoNewPrivileges => 4,
            Self::Namespaces => 5,
            Self::TemporaryRoot => 6,
            Self::RuntimeFiles => 7,
            Self::RootSwitch => 8,
            Self::Identity => 9,
            Self::Seccomp => 10,
            Self::Exec => 11,
        }
    }
}
