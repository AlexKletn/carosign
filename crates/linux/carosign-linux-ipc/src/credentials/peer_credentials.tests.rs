use super::*;

#[test]
fn zero_pid_is_refused() {
    assert!(PeerCredentials::from_kernel(0, 0, 0).is_err());
}

#[test]
fn binding_differs_between_peers() {
    let a = PeerCredentials::from_kernel(100, 1000, 1000).unwrap();
    let b = PeerCredentials::from_kernel(101, 1000, 1000).unwrap();
    assert_ne!(a.binding(), b.binding());
    assert_eq!(a.binding().len(), 32);
}
