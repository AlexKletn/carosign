use super::*;

#[test]
fn silent_client_cannot_hold_hello_open() {
    let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let peer = carosign_linux::sys::PeerUser {
        pid: std::process::id(),
        uid: 1000,
        gid: 1000,
    };
    let credentials = carosign_linux_ipc::credentials::PeerCredentials::from_kernel(
        peer.pid, peer.uid, peer.gid,
    )
    .unwrap();
    let connection = Connection {
        session: carosign_linux_ipc::client::session::ClientSession::new(credentials, [1; 16]),
        peer,
        fd: pair.platform,
    };
    assert!(matches!(
        wait_for_hello(&connection, std::time::Duration::from_millis(5)),
        Err(Ended::Refused(_))
    ));
    drop(pair.worker);
}

#[test]
fn unsupported_requests_sound_in_english() {
    // ⚠️ The journal screens everything outside of ASCII: the word must be Latin.
    for request in [
        ClientRequest::ListAuthenticators,
        ClientRequest::GetHealth,
        ClientRequest::BeginEnrollment {
            idempotency_key: vec![1],
            authenticator: vec![2],
            label: "палец".into(),
            subject: carosign_protocol::carosign::common::v1::SubjectSelector {
                selector: Some(
                    carosign_protocol::carosign::common::v1::subject_selector::Selector::CurrentSubject(
                        true,
                    ),
                ),
            },
            replace: None,
            authority_request_ref: vec![],
        },
    ] {
        let word = unsupported(&request);
        assert!(word.is_ascii(), "не ASCII: {word}");
        assert_ne!(word, "begin verification");
    }
}
