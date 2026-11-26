use std::ffi::CString;
use std::num::NonZeroU64;
use std::os::fd::AsRawFd;
use std::os::fd::OwnedFd;

use super::*;
use std::os::fd::AsFd;

fn limits() -> Limits {
    Limits {
        cpu_quota_us: NonZeroU64::new(50_000).unwrap(),
        cpu_period_us: NonZeroU64::new(100_000).unwrap(),
        memory_bytes: NonZeroU64::new(64 * 1024 * 1024).unwrap(),
        processes: NonZeroU64::new(2).unwrap(),
    }
}

#[test]
fn reject_untrusted_path_component_and_invalid_period() {
    let root = std::fs::File::open("/").unwrap();
    assert!(matches!(
        PreparedCgroup::create(root.as_fd(), "../other", limits()),
        Err(CgroupError::InvalidName)
    ));
    let mut bad = limits();
    bad.cpu_period_us = NonZeroU64::new(999).unwrap();
    assert!(matches!(
        PreparedCgroup::create(root.as_fd(), "worker", bad),
        Err(CgroupError::InvalidCpuPeriod)
    ));
}

#[test]
fn reject_non_cgroup_filesystem_before_creating_directory() {
    let root = std::fs::File::open(std::env::temp_dir()).unwrap();
    assert!(matches!(
        PreparedCgroup::create(root.as_fd(), "carosign-must-not-create", limits()),
        Err(CgroupError::WrongFilesystem)
    ));
}

#[test]
fn cgroup_root_reports_kernel_refusal_without_fallback_when_not_delegated() {
    let root = std::fs::File::open("/sys/fs/cgroup").unwrap();
    // We only make a real call if the root is not available for recording.
    // SAFETY: The descriptor is open, the kernel fills the structure.
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    assert_eq!(unsafe { libc::fstatvfs(root.as_raw_fd(), &mut stat) }, 0);
    if stat.f_flag & libc::ST_RDONLY != 0 {
        assert!(matches!(
            PreparedCgroup::create(root.as_fd(), "carosign-no-fallback-test", limits()),
            Err(CgroupError::Kernel {
                stage: "create",
                ..
            })
        ));
    }
}

#[test]
fn failed_join_closes_gate_without_releasing_child() {
    use crate::process::gate::Gate;
    use std::time::Duration;
    let root = std::env::temp_dir().join(format!("carosign-cgroup-gate-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(root.join("worker")).unwrap();
    let parent = std::fs::File::open(&root).unwrap();
    let directory = std::fs::File::open(root.join("worker")).unwrap();
    let group = PreparedCgroup {
        parent: parent.into(),
        directory: directory.into(),
        name: CString::new("worker").unwrap(),
    };
    let (sender, receiver) = Gate::new().unwrap().into_parts();
    assert!(matches!(
        group.join_current_and_wait(receiver, Duration::from_millis(100)),
        Err(CgroupError::Kernel {
            stage: "cgroup.procs",
            ..
        })
    ));
    assert!(sender.release().is_err());
    drop(group);
    std::fs::remove_dir(root).unwrap();
}

#[test]
#[ignore = "вызывается отдельным тестовым процессом"]
fn helper_child_reports_failed_cgroup() {
    use crate::process::gate::{ErrorWriter, GateReceiver};
    use std::os::fd::FromRawFd;
    use std::time::Duration;
    let root = std::path::PathBuf::from(std::env::var_os("CAROSIGN_HELPER_TEST_ROOT").unwrap());
    let group = PreparedCgroup {
        parent: std::fs::File::open(&root).unwrap().into(),
        directory: std::fs::File::open(root.join("worker")).unwrap().into(),
        name: CString::new("worker").unwrap(),
    };
    // SAFETY: dup returns new descriptors belonging to the assistant.
    let gate_fd = unsafe { libc::dup(libc::STDIN_FILENO) };
    let report_fd = unsafe { libc::dup(libc::STDERR_FILENO) };
    assert!(gate_fd >= 0 && report_fd >= 0);
    let gate = GateReceiver::from_owned(unsafe { OwnedFd::from_raw_fd(gate_fd) });
    let report = ErrorWriter::from_owned(unsafe { OwnedFd::from_raw_fd(report_fd) });
    let failure = crate::process::helper::enter(
        &group,
        gate,
        report,
        Duration::from_secs(1),
        std::num::NonZeroU64::new(1_048_576).unwrap(),
        crate::process::helper::RuntimeIdentity {
            uid: 65534,
            gid: 65534,
        },
        &[],
    );
    assert_eq!(failure.stage, crate::process::helper::HelperStage::Cgroup);
    assert_eq!(failure.errno, libc::ENOENT);
    assert!(!failure.report_failed);
}

#[test]
fn separate_helper_reports_cgroup_failure_and_closes_gate() {
    use crate::process::gate::{ErrorPipe, ExecReport, Gate};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    struct ChildGuard(std::process::Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let root = std::env::temp_dir().join(format!("carosign-helper-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(root.join("worker")).unwrap();
    let (sender, receiver) = Gate::new().unwrap().into_parts();
    let (reader, writer) = ErrorPipe::new().unwrap().into_parts();
    let mut child = ChildGuard(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "process::cgroup::tests::helper_child_reports_failed_cgroup",
                "--ignored",
            ])
            .env("CAROSIGN_HELPER_TEST_ROOT", &root)
            .stdin(Stdio::from(receiver.into_owned()))
            .stdout(Stdio::null())
            .stderr(Stdio::from(writer.into_owned()))
            .spawn()
            .unwrap(),
    );
    assert_eq!(
        reader.read(Duration::from_secs(3)).unwrap(),
        ExecReport::Failure {
            stage: 1,
            errno: libc::ENOENT
        }
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(Instant::now() < deadline, "помощник не завершился");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(sender.release().is_err());
    std::fs::remove_dir(root).unwrap();
}
