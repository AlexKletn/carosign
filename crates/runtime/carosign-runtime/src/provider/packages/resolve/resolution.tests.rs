use super::super::*;
use crate::provider::packages::Profile;
use crate::provider::packages::profile::MatchRule;

#[test]
fn package_choice_cannot_silently_choose_between_matching_profiles() {
    let profile = |id: &str| Profile {
        id: id.into(),
        matches: vec![MatchRule {
            key: "device".into(),
            value: "sensor".into(),
        }],
    };
    let installed = vec![
        ("first".into(), profile("a")),
        ("first".into(), profile("b")),
        ("second".into(), profile("c")),
    ];
    let inventory = vec![("device".into(), "sensor".into())];
    for result in [
        resolve(&installed, &inventory, &[], Some("first")),
        resolve(&installed, &inventory, &["first".into()], None),
    ] {
        assert!(
            matches!(result, Resolution::Ambiguous(ref candidates) if candidates.len() == 2 && candidates.iter().all(|candidate| candidate.package == "first"))
        );
    }
    assert!(
        matches!(resolve(&installed, &inventory, &["second".into()], None), Resolution::Chosen(ref candidate) if candidate.profile == "c")
    );
}
