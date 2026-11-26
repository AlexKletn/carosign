use super::super::cgroup::PreparedCgroup;
use super::super::gate::ErrorWriter;
use super::super::gate::GateReceiver;
use super::super::namespaces;
use super::super::tmpfs_root;
use std::ffi::CString;
use std::num::NonZeroU64;
use std::os::fd::AsFd;
use std::os::fd::AsRawFd;
use std::os::fd::OwnedFd;
use std::time::Duration;

use super::super::gate::{ErrorPipe, ExecReport};
use super::*;
use std::process::{Command, Stdio};
use std::time::Instant;

#[test]
fn separate_owned_helper_refuses_closed_gate() {
    use std::os::fd::FromRawFd;
    use std::os::unix::process::CommandExt;
    const CHILD: &str = "CAROSIGN_OWNED_GATE_FIXTURE";
    if let Some(path) = std::env::var_os(CHILD) {
        let dir = std::fs::File::open(path).unwrap();
        let parent = dir.try_clone().unwrap();
        let group = PreparedCgroup::fixture(parent.into(), dir.into());
        // SAFETY: The parent only passed these two descriptors to this process.
        let gate = GateReceiver::from_owned(unsafe { OwnedFd::from_raw_fd(20) });
        let report = ErrorWriter::from_owned(unsafe { OwnedFd::from_raw_fd(21) });
        let null = || std::fs::File::open("/dev/null").unwrap().into();
        let execution = OwnedExecution {
            executable: null(),
            worker: null(),
            standard: [null(), null(), null()],
            args: vec![CString::new("fixture").unwrap()],
            allowed_syscalls: vec![],
        };
        OwnedHelper {
            group,
            gate,
            report,
            timeout: Duration::from_secs(5),
            root_size: NonZeroU64::new(1_048_576).unwrap(),
            identity: RuntimeIdentity {
                uid: 65534,
                gid: 65534,
            },
            runtime: vec![],
            execution,
        }
        .run();
    }
    let path = std::env::temp_dir().join(format!("carosign-gate-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("cgroup.procs"), b"").unwrap();
    let (sender, receiver) = super::super::gate::Gate::new().unwrap().into_parts();
    let (reader, writer) = ErrorPipe::new().unwrap().into_parts();
    // High numbers exclude intersection with designated FD subsidiaries.
    let duplicate = |fd: i32| {
        let value = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 100) };
        assert!(value >= 0);
        unsafe { OwnedFd::from_raw_fd(value) }
    };
    let gate_fd = duplicate(receiver.into_owned().as_raw_fd());
    let report_fd = duplicate(writer.into_owned().as_raw_fd());
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .arg("--exact")
        .arg("process::helper::tests::separate_owned_helper_refuses_closed_gate")
        .env(CHILD, &path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // SAFETY: Only async-signal-safe dup2 can be performed before exec.
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(gate_fd.as_raw_fd(), 20) < 0
                || libc::dup2(report_fd.as_raw_fd(), 21) < 0
                || libc::dup2(gate_fd.as_raw_fd(), 1000) < 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().unwrap();
    drop(command);
    let deadline = Instant::now() + Duration::from_secs(5);
    while std::fs::read(path.join("cgroup.procs")).unwrap() != b"0" {
        assert!(
            Instant::now() < deadline,
            "помощник не вошёл в испытательную группу"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(child.try_wait().unwrap().is_none());
    // Intentionally inherited FD is closed before the gate is allowed.
    assert!(!std::path::Path::new(&format!("/proc/{}/fd/1000", child.id())).exists());
    drop(sender);
    assert_eq!(
        reader.read(Duration::from_secs(5)).unwrap(),
        ExecReport::Failure {
            stage: HelperStage::Gate.wire(),
            errno: libc::EIO,
        }
    );
    assert_eq!(child.wait().unwrap().code(), Some(126));
    std::fs::remove_file(path.join("cgroup.procs")).unwrap();
    std::fs::remove_dir(path).unwrap();
}

#[test]
fn owned_helper_reports_and_exits_on_invalid_root() {
    use std::os::fd::FromRawFd;
    const CHILD: &str = "CAROSIGN_OWNED_HELPER_CHILD";
    if std::env::var_os(CHILD).is_some() {
        // The stderr leads the report to the parent; the stderr itself is replaced by
        // /dev/null.
        let report_fd = unsafe { libc::dup(2) };
        assert!(report_fd >= 3);
        let null = std::fs::OpenOptions::new()
            .write(true)
            .open("/dev/null")
            .unwrap();
        assert_eq!(unsafe { libc::dup2(null.as_raw_fd(), 2) }, 2);
        drop(null);
        let report = ErrorWriter::from_owned(unsafe { OwnedFd::from_raw_fd(report_fd) });
        let standard = unsafe {
            [
                OwnedFd::from_raw_fd(0),
                OwnedFd::from_raw_fd(1),
                OwnedFd::from_raw_fd(2),
            ]
        };
        let mut sockets = [0; 2];
        assert_eq!(
            unsafe {
                libc::socketpair(libc::AF_UNIX, libc::SOCK_SEQPACKET, 0, sockets.as_mut_ptr())
            },
            0
        );
        let worker = unsafe { OwnedFd::from_raw_fd(sockets[0]) };
        drop(unsafe { OwnedFd::from_raw_fd(sockets[1]) });
        execute_prepared(
            std::fs::File::open("/proc").unwrap().into(),
            report,
            RuntimeIdentity {
                uid: 65534,
                gid: 65534,
            },
            OwnedExecution {
                executable: std::fs::File::open("/bin/true").unwrap().into(),
                worker,
                standard,
                args: vec![CString::new("worker").unwrap()],
                allowed_syscalls: vec![
                    libc::SYS_write,
                    libc::SYS_exit_group,
                    libc::SYS_exit,
                    libc::SYS_execveat,
                    libc::SYS_fstat,
                    libc::SYS_pread64,
                ],
            },
        );
    }
    let output = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("process::helper::tests::owned_helper_reports_and_exits_on_invalid_root")
        .env(CHILD, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(126));
    assert_eq!(
        output.stderr,
        [HelperStage::RootSwitch.wire(), 0, 0, 0, libc::EIO as u8]
    );
}

#[test]
fn after_gate_restricts_privileges_but_still_refuses_exec() {
    const CHILD: &str = "CAROSIGN_HELPER_AFTER_GATE_CHILD";
    if let Some(mode) = std::env::var_os(CHILD) {
        let (reader, writer) = ErrorPipe::new().unwrap().into_parts();
        let size = NonZeroU64::new(1_048_576).unwrap();
        let identity = RuntimeIdentity {
            uid: 65534,
            gid: 65534,
        };
        let result = if mode == "namespace-refusal" {
            after_gate_with(
                writer,
                size,
                identity,
                &[],
                || {
                    // SAFETY: Attribute query only concerns the current process.
                    assert_eq!(
                        unsafe { libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) },
                        1
                    );
                    Err(namespaces::NamespaceError {
                        stage: namespaces::NamespaceStage::Create,
                        source: std::io::Error::from_raw_os_error(libc::EPERM),
                    })
                },
                |_| panic!("корень нельзя создавать до пространств имён"),
            )
        } else if mode == "root-refusal" {
            after_gate_with(
                writer,
                size,
                identity,
                &[],
                || Ok(()),
                |requested| {
                    assert_eq!(requested, size);
                    Err(tmpfs_root::RootError {
                        stage: tmpfs_root::RootStage::OpenFilesystem,
                        source: std::io::Error::from_raw_os_error(libc::EPERM),
                    })
                },
            )
        } else if mode == "runtime-refusal" {
            let file = std::fs::File::open("/dev/null").unwrap();
            let files = [RuntimeFile {
                path: "../escape",
                source: file.as_fd(),
            }];
            after_gate_with(
                writer,
                size,
                identity,
                &files,
                || Ok(()),
                |_| Ok(std::fs::File::open("/proc").unwrap().into()),
            )
        } else {
            after_gate(writer, size, identity, &[])
        };
        if mode == "namespace-refusal" {
            assert_eq!(result.stage, HelperStage::Namespaces);
        } else if mode == "root-refusal" {
            assert_eq!(result.stage, HelperStage::TemporaryRoot);
        } else if mode == "runtime-refusal" {
            assert_eq!(result.stage, HelperStage::RuntimeFiles);
        } else {
            assert!(matches!(
                result.stage,
                HelperStage::Namespaces | HelperStage::TemporaryRoot | HelperStage::Isolation
            ));
        }
        assert!(result.errno > 0);
        if mode == "namespace-refusal" {
            assert_eq!(result.errno, libc::EPERM);
        }
        assert!(!result.report_failed);
        assert_eq!(
            reader.read(Duration::from_secs(1)).unwrap(),
            ExecReport::Failure {
                stage: result.stage.wire(),
                errno: result.errno,
            }
        );
        // SAFETY: Request the current process flag without pointers.
        assert_eq!(
            unsafe { libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) },
            1
        );
        return;
    }
    for mode in [
        "real",
        "namespace-refusal",
        "root-refusal",
        "runtime-refusal",
    ] {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("process::helper::tests::after_gate_restricts_privileges_but_still_refuses_exec")
            .env(CHILD, mode)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    assert!(status.success());
                    break;
                }
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                other => {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("помощник не завершился вовремя: {other:?}");
                }
            }
        }
    }
}
