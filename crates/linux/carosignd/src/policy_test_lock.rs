pub(crate) fn policy_test_lock() -> std::fs::File {
    use std::os::fd::AsRawFd;
    let lock = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open("/dev/shm/carosign-policy-tests.lock")
        .unwrap();
    // SAFETY: lock holds the descriptor open until the end of the test.
    assert_eq!(unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) }, 0);
    lock
}
