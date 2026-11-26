use std::io;
use std::os::fd::AsFd;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;

use super::*;
#[cfg(target_arch = "x86_64")]
use std::os::unix::process::CommandExt;

#[test]
fn verifier_is_matched_by_held_inode_not_path_or_contents() {
    use std::io::Write;
    let make = || {
        let raw = unsafe { libc::memfd_create(c"verifier-test".as_ptr(), libc::MFD_CLOEXEC) };
        assert!(raw >= 0);
        let mut file = unsafe { std::fs::File::from_raw_fd(raw) };
        file.write_all(b"same contents").unwrap();
        file
    };
    let first = make();
    let second = make();
    let runtime = PinnedRuntime {
        entries: vec![Entry {
            name: "usr/bin/gpgv".into(),
            source: first.try_clone().unwrap().into(),
        }],
    };
    assert!(
        runtime
            .contains_verifier("usr/bin/gpgv", first.as_fd())
            .unwrap()
    );
    assert!(
        !runtime
            .contains_verifier("usr/bin/gpgv", second.as_fd())
            .unwrap()
    );
    assert!(
        !runtime
            .contains_verifier("usr/bin/other", first.as_fd())
            .unwrap()
    );
}

#[test]
fn only_relative_unambiguous_names_are_accepted() {
    for name in [
        "", "/lib/a", "../a", "lib/../a", "lib//a", "lib/./a", "a\0b",
    ] {
        assert!(!valid_name(name), "{name:?}");
    }
    assert!(valid_name("lib64/ld-linux-aarch64.so.1"));
    assert!(valid_name("usr/lib/libgcrypt.so.20"));
}

#[test]
fn unmeasured_or_nonregular_sources_never_become_runtime() {
    let directory = std::fs::File::open("/").unwrap();
    let file = RuntimeFile {
        name: "lib/loader",
        source: directory.as_fd(),
        approved_verity: [0; 32],
    };
    assert!(matches!(
        PinnedRuntime::prepare(&[file]),
        Err(RuntimeError::InvalidFile)
    ));
}

#[test]
fn system_file_without_approved_measurement_is_rejected() {
    let source = std::fs::File::open("/usr/bin/gpgv").unwrap();
    let file = RuntimeFile {
        name: "usr/bin/gpgv",
        source: source.as_fd(),
        approved_verity: [0; 32],
    };
    // Regardless of fs-verity support, zero unconfirmed measurement cannot turn a file into
    // an approved runtime.
    assert!(PinnedRuntime::prepare(&[file]).is_err());
}

#[test]
#[ignore = "требует user и mount namespace и разрешённое монтирование"]
fn held_file_is_mounted_read_only_in_separate_namespace() {
    const CHILD: &str = "CAROSIGN_SIGNATURE_RUNTIME_MOUNT_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let result = std::process::Command::new("unshare")
                .args(["--user", "--map-root-user", "--mount", "--fork"])
                .arg(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "discovery::signature_runtime::tests::held_file_is_mounted_read_only_in_separate_namespace",
                    "--ignored",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .output()
                .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    }
    // SAFETY: A separate process has its own mount namespace.
    assert_eq!(
        unsafe {
            libc::mount(
                std::ptr::null(),
                c"/".as_ptr(),
                std::ptr::null(),
                libc::MS_REC | libc::MS_PRIVATE,
                std::ptr::null(),
            )
        },
        0
    );
    let path = std::env::temp_dir().join(format!(
        "carosign-signature-runtime-{}",
        std::process::id()
    ));
    std::fs::create_dir(&path).unwrap();
    let root = std::fs::File::open(&path).unwrap();
    let source = std::fs::File::open("/usr/bin/gpgv").unwrap();
    // The fixture only experiences mounting, does not confirm fs-verity.
    let runtime = PinnedRuntime {
        entries: vec![Entry {
            name: "bin/gpgv".into(),
            source: source.into(),
        }],
    };
    runtime.attach(root.as_fd()).unwrap();
    let target = path.join("bin/gpgv");
    assert_eq!(&std::fs::read(&target).unwrap()[..4], b"\x7fELF");
    assert!(
        std::fs::OpenOptions::new()
            .write(true)
            .open(&target)
            .is_err()
    );
    let name = std::ffi::CString::new(target.as_os_str().as_encoded_bytes()).unwrap();
    // SAFETY: Only the mounting created by this process is removed.
    assert_eq!(unsafe { libc::umount2(name.as_ptr(), libc::MNT_DETACH) }, 0);
    std::fs::remove_file(target).unwrap();
    std::fs::remove_dir(path.join("bin")).unwrap();
    std::fs::remove_dir(path).unwrap();
}

/// Checks the execution of this dynamic gpgv in a separate root.
/// Test files do not pass fs-verity and do not confirm the policy.
#[test]
#[cfg(target_arch = "x86_64")]
#[ignore = "требует user/mount namespace и системный gpgv с библиотеками"]
fn dynamic_gpgv_runs_with_only_pinned_runtime_in_private_root() {
    const CHILD: &str = "CAROSIGN_SIGNATURE_RUNTIME_EXEC_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let result = std::process::Command::new("unshare")
                .args(["--user", "--map-root-user", "--mount", "--fork"])
                .arg(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "discovery::signature_runtime::tests::dynamic_gpgv_runs_with_only_pinned_runtime_in_private_root",
                    "--ignored",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .output()
                .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    }
    // SAFETY: A separate process owns its own mounting space.
    assert_eq!(
        unsafe {
            libc::mount(
                std::ptr::null(),
                c"/".as_ptr(),
                std::ptr::null(),
                libc::MS_REC | libc::MS_PRIVATE,
                std::ptr::null(),
            )
        },
        0
    );
    let path =
        std::env::temp_dir().join(format!("carosign-signature-exec-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    let fixture =
        std::env::temp_dir().join(format!("carosign-signature-keys-{}", std::process::id()));
    std::fs::create_dir(&fixture).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fixture, std::fs::Permissions::from_mode(0o700)).unwrap();
    let gpg = |args: &[&str]| {
        std::process::Command::new("/usr/bin/gpg")
            .args([
                "--no-options",
                "--batch",
                "--yes",
                "--pinentry-mode",
                "loopback",
                "--passphrase",
                "",
                "--homedir",
            ])
            .arg(&fixture)
            .args(args)
            .output()
            .unwrap()
    };
    let generated = gpg(&[
        "--quick-generate-key",
        "fixture <fixture@example.invalid>",
        "ed25519",
        "sign",
        "0",
    ]);
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let listing = gpg(&["--with-colons", "--list-keys"]);
    assert!(listing.status.success());
    let listing = String::from_utf8(listing.stdout).unwrap();
    let primary = listing
        .lines()
        .find(|line| line.starts_with("fpr:"))
        .unwrap()
        .split(':')
        .nth(9)
        .unwrap()
        .to_owned();
    let keys = gpg(&["--export"]);
    assert!(keys.status.success() && !keys.stdout.is_empty());
    std::fs::write(fixture.join("trusted.gpg"), keys.stdout).unwrap();
    std::fs::write(fixture.join("artifact"), b"signed artifact").unwrap();
    std::fs::write(fixture.join("tampered"), b"signed artifacT").unwrap();
    let artifact = fixture.join("artifact");
    let signature = fixture.join("artifact.sig");
    let signed = gpg(&[
        "--detach-sign",
        "--output",
        signature.to_str().unwrap(),
        artifact.to_str().unwrap(),
    ]);
    assert!(
        signed.status.success(),
        "{}",
        String::from_utf8_lossy(&signed.stderr)
    );
    let files = [
        ("usr/bin/gpgv", "/usr/bin/gpgv"),
        ("usr/bin/test", "/usr/bin/test"),
        (
            "lib64/ld-linux-x86-64.so.2",
            "/usr/lib64/ld-linux-x86-64.so.2",
        ),
        ("usr/lib/libz.so.1", "/usr/lib/libz.so.1"),
        ("usr/lib/libbz2.so.1.0", "/usr/lib/libbz2.so.1.0"),
        ("usr/lib/libgcrypt.so.20", "/usr/lib/libgcrypt.so.20"),
        ("usr/lib/libassuan.so.9", "/usr/lib/libassuan.so.9"),
        ("usr/lib/libnpth.so.0", "/usr/lib/libnpth.so.0"),
        ("usr/lib/libgpg-error.so.0", "/usr/lib/libgpg-error.so.0"),
        ("usr/lib/libc.so.6", "/usr/lib/libc.so.6"),
    ];
    let isolated = PinnedRuntime {
        entries: files
            .iter()
            .map(|(name, source)| Entry {
                name: (*name).to_owned(),
                source: std::fs::File::open(source).unwrap().into(),
            })
            .collect(),
    };
    let verifier_file = std::fs::File::open("/usr/bin/gpgv").unwrap();
    let verifier =
        super::super::signature_inputs::PinnedVerifier::fixture(verifier_file.as_fd()).unwrap();
    let artifact_file = std::fs::File::open(fixture.join("artifact")).unwrap();
    let signature_file = std::fs::File::open(fixture.join("artifact.sig")).unwrap();
    let key_file = std::fs::File::open(fixture.join("trusted.gpg")).unwrap();
    let inputs = super::super::signature_inputs::SignatureInputs::fixture(
        artifact_file.as_fd(),
        signature_file.as_fd(),
        key_file.as_fd(),
    )
    .unwrap();
    // SAFETY: This is done in a separate single-threaded testing process.
    unsafe {
        inputs.verify_in_single_threaded_helper(
            &verifier,
            &isolated,
            &primary,
            std::time::Duration::from_secs(10),
        )
    }
    .unwrap();
    let tampered_file = std::fs::File::open(fixture.join("tampered")).unwrap();
    let bad_inputs = super::super::signature_inputs::SignatureInputs::fixture(
        tampered_file.as_fd(),
        signature_file.as_fd(),
        key_file.as_fd(),
    )
    .unwrap();
    // SAFETY: The same separate test helper is used, and the snapshots are sealed against writes.
    assert!(
        unsafe {
            bad_inputs.verify_in_single_threaded_helper(
                &verifier,
                &isolated,
                &primary,
                std::time::Duration::from_secs(10),
            )
        }
        .is_err()
    );
    let mut entries: Vec<Entry> = files
        .iter()
        .map(|(name, source)| Entry {
            name: (*name).to_owned(),
            source: std::fs::File::open(source).unwrap().into(),
        })
        .collect();
    for name in ["artifact", "artifact.sig", "trusted.gpg", "tampered"] {
        entries.push(Entry {
            name: format!("inputs/{name}"),
            source: std::fs::File::open(fixture.join(name)).unwrap().into(),
        });
    }
    let runtime = PinnedRuntime { entries };
    let root = std::fs::File::open(&path).unwrap();
    runtime.attach(root.as_fd()).unwrap();
    let root_fd = root.as_raw_fd();
    let mut command = std::process::Command::new("/usr/bin/gpgv");
    command
        .args([
            "--homedir",
            "/nonexistent",
            "--status-fd=1",
            "--keyring",
            "/inputs/trusted.gpg",
            "/inputs/artifact.sig",
            "/inputs/artifact",
        ])
        .env_clear();
    // SAFETY: A single child process goes only to the collected root.
    unsafe {
        command.pre_exec(move || {
            if libc::fchdir(root_fd) != 0
                || libc::chroot(c".".as_ptr()) != 0
                || libc::chdir(c"/".as_ptr()) != 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        super::super::signature_status::check(true, &result.stdout, &primary),
        Ok(())
    );
    let mut tampered = std::process::Command::new("/usr/bin/gpgv");
    tampered
        .args([
            "--homedir",
            "/nonexistent",
            "--status-fd=1",
            "--keyring",
            "/inputs/trusted.gpg",
            "/inputs/artifact.sig",
            "/inputs/tampered",
        ])
        .env_clear();
    // SAFETY: The process goes to the same private root with attached files.
    unsafe {
        tampered.pre_exec(move || {
            if libc::fchdir(root_fd) != 0
                || libc::chroot(c".".as_ptr()) != 0
                || libc::chdir(c"/".as_ptr()) != 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let rejected = tampered.output().unwrap();
    assert!(!rejected.status.success());
    assert!(super::super::signature_status::check(false, &rejected.stdout, &primary).is_err());
    let mut command = std::process::Command::new("/usr/bin/test");
    command.args(["!", "-e", "/etc/passwd"]).env_clear();
    // SAFETY: A separate child process goes into the same closed root.
    unsafe {
        command.pre_exec(move || {
            if libc::fchdir(root_fd) != 0
                || libc::chroot(c".".as_ptr()) != 0
                || libc::chdir(c"/".as_ptr()) != 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    assert!(command.status().unwrap().success());
    for entry in &runtime.entries {
        let target = path.join(&entry.name);
        let target = std::ffi::CString::new(target.as_os_str().as_encoded_bytes()).unwrap();
        // SAFETY: Only the mounting created in this test is removed.
        assert_eq!(
            unsafe { libc::umount2(target.as_ptr(), libc::MNT_DETACH) },
            0
        );
    }
    std::fs::remove_dir_all(path).unwrap();
    let _ = std::process::Command::new("/usr/bin/gpgconf")
        .arg("--homedir")
        .arg(&fixture)
        .args(["--kill", "gpg-agent"])
        .output();
    std::fs::remove_dir_all(fixture).unwrap();
}
