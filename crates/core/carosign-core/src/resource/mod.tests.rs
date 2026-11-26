use crate::ids::ResourceId;

use super::*;

fn claim(byte: u8, mode: AccessMode) -> ResourceClaim {
    ResourceClaim {
        id: ResourceId::from_bytes([byte; 16]).expect("ненулевой"),
        mode,
    }
}

#[test]
fn set_is_sorted_by_bytes_not_by_input_order() {
    let set = ResourceSet::new(vec![
        claim(0x10, AccessMode::Mediated),
        claim(0x02, AccessMode::Exclusive),
        claim(0x80, AccessMode::SharedRead),
    ])
    .expect("валид");
    let order: Vec<u8> = set.claims().iter().map(|c| c.id.as_bytes()[0]).collect();
    assert_eq!(order, vec![0x02, 0x10, 0x80]);
}

#[test]
fn duplicate_resource_is_rejected() {
    assert!(
        ResourceSet::new(vec![
            claim(1, AccessMode::Exclusive),
            claim(1, AccessMode::SharedRead)
        ])
        .is_err()
    );
}

#[test]
fn set_is_not_empty_and_is_bounded() {
    assert!(ResourceSet::new(Vec::new()).is_err());
    let too_many: Vec<_> = (1..=9u8).map(|b| claim(b, AccessMode::Exclusive)).collect();
    assert!(ResourceSet::new(too_many).is_err());
    let exactly: Vec<_> = (1..=8u8).map(|b| claim(b, AccessMode::Exclusive)).collect();
    assert!(ResourceSet::new(exactly).is_ok());
}
