use carosign_protocol::carosign::provider::v1 as provider;

use super::*;

#[test]
fn mock_state_survives_reload_and_rejects_damage() {
    let path = std::env::temp_dir().join(format!("carosign-mock-state-{}", std::process::id()));
    let mut records = MockEnrollments::new();
    assert!(load_mock_state(&path).unwrap().is_empty());
    let subject = common::SubjectRef {
        authority_id: vec![1; 16],
        subject_id: vec![2; 16],
    };
    let enrollment = common::EnrollmentRef {
        enrollment_id: vec![3; 16],
        authenticator_id: vec![4; 16],
        generation: 1,
    };
    records.insert(
        enrollment.enrollment_id.clone(),
        (subject, enrollment, vec![5; 16]),
    );
    save_mock_state(&path, &records).unwrap();
    assert_eq!(load_mock_state(&path).unwrap(), records);
    std::fs::write(&path, [0xff]).unwrap();
    assert!(load_mock_state(&path).is_err());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn receive_rejects_truncated_packet_and_owns_descriptor() {
    use std::os::fd::{FromRawFd, OwnedFd};
    let mut fds = [0; 2];
    // SAFETY: The array has room for two descriptors.
    assert_eq!(
        unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_SEQPACKET, 0, fds.as_mut_ptr()) },
        0
    );
    // SAFETY: Each descriptor has a single owner.
    let (_sender, _receiver) =
        unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    let packet = vec![1u8; carosign_protocol::limits::FRAME_MAX + 1];
    // SAFETY: The buffer and descriptor are active during the call.
    assert_eq!(
        unsafe { libc::send(fds[0], packet.as_ptr().cast(), packet.len(), 0) },
        packet.len() as isize
    );
    assert!(receive(fds[1]).is_err());

    let mut byte = [1u8];
    let mut iov = libc::iovec {
        iov_base: byte.as_mut_ptr().cast(),
        iov_len: 1,
    };
    // The u64 array provides alignment of the cmsghdr header.
    let mut control = [0u64; 4];
    // SAFETY: The zero structure is suitable for filling sendmsg fields.
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;
    message.msg_control = control.as_mut_ptr().cast();
    // SAFETY: sized RawFd And it fits into control.
    message.msg_controllen =
        unsafe { libc::CMSG_SPACE(std::mem::size_of::<RawFd>() as u32) as usize };
    // SAFETY: Control is aligned and has room for a header and one RawFd.
    unsafe {
        let header = libc::CMSG_FIRSTHDR(&message);
        assert!(!header.is_null());
        (*header).cmsg_level = libc::SOL_SOCKET;
        (*header).cmsg_type = libc::SCM_RIGHTS;
        (*header).cmsg_len = libc::CMSG_LEN(std::mem::size_of::<RawFd>() as u32) as usize;
        std::ptr::write_unaligned(libc::CMSG_DATA(header).cast::<RawFd>(), fds[0]);
        assert_eq!(libc::sendmsg(fds[0], &message, 0), 1);
    }
    assert_eq!(receive(fds[1]).unwrap().unwrap().1.len(), 1);
}

#[test]
fn channel_requires_a_connected_unix_seqpacket_socket() {
    use std::os::fd::{FromRawFd, OwnedFd};
    let mut fds = [0; 2];
    // SAFETY: The array has room for two descriptors.
    assert_eq!(
        unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_SEQPACKET, 0, fds.as_mut_ptr()) },
        0
    );
    // SAFETY: Each returned descriptor is transferred to exactly one owner.
    let (_a, _b) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    assert!(validate_channel(fds[0]).is_ok());
    assert!(validate_channel(-1).is_err());
    let mut streams = [0; 2];
    // SAFETY: The array has room for two descriptors.
    assert_eq!(
        unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, streams.as_mut_ptr()) },
        0
    );
    // SAFETY: Each returned descriptor is transferred to exactly one owner.
    let (_a, _b) = unsafe {
        (
            OwnedFd::from_raw_fd(streams[0]),
            OwnedFd::from_raw_fd(streams[1]),
        )
    };
    assert!(validate_channel(streams[0]).is_err());
}

#[test]
fn verification_rejects_empty_foreign_and_duplicate_bindings() {
    let auth = common::AuthenticatorRef {
        authenticator_id: vec![1; 16],
        device_epoch: 1,
    };
    let binding = provider::EnrollmentBinding {
        enrollment: Some(common::EnrollmentRef {
            enrollment_id: vec![2; 16],
            authenticator_id: vec![1; 16],
            generation: 1,
        }),
        kind: provider::EnrollmentBindingKind::EnrollmentBindingProviderOpaque as i32,
        provider_opaque_ref: vec![3],
        format_id: String::new(),
        format_version: 0,
    };
    let mut request = provider::StartVerification {
        authenticator: Some(auth.clone()),
        subject: Some(common::SubjectRef {
            authority_id: vec![4; 16],
            subject_id: vec![5; 16],
        }),
        purpose: common::VerificationPurpose::PurposeSessionLogin as i32,
        challenge_ref: vec![6; 16],
        recipient_ref: vec![7; 16],
        permitted_enrollments: vec![binding.clone()],
    };
    assert!(valid_verification(&request, &auth));
    request.permitted_enrollments.clear();
    assert!(!valid_verification(&request, &auth));
    request.permitted_enrollments = vec![binding.clone(), binding.clone()];
    assert!(!valid_verification(&request, &auth));
    request.permitted_enrollments = vec![binding];
    request.permitted_enrollments[0]
        .enrollment
        .as_mut()
        .unwrap()
        .authenticator_id = vec![8; 16];
    assert!(!valid_verification(&request, &auth));
}
