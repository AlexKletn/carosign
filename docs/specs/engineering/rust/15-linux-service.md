# 15. `carosignd` и Linux composition root

## Binary modules

```text
crates/linux/carosignd/src/
  main.rs
  args.rs               # CLI and config flags
  config.rs             # read strict config version
  logging.rs            # structured, privacy-safe logging
  bootstrap.rs          # safe start ordering
  dependencies.rs       # wire concrete Ports
  server.rs             # create public socket/bridge
  readiness.rs          # degraded/recovering/ready
  shutdown.rs           # signal and drain
  health.rs             # health endpoint and admin-only details
  migrations.rs         # controlled storage upgrade
```

`main` parses config, sets up security infrastructure, creates concrete adapters and calls `RuntimeBuilder`; do not implement domain decisions in HTTP/D-Bus/PAM handlers. Dependencies injected explicitly, avoid global mutable service singleton.

## Linux lib (`carosign-linux`)

`inventory`, `identity`, `policy`, `systemd` optional, `time`, `paths`, `process`, `resource_ports`, `key_storage`. Must remain replaceable in Veles composition. Avoid making `libsystemd` mandatory for embedded runtime.

## Config layers

Defaults < `/etc/carosign/...` policy < system-managed overrides. Unknown security-critical config keys fail startup. Provider package settings are scoped to package/profile and validated against manifest schema. Client never supplies arbitrary worker executable paths.

## Startup gate

Before public service readiness: verify storage schema/integrity, load incomplete mutation journal, mark blocked scopes, load authority/assessor, start provider manager. If journal corrupt, fail closed for affected scopes instead of creating empty state. `Ready` can coexist with specific unavailable devices; health exposes degraded reason without secrets.

## Shutdown gate

Stop new requests, revoke owner scopes, cancel verification, persist unresolved mutations, drain/close workers, quiesce resource broker, flush/close DB. System shutdown deadline may force exit; on next boot journal must still prevent blind replay.

## Permissions and isolation

`carosignd` should not run with unrestricted raw hardware privileges by default; optional broker handles elevated operations. Unix public socket controlled by configured access policy and always independently authorizes requests. PAM module acts as client only.
