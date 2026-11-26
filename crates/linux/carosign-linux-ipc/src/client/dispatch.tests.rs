use super::*;

#[test]
fn only_hello_before_hello() {
    let hello = ClientRequest::Hello {
        major: 1,
        min_minor: 0,
        max_minor: 0,
    };
    assert!(allowed(true, &hello));
    assert!(!allowed(true, &ClientRequest::GetHealth));
    assert!(!allowed(false, &hello));
    assert!(allowed(false, &ClientRequest::GetHealth));
}
