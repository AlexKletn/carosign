use std::{
    error::Error,
    io,
    path::{Path, PathBuf},
    process::Command,
};

use carosign_linux::policy::{PolicyWatchState, ProtectedPolicySnapshot};
use carosignd::{config::Config, provider_supervisor::ProviderProfile};

pub(super) struct Configuration<'a> {
    pub(super) service: &'a Path,
}

impl<'a> Configuration<'a> {
    pub(super) fn new(service: &'a Path) -> Self {
        Self { service }
    }

    pub(super) fn load(&self) -> Result<Config, Box<dyn Error>> {
        Ok(Config::parse(&read(self.service)?)?)
    }

    pub(super) fn path(&self, target: &str, args: &[String]) -> Result<PathBuf, Box<dyn Error>> {
        let path = match (target, args) {
            ("service", []) => self.service.to_owned(),
            ("policy", []) => self.load()?.policy.into(),
            ("providers", []) => carosignd::config::DEFAULT_PROVIDER_ALLOWLIST.into(),
            ("profile", [path]) if Path::new(path).is_absolute() => PathBuf::from(path),
            ("profile", []) => self
                .load()?
                .provider_profile
                .ok_or("Specify an absolute execution profile path")?
                .into(),
            _ => {
                return Err(
                    "Expected service, policy, providers or profile [/absolute/path]".into(),
                );
            }
        };
        Ok(path)
    }

    pub(super) fn check(&self, target: &str, path: &Path) -> Result<(), Box<dyn Error>> {
        match target {
            "service" => {
                Config::parse(&read(path)?)?;
            }
            "policy" => {
                carosign_adapters::policy::parse(&read(path)?)?;
            }
            "providers" => carosignd::config::validate_provider_catalog(&read(path)?)?,
            "profile" => {
                ProviderProfile::open(path)?;
            }
            _ => return Err("Unknown configuration file type".into()),
        }
        Ok(())
    }

    pub(super) fn edit(&self, target: &str, path: &Path) -> Result<(), Box<dyn Error>> {
        let status = Command::new("/usr/bin/sudoedit")
            .arg("--")
            .arg(path)
            .status()?;
        if !status.success() {
            return Err(format!("sudoedit exited with {status}").into());
        }
        self.check(target, path)?;
        println!(
            "Saved and validated {}. Restart carosignd to apply changes.",
            path.display()
        );
        Ok(())
    }
}

pub(super) fn read(path: &Path) -> io::Result<String> {
    let (bytes, mut watch) = ProtectedPolicySnapshot::open(path)?.into_parts();
    if watch.check()? != PolicyWatchState::Unchanged {
        return Err(io::Error::other("Configuration changed while reading"));
    }
    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}
