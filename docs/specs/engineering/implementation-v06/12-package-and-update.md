# 12. Independently installable provider packages and updates

## Physical package contents

```
/usr/libexec/biometric/providers/<package>/worker
/usr/lib/carosign/providers/<package>/package.toml
/usr/lib/carosign/providers/<package>/profiles/<profile>.toml
/usr/share/doc/carosign-provider-<package>/...
/var/lib/carosign/providers/<package>/    # state; not removed on package uninstall
```

Paths represent Linux baseline; `libexec` may differ by distro adapter, but the manifest schema and trust semantics do not. Main platform starts with **zero providers**. Do not package providers as mandatory dependencies of core service.

## Manifest schema (baseline TOML v1)

```toml
schema = 1
[package]
id = "org.example.carosign.gxfp5130"
version = "1.0.0"
[worker]
executable = "worker"            # verified path relative to package root
instance_model = "per-device"
[protocol]
major = 1
min_minor = 2
max_minor = 2
[[profiles]]
id = "matebook-gt14"
path = "profiles/matebook-gt14.toml"
```

Parser uses exact schema with `deny_unknown_fields` for privileged manifest sections, rejects absolute/parent-traversing relative executable paths, duplicate IDs, impossible protocol range and invalid profiles. `Profile` declares match rules, resource *needs*, device-specific config schema, optional interaction needs; none of these grants privilege or hardware assurance. Package origin/trust is established by admin-controlled package manager and root-owned policy; manifest text is untrusted data until verified.

## Provider identity and binding

`PackageId` + version + trusted installed artifact identity → supervisor spawn. `ProviderId` identifies implementation family, `ProfileId` matching data, `WorkerInstanceId` process launch; none is stable `AuthenticatorId`. Hardware identity must be independently resolved; same chip model is not sufficient to inherit enrollment mappings.

## Lifecycle/update algorithm

1. Stage new package version without changing active worker.
2. Validate manifest/profile and wire version; identify migration requirements.
3. Stop admission for affected authenticators, drain/cancel verification; do not interrupt mutating phase without journal/recovery.
4. Confirm physical quiesce; if uncertain, block swap and enter recovery.
5. Stage/commit **durable** metadata/provider-format migration as needed, with old data kept for rollback if compatible.
6. Spawn new worker and bind to same verified hardware identity; revalidate declared capabilities and assessment.
7. Promote to active after safe readiness. On failure roll back **only if** old state format compatible; otherwise quarantine and preserve data.

Package uninstall never removes enrollments/registry/keys by default. If worker missing, `ProviderUnavailable`; if reinstalled, binding only after hardware and template compatibility checks. No automatic re-enrollment or deletion.

## Multiple provider candidates

Explicit admin device binding first. Else sole approved match. Else configured priority among approved providers. If still ambiguous → `Ambiguous`, no arbitrarily chosen worker. Active probe must be separately approved as non-mutating. Concurrent ownership of same physical sensor forbidden unless broker can prove disjoint resource sets.

## Languages and SDK

Rust SDK is one optional reference. C/GLib libfprint bridge uses same `.proto`, handshake, errors and conformance suite. Other languages can implement messages manually. No `#[repr(C)]` shared memory ABI with Rust runtime. For high-bandwidth buffers, fd-based transfer is versioned separately and never required for initial fingerprint provider control plane.

### v0.6.6 package sample override

Any earlier manifest example `min_minor=2` is historical. New/updated packages implementing this baseline must negotiate provider **minor >=3** for explicit planned enrollment identities. [34](34-v066-enrollment-command-projection.md).
