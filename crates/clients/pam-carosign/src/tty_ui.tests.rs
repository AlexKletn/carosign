use super::*;
use std::os::fd::FromRawFd;

fn pty() -> (std::fs::File, TtyUi) {
    let mut master = -1;
    let mut slave = -1;
    let rc = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    assert_eq!(rc, 0);
    let master = unsafe { std::fs::File::from_raw_fd(master) };
    let slave = unsafe { std::fs::File::from_raw_fd(slave) };
    (
        master,
        TtyUi::from_tty(
            slave,
            Arc::new(Localizer::system(carosign_l10n::Preferences::new(
                "en", None,
            ))),
        )
        .unwrap(),
    )
}

#[test]
fn displays_observed_attempt_and_client_wait_limit() {
    let (mut master, mut ui) = pty();
    let attempts = Arc::new(AtomicUsize::new(2));
    ui.show(attempts, None, false);
    let mut buf = [0u8; 512];
    let count = master.read(&mut buf).unwrap();
    let text = std::str::from_utf8(&buf[..count]).unwrap();
    assert!(text.contains("attempt 2"), "{text}");
    assert!(text.contains("60 s remaining"), "{text}");
}

#[test]
fn daemon_deadline_shortens_wait_on_this_boot() {
    let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").unwrap();
    let hex: String = boot.trim().chars().filter(|c| *c != '-').collect();
    let boot_id = (0..16)
        .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
        .collect();
    let mut now: libc::timespec = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut now) },
        0
    );
    let deadline = carosign_protocol::carosign::common::v1::BootDeadline {
        boot_id,
        boottime_ns: now.tv_sec as u64 * 1_000_000_000 + now.tv_nsec as u64 + 5_000_000_000,
    };
    let remaining = wait_limit(Some(&deadline));
    assert!(remaining <= Duration::from_secs(5));
    assert!(remaining > Duration::from_secs(4));
}

#[test]
fn enter_cancels_without_changing_terminal_mode() {
    let (mut master, ui) = pty();
    let fd = ui.tty.as_raw_fd();
    let mut before: libc::termios = unsafe { std::mem::zeroed() };
    assert_eq!(unsafe { libc::tcgetattr(fd, &mut before) }, 0);
    let (_stop, done) = std::sync::mpsc::channel::<()>();
    let worker = std::thread::spawn(move || ui.wait(done));
    master.write_all(b"\n").unwrap();
    assert!(worker.join().unwrap());
    let mut after: libc::termios = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { libc::tcgetattr(master.as_raw_fd(), &mut after) },
        0
    );
    assert_eq!(
        before.c_lflag & !(libc::ECHO | libc::ECHONL),
        after.c_lflag & !(libc::ECHO | libc::ECHONL)
    );
    assert_eq!(after.c_lflag & (libc::ECHO | libc::ECHONL), libc::ECHO);
}

#[test]
fn early_password_is_not_echoed_and_echo_is_restored() {
    let (mut master, ui) = pty();
    master.write_all(b"early-password\n").unwrap();
    let mut fd = libc::pollfd {
        fd: master.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    assert_eq!(unsafe { libc::poll(&mut fd, 1, 100) }, 0);
    drop(ui);
    let mut settings: libc::termios = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { libc::tcgetattr(master.as_raw_fd(), &mut settings) },
        0
    );
    assert_ne!(settings.c_lflag & libc::ECHO, 0);
}

#[test]
fn early_password_is_discarded_before_the_next_prompt() {
    let (mut master, ui) = pty();
    let reader = ui.tty.try_clone().unwrap();
    let (_stop, done) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || ui.wait(done));
    master.write_all(b"password\n").unwrap();
    assert!(worker.join().unwrap());
    let mut fd = libc::pollfd {
        fd: reader.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    assert_eq!(unsafe { libc::poll(&mut fd, 1, 100) }, 0);
}

#[test]
fn immediate_result_still_shows_a_readable_status() {
    let (mut master, mut ui) = pty();
    let (stop, done) = std::sync::mpsc::channel();
    ui.show(Arc::new(AtomicUsize::new(1)), None, false);
    let started = Instant::now();
    stop.send(()).unwrap();
    assert!(!ui.wait(done));
    assert!(started.elapsed() >= Duration::from_millis(300));
    let mut output = [0u8; 512];
    let count = master.read(&mut output).unwrap();
    let text = String::from_utf8_lossy(&output[..count]);
    assert!(text.contains("Verifying"));
}
