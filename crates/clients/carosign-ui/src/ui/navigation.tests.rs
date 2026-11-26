use super::*;

#[test]
fn compact_navigation_starts_at_sections_and_back_has_no_history() {
    let mut nav = Navigation::default();
    nav.resize(600.0);
    assert!(nav.page() == Page::Sections);
    nav.enter(Page::Fingerprints);
    nav.enter(Page::Policies);
    nav.back();
    assert!(nav.page() == Page::Sections);
    nav.resize(780.0);
    assert!(nav.page() == Page::Platform);
    nav.resize(600.0);
    assert!(nav.page() == Page::Platform);
    nav.enter(Page::Policies);
    nav.resize(600.0);
    assert!(nav.page() == Page::Policies);
}

#[test]
fn wide_navigation_opens_the_platform() {
    let mut nav = Navigation::default();
    nav.resize(780.0);
    assert!(!nav.compact());
    assert!(nav.page() == Page::Platform);
    nav.enter(Page::Fingerprints);
    nav.resize(900.0);
    assert!(nav.page() == Page::Fingerprints);
}
