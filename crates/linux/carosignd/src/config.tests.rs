#[test]
fn fence_store_must_be_separate_and_absolute() {
    let config =
        super::Config::parse("resource_fence_store=/var/lib/carosign/fences.redb").unwrap();
    assert_eq!(
        config.resource_fence_store.as_deref(),
        Some("/var/lib/carosign/fences.redb")
    );
    assert!(super::Config::parse("resource_fence_store=relative").is_err());
    assert!(super::Config::parse("resource_fence_store=/var/lib/carosign/metadata.redb").is_err());
}
#[test]
fn systemd_configuration_matches_runtime_and_state_directories() {
    let config = super::Config::parse(include_str!(
        "../../../../packaging/systemd/carosignd.conf"
    ))
    .expect("штатные настройки должны разбираться");
    assert_eq!(config.socket, "/run/carosignd/control.sock");
    assert_eq!(config.store, "/var/lib/carosign/metadata.redb");
    assert!(config.provider_profile.is_none());
}
use super::*;

#[test]
fn defaults_are_within_ceilings() {
    assert!(Config::default().validate().is_ok());
}

#[test]
fn loosening_beyond_the_ceiling_is_refused() {
    assert!(Config::parse("ops_per_connection=64").is_err());
}

#[test]
fn connection_limit_requires_explicit_approval_to_exceed_default() {
    for limit in [1, 4] {
        assert!(Config::parse(&format!("ops_per_connection={limit}")).is_ok());
    }
    for limit in [0, 5, 16, 17] {
        assert!(Config::parse(&format!("ops_per_connection={limit}")).is_err());
    }
}

#[test]
fn unknown_field_is_refused() {
    assert!(Config::parse("admin=yes").is_err());
}

#[test]
fn paths_are_usable_by_the_system_calls_before_opening_store() {
    // SAFETY: zero-state sockaddr_un It is permissible to determine the length.
    let address: libc::sockaddr_un = unsafe { core::mem::zeroed() };
    let limit = address.sun_path.len();
    let valid = format!("/{}", "a".repeat(limit - 2));
    let too_long = format!("/{}", "a".repeat(limit - 1));
    assert!(Config::parse(&format!("socket={valid}")).is_ok());
    assert!(matches!(
        Config::parse(&format!("socket={too_long}")),
        Err(ConfigError::BadPath { field: "socket" })
    ));
    assert!(matches!(
        Config::parse("socket=/bad\0path"),
        Err(ConfigError::BadPath { field: "socket" })
    ));
    assert!(matches!(
        Config::parse("store=/bad\0path"),
        Err(ConfigError::BadPath { field: "store" })
    ));
}

#[test]
fn authenticator_queue_accepts_zero_and_protocol_ceiling() {
    assert_eq!(Config::default().queued_per_authenticator, 16);
    for limit in [0, 1, 64] {
        assert_eq!(
            Config::parse(&format!("queued_per_authenticator={limit}"))
                .unwrap()
                .queued_per_authenticator,
            limit
        );
    }
    assert!(Config::parse("queued_per_authenticator=65").is_err());
}
