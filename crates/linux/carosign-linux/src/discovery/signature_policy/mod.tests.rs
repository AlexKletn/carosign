use super::*;

fn valid() -> String {
    let digest = "ab".repeat(32);
    format!(
        "schema=1\nkey_policy_id='01010101010101010101010101010101'\nprimary_fingerprint='{}'\nkeyring_verity='{digest}'\nverifier_verity='{digest}'\nruntime=[{{path='usr/bin/gpgv',verity='{digest}'}}]\n",
        "AB".repeat(20)
    )
}

#[test]
fn policy_is_bounded_unambiguous_and_binds_verifier_to_runtime() {
    let good = valid();
    let parsed = Rules::parse(good.as_bytes()).unwrap();
    assert_eq!(parsed.policy_id, [1; 16]);
    assert_eq!(parsed.runtime.get("usr/bin/gpgv"), Some(&parsed.verifier));
    for bad in [
            good.replace("key_policy_id='01010101010101010101010101010101'\n", ""),
            good.replace("01010101010101010101010101010101", "00000000000000000000000000000000"),
            good.replace("01010101010101010101010101010101", "0101"),
            good.replace("schema=1", "schema=2"),
            format!("{good}unknown=1\n"),
            good.replace("usr/bin/gpgv", "../gpgv"),
            good.replace("usr/bin/gpgv", "/usr/bin/gpgv"),
            good.replace("verifier_verity='ab", "verifier_verity='cd"),
            good.replace("primary_fingerprint='ABAB", "primary_fingerprint='zzzz"),
            good.replace("primary_fingerprint='ABAB", "primary_fingerprint='abab"),
            good.replace("runtime=[", "runtime=[{path='usr/bin/gpgv',verity='abababababababababababababababababababababababababababababababab'},"),
        ] {
            assert!(Rules::parse(bad.as_bytes()).is_err(), "не отвергнута политика: {bad}");
        }
}
