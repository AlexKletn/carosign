use super::gpg_home::GpgHome;
use super::*;
use crate::discovery::archive::fixture_decoder::Decoder;
use std::process::Command;

const PRIMARY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const SUBKEY: &str = "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";

#[test]
fn requires_one_valid_signature_from_approved_primary() {
    let valid = format!(
        "[GNUPG:] NEWSIG\n[GNUPG:] VALIDSIG {SUBKEY} 2026-01-01 1 0 4 0 22 8 00 {PRIMARY}\n"
    );
    assert_eq!(check(true, valid.as_bytes(), PRIMARY), Ok(()));
    assert_eq!(
        check(false, valid.as_bytes(), PRIMARY),
        Err(SignatureRefusal::VerifierFailed)
    );
    assert_eq!(
        check(true, valid.as_bytes(), SUBKEY),
        Err(SignatureRefusal::NotApproved)
    );
    assert_eq!(
        check(true, format!("{valid}{valid}").as_bytes(), PRIMARY),
        Err(SignatureRefusal::InvalidStatus)
    );
    assert_eq!(
        check(
            true,
            format!("{valid}[GNUPG:] EXPKEYSIG x y\n").as_bytes(),
            PRIMARY
        ),
        Err(SignatureRefusal::InvalidStatus)
    );
    assert_eq!(
        check(true, b"[GNUPG:] GOODSIG x y\n", PRIMARY),
        Err(SignatureRefusal::InvalidStatus)
    );
    assert_eq!(
        check(true, valid.as_bytes(), "untrusted"),
        Err(SignatureRefusal::InvalidFingerprint)
    );
}

#[test]
fn real_detached_signature_rejects_changed_artifact_and_wrong_signer() {
    const CHILD: &str = "CAROSIGN_GPGV_HELPER_CHILD";
    if std::env::var_os(CHILD).is_some() {
        use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
        let dir = std::path::PathBuf::from(std::env::var_os("CAROSIGN_GPGV_FIXTURE").unwrap());
        let artifact = std::fs::File::open(dir.join("artifact")).unwrap();
        let signature = std::fs::File::open(dir.join("artifact.sig")).unwrap();
        let keyring = std::fs::File::open(dir.join("trusted.gpg")).unwrap();
        let primary = std::env::var("CAROSIGN_GPGV_PRIMARY").unwrap();
        // SAFETY: Create a separate test FD without CLOEXEC.
        let raw = unsafe { libc::fcntl(artifact.as_raw_fd(), libc::F_DUPFD, 64) };
        assert!(raw >= 64);
        // SAFETY: fcntl returned a new test-owned descriptor.
        let unrelated = unsafe { OwnedFd::from_raw_fd(raw) };
        assert_eq!(
            unsafe { libc::fcntl(raw, libc::F_GETFD) } & libc::FD_CLOEXEC,
            0
        );
        // The fixture checks the transport of sealed images; it does not issue authority and
        // is only available to tests.
        let inputs = super::super::signature_inputs::SignatureInputs::fixture(
            artifact.as_fd(),
            signature.as_fd(),
            keyring.as_fd(),
        )
        .unwrap();
        // SAFETY: This is a separate single-threaded test assistant.
        let verifier_path = dir.join("verifier-pinned");
        std::fs::copy("/usr/bin/gpgv", &verifier_path).unwrap();
        let executable = std::fs::File::open(&verifier_path).unwrap();
        let verifier =
            super::super::signature_inputs::PinnedVerifier::fixture(executable.as_fd()).unwrap();
        // Replacing the name after opening does not change the executable file.
        std::fs::rename(&verifier_path, dir.join("verifier-original")).unwrap();
        std::fs::write(&verifier_path, b"not an executable").unwrap();
        let result = unsafe {
            inputs.verify_unisolated_fixture(&verifier, &primary, std::time::Duration::from_secs(5))
        };
        std::fs::remove_file(&verifier_path).unwrap();
        std::fs::remove_file(dir.join("verifier-original")).unwrap();
        // Alien FD remains with the assistant, but is not inherited through exec.
        assert_ne!(
            unsafe { libc::fcntl(unrelated.as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC,
            0
        );
        assert_eq!(
            result.is_ok(),
            std::env::var_os("CAROSIGN_GPGV_EXPECT_OK").is_some(),
            "{result:?}"
        );
        if let Ok(checked) = result {
            // The test copy does not approve the measurement of a set of keys; the
            // fingerprint of a validated key is not lost.
            assert_eq!(checked.signature_inputs(), (primary.as_str(), None));
            assert_eq!(checked.verifier_verity(), None);
            let archive = super::super::archive::inspect(&checked, &Decoder, "default").unwrap();
            let stage = dir.join("stage");
            std::fs::create_dir(&stage).unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&stage, std::fs::Permissions::from_mode(0o700)).unwrap();
            let root = std::fs::File::open(&stage).unwrap();
            let files = super::super::archive::stage(&archive, root.as_fd()).unwrap();
            assert_eq!(files.len(), 3);
            assert_eq!(
                std::fs::read(stage.join("bin/worker")).unwrap(),
                b"signed worker"
            );
            let measured_stage = dir.join("measured-stage");
            std::fs::create_dir(&measured_stage).unwrap();
            std::fs::set_permissions(&measured_stage, std::fs::Permissions::from_mode(0o700))
                .unwrap();
            let measured_root = std::fs::File::open(&measured_stage).unwrap();
            let policy = format!(
                "schema=1\npackage='org.example.test'\nversion='1'\nprofile='default'\narchitecture='{}'\nexecutable='bin/worker'\nuid=42\ngid=43\nmemory_bytes=4096\ncpu_quota_us=5000\ncpu_period_us=10000\nprocesses=1\nruntime=[]\nsyscalls=[0,1]\n",
                std::env::consts::ARCH
            );
            let policy =
                super::super::supervisor_policy::SupervisorPolicy::parse(policy.as_bytes())
                    .unwrap();
            match super::super::archive::inspect_policy_and_stage_measured(
                &checked,
                &Decoder,
                &policy,
                measured_root.as_fd(),
            ) {
                Ok(measured) => assert_eq!(measured.len(), 3),
                Err(super::super::archive::ArchiveError::Io(error))
                    if matches!(
                        error.raw_os_error(),
                        Some(libc::ENOTTY | libc::EOPNOTSUPP | libc::EINVAL)
                    ) =>
                {
                    eprintln!("fs-verity недоступен: {error}");
                }
                Err(error) => panic!("неожиданная ошибка подготовки: {error:?}"),
            }
            assert_eq!(
                std::fs::metadata(measured_stage.join("bin/worker"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o555
            );
        }
        return;
    }
    if Command::new("/usr/bin/gpg")
        .arg("--version")
        .output()
        .is_err()
        || Command::new("/usr/bin/gpgv")
            .arg("--version")
            .output()
            .is_err()
    {
        eprintln!("gpg/gpgv недоступны: системное испытание подписи пропущено");
        return;
    }
    let dir = std::env::temp_dir().join(format!("carosign-signature-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let _cleanup = GpgHome(dir.clone());
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let run_gpg = |args: &[&str]| {
        Command::new("/usr/bin/gpg")
            .arg("--no-options")
            .arg("--homedir")
            .arg(&dir)
            .args([
                "--batch",
                "--yes",
                "--pinentry-mode",
                "loopback",
                "--passphrase",
                "",
            ])
            .args(args)
            .output()
            .unwrap()
    };
    let generated = run_gpg(&[
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
    let listing = run_gpg(&["--with-colons", "--list-keys"]);
    assert!(listing.status.success());
    let listing = String::from_utf8(listing.stdout).unwrap();
    let primary = listing
        .lines()
        .find(|line| line.starts_with("fpr:"))
        .unwrap()
        .split(':')
        .nth(9)
        .unwrap();
    assert!(fingerprint(primary));
    let keyring = run_gpg(&["--export"]);
    assert!(keyring.status.success() && !keyring.stdout.is_empty());
    std::fs::write(dir.join("trusted.gpg"), keyring.stdout).unwrap();
    let mut archive = tar::Builder::new(Vec::new());
    for (name, body) in [
        ("package.toml", &b"manifest"[..]),
        ("bin/worker", &b"signed worker"[..]),
        ("profiles/default.toml", &b"profile"[..]),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        archive.append_data(&mut header, name, body).unwrap();
    }
    std::fs::write(dir.join("artifact"), archive.into_inner().unwrap()).unwrap();
    let signed = run_gpg(&[
        "--detach-sign",
        "--output",
        dir.join("artifact.sig").to_str().unwrap(),
        dir.join("artifact").to_str().unwrap(),
    ]);
    assert!(signed.status.success());
    let verify = || {
        Command::new("/usr/bin/gpgv")
            .arg("--status-fd=1")
            .arg("--keyring")
            .arg(dir.join("trusted.gpg"))
            .arg(dir.join("artifact.sig"))
            .arg(dir.join("artifact"))
            .output()
            .unwrap()
    };
    let result = verify();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(check(true, &result.stdout, primary), Ok(()));
    assert_eq!(
        check(true, &result.stdout, PRIMARY),
        Err(SignatureRefusal::NotApproved)
    );
    let run_helper = |fingerprint: &str, expected_ok: bool| {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args(["--exact", "discovery::signature_status::tests::real_detached_signature_rejects_changed_artifact_and_wrong_signer"])
                .env(CHILD, "1")
                .env("CAROSIGN_GPGV_FIXTURE", &dir)
                .env("CAROSIGN_GPGV_PRIMARY", fingerprint);
        if expected_ok {
            command.env("CAROSIGN_GPGV_EXPECT_OK", "1");
        } else {
            command.env_remove("CAROSIGN_GPGV_EXPECT_OK");
        }
        assert!(command.status().unwrap().success());
    };
    run_helper(primary, true);
    run_helper(PRIMARY, false);
    std::fs::write(dir.join("artifact"), b"changed artifact").unwrap();
    let result = verify();
    assert!(!result.status.success());
    assert_eq!(
        check(false, &result.stdout, primary),
        Err(SignatureRefusal::VerifierFailed)
    );
    run_helper(primary, false);
}
