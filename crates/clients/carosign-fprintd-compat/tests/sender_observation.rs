//! Exercises D-Bus sender observation across process boundaries.

use carosign_fprintd_compat::sender::observe_sender;
use dbus::blocking::Connection;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};

#[test]
#[ignore = "требует отдельную сессионную шину D-Bus с ProcessFD"]
fn process_sender_disappearance() {
    if std::env::var_os("CAROSIGN_SENDER_CHILD").is_some() {
        let bus = Connection::new_session().unwrap();
        println!("{}", bus.unique_name());
        std::io::stdout().flush().unwrap();
        let mut b = [0u8; 1];
        let _ = std::io::stdin().read(&mut b);
        return;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "process_sender_disappearance",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("CAROSIGN_SENDER_CHILD", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    let sender = loop {
        line.clear();
        assert_ne!(output.read_line(&mut line).unwrap(), 0);
        if let Some(name) = line.split_whitespace().find(|word| word.starts_with(':')) {
            break name.to_owned();
        }
    };
    let bus = Connection::new_session().unwrap();
    assert!(observe_sender(&bus, "org.freedesktop.DBus").is_err());
    let observed = observe_sender(&bus, &sender).unwrap();
    assert_eq!(observed.unique_name(), sender);
    // The credentials and ProcessFD come from the same reply. Neither establishes bus trust or authority.
    assert_eq!(
        observed.credentials(),
        (unsafe { libc::geteuid() }, child.id())
    );
    assert!(observed.still_owned(&bus));
    drop(child.stdin.take());
    assert!(child.wait().unwrap().success());
    assert!(!observed.still_owned(&bus));
    assert!(observe_sender(&bus, &sender).is_err());
}
