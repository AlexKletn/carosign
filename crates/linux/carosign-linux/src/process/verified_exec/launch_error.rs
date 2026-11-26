use core::fmt;

/// What's wrong?
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LaunchError {
    /// The catalogue is not at a trusted root.
    UntrustedRoot,
    /// Not an ordinary file (link? device? directory?).
    NotRegularFile,
    /// The owner is not a root.
    NotRootOwned,
    /// Strangers are texting.
    WorldWritable,
    /// The manifesto didn't pass.
    BadManifest,
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UntrustedRoot => "каталог не под доверенным корнем",
            Self::NotRegularFile => "исполняемый файл не обычный файл",
            Self::NotRootOwned => "владелец файла не корень",
            Self::WorldWritable => "файл доступен на запись посторонним",
            Self::BadManifest => "манифест не прошёл проверку",
        })
    }
}

impl core::error::Error for LaunchError {}
