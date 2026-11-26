# 14. Linux Resource Broker, FD ownership and sandbox

## Logical modules (`carosign-linux/src`)

```text
resources/
  inventory.rs           # HardwareDescriptor from sysfs/udev/ACPI/DT
  matchers.rs            # stable vs ephemeral identity attributes
  registry.rs            # resource graph/ownership
  lease.rs               # scoped acquisition/fence
  fd_delegate.rs         # SCM_RIGHTS + type validation
  ioctl_policy.rs        # safe mediated command classes
  tee_port.rs            # QSEE/QTEE plugin interface
  time.rs                # CLOCK_BOOTTIME, boot identity
  key_port.rs            # optional key provider integration
process/
  spawn.rs               # sealed executable reference, inherited IPC
  sandbox.rs             # namespaces, seccomp, cgroup, uid
  pidfd.rs               # process liveness
  file_identity.rs       # trusted install dirs and no TOCTOU
```

## Optional privileged broker (`crates/linux/carosign-broker`)

```text
src/
  main.rs
  server.rs
  policy.rs              # installed/trusted policy, NOT manifest authority
  claims.rs
  fd_validation.rs
  device_rpc.rs          # restricted operations if raw fd too powerful
  revoke.rs
  recovery.rs
  audit.rs
```

## Security rules

1. Package manifest declares needed resources but does not authorize them.
2. Broker policy binds approved package/profile/device identity to allowed ioctl classes or scoped descriptors.
3. `SCM_RIGHTS` transfers FD access; closing manager copy **does not revoke** worker FD. If revocation cannot be enforced via kernel/driver, mediate calls or isolate/terminate worker and potentially quarantine device.
4. Do not open raw QSEE or unrestricted UFS/RPMB nodes for arbitrary untrusted providers.
5. Worker generally has no network, only approved device/storage paths, process/memory limits, and dedicated user identity.
6. Listener serving callbacks of several TAs is a separate long-lived owner; provider gets scoped service not raw administrator access.

## Resource states

`Available → Reserved → Active → Quiescing → Available`; fault transitions `Uncertain → Recovering → Available/Unavailable`. Quiesce confirmation must describe **physical** state, not only logical lease closure.

## Security of packaged executable

Use trusted root-owned filesystem with safe resolution of manifest and binary path, immutable reference where possible, and spawn binding to verified executable. Merely comparing a path string before `exec` is TOCTOU. Handle process lifetime with pidfd where available. Details of exact Linux syscalls are platform integration, not provider protocol.

## Tests

Fake descriptor extra/missing FD; worker with wrong package identity; attempt to claim different sensor; lease revoke with open FD; broker crashes; listener poisoning; stale device epoch; sandbox escape attempt in isolated test environment.
