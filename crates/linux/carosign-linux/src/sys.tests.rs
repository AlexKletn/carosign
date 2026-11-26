use super::*;

#[test]
fn peer_credentials_reject_incomplete_or_unrepresentable_pid() {
    let size = core::mem::size_of::<libc::ucred>() as libc::socklen_t;
    for pid in [0, -1, libc::pid_t::MIN] {
        let credentials = libc::ucred {
            pid,
            uid: 1000,
            gid: 1000,
        };
        assert!(matches!(
            checked_peer_credentials(credentials, size),
            Err(SysError {
                errno: libc::EPROTO
            })
        ));
    }
    let credentials = libc::ucred {
        pid: 1,
        uid: 1000,
        gid: 1000,
    };
    assert!(matches!(
        checked_peer_credentials(credentials, size - 1),
        Err(SysError {
            errno: libc::EPROTO
        })
    ));
    let pair = socketpair_seqpacket().unwrap();
    let peer = peer_credentials(&pair.platform).unwrap();
    assert_eq!(peer.pid, std::process::id());
}

#[test]
fn random_fills_short_reads_and_bounds_interruptions() {
    let mut calls = 0;
    let bytes = fill_random(|buffer| {
        calls += 1;
        if calls == 1 {
            return Err(SysError { errno: libc::EINTR });
        }
        let count = buffer.len().min(3);
        buffer[..count].fill(calls as u8);
        Ok(count)
    })
    .unwrap();
    assert_eq!(calls, 7);
    assert_eq!(&bytes[..3], &[2, 2, 2]);
    assert_eq!(bytes[15], 7);

    assert_eq!(fill_random(|_| Ok(0)), Err(SysError { errno: libc::EIO }));
    assert_eq!(
        fill_random(|_| Err(SysError {
            errno: libc::EAGAIN
        })),
        Err(SysError {
            errno: libc::EAGAIN
        })
    );
    let mut interrupts = 0;
    assert_eq!(
        fill_random(|_| {
            interrupts += 1;
            Err(SysError { errno: libc::EINTR })
        }),
        Err(SysError { errno: libc::EINTR })
    );
    assert_eq!(interrupts, 9);
}

#[test]
fn boot_id_requires_nonzero_canonical_kernel_format() {
    let id = parse_boot_id("00112233-4455-6677-8899-aabbccddeeff\n").unwrap();
    assert_eq!(
        id,
        [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff
        ]
    );
    for value in [
        "00000000-0000-0000-0000-000000000000",
        "00112233445566778899aabbccddeeff",
        "-00112233-4455-6677-8899aabbccddeeff",
        "00112233-4455-6677-8899-aabbccddeefg",
    ] {
        assert_eq!(
            parse_boot_id(value),
            Err(SysError {
                errno: libc::EINVAL
            })
        );
    }
}

#[test]
fn invalid_or_overflowing_timespec_is_not_a_timestamp() {
    assert_eq!(
        timespec_ns(libc::timespec {
            tv_sec: 3,
            tv_nsec: 7
        }),
        Ok(3_000_000_007)
    );
    for value in [
        libc::timespec {
            tv_sec: -1,
            tv_nsec: 0,
        },
        libc::timespec {
            tv_sec: 0,
            tv_nsec: -1,
        },
        libc::timespec {
            tv_sec: 0,
            tv_nsec: 1_000_000_000,
        },
    ] {
        assert_eq!(
            timespec_ns(value),
            Err(SysError {
                errno: libc::EINVAL
            })
        );
    }
    #[cfg(target_pointer_width = "64")]
    {
        let seconds = 20_000_000_000 as libc::time_t;
        assert_eq!(
            timespec_ns(libc::timespec {
                tv_sec: seconds,
                tv_nsec: 0
            }),
            Err(SysError {
                errno: libc::EOVERFLOW
            })
        );
    }
}

#[test]
fn boottime_moves_forward() {
    let a = boottime_ns().expect("часы");
    let b = boottime_ns().expect("часы");
    assert!(b >= a);
}

#[test]
fn boot_id_is_16_bytes() {
    let id = boot_id().expect("номер загрузки");
    assert!(id.iter().any(|byte| *byte != 0));
}

#[test]
fn socketpair_and_fd_pass() {
    let pair = socketpair_seqpacket().expect("пара");
    let memory = memfd("probe").expect("memfd");
    seal_read_only(&memory).expect("печать");
    assert!(!is_sealed(&memory).expect("проверка"));
    let readonly: OwnedFd = std::fs::File::open(format!("/proc/self/fd/{}", memory.as_raw_fd()))
        .unwrap()
        .into();
    assert!(is_sealed(&readonly).expect("проверка"));
    send_fd(&pair.platform, &readonly).expect("передано");
    let received = recv_fds(&pair.worker, 4).expect("принято");
    assert_eq!(received.len(), 1);
    assert!(is_sealed(&received[0]).expect("проверка принятого"));
    // SAFETY: The returned descriptor is valid, and `F_GETFD` only reads its flags.
    let flags = unsafe { libc::fcntl(received[0].as_raw_fd(), libc::F_GETFD) };
    assert_ne!(flags & libc::FD_CLOEXEC, 0);
    drop(received);
}

#[test]
fn sending_descriptor_to_closed_peer_returns_error() {
    let pair = socketpair_seqpacket().expect("пара");
    let memory = memfd("closed-peer").expect("memfd");
    // Another test can simultaneously create a process and briefly inherit the socket until
    // exec. shutdown completes the link itself even if there is such a copy; closing the
    // descriptor alone is not enough.
    // SAFETY: The socket belongs to the test and is valid until the call is returned.
    assert_eq!(
        unsafe { libc::shutdown(pair.worker.as_raw_fd(), libc::SHUT_RDWR) },
        0
    );
    drop(pair.worker);
    let error = send_fd(&pair.platform, &memory).expect_err("закрытый сокет");
    assert!(matches!(error.errno, libc::EPIPE | libc::ECONNRESET));
}

#[test]
fn truncated_ancillary_data_closes_every_received_fd() {
    let pair = socketpair_seqpacket().expect("пара");
    let memory = memfd("carosign-truncation-unique").expect("memfd");
    let target = std::fs::read_link(format!("/proc/self/fd/{}", memory.as_raw_fd())).unwrap();
    let count = || {
        std::fs::read_dir("/proc/self/fd")
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| std::fs::read_link(entry.path()).ok().as_ref() == Some(&target))
            .count()
    };
    let before = count();
    send_fds(
        &pair.platform,
        &[
            &memory, &memory, &memory, &memory, &memory, &memory, &memory, &memory,
        ],
    )
    .expect("отправлено");
    let error = recv_fds(&pair.worker, 1).expect_err("обрезанный контроль — отказ");
    assert_eq!(error.errno, libc::EMSGSIZE);
    assert_eq!(count(), before, "принятые до обрезки дескрипторы закрыты");
}

#[test]
fn seals_require_seal_seal_and_cannot_be_added_afterwards() {
    let partial = memfd("partial-seals").expect("memfd");
    let flags = libc::F_SEAL_WRITE | libc::F_SEAL_SHRINK | libc::F_SEAL_GROW;
    // SAFETY: The descriptor is open and valid, the argument is a set of seals.
    assert_eq!(
        unsafe { libc::fcntl(partial.as_raw_fd(), libc::F_ADD_SEALS, flags) },
        0
    );
    let partial_readonly: OwnedFd =
        std::fs::File::open(format!("/proc/self/fd/{}", partial.as_raw_fd()))
            .unwrap()
            .into();
    assert!(!is_sealed(&partial_readonly).expect("проверка"));

    let complete = memfd("complete-seals").expect("memfd");
    seal_read_only(&complete).expect("печати");
    assert!(!is_sealed(&complete).expect("проверка"));
    let readonly: OwnedFd = std::fs::File::open(format!("/proc/self/fd/{}", complete.as_raw_fd()))
        .unwrap()
        .into();
    assert!(is_sealed(&readonly).expect("проверка"));
    // SAFETY: The descriptor is valid, the command does not change the process memory.
    let flags = unsafe { libc::fcntl(complete.as_raw_fd(), libc::F_GET_SEALS) };
    assert_ne!(flags & libc::F_SEAL_SEAL, 0);
    // Printing a file does not make the descriptor open only for reading.
    // SAFETY: The descriptor is valid, the command reads the opening flags.
    let mode = unsafe { libc::fcntl(complete.as_raw_fd(), libc::F_GETFL) };
    assert_eq!(mode & libc::O_ACCMODE, libc::O_RDWR);
}

#[test]
fn stat_sees_regular_file_and_owner() {
    let executable = std::env::current_exe().expect("путь");
    let identity = stat(executable.to_str().expect("UTF-8 путь")).expect("stat");
    assert!(identity.regular);
    assert!(identity.inode != 0);
    assert!(!stat("/proc/self/exe").expect("ссылка").regular);
    let fd = open_read(executable.to_str().unwrap()).expect("открыт");
    assert_eq!(launch_metadata(&fd).expect("fstat").identity, identity);
}

#[test]
fn pidfd_observes_a_live_process() {
    assert!(matches!(
        PidFd::open(0),
        Err(SysError {
            errno: libc::EINVAL
        })
    ));
    assert!(matches!(
        PidFd::open(u32::MAX),
        Err(SysError {
            errno: libc::EINVAL
        })
    ));
    let me = PidFd::open(std::process::id()).expect("pidfd");
    assert!(!me.has_exited().expect("состояние"));
}

#[test]
fn pidfd_still_reports_exit_after_child_is_reaped() {
    let mut child = std::process::Command::new("/bin/true").spawn().unwrap();
    let process = PidFd::open(child.id()).unwrap();
    assert!(child.wait().unwrap().success());
    assert!(process.has_exited().unwrap());
}

#[test]
fn pidfd_stops_and_observes_actual_child_with_bounded_wait() {
    let mut child = std::process::Command::new("/bin/sleep")
        .arg("30")
        .spawn()
        .unwrap();
    let process = PidFd::open(child.id()).unwrap();
    assert!(!process.wait_exited(std::time::Duration::ZERO).unwrap());
    process.kill().unwrap();
    assert!(
        process
            .wait_exited(std::time::Duration::from_secs(5))
            .unwrap()
    );
    assert!(!child.wait().unwrap().success());
    assert!(process.wait_exited(std::time::Duration::ZERO).unwrap());
}

#[test]
fn missing_file_is_an_error_not_zero() {
    assert!(open_read("/nonexistent/definitely").is_err());
    assert!(stat("/nonexistent/definitely").is_err());
}
