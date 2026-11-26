use super::approved_path;

#[test]
fn approved_paths_obey_both_component_and_total_byte_limits() {
    let component = "a".repeat(255);
    assert!(approved_path(&component));
    assert!(!approved_path(&"a".repeat(256)));
    let part = "a".repeat(254);
    let max_path = format!("{part}/{part}/{part}/{part}/aaaa");
    assert_eq!(max_path.len(), 1024);
    assert!(approved_path(&max_path));
    assert!(!approved_path(&format!("{max_path}a")));
    for path in [
        "/etc/key",
        "foo/",
        "foo//bar",
        "foo/./bar",
        "foo/../bar",
        "foo/\0bar",
    ] {
        assert!(!approved_path(path));
    }
}
