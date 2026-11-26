use super::super::{MatchRule, Profile, matches};

#[test]
fn conflicting_inventory_values_never_select_a_profile() {
    let profile = Profile {
        id: "fixture".into(),
        matches: vec![MatchRule {
            key: "device".into(),
            value: "expected".into(),
        }],
    };
    for inventory in [
        vec![
            ("device".into(), "expected".into()),
            ("device".into(), "other".into()),
        ],
        vec![
            ("device".into(), "other".into()),
            ("device".into(), "expected".into()),
        ],
    ] {
        assert!(!matches(&profile, &inventory));
    }
    assert!(matches(&profile, &[("device".into(), "expected".into())]));
}
