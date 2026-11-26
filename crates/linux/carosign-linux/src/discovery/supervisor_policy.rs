//! Analysis of administrative policy: data are not proof of installation.
#![allow(dead_code)]

use std::collections::BTreeSet;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SupervisorPolicy {
    pub package: String,
    pub version: String,
    pub profile: String,
    pub architecture: String,
    pub executable: String,
    pub uid: u32,
    pub gid: u32,
    pub memory_bytes: u64,
    pub cpu_quota_us: u64,
    pub cpu_period_us: u64,
    pub processes: u64,
    pub runtime: Vec<String>,
    pub syscalls: Vec<u32>,
}

fn path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 1024
        && value.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part.len() <= 255
                && !part.bytes().any(|b| b == 0 || b.is_ascii_control())
        })
}

fn ident(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b".-_".contains(&b))
}

impl SupervisorPolicy {
    /// Strict TOML v1 parsing; the result remains unverified data.
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() > 65536 {
            return Err("size");
        }
        let text = std::str::from_utf8(bytes).map_err(|_| "utf8")?;
        let mut t = text.parse::<toml::Table>().map_err(|_| "toml")?;
        fn string(t: &mut toml::Table, k: &str) -> Result<String, &'static str> {
            t.remove(k)
                .and_then(|v| v.as_str().map(str::to_owned))
                .ok_or("field")
        }
        fn number(t: &mut toml::Table, k: &str) -> Result<u64, &'static str> {
            t.remove(k)
                .and_then(|v| v.as_integer())
                .and_then(|v| u64::try_from(v).ok())
                .filter(|v| *v > 0)
                .ok_or("field")
        }
        if number(&mut t, "schema")? != 1 {
            return Err("schema");
        }
        let package = string(&mut t, "package")?;
        let version = string(&mut t, "version")?;
        let profile = string(&mut t, "profile")?;
        let architecture = string(&mut t, "architecture")?;
        let executable = string(&mut t, "executable")?;
        let uid = u32::try_from(number(&mut t, "uid")?).map_err(|_| "uid")?;
        let gid = u32::try_from(number(&mut t, "gid")?).map_err(|_| "gid")?;
        let memory_bytes = number(&mut t, "memory_bytes")?;
        let cpu_quota_us = number(&mut t, "cpu_quota_us")?;
        let cpu_period_us = number(&mut t, "cpu_period_us")?;
        let processes = number(&mut t, "processes")?;
        let runtime = t
            .remove("runtime")
            .and_then(|v| v.as_array().cloned())
            .ok_or("runtime")?
            .into_iter()
            .map(|v| v.as_str().map(str::to_owned).ok_or("runtime"))
            .collect::<Result<Vec<_>, _>>()?;
        let syscalls = t
            .remove("syscalls")
            .and_then(|v| v.as_array().cloned())
            .ok_or("syscalls")?
            .into_iter()
            .map(|v| {
                v.as_integer()
                    .and_then(|n| u32::try_from(n).ok())
                    .filter(|n| *n < 0x4000_0000)
                    .ok_or("syscalls")
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !t.is_empty()
            || carosign_core::provider::PackageName::new(&package)
                .map_or(true, |name| name.as_str() != package)
            || !matches!(architecture.as_str(), "x86_64" | "aarch64")
            || architecture != std::env::consts::ARCH
            || !ident(&profile)
            || version.is_empty()
            || version.len() > 128
            || version
                .bytes()
                .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
            || !path(&executable)
            || !(1000..=1_000_000).contains(&cpu_period_us)
            || runtime.len() > 128
            || runtime.iter().any(|p| !path(p))
            || runtime.iter().collect::<BTreeSet<_>>().len() != runtime.len()
            || syscalls.is_empty()
            || syscalls.len() > 128
            || syscalls.iter().collect::<BTreeSet<_>>().len() != syscalls.len()
            // The basic profile does not allow you to create new sockets. io_uring It is also
            // prohibited: it allows you to submit a socket creation operation bypassing the
            // verification of a separate socket system call.
            || syscalls.iter().any(|&n| {
                [
                    libc::SYS_socket,
                    libc::SYS_socketpair,
                    libc::SYS_io_uring_setup,
                    libc::SYS_io_uring_enter,
                    libc::SYS_io_uring_register,
                ]
                .contains(&i64::from(n))
            })
        {
            return Err("invalid");
        }
        Ok(Self {
            package,
            version,
            profile,
            architecture,
            executable,
            uid,
            gid,
            memory_bytes,
            cpu_quota_us,
            cpu_period_us,
            processes,
            runtime,
            syscalls,
        })
    }
}

#[path = "supervisor_policy.tests.rs"]
#[cfg(test)]
mod tests;
