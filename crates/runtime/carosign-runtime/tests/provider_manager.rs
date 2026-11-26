//! Provider Manager: manifesto, profiles, selection, zero providers.
//!
//! ⚠️ What is being checked is what breaks quietly: an unfamiliar manifesto field, a
//! dangerous path, a profile without rules, an alphabetical choice, binding registrations to
//! someone else's device.

use carosign_core::device::DeviceLocator;
use carosign_runtime::provider::inventory::{Inventory, InventoryEntry};
use carosign_runtime::provider::manager::{Installed, ManagerError, ProviderManager};
use carosign_runtime::provider::packages::{self, Resolution, check_relative_path};
use carosign_runtime::provider::session::{Session, SessionError};

const MANIFEST: &str = r#"
schema = 1
[package]
id = "org.example.carosign.gxfp5130"
version = "1.0.0"
[worker]
executable = "worker"
instance_model = "per-device"
[protocol]
major = 1
min_minor = 4
max_minor = 4
[[profiles]]
id = "matebook-gt14"
path = "profiles/matebook-gt14.toml"
"#;

const PROFILE: &str = r#"
id = "matebook-gt14"
match.usb.idVendor = "27c6"
match.usb.idProduct = "5a13"
"#;

fn inventory(signals: &[(&str, &str)]) -> Inventory {
    let mut inventory = Inventory::new();
    inventory.push(InventoryEntry {
        locator: DeviceLocator::new("/dev/hidraw0").expect("адрес"),
        signals: signals
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
    });
    inventory
}

fn package(id: &str) -> Installed {
    Installed {
        manifest: packages::Manifest {
            schema: 1,
            package: packages::PackageSection {
                id: id.to_string(),
                version: "1.0.0".into(),
            },
            worker: packages::WorkerSection {
                executable: "worker".into(),
                instance_model: packages::InstanceModel::PerDevice,
            },
            protocol: packages::ProtocolSection {
                major: 1,
                min_minor: 4,
                max_minor: 4,
            },
            profiles: vec![packages::ProfileEntry {
                id: "matebook-gt14".into(),
                path: "profiles/matebook-gt14.toml".into(),
            }],
        },
        profiles: vec![packages::parse_profile(PROFILE).expect("профиль")],
        trusted: true,
    }
}

#[test]
fn undeclared_or_duplicate_profiles_are_not_candidates() {
    let signals = inventory(&[("usb.idVendor", "27c6"), ("usb.idProduct", "5a13")]);
    let mut unlisted = package("org.example.unlisted");
    unlisted.manifest.profiles.clear();
    let mut duplicate_file = package("org.example.duplicate");
    duplicate_file
        .profiles
        .push(duplicate_file.profiles[0].clone());
    let mut duplicate_entry = package("org.example.entries");
    duplicate_entry
        .manifest
        .profiles
        .push(duplicate_entry.manifest.profiles[0].clone());
    let mut manager = ProviderManager::new();
    for item in [unlisted, duplicate_file, duplicate_entry] {
        manager.install(item).unwrap();
    }
    assert_eq!(manager.chosen(&signals), None);
}

#[test]
fn manifest_parses_and_keeps_what_matters() {
    let manifest = packages::parse_manifest(MANIFEST).expect("манифест верен");
    assert_eq!(manifest.package.id, "org.example.carosign.gxfp5130");
    assert_eq!(manifest.worker.executable, "worker");
    assert_eq!(manifest.profiles.len(), 1);
}

#[test]
fn unknown_field_in_privileged_section_is_refused() {
    // ⚠️ Document 12: `deny_unknown_fields` in privileged sections.
    let text = MANIFEST.replace(
        "instance_model = \"per-device\"",
        "instance_model = \"per-device\"\n    caps = \"all\"",
    );
    assert!(packages::parse_manifest(&text).is_err());
}

#[test]
fn unsafe_executable_path_is_refused() {
    for path in ["/bin/sh", "../worker", "a/../../b", "worker/", ""] {
        assert!(
            check_relative_path(path).is_err(),
            "путь {path:?} обязан быть отвергнут"
        );
    }
    assert!(check_relative_path("worker").is_ok());
    assert!(check_relative_path("bin/worker-2").is_ok());
}

#[test]
fn manifest_rejects_duplicate_fields_sections_and_unsafe_profiles() {
    let variants = [
        MANIFEST.replace("schema = 1", "schema = 1\nschema = 1"),
        MANIFEST.replace(
            "executable = \"worker\"",
            "executable = \"worker\"\nexecutable = \"other\"",
        ),
        format!("{MANIFEST}\n[worker]\nexecutable = \"worker\"\ninstance_model = \"per-device\"\n"),
        MANIFEST.replace("executable = \"worker\"", "executable = \"../worker\""),
        MANIFEST.replace("profiles/matebook-gt14.toml", "../profile.toml"),
        MANIFEST.replace("instance_model = \"per-device\"", ""),
        MANIFEST.replace("min_minor = 4", "min_minor = 5"),
        MANIFEST.replace(
            "min_minor = 4\nmax_minor = 4",
            "min_minor = 0\nmax_minor = 2",
        ),
        format!(
            "{MANIFEST}\n[[profiles]]\nid = \"matebook-gt14\"\npath = \"profiles/other.toml\"\n"
        ),
    ];
    for text in variants {
        assert!(
            packages::parse_manifest(&text).is_err(),
            "повреждённый манифест принят"
        );
    }
}

#[test]
fn manifest_accepts_older_minimum_when_minor_four_is_available() {
    let text = MANIFEST.replace("min_minor = 4", "min_minor = 0");
    assert!(packages::parse_manifest(&text).is_ok());
}

#[test]
fn manifest_requires_a_protocol_version_the_platform_can_negotiate() {
    // A maximum of >=4 does not prove the intersection of ranges.
    let future_only = MANIFEST.replace(
        "min_minor = 4\nmax_minor = 4",
        "min_minor = 5\nmax_minor = 9",
    );
    assert!(matches!(
        packages::parse_manifest(&future_only),
        Err(packages::ManifestError::Invalid {
            field: "protocol",
            ..
        })
    ));
    let incompatible_major = MANIFEST.replace("major = 1", "major = 2");
    assert!(packages::parse_manifest(&incompatible_major).is_err());
    let overlaps = MANIFEST.replace(
        "min_minor = 4\nmax_minor = 4",
        "min_minor = 2\nmax_minor = 9",
    );
    assert!(packages::parse_manifest(&overlaps).is_ok());
}

#[test]
fn manifest_uses_toml_string_semantics_before_validating_paths() {
    let literal = MANIFEST.replace("\"worker\"", "'worker'");
    assert_eq!(
        packages::parse_manifest(&literal)
            .unwrap()
            .worker
            .executable,
        "worker"
    );
    let escaped = MANIFEST.replace("\"worker\"", r#""\u002e\u002e/worker""#);
    assert!(packages::parse_manifest(&escaped).is_err());
    let malformed = MANIFEST.replace("\"worker\"", r#""worker\q""#);
    assert!(packages::parse_manifest(&malformed).is_err());
    let with_hash = MANIFEST.replace("\"worker\"", "'worker#1'");
    assert_eq!(
        packages::parse_manifest(&with_hash)
            .unwrap()
            .worker
            .executable,
        "worker#1"
    );
}

#[test]
fn manifest_validates_and_normalizes_package_identity() {
    for invalid in [
        "single",
        "org..example",
        "org.example/worker",
        "org.пример",
        &format!("{}.b", "x".repeat(127)),
    ] {
        let text = MANIFEST.replace("org.example.carosign.gxfp5130", invalid);
        assert!(packages::parse_manifest(&text).is_err());
    }
    for invalid in ["1.0", "latest", "1.0.0.0", "1.0.-1"] {
        let text = MANIFEST.replace("1.0.0", invalid);
        assert!(packages::parse_manifest(&text).is_err());
    }
    let text = MANIFEST.replace(
        "org.example.carosign.gxfp5130",
        "Org.Example.CaroSign.GXFP5130",
    );
    assert_eq!(
        packages::parse_manifest(&text).unwrap().package.id,
        "org.example.carosign.gxfp5130"
    );
}

#[test]
fn profile_without_rules_is_refused() {
    let text = r#"id = "empty""#;
    assert!(packages::parse_profile(text).is_err());
}

#[test]
fn profile_parses_toml_before_matching_inventory() {
    use carosign_runtime::provider::packages::profile::matches;
    let text = "id = 'test'\n[match.usb]\nidVendor = \"27\\u00636\" # производитель\n";
    let profile = packages::parse_profile(text).unwrap();
    assert!(matches(&profile, &[("usb.idVendor".into(), "27c6".into())]));
    for bad in [
        "id = 'test'\nmatch.usb.idVendor = 27",
        "id = 'test'\nmatch.usb.idVendor = ''",
        "id = 'test'\nmatch.usb.idVendor = '27c6'\nmatch.usb.idVendor = 'other'",
        "id = 'test'\nmatch.\"usb.idVendor\" = '27c6'",
        "id = 'test'\nmatch.usb.idVendor = \"27\\q\"",
    ] {
        assert!(packages::parse_profile(bad).is_err(), "{bad}");
    }
}

#[test]
fn constructed_empty_profile_does_not_match_any_device() {
    use carosign_runtime::provider::packages::profile::{MatchRule, Profile, matches};
    let inventory = vec![("".to_owned(), "".to_owned())];
    let mut profile = Profile {
        id: "empty".into(),
        matches: Vec::new(),
    };
    assert!(!matches(&profile, &inventory));
    profile.matches.push(MatchRule {
        key: String::new(),
        value: String::new(),
    });
    assert!(!matches(&profile, &inventory));
}

#[test]
fn zero_providers_is_healthy() {
    // P-12: Zero set is the norm.
    let manager = ProviderManager::new();
    assert!(manager.is_empty());
    assert_eq!(
        manager.chosen(&inventory(&[("usb.idVendor", "27c6")])),
        None
    );
}

#[test]
fn sole_match_is_chosen_and_ambiguous_is_not_guessed() {
    let mut manager = ProviderManager::new();
    manager
        .install(package("org.example.one"))
        .expect("ставится");
    let sole = manager.chosen(&inventory(&[
        ("usb.idVendor", "27c6"),
        ("usb.idProduct", "5a13"),
    ]));
    assert_eq!(
        sole.map(|candidate| candidate.package),
        Some("org.example.one".to_string())
    );

    manager
        .install(package("org.example.two"))
        .expect("ставится");
    // Two of them fit, alphabetical choice is forbidden.
    assert_eq!(
        manager.chosen(&inventory(&[
            ("usb.idVendor", "27c6"),
            ("usb.idProduct", "5a13"),
        ])),
        None
    );

    // The priority of politics removes ambiguity.
    manager.set_preferred(vec!["org.example.two".into()]);
    assert_eq!(
        manager
            .chosen(&inventory(&[
                ("usb.idVendor", "27c6"),
                ("usb.idProduct", "5a13"),
            ]))
            .map(|candidate| candidate.package),
        Some("org.example.two".to_string())
    );
}

#[test]
fn untrusted_package_is_not_considered() {
    let mut manager = ProviderManager::new();
    let mut untrusted = package("org.example.evil");
    untrusted.trusted = false;
    assert_eq!(manager.install(untrusted), Err(ManagerError::Untrusted));
}

#[test]
fn uninstall_keeps_nothing_and_does_not_panic() {
    let mut manager = ProviderManager::new();
    manager
        .install(package("org.example.one"))
        .expect("ставится");
    assert!(manager.uninstall("org.example.one"));
    assert!(!manager.uninstall("org.example.one"));
    assert!(manager.is_empty());
}

#[test]
fn duplicate_install_is_refused() {
    let mut manager = ProviderManager::new();
    manager
        .install(package("org.example.one"))
        .expect("ставится");
    assert_eq!(
        manager.install(package("org.example.one")),
        Err(ManagerError::AlreadyInstalled)
    );
    assert_eq!(
        manager.install(package("ORG.EXAMPLE.ONE")),
        Err(ManagerError::AlreadyInstalled)
    );
}

#[test]
fn admin_choice_wins_but_only_if_it_matches() {
    let mut manager = ProviderManager::new();
    manager
        .install(package("org.example.one"))
        .expect("ставится");
    let resolved = manager.resolve(
        &inventory(&[("usb.idVendor", "27c6"), ("usb.idProduct", "5a13")]),
        Some("org.example.one"),
    );
    assert!(matches!(resolved[0].1, Resolution::Chosen(_)));

    let resolved = manager.resolve(
        &inventory(&[("usb.idVendor", "27c6"), ("usb.idProduct", "5a13")]),
        Some("org.example.missing"),
    );
    assert!(matches!(resolved[0].1, Resolution::AdminChoiceInvalid(_)));
}

#[test]
fn non_matching_inventory_yields_none() {
    let mut manager = ProviderManager::new();
    manager
        .install(package("org.example.one"))
        .expect("ставится");
    let resolved = manager.resolve(&inventory(&[("usb.idVendor", "0000")]), None);
    assert!(matches!(resolved[0].1, Resolution::None));
}

#[test]
fn worker_loss_leaves_hardware_uncertain() {
    use carosign_core::device::DeviceEpoch;
    use carosign_core::ids::WorkerInstanceId;
    use carosign_runtime::ports::QuiesceAssessment;

    let mut manager = ProviderManager::new();
    let instance = WorkerInstanceId::from_bytes([7u8; 16]).expect("валид");
    manager
        .supervisor_mut()
        .start(instance, DeviceEpoch::FIRST)
        .expect("запуск");
    assert_eq!(manager.after_loss(), QuiesceAssessment::Uncertain);
}

#[test]
fn session_refuses_second_terminal() {
    use carosign_core::ids::CommandId;
    let mut session = Session::default();
    session.accept(CommandId::from_bytes([1u8; 16]).expect("валид"));
    session.terminal().expect("первый итог");
    assert_eq!(session.terminal(), Err(SessionError::SecondTerminal));
}

#[test]
fn session_rejects_terminal_before_acceptance_without_changing_state() {
    let mut session = Session::default();
    let original = session.clone();
    assert_eq!(session.terminal(), Err(SessionError::NotAccepted));
    assert_eq!(session, original);
}
