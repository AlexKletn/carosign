use super::*;

#[test]
fn entry_point_fails_closed_and_allows_configured_fallback() {
    assert_eq!(
        pam_sm_authenticate(core::ptr::null_mut(), 0, 0, core::ptr::null()),
        PamCode::AuthInfoUnavail.raw()
    );
}
