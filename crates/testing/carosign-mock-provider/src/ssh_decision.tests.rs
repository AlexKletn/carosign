use super::*;
use std::io::Write;
use std::os::fd::AsRawFd;

#[test]
fn decision_requires_fifo_and_accepts_only_explicit_choice() {
    let path = std::env::temp_dir().join(format!("carosign-mock-fifo-{}", std::process::id()));
    let path_string = path.to_str().unwrap().to_owned();
    let c_path = std::ffi::CString::new(path_string.as_str()).unwrap();
    // SAFETY: The path indicates a NUL-terminated string.
    assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
    let (worker, _peer) = std::os::unix::net::UnixStream::pair().unwrap();
    let writer_path = path.clone();
    let writer = std::thread::spawn(move || {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(writer_path)
            .unwrap();
        file.write_all(b"match\n").unwrap();
    });
    let result = wait_ssh_decision_at(worker.as_raw_fd(), &path_string, unsafe { libc::geteuid() });
    writer.join().unwrap();
    assert!(matches!(
        result,
        Ok(Some(provider::VerificationStatus::VerifyMatched))
    ));
    std::fs::remove_file(path).unwrap();
}
