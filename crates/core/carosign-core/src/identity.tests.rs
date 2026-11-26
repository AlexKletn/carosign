use super::*;
use crate::ids::{AuthorityId, SubjectId};

fn subject(authority_byte: u8, subject_byte: u8) -> SubjectRef {
    SubjectRef {
        authority: AuthorityId::from_bytes([authority_byte; 16]).expect("ненулевой"),
        subject: SubjectId::from_bytes([subject_byte; 16]).expect("ненулевой"),
    }
}

#[test]
fn same_subject_different_authority_is_unequal() {
    // C-02: identical SubjectId different AuthorityId - not the same couple.
    let a = subject(1, 7);
    let b = subject(2, 7);
    assert_ne!(a, b);
    assert_ne!(a.to_canonical(), b.to_canonical());
}

#[test]
fn hash_maps_keep_subjects_apart() {
    // C-02b: The hash table does not mix pairs or mix areas.
    use std::collections::{HashMap, HashSet};

    let a = subject(1, 7);
    let b = subject(2, 7);
    let mut set = HashSet::new();
    assert!(set.insert(a));
    assert!(set.insert(b));
    assert_eq!(set.len(), 2);

    let mut map = HashMap::new();
    map.insert(a, "первый");
    map.insert(b, "второй");
    assert_eq!(map.get(&a), Some(&"первый"));
    assert_eq!(map.get(&b), Some(&"второй"));
}

#[test]
fn scope_prefix_differs_by_authority() {
    // Area prefix: authority | subject | authenticator, 48 bytes.
    let prefix = |s: SubjectRef| {
        let mut out = [0u8; SUBJECT_SCOPE_PREFIX_LEN];
        out[..SUBJECT_REF_LEN].copy_from_slice(&s.to_canonical());
        out[SUBJECT_REF_LEN..].copy_from_slice(&[3u8; 16]);
        out
    };
    assert_ne!(prefix(subject(1, 7)), prefix(subject(2, 7)));
}

#[test]
fn canonical_round_trip() {
    let s = subject(0xaa, 0xbb);
    let bytes = s.to_canonical();
    assert_eq!(bytes.len(), SUBJECT_REF_LEN);
    assert_eq!(SubjectRef::from_canonical(&bytes), Ok(s));
    assert!(SubjectRef::from_canonical(&bytes[..31]).is_err());
    assert!(SubjectRef::from_canonical(&[0u8; 32]).is_err());
}

#[test]
fn account_hint_is_bounded_and_plain() {
    assert!(AccountHint::new("alex").is_ok());
    assert!(AccountHint::new("").is_err());
    assert!(AccountHint::new(&"x".repeat(65)).is_err());
    assert!(AccountHint::new("ali\u{0000}ce").is_err());
}

#[test]
fn opaque_selector_is_bounded() {
    assert!(SubjectSelector::opaque(vec![1]).is_ok());
    assert!(SubjectSelector::opaque(Vec::new()).is_err());
    assert!(SubjectSelector::opaque(vec![1; 257]).is_err());
}

#[test]
fn subject_selector_debug_does_not_expose_opaque_bytes() {
    let selector = SubjectSelector::opaque(b"private-selector".to_vec()).unwrap();
    assert_eq!(format!("{selector:?}"), "SubjectSelector::Opaque(..)");
}
