use super::*;
use std::os::unix::fs::MetadataExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn namespace_change_is_confined_to_child_and_fails_closed() {
    const CHILD: &str = "CAROSIGN_NAMESPACE_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let mount_before = std::fs::metadata("/proc/thread-self/ns/mnt").unwrap().ino();
        let net_before = std::fs::metadata("/proc/thread-self/ns/net").unwrap().ino();
        match enter() {
            Ok(()) => {
                assert_ne!(
                    mount_before,
                    std::fs::metadata("/proc/thread-self/ns/mnt").unwrap().ino()
                );
                assert_ne!(
                    net_before,
                    std::fs::metadata("/proc/thread-self/ns/net").unwrap().ino()
                );
                let devices = std::fs::read_to_string("/proc/thread-self/net/dev").unwrap();
                let names: Vec<_> = devices
                    .lines()
                    .skip(2)
                    .filter_map(|line| line.split_once(':').map(|(name, _)| name.trim()))
                    .collect();
                assert!(names.iter().all(|name| *name == "lo"));
                let mountinfo = std::fs::read_to_string("/proc/thread-self/mountinfo").unwrap();
                for line in mountinfo.lines() {
                    let optional = line.split_once(" - ").unwrap().0;
                    assert!(
                        !optional
                            .split_whitespace()
                            .any(|field| field.starts_with("shared:")
                                || field.starts_with("master:"))
                    );
                }
            }
            Err(error) => {
                assert!(error.source.raw_os_error().is_some());
                eprintln!("namespace недоступен: {:?}: {}", error.stage, error.source);
            }
        }
        return;
    }
    let mount_before = std::fs::metadata("/proc/self/ns/mnt").unwrap().ino();
    let net_before = std::fs::metadata("/proc/self/ns/net").unwrap().ino();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "process::namespaces::tests::namespace_change_is_confined_to_child_and_fails_closed",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .stdin(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "тест пространства имён: {status}");
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("срок процесса испытания истёк");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        mount_before,
        std::fs::metadata("/proc/self/ns/mnt").unwrap().ino()
    );
    assert_eq!(
        net_before,
        std::fs::metadata("/proc/self/ns/net").unwrap().ino()
    );
}
