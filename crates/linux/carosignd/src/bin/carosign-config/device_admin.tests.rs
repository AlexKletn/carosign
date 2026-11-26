use super::*;

#[test]
fn change_only_the_selected_device() {
    let first = "01010101010101010101010101010101";
    let second = "02020202020202020202020202020202";
    let original = format!("mock 412 413 {first} /etc/first.conf\nmock 412 413 {second}\n");
    let disabled = update(&original, first, true).unwrap();
    assert_eq!(
        disabled,
        format!("mock 412 413 {first} /etc/first.conf disabled\nmock 412 413 {second}\n")
    );
    assert_eq!(update(&disabled, first, true), Some(disabled.clone()));
    assert_eq!(update(&disabled, first, false), Some(original));
}
