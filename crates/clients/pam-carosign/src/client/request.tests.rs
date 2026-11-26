use super::super::*;
use carosign_protocol::carosign::common::v1 as common;

#[test]
fn request_debug_does_not_expose_caller_fields() {
    let request = Request {
        user: "личное-имя".into(),
        service: "частная-служба".into(),
        connection: "частное-соединение".into(),
    };
    let rendered = format!("{request:?}");
    for field in [&request.user, &request.service, &request.connection] {
        assert!(!rendered.contains(field));
    }
    assert!(rendered.contains("Request"));
}

#[test]
fn request_uses_explicit_account_and_rejects_invalid_ids() {
    let request = Request {
        user: "alice".into(),
        service: "sudo".into(),
        connection: String::new(),
    };
    let purpose = common::VerificationPurpose::PurposePrivilegedAction;
    let wire = verification_request(&request, vec![1; 16], [2; 16], purpose).unwrap();
    assert!(matches!(wire.subject.unwrap().selector,
            Some(common::subject_selector::Selector::AccountHint(user)) if user == "alice"));
    assert_eq!(wire.purpose_hint, purpose as i32);
    assert!(verification_request(&request, vec![0; 16], [2; 16], purpose).is_err());
    assert!(verification_request(&request, vec![1; 16], [0; 16], purpose).is_err());
}
