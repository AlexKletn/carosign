use super::*;

#[test]
fn package_name_is_canonical_lowercase_reverse_dns() {
    let name = PackageName::new("Org.Example.CaroSign.Gxfp5130").expect("валид");
    assert_eq!(name.as_str(), "org.example.carosign.gxfp5130");
}

#[test]
fn package_name_rejects_nonsense() {
    assert!(PackageName::new("").is_err());
    assert!(PackageName::new("a").is_err()); // piece
    assert!(PackageName::new("a.").is_err());
    assert!(PackageName::new(".a").is_err());
    assert!(PackageName::new("a..b").is_err());
    assert!(PackageName::new("-a.b").is_err());
    assert!(PackageName::new("a b.c").is_err());
    assert!(PackageName::new("a/../../etc").is_err());
    assert!(PackageName::new(&format!("{}.b", "x".repeat(127))).is_err());
}

#[test]
fn version_format() {
    assert!(PackageVersion::new("1.0.0").is_ok());
    assert!(PackageVersion::new("1.0").is_err());
    assert!(PackageVersion::new("1.0.0-rc1").is_err());
    assert!(PackageVersion::new("").is_err());
}
