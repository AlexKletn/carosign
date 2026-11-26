use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use carosign_adapters::policy;
use tempfile::NamedTempFile;

use super::{Configuration, configuration};

const MAX_POLICY_BYTES: usize = 64 * 1024;

pub(super) fn run(arguments: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    // SAFETY: geteuid has no inputs and does not access caller memory.
    if unsafe { libc::geteuid() } != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "administrator privileges required",
        )
        .into());
    }
    let config = Configuration::new(Path::new("/etc/carosign/carosignd.conf"));
    let path = config.path("policy", &[])?;
    match arguments {
        [command] if command == "read" => {
            io::stdout()
                .lock()
                .write_all(configuration::read(&path)?.as_bytes())?;
        }
        [command] if command == "write" => {
            let mut input = io::stdin().lock();
            let expected = read_document(&mut input)?;
            let proposed = read_document(&mut input)?;
            if input.read(&mut [0])? != 0 {
                return Err(
                    io::Error::new(io::ErrorKind::InvalidData, "trailing policy data").into(),
                );
            }
            policy::parse(&proposed)?;
            if !replace(&path, &expected, &proposed)? {
                io::stdout().lock().write_all(b"conflict")?;
                return Ok(());
            }
            io::stdout().lock().write_all(b"saved")?;
        }
        _ => return Err("expected policy-admin read or write".into()),
    }
    Ok(())
}

fn read_document(input: &mut impl Read) -> io::Result<String> {
    let mut length = [0; 4];
    input.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    if length > MAX_POLICY_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "policy is too large",
        ));
    }
    let mut bytes = vec![0; length];
    input.read_exact(&mut bytes)?;
    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub(super) fn replace(path: &Path, expected: &str, proposed: &str) -> io::Result<bool> {
    configuration::read(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing policy directory"))?;
    let directory = File::open(parent)?;
    directory.lock()?;
    if configuration::read(path)? != expected {
        return Ok(false);
    }
    if proposed == expected {
        return Ok(true);
    }
    let mut file = NamedTempFile::new_in(parent)?;
    file.write_all(proposed.as_bytes())?;
    file.as_file().sync_all()?;
    if configuration::read(path)? != expected {
        return Ok(false);
    }
    file.persist(path)?;
    directory.sync_all()?;
    Ok(true)
}

#[path = "policy_admin.tests.rs"]
#[cfg(test)]
mod tests;
