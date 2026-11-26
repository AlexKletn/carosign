use super::super::fd_cleanup::AllowedFd;
use std::os::fd::OwnedFd;

use super::super::fd_cleanup::ExecDisposition;
use super::*;
use std::os::fd::AsRawFd;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn refuses_non_tmpfs_before_namespace_change() {
    let fd: OwnedFd = std::fs::File::open("/proc").unwrap().into();
    assert_eq!(enter(fd, &[]).unwrap_err().stage, SwitchStage::CheckRoot);
}

#[test]
fn prepared_root_still_rejects_non_tmpfs() {
    let fd: OwnedFd = std::fs::File::open("/proc").unwrap().into();
    assert_eq!(
        enter_prepared(fd, &[]).unwrap_err().stage,
        SwitchStage::CheckRoot
    );
}

#[test]
fn switches_only_in_child_and_closes_old_root_fd() {
    const CHILD: &str = "CAROSIGN_ROOT_SWITCH_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let rejected_root =
            super::super::tmpfs_root::create(std::num::NonZeroU64::new(1_048_576).unwrap())
                .unwrap();
        let forbidden = std::fs::File::open("/").unwrap();
        let forbidden_fd = AllowedFd {
            fd: forbidden.as_raw_fd(),
            disposition: ExecDisposition::Close,
        };
        assert_eq!(
            enter(rejected_root, &[forbidden_fd]).unwrap_err().stage,
            SwitchStage::CheckDescriptors
        );
        drop(forbidden);
        let root = super::super::tmpfs_root::create(std::num::NonZeroU64::new(1_048_576).unwrap())
            .unwrap();
        let old = std::fs::File::open("/").unwrap();
        let old_number = old.as_raw_fd();
        // We do not leave Rust with the owner of the FD, which will be closed through the
        // close_range.
        std::mem::forget(old);
        let allowed = [0, 1, 2].map(|fd| AllowedFd {
            fd,
            disposition: ExecDisposition::Inherit,
        });
        enter(root, &allowed).unwrap();
        assert_eq!(unsafe { libc::fcntl(old_number, libc::F_GETFD) }, -1);
        assert!(!std::path::Path::new("/dev").exists());
        assert!(!std::path::Path::new("/proc").exists());
        assert!(!std::path::Path::new("/etc").exists());
        return;
    }
    if std::env::var_os("CAROSIGN_ROOT_SWITCH_REQUIRE_SUCCESS").is_none() {
        return;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "process::root_switch::tests::switches_only_in_child_and_closes_old_root_fd",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .stdin(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("истёк срок процесса");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(std::path::Path::new("/proc").exists());
}

#[test]
fn mounted_runtime_survives_switch_without_old_root() {
    use std::os::fd::AsFd;

    const CHILD: &str = "CAROSIGN_ROOT_CHAIN_CHILD";
    const REQUIRE: &str = "CAROSIGN_ROOT_CHAIN_REQUIRE_SUCCESS";
    if std::env::var_os(CHILD).is_some() {
        let source_path =
            std::env::temp_dir().join(format!("carosign-root-chain-{}", std::process::id()));
        std::fs::write(&source_path, b"runtime fixture").unwrap();
        let source = std::fs::File::open(&source_path).unwrap();
        let root = super::super::tmpfs_root::create(std::num::NonZeroU64::new(1_048_576).unwrap())
            .unwrap();
        let mount = &super::super::runtime_mount::attach_file;
        assert!(mount(root.as_fd(), "usr/../escape", source.as_fd()).is_err());
        super::super::runtime_mount::attach_file_for_identity(
            root.as_fd(),
            "usr/lib/runtime",
            source.as_fd(),
            1,
            1,
        )
        .unwrap();
        super::super::tmpfs_root::assign_owner(&root, 1, 1).unwrap();
        drop(source);
        std::fs::remove_file(&source_path).unwrap();
        let old_root = std::fs::File::open("/").unwrap();
        let old_fd = old_root.as_raw_fd();
        // close_range The FD is removed; Rust no longer has to close it again.
        std::mem::forget(old_root);
        let allowed = [0, 1, 2].map(|fd| AllowedFd {
            fd,
            disposition: ExecDisposition::Inherit,
        });
        enter(root, &allowed).unwrap();
        assert_eq!(unsafe { libc::fcntl(old_fd, libc::F_GETFD) }, -1);
        let test_uid = if std::env::var_os("CAROSIGN_ROOT_CHAIN_OTHER_UID").is_some() {
            2
        } else {
            1
        };
        super::super::privileges::drop_identity(test_uid, test_uid).unwrap();
        if test_uid == 2 {
            assert!(std::fs::read("/usr/lib/runtime").is_err());
            return;
        }
        assert_eq!(
            std::fs::read("/usr/lib/runtime").unwrap(),
            b"runtime fixture"
        );
        assert!(std::fs::write("/usr/lib/runtime", b"changed").is_err());
        assert!(std::fs::write("/usr/lib/new", b"changed").is_err());
        use std::os::unix::fs::PermissionsExt;
        assert!(
            std::fs::set_permissions("/usr/lib", std::fs::Permissions::from_mode(0o700)).is_err()
        );
        assert!(!std::path::Path::new("/dev").exists());
        assert!(!std::path::Path::new("/proc").exists());
        assert!(!std::path::Path::new("/etc").exists());
        return;
    }
    if std::env::var_os(REQUIRE).is_none() {
        return;
    }
    for other in [false, true] {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "process::root_switch::tests::mounted_runtime_survives_switch_without_old_root",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .stdin(Stdio::null());
        if other {
            command.env("CAROSIGN_ROOT_CHAIN_OTHER_UID", "1");
        }
        let mut child = command.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "цепочка корня: {status}");
                break;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("истёк срок процесса цепочки корня");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    assert!(std::path::Path::new("/proc").exists());
}

/// Test run of real mok does not confirm the origin of the package or the cgroup setting.
/// requires static test ELF and user namespace.
#[test]
#[cfg(target_arch = "x86_64")]
fn confined_mock_performs_handshake() {
    use carosign_protocol::carosign::provider::v1::{self as provider, provider_frame::Payload};
    use carosign_provider_sdk::wire::Channel;
    use std::ffi::CString;
    use std::os::fd::{AsFd, FromRawFd};

    const CHILD: &str = "CAROSIGN_SANDBOX_MOCK_CHILD";
    const BINARY: &str = "CAROSIGN_SANDBOX_MOCK_BINARY";
    const SOCKET: &str = "CAROSIGN_SANDBOX_MOCK_SOCKET";
    struct ChildGuard(std::process::Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            if matches!(self.0.try_wait(), Ok(None)) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }
    if std::env::var_os(CHILD).is_some() {
        let executable = std::fs::File::open(std::env::var_os(BINARY).unwrap()).unwrap();
        let fd: i32 = std::env::var(SOCKET).unwrap().parse().unwrap();
        if std::env::var_os("CAROSIGN_TEST_OWNED_HELPER").is_some() {
            use super::super::helper::{
                OwnedExecution, OwnedRuntimeFile, RuntimeIdentity, after_gate_owned,
            };
            use std::os::fd::OwnedFd;
            let report_fd = unsafe { libc::dup(2) };
            assert!(report_fd >= 3);
            let null = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open("/dev/null")
                .unwrap();
            for target in 0..=2 {
                assert_eq!(unsafe { libc::dup2(null.as_raw_fd(), target) }, target);
            }
            drop(null);
            let calls = vec![
                libc::SYS_fstat,
                libc::SYS_pread64,
                libc::SYS_execveat,
                libc::SYS_exit_group,
                libc::SYS_write,
                libc::SYS_read,
                libc::SYS_close,
                libc::SYS_mmap,
                libc::SYS_mprotect,
                libc::SYS_munmap,
                libc::SYS_brk,
                libc::SYS_rt_sigaction,
                libc::SYS_rt_sigprocmask,
                libc::SYS_rt_sigreturn,
                libc::SYS_ioctl,
                libc::SYS_readv,
                libc::SYS_writev,
                libc::SYS_madvise,
                libc::SYS_sched_yield,
                libc::SYS_getpid,
                libc::SYS_gettid,
                libc::SYS_futex,
                libc::SYS_clock_gettime,
                libc::SYS_arch_prctl,
                libc::SYS_set_tid_address,
                libc::SYS_set_robust_list,
                libc::SYS_prlimit64,
                libc::SYS_getrandom,
                libc::SYS_sigaltstack,
                libc::SYS_getsockopt,
                libc::SYS_getpeername,
                libc::SYS_sendto,
                libc::SYS_recvmsg,
                libc::SYS_fcntl,
                libc::SYS_poll,
                libc::SYS_ppoll,
                libc::SYS_exit,
            ];
            let runtime = vec![OwnedRuntimeFile {
                path: "usr/bin/mock".into(),
                source: executable.try_clone().unwrap().into(),
            }];
            let execution = OwnedExecution {
                executable: executable.into(),
                worker: unsafe { OwnedFd::from_raw_fd(fd) },
                standard: [0, 1, 2].map(|fd| unsafe { OwnedFd::from_raw_fd(fd) }),
                args: vec![
                    CString::new("carosign-mock-provider").unwrap(),
                    CString::new(fd.to_string()).unwrap(),
                    CString::new("no-sample").unwrap(),
                ],
                allowed_syscalls: calls,
            };
            after_gate_owned(
                super::super::gate::ErrorWriter::from_owned(unsafe {
                    OwnedFd::from_raw_fd(report_fd)
                }),
                std::num::NonZeroU64::new(16 * 1024 * 1024).unwrap(),
                RuntimeIdentity { uid: 1, gid: 1 },
                runtime,
                execution,
            );
        }
        let root =
            super::super::tmpfs_root::create(std::num::NonZeroU64::new(16 * 1024 * 1024).unwrap())
                .unwrap();
        super::super::runtime_mount::attach_file_for_identity(
            root.as_fd(),
            "usr/bin/mock",
            executable.as_fd(),
            1,
            1,
        )
        .unwrap();
        super::super::tmpfs_root::assign_owner(&root, 1, 1).unwrap();
        let allowed = [0, 1, 2, fd].map(|fd| AllowedFd {
            fd,
            disposition: ExecDisposition::Inherit,
        });
        let mut allowed = allowed.to_vec();
        allowed.push(AllowedFd {
            fd: executable.as_raw_fd(),
            disposition: ExecDisposition::Close,
        });
        enter(root, &allowed).unwrap();
        super::super::privileges::drop_identity(1, 1).unwrap();
        super::super::privileges::set_no_new_privileges().unwrap();
        // The test list is not a production policy.
        let calls = [
            libc::SYS_fstat,
            libc::SYS_pread64,
            libc::SYS_execveat,
            libc::SYS_exit_group,
            libc::SYS_write,
            libc::SYS_read,
            libc::SYS_close,
            libc::SYS_mmap,
            libc::SYS_mprotect,
            libc::SYS_munmap,
            libc::SYS_brk,
            libc::SYS_rt_sigaction,
            libc::SYS_rt_sigprocmask,
            libc::SYS_rt_sigreturn,
            libc::SYS_ioctl,
            libc::SYS_readv,
            libc::SYS_writev,
            libc::SYS_madvise,
            libc::SYS_sched_yield,
            libc::SYS_getpid,
            libc::SYS_gettid,
            libc::SYS_futex,
            libc::SYS_clock_gettime,
            libc::SYS_arch_prctl,
            libc::SYS_set_tid_address,
            libc::SYS_set_robust_list,
            libc::SYS_prlimit64,
            libc::SYS_getrandom,
            libc::SYS_sigaltstack,
            libc::SYS_getsockopt,
            libc::SYS_getpeername,
            libc::SYS_sendto,
            libc::SYS_recvmsg,
            libc::SYS_fcntl,
            libc::SYS_poll,
            libc::SYS_ppoll,
            libc::SYS_exit,
        ];
        super::super::seccomp::install(&calls).unwrap();
        let args = [
            CString::new("carosign-mock-provider").unwrap(),
            CString::new(fd.to_string()).unwrap(),
            CString::new("no-sample").unwrap(),
        ];
        let result = super::super::pinned_exec::execute(executable.as_fd(), &args);
        let _ = result;
        unsafe { libc::_exit(126) };
    }
    let Some(binary) = std::env::var_os(BINARY) else {
        return;
    };
    if std::env::var_os("CAROSIGN_TEST_OWNED_HELPER").is_some() {
        println!("Испытание owned-helper после cgroup и gate");
    }
    let mut fds = [-1; 2];
    assert_eq!(
        unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_SEQPACKET, 0, fds.as_mut_ptr()) },
        0
    );
    let parent = unsafe { std::os::fd::OwnedFd::from_raw_fd(fds[0]) };
    let worker = unsafe { std::os::fd::OwnedFd::from_raw_fd(fds[1]) };
    let mut child = ChildGuard(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "process::root_switch::tests::confined_mock_performs_handshake",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env(BINARY, binary)
            .env(SOCKET, worker.as_raw_fd().to_string())
            .stdin(Stdio::null())
            .spawn()
            .unwrap(),
    );
    drop(worker);
    let timeout = libc::timeval {
        tv_sec: 5,
        tv_usec: 0,
    };
    assert_eq!(
        unsafe {
            libc::setsockopt(
                parent.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_RCVTIMEO,
                (&timeout as *const libc::timeval).cast(),
                std::mem::size_of_val(&timeout) as libc::socklen_t,
            )
        },
        0
    );
    let mut channel = Channel::new();
    let mut data = vec![0u8; 65536];
    let count = unsafe { libc::recv(parent.as_raw_fd(), data.as_mut_ptr().cast(), data.len(), 0) };
    assert!(
        count > 0,
        "mock не прислал hello: {:?}",
        std::io::Error::last_os_error()
    );
    let hello = channel.accept(&data[..count as usize]).unwrap();
    assert!(matches!(hello.payload, Some(Payload::WorkerHello(_))));
    let frame = provider::ProviderFrame {
        major: 1,
        minor: 3,
        worker_instance_id: vec![1; 16],
        frame_seq: 0,
        command_id: vec![],
        operation_id: vec![],
        device_epoch: 0,
        uid: None,
        payload: Some(Payload::Initialize(provider::Initialize {
            worker_instance_id: vec![1; 16],
            selected_minor: 3,
            package_id: vec![2; 16],
            profile_id: vec![3; 16],
            enabled_features: vec![provider::Feature::BasicOperations as i32],
        })),
    };
    let wire = channel.prepare(frame).unwrap();
    assert_eq!(
        unsafe {
            libc::send(
                parent.as_raw_fd(),
                wire.as_ptr().cast(),
                wire.len(),
                libc::MSG_NOSIGNAL,
            )
        },
        wire.len() as isize
    );
    let count = unsafe { libc::recv(parent.as_raw_fd(), data.as_mut_ptr().cast(), data.len(), 0) };
    assert!(
        count > 0,
        "mock не прислал initialized: {:?}",
        std::io::Error::last_os_error()
    );
    assert!(
        matches!(channel.accept(&data[..count as usize]).unwrap().payload, Some(Payload::Initialized(ref x)) if x.ready_for_binding)
    );

    use carosign_protocol::carosign::common::v1 as common;
    let authenticator = common::AuthenticatorRef {
        authenticator_id: vec![7; 16],
        device_epoch: 1,
    };
    let subject = common::SubjectRef {
        authority_id: vec![4; 16],
        subject_id: vec![5; 16],
    };
    let enrollment = common::EnrollmentRef {
        enrollment_id: vec![6; 16],
        authenticator_id: vec![7; 16],
        generation: 1,
    };
    let send_frame = |channel: &mut Channel, command: u8, operation: Option<u8>, payload| {
        let frame = provider::ProviderFrame {
            major: 1,
            minor: 3,
            worker_instance_id: vec![1; 16],
            frame_seq: 0,
            command_id: vec![command; 16],
            operation_id: operation.map(|value| vec![value; 16]).unwrap_or_default(),
            device_epoch: 1,
            uid: matches!(
                &payload,
                Payload::StartVerification(_)
                    | Payload::StartEnrollment(_)
                    | Payload::ContinueEnrollment(_)
                    | Payload::StartRemoval(_)
                    | Payload::PrepareEnrollmentChallenge(_)
            )
            .then_some(1000),
            payload: Some(payload),
        };
        let wire = channel.prepare(frame).unwrap();
        // SAFETY: The descriptor and buffer are valid at the time the packet is transmitted.
        assert_eq!(
            unsafe {
                libc::send(
                    parent.as_raw_fd(),
                    wire.as_ptr().cast(),
                    wire.len(),
                    libc::MSG_NOSIGNAL,
                )
            },
            wire.len() as isize
        );
    };
    let receive = |channel: &mut Channel, command: u8, operation: Option<u8>| {
        let mut buffer = vec![0; 65536];
        // SAFETY: The buffer and open socket are valid for the duration of the reading.
        let count = unsafe {
            libc::recv(
                parent.as_raw_fd(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                0,
            )
        };
        assert!(
            count > 0,
            "мок не ответил: {:?}",
            std::io::Error::last_os_error()
        );
        let frame = channel.accept(&buffer[..count as usize]).unwrap();
        assert_eq!(frame.worker_instance_id, vec![1; 16]);
        assert_eq!(frame.command_id, vec![command; 16]);
        assert_eq!(
            frame.operation_id,
            operation.map(|value| vec![value; 16]).unwrap_or_default()
        );
        assert_eq!(frame.device_epoch, 1);
        frame.payload.unwrap()
    };
    send_frame(
        &mut channel,
        40,
        None,
        Payload::BindAuthenticator(provider::BindAuthenticator {
            authenticator: Some(authenticator.clone()),
            profile_id: vec![3; 16],
            lease_id: vec![7; 16],
            fd_slots: vec![],
        }),
    );
    assert!(
        matches!(receive(&mut channel, 40, None), Payload::AuthenticatorReady(value) if value.authenticator == Some(authenticator.clone()))
    );
    send_frame(
        &mut channel,
        41,
        Some(42),
        Payload::StartEnrollment(provider::StartEnrollment {
            authenticator: Some(authenticator.clone()),
            subject: Some(subject.clone()),
            mutation_id: vec![43; 16],
            label: "test".into(),
            replace: None,
            replace_binding: None,
            enrollment_authorization_ref: vec![],
            enrollment_challenge_digest: vec![],
            planned_enrollment: Some(enrollment.clone()),
        }),
    );
    assert!(
        matches!(receive(&mut channel, 41, Some(42)), Payload::CommandAck(value) if value.accepted)
    );
    assert!(
        matches!(receive(&mut channel, 41, Some(42)), Payload::MutationOutcome(value) if value.provider_opaque_ref == vec![6; 16])
    );
    send_frame(
        &mut channel,
        44,
        Some(45),
        Payload::StartVerification(provider::StartVerification {
            authenticator: Some(authenticator),
            subject: Some(subject),
            purpose: common::VerificationPurpose::PurposeSessionUnlock as i32,
            challenge_ref: vec![8; 16],
            recipient_ref: vec![9; 16],
            permitted_enrollments: vec![provider::EnrollmentBinding {
                enrollment: Some(enrollment),
                kind: provider::EnrollmentBindingKind::EnrollmentBindingProviderOpaque as i32,
                provider_opaque_ref: vec![6; 16],
                format_id: String::new(),
                format_version: 0,
            }],
        }),
    );
    assert!(
        matches!(receive(&mut channel, 44, Some(45)), Payload::CommandAck(value) if value.accepted)
    );
    assert!(
        matches!(receive(&mut channel, 44, Some(45)), Payload::VerificationOutcome(value) if value.status == provider::VerificationStatus::VerifyNoUsableSample as i32 && value.evidence.is_none() && value.matched_enrollment.is_none())
    );
    drop(parent);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.0.kill();
            let _ = child.0.wait();
            panic!("срок mock-процесса истёк");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The test profile is not the policy of the production provider.
#[test]
#[cfg(target_arch = "x86_64")]
fn confined_process_executes_pinned_static_fixture() {
    use std::ffi::CString;
    use std::os::fd::AsFd;

    const CHILD: &str = "CAROSIGN_HELPER_CHAIN_CHILD";
    const REQUIRE: &str = "CAROSIGN_HELPER_CHAIN_REQUIRE_SUCCESS";
    if std::env::var_os(CHILD).is_some() {
        let executable = std::fs::File::open("/bin/busybox").unwrap();
        let root = super::super::tmpfs_root::create(std::num::NonZeroU64::new(1_048_576).unwrap())
            .unwrap();
        super::super::runtime_mount::attach_file_for_identity(
            root.as_fd(),
            "usr/bin/busybox",
            executable.as_fd(),
            1,
            1,
        )
        .unwrap();
        super::super::tmpfs_root::assign_owner(&root, 1, 1).unwrap();
        let old_root = std::fs::File::open("/").unwrap();
        let old_fd = old_root.as_raw_fd();
        std::mem::forget(old_root);
        let allowed = [
            AllowedFd {
                fd: 0,
                disposition: ExecDisposition::Inherit,
            },
            AllowedFd {
                fd: 1,
                disposition: ExecDisposition::Inherit,
            },
            AllowedFd {
                fd: 2,
                disposition: ExecDisposition::Inherit,
            },
            AllowedFd {
                fd: executable.as_raw_fd(),
                disposition: ExecDisposition::Close,
            },
        ];
        enter(root, &allowed).unwrap();
        assert_eq!(unsafe { libc::fcntl(old_fd, libc::F_GETFD) }, -1);
        super::super::privileges::drop_identity(1, 1).unwrap();
        super::super::privileges::set_no_new_privileges().unwrap();
        assert!(std::path::Path::new("/usr/bin/busybox").is_file());
        assert!(!std::path::Path::new("/dev").exists());
        assert!(!std::path::Path::new("/proc").exists());
        // Only system calls of the test static busybox and necessary for checking and exec of
        // the most attached file.
        let deny_exec = std::env::var_os("CAROSIGN_HELPER_CHAIN_DENY_EXEC").is_some();
        let mut calls = vec![
            libc::SYS_fstat,
            libc::SYS_pread64,
            libc::SYS_arch_prctl,
            libc::SYS_set_tid_address,
            libc::SYS_prctl,
            libc::SYS_getuid,
            libc::SYS_getgid,
            libc::SYS_newfstatat,
            libc::SYS_rt_sigprocmask,
            libc::SYS_setgid,
            libc::SYS_setuid,
            libc::SYS_exit_group,
            libc::SYS_write,
        ];
        if !deny_exec {
            calls.push(libc::SYS_execveat);
        }
        super::super::seccomp::install(&calls).unwrap();
        // SAFETY: Check that the filter does not allow the call.
        assert_eq!(unsafe { libc::syscall(libc::SYS_getppid) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EPERM)
        );
        let args = [
            CString::new("busybox").unwrap(),
            CString::new("true").unwrap(),
        ];
        let result = super::super::pinned_exec::execute(executable.as_fd(), &args);
        if deny_exec {
            let expected = result.unwrap_err().raw_os_error() == Some(libc::EPERM);
            // SAFETY: After installing the filter, only the test process is completed.
            unsafe { libc::_exit(if expected { 0 } else { 1 }) };
        }
        result.unwrap();
        panic!("exec неожиданно вернулся");
    }
    if std::env::var_os(REQUIRE).is_none() {
        return;
    }
    for deny_exec in [false, true] {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "process::root_switch::tests::confined_process_executes_pinned_static_fixture",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .stdin(Stdio::null());
        if deny_exec {
            command.env("CAROSIGN_HELPER_CHAIN_DENY_EXEC", "1");
        }
        let mut child = command.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "цепочка помощника: {status}");
                break;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("истёк срок процесса помощника");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
