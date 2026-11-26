# 01. Workspace и границы публикации

## Каталог основной платформы (предлагаемый)

```text
carosign/
  Cargo.toml                       # workspace only, resolver=3 when supported
  Cargo.lock                       # versioned for runnable workspace
  crates/
    core/
      carosign-core/               # pure types, invariants
      carosign-engine/             # pure reducers, state transition decisions
      carosign-evidence/           # verification dispatch, trusted assessment ports
    runtime/
      carosign-runtime/            # async orchestration and actors
      carosign-persistence/        # repository traits and durable implementations
      carosign-adapters/           # concrete adapters for runtime ports
    protocol/
      carosign-protocol/           # generated wire types / schema-version mapping
      carosign-provider-sdk/       # OPTIONAL Rust worker helper library
    linux/
      carosign-linux/              # Linux discovery, credential sources, process primitives
      carosign-linux-ipc/          # Unix seqpacket client/provider transport
      carosignd/                   # system service composition root
      carosign-broker/             # optional privileged Linux resource broker
    clients/
      carosignctl/                 # CLI; unprivileged client
      carosign-fprintd-compat/     # optional D-Bus facade, NOT libfprint bridge
      pam-carosign/                # isolated PAM cdylib
    testing/
      carosign-testkit/            # fake authority, clock, devices, fault injection
      carosign-mock-provider/      # standalone test provider workspace
    tools/
      carosign-fence-inspect/      # standalone diagnostics workspace
  protocols/                       # authoritative .proto source after freeze
    client/v1/
    provider/v1/
  providers/                       # examples only; separate packages/releases
    gxfp5130/                     # Rust workspace in its OWN repository/package
    goodix-qsee/                  # independent Rust workspace
    nord4-qtee/                  # separate or embedded Veles project
    libfprint/                    # C/GLib; independent non-Cargo build
  conformance/                     # black-box wire-suite; language agnostic
  packaging/                       # Linux packaging and profiles
```

`providers/*` above are *reference checkout locations*, **not** required workspace members. Packaging should not imply that installing `carosign` installs hardware providers. Separate package versions and storage schema versions are maintained per provider.

## Crate roles and visibility

| Crate | Type | Public API? | Internal modules? | Depends on |
|---|---|---|---|---|
| core | lib | yes, domain | yes | std, minimal deps |
| engine | lib | limited internal stable | yes | core |
| protocol | lib | wire bindings | yes | generated schema |
| runtime | lib | integration API | yes | core, engine, persistence/evidence, protocol ports |
| provider-sdk | lib, optional | developer SDK | yes | protocol, transport abstractions |
| persistence | lib | internal port + implementations | yes | core |
| evidence | lib | verifier port + implementations | yes | core |
| linux | lib | platform integration | yes | core, Linux-only facilities |
| linux-ipc | lib | transport internals | yes | protocol, Linux primitives |
| testkit | dev lib | test-only | yes | domain/runtime/protocol |
| carosignd | executable | IPC public only | yes | runtime + Linux adapters |
| broker | executable | private IPC only | yes | linux infrastructure |
| PAM | cdylib | PAM ABI | yes | client IPC SDK, FFI |

## Build profiles and dependency policy

- No `libc`, `tokio`, `glib`, DBus, QSEE or Veles dependencies in `carosign-core` or `carosign-engine`.
- `unsafe` denied in pure domain and engine. Each FFI/syscall wrapper is explicitly isolated and code-reviewed.
- `carosign-runtime` defines no `main()`, no system paths and no fixed executor in its exposed types. Initial executor adapter MAY use Tokio internally; no Tokio channel/task handles may appear in domain/public protocol.
- Linux binaries choose storage, key provider, logger, actual executor, package directories and system policy in `main`/composition modules.
- Version all wire schemas and storage formats independently from crate semver.
- Use explicit feature flags; `default` features must not silently pull optional NPU, TPM, GLib or QSEE dependencies.
- Dependency direction is one-way: binaries → integration crates → runtime → engine/core. Never `core → carosignd` and never provider SDK → core implementation internals.

## File naming conventions

`mod.rs` is not mandatory; prefer `foo.rs` with `foo/bar.rs` when it improves discoverability. Internal modules `pub(crate)` by default. Expose only named facade types in `lib.rs`. No sprawling `utils.rs` or `common.rs`; types live with the owner of their invariants.
