use super::*;

#[test]
fn scoped_queries_are_not_reinterpreted_as_list_all() {
    for request in [
        ClientRequest::GetAuthenticator {
            authenticator: vec![1; 16],
        },
        ClientRequest::ListEnrollments {
            authenticator: vec![1; 16],
            subject: common::SubjectSelector {
                selector: Some(common::subject_selector::Selector::CurrentSubject(true)),
            },
            authority_request_ref: vec![],
        },
    ] {
        assert_eq!(to_domain(&request).unwrap(), None);
    }
    assert_eq!(
        to_domain(&ClientRequest::GetAuthenticator {
            authenticator: vec![0; 16]
        }),
        Err(MappingError::BadId)
    );
}
