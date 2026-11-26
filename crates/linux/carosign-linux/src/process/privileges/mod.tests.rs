use super::*;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn root_identity_is_refused_without_changing_process() {
    assert_eq!(
        drop_identity(0, 1).unwrap_err().stage,
        PrivilegeStage::ValidateIdentity
    );
    assert_eq!(
        drop_identity(1, 0).unwrap_err().stage,
        PrivilegeStage::ValidateIdentity
    );
}

#[test]
fn no_new_privileges_is_set_in_separate_process() {
    const CHILD: &str = "CAROSIGN_TEST_NO_NEW_PRIVILEGES_CHILD";
    if std::env::var_os(CHILD).is_some() {
        set_no_new_privileges().expect("установить ограничение");
        set_no_new_privileges().expect("повторная проверка ограничения");
        return;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("process::privileges::tests::no_new_privileges_is_set_in_separate_process")
        .env(CHILD, "1")
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
                std::thread::sleep(Duration::from_millis(10));
            }
            other => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("помощник не завершился вовремя: {other:?}");
            }
        }
    }
}
