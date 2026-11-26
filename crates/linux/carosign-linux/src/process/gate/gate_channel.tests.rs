use std::io;
use std::os::fd::{FromRawFd, OwnedFd};
use std::time::{Duration, Instant};

use super::{Gate, GateReceiver, child_guard::ChildGuard};

#[test]
#[ignore = "вызывается отдельным тестовым процессом"]
fn gate_child() {
    // SAFETY: dup creates a new stdin descriptor that belongs to us.
    let fd = unsafe { libc::dup(libc::STDIN_FILENO) };
    assert!(fd >= 0);
    // SAFETY: The successful DUP returned its own descriptor.
    let receiver = GateReceiver::from_owned(unsafe { OwnedFd::from_raw_fd(fd) });
    let result = receiver.wait(Duration::from_secs(2));
    assert_eq!(
        result.is_ok(),
        std::env::var_os("GATE_EXPECT_RELEASE").is_some()
    );
}

#[test]
fn separate_process_waits_for_release_and_refuses_parent_disconnect() {
    use std::process::{Command, Stdio};
    let binary = std::env::current_exe().unwrap();
    for release in [true, false] {
        let (sender, receiver) = Gate::new().unwrap().into_parts();
        let mut command = Command::new(&binary);
        command
            .args(["--exact", "process::gate::tests::gate_child", "--ignored"])
            .stdin(Stdio::from(receiver.into_owned()))
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if release {
            command.env("GATE_EXPECT_RELEASE", "1");
        }
        let mut child = ChildGuard(command.spawn().unwrap());
        assert!(child.0.try_wait().unwrap().is_none());
        if release {
            sender.release().unwrap();
        } else {
            drop(sender);
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            assert!(Instant::now() < deadline, "тестовый процесс не завершился");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

#[test]
fn gate_refuses_closed_and_timeout_and_accepts_release() {
    let (sender, receiver) = Gate::new().unwrap().into_parts();
    drop(sender);
    assert!(receiver.wait(Duration::from_secs(1)).is_err());
    let (_sender, receiver) = Gate::new().unwrap().into_parts();
    assert_eq!(
        receiver.wait(Duration::from_millis(10)).unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
    let (sender, receiver) = Gate::new().unwrap().into_parts();
    sender.release().unwrap();
    receiver.wait(Duration::from_secs(1)).unwrap();
    let (sender, receiver) = Gate::new().unwrap().into_parts();
    drop(receiver);
    assert!(sender.release().is_err());
}
