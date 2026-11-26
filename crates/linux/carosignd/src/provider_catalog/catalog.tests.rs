use super::*;

#[test]
fn allowlist_rejects_aliases_and_ambiguous_ownership() {
    let id = "01010101010101010101010101010101";
    let other = "02020202020202020202020202020202";
    let entry = format!("fingerprint 412 413 {id}");
    assert_eq!(ProviderCatalog::parse(entry.as_bytes()).unwrap().len(), 1);
    let configured = format!("{entry} /etc/carosign/example.conf");
    let profile = ProviderCatalog::parse(configured.as_bytes())
        .unwrap()
        .remove(0);
    assert_eq!(
        profile
            .devices
            .values()
            .next()
            .and_then(|device| device.profile.as_deref()),
        Some(Path::new("/etc/carosign/example.conf"))
    );
    let multi = format!("{configured}\nfingerprint 412 413 {other} /etc/carosign/second.conf");
    let publications = ProviderCatalog::parse(multi.as_bytes()).unwrap();
    assert_eq!(publications.len(), 1);
    assert_eq!(publications[0].devices.len(), 2);
    let other_id = carosign_core::ids::AuthenticatorId::from_bytes([2; 16]).unwrap();
    assert_eq!(
        publications[0].devices[&other_id].profile.as_deref(),
        Some(Path::new("/etc/carosign/second.conf"))
    );
    for invalid in ["relative.conf", "/etc/example.conf extra"] {
        assert!(ProviderCatalog::parse(format!("{entry} {invalid}").as_bytes()).is_err());
    }
    for extra in [
        format!("fingerprint 414 415 {other}"),
        format!("fingerprint 412 415 {other}"),
        format!("fingerprint 412 413 {id}"),
        format!("another 412 415 {other}"),
        format!("another 414 415 {id}"),
        format!("../foreign 414 415 {other}"),
        format!("root 0 415 {other}"),
    ] {
        assert!(ProviderCatalog::parse(format!("{entry}\n{extra}").as_bytes()).is_err());
    }
}

#[test]
fn disabling_one_device_preserves_its_profile_and_leaves_its_sibling_enabled() {
    let text = b"mock 412 413 01010101010101010101010101010101 /etc/carosign/first.conf disabled\nmock 412 413 02020202020202020202020202020202";
    let publications = ProviderCatalog::parse(text).unwrap();
    let publication = &publications[0];
    let first = carosign_core::ids::AuthenticatorId::from_bytes([1; 16]).unwrap();
    let second = carosign_core::ids::AuthenticatorId::from_bytes([2; 16]).unwrap();
    assert_eq!(
        publication.devices[&first].profile.as_deref(),
        Some(Path::new("/etc/carosign/first.conf"))
    );
    assert_eq!(
        ProviderCatalog::require_enabled(publication, first)
            .unwrap_err()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
    assert!(ProviderCatalog::require_enabled(publication, second).is_ok());
    let catalog = ProviderCatalog::new(true);
    let mut descriptor = AuthenticatorDescriptor {
        available: true,
        ..Default::default()
    };
    assert_eq!(
        catalog.classify_device(publication, first, &mut descriptor),
        UnavailabilityReason::DisabledByAdministrator
    );
    let inventory_only = b"mock 412 413 01010101010101010101010101010101 disabled";
    let publications = ProviderCatalog::parse(inventory_only).unwrap();
    assert!(publications[0].devices[&first].disabled);
    assert!(publications[0].devices[&first].profile.is_none());
}

#[test]
fn a_device_without_an_execution_profile_is_inventory_only() {
    let id = carosign_core::ids::AuthenticatorId::from_bytes([1; 16]).unwrap();
    let publications =
        ProviderCatalog::parse(b"mock 412 413 01010101010101010101010101010101").unwrap();
    let mut descriptor = AuthenticatorDescriptor {
        available: true,
        ..Default::default()
    };
    let catalog = ProviderCatalog::new(false);
    assert_eq!(
        catalog.classify_device(&publications[0], id, &mut descriptor),
        UnavailabilityReason::InventoryOnly
    );
}
