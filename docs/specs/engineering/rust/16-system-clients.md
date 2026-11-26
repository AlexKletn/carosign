# 16. Linux clients: PAM, CLI, fprintd-compatible facade

## PAM crate (`crates/clients/pam-carosign`)

```text
src/
  lib.rs
  pam_ffi.rs             # only unsafe FFI boundary
  options.rs             # socket, timeout, fallback, no secrets in CLI
  conversation.rs        # PAM messages/cancellation
  client.rs              # call stable biometric client protocol
  result_map.rs          # match/deny/unavailable mappings
  lifecycle.rs           # close session/owner handle on cancel
```

`pam_sm_authenticate` calls `BeginVerification` with trusted PAM context via daemon's policy. It does not access sensor/templates. On error/unavailable it returns `PAM_AUTHINFO_UNAVAIL` to permit correctly configured password fallback; no implicit system wide `sufficient` policy is installed by package. Do not expose keyring password via `PAM_AUTHTOK` in core. If needed, separate explicitly-authorized secrets integration.

## CLI binary (`crates/clients/carosignctl`)

```text
src/
  main.rs
  cli.rs
  client.rs
  commands/{devices,enroll,verify,remove,list,health,operation}.rs
  output/{human,json}.rs
```

Unprivileged CLI sends requests, displays semantic progress and codes, handles Ctrl-C as cancellation request with delayed final outcome. `--user` is only selector, not permission.

## D-Bus fprintd facade (`crates/clients/carosign-fprintd-compat`)

```text
src/
  main.rs
  dbus_server.rs
  device_mapping.rs
  claim_state.rs
  enroll_mapping.rs
  verify_mapping.rs
  error_mapping.rs
  authz.rs
```

Inbound `libfprint` provider and outbound `fprintd`-compatible D-Bus are separate modules/packages. Do not simultaneously let independent fprintd own a sensor used by native libfprint provider; discovery must enforce exclusive owner. fprintd API cannot express every evidence/uncertain mutation state, so facade must document safe degradation, not invent success.

## UI clients

GUI belongs outside core; subscribe to `OperationEvent`. Status overlay should be bound to authorized session/operation (not global datagram socket) and not receive raw frame data. For under-display FOD, physical touch/illumination belongs to separate trusted interaction controller, not ordinary overlay.

## Integration tests

PAM fallback works when daemon unavailable; D-Bus duplicate claimant rejected; CLI cannot enroll cross-user without grant; UI reconnect snapshot; secret release never occurs just because provider returns Matched.
