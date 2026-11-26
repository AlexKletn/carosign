use super::*;
use std::os::fd::AsFd;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn exec_uses_pinned_elf_and_rejects_script() {
    const CHILD: &str = "CAROSIGN_PINNED_EXEC_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let dir = std::env::temp_dir().join(format!("carosign-exec-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("worker");
        std::fs::copy("/bin/true", &path).unwrap();
        let pinned = std::fs::File::open(&path).unwrap();
        std::fs::rename(&path, dir.join("pinned")).unwrap();
        std::fs::write(&path, b"#!/bin/sh\nexit 42\n").unwrap();
        let script = std::fs::File::open(&path).unwrap();
        assert_eq!(
            execute(script.as_fd(), &[CString::new("worker").unwrap()])
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(dir.join("pinned")).unwrap();
        std::fs::remove_dir(&dir).unwrap();
        execute(pinned.as_fd(), &[CString::new("worker").unwrap()]).unwrap();
        panic!("exec неожиданно вернулся");
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("process::pinned_exec::tests::exec_uses_pinned_elf_and_rejects_script")
        .env(CHILD, "1")
        .stdout(Stdio::null())
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
            panic!("истёк срок дочернего процесса");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
