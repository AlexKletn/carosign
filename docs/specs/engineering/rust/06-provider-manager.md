# 06. Provider Manager: package → worker → authenticator

## Modules (`carosign-runtime/src/providers`)

```text
providers/
  mod.rs
  package_catalog.rs         # trusted package paths, manifest parsing
  manifest.rs                # semantic validation, version/feature constraints
  profile_catalog.rs         # device profiles, schema IDs
  discovery.rs               # consume platform DeviceInventory
  matcher.rs                 # passive matching only
  probe.rs                   # restricted read-only probing, if allowed
  resolver.rs                # select one candidate per exclusive resource
  instance_registry.rs       # persistent package/profile/instance links
  lifecycle.rs               # worker FSM
  spawn.rs                   # process supervisor PORT, no raw process calls here
  handshake.rs               # version + instance checks
  supervisor.rs              # heartbeat, liveness, crash loop, kill/quarantine
  binding.rs                 # device + lease + worker epoch fence
  upgrade.rs                 # stage, drain, compatibility, activate, rollback
  health.rs                  # device vs worker status
```

## Identity distinctions

`PackageId` stable installed extension identity; `PackageVersion` is software release; `WorkerInstanceId` is ephemeral (new each spawn); `DeviceIdentity` is physical evidence; `AuthenticatorId` is persistent logical authenticator; `DeviceEpoch` fences stale sessions. **Never** derive `AuthenticatorId` solely from `/dev/*` path.

## Startup protocol

`PackageCatalog::scan()` reads a trusted root and validates manifests (not code trust). `Discovery::observe()` forms candidates. `Resolver` selects an approved package/profile for a device; no auto-priority granted by manifest. `Supervisor::spawn()` asks platform spawn port to create process + private channel. Handshake checks version, worker identity and chosen feature set. `Binding` negotiates lease then publishes authenticator after `DeviceReady`.

## Process supervisor contract

`SpawnWorker { immutable_executable_ref, package_id, profile_id, inherited_channel, sandbox_policy }` → `WorkerProcessHandle { instance_id, liveness_observer, ipc }`. The Linux implementation verifies executable identity/path and permissions, passes an inherited seqpacket fd, tracks pidfd where available, and uses a bounded restart policy. Workers must not open public listener.

## Upgrade protocol

`Stage → VerifyPackage → Drain → SafeToReplace? → StopOld → Migrate? → StartNew → Rebind → Activate`. If an incomplete mutation, reserve scope and block activation. If template format migration impossible, retain old provider or mark unavailable; never erase. Rollback possible only if state schema compatible or reversible.

## Device match policy

Passive hardware ID rules first. Active probe only under restricted privileges and with explicit no-permanent-mutation safety. More than one approved candidate: administrator selection or explicit priority policy. No random winner. A provider cannot elevate hardware-assurance by claiming support in manifest.

## Failures

- Worker heartbeat alive but command deadline missed → operation-specific hang, not healthy device.
- Process exits during verify → drop evidence, fail closed.
- Process exits during mutation → mark scope pending reconciliation.
- Repeated crash → quarantine provider instance, keep registry.
- Package removed → authenticator unavailable, preserve enrollment binding and provider state.
