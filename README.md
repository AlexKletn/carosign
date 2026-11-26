# CaroSign

[Русский](README.ru.md)

> Hail, Meat-Bioform!

CaroSign enrolls, verifies, and deletes fingerprints through separate hardware
providers. Linux clients connect through `carosignctl`, PAM, or the fprintd D-Bus
interface. The [CaroSign desktop app](crates/clients/carosign-ui/README.md) shows the
status of the service, providers, and devices.

[Specification v0.6.6](docs/specs/README.md) ·
[Documentation](docs/README.md) ·
[Command-line tools](docs/guides/headless-clients.md) ·
[Arch Linux installation](packaging/arch/README.md)

Clients let you choose the language and style of prompts independently.
Translation catalogs are available as packages. See the
[client localization guide](docs/contracts/client-localization.md).

## Current status

**CaroSign is still in development.** The Linux service listens on a client
socket and runs the broker and provider in separate processes. Tests with the
mock provider cover enrollment, verification, and removal, including flows
through PAM and the fprintd D-Bus adapter. If a mutation is cancelled or the
client disconnects, CaroSign keeps any affected scope with an uncertain state
blocked until reconciliation.

Enrollment, verification, and PAM login have also been tested on Linux with the
separate TB371FC hardware provider. Hardware attestation is outside the scope of
these tests. Resource recovery and desktop integration still need work before
release.

The initial Linux release uses the simplified system trust model described in
the [release scope](docs/specs/engineering/implementation-v06/35-first-release-scope.md).
See the [provider profile](docs/contracts/linux-provider-profile.md) for its format
and the [systemd guide](packaging/systemd/README.md) for the service units and an
example configuration.

Run `carosignd init-store /absolute/path` to create a metadata store. The command
preserves any existing file at that path. To specify the store when checking
service startup, run `carosignd --config /absolute/path` with a configuration
file containing `store=/absolute/path/metadata.redb`.

The service requires a protected system policy and provider profile. The
[Arch Linux preview package](packaging/arch/README.md) includes the systemd and
D-Bus integration. Install a provider separately and configure PAM explicitly
if you want to use fingerprint authentication.

## Repository layout

The [documentation index](docs/README.md) provides an overview.

| Contents | Path |
| --- | --- |
| Normative specification | `docs/specs/engineering/implementation-v06/` |
| Implementation work orders W00–W15 | `docs/specs/engineering/implementation-v06/16-implementation-workorders.md` |
| IPC schemas used by the build | `contracts/proto/carosign/` |
| Domain types, transitions, and evidence evaluation | `crates/core/` |
| Operation execution, persistence, and port adapters | `crates/runtime/` |
| IPC protocol and provider SDK | `crates/protocol/` |
| Linux services, resources, and transport | `crates/linux/` |
| Desktop app, CLI, PAM, and fprintd D-Bus adapter | `crates/clients/` |
| Test implementations and fixtures | `crates/testing/` |
| Rust diagnostics tools | `crates/tools/` |
| Mock provider and resource-fence inspection tool, each in its own Cargo workspace | `crates/testing/carosign-mock-provider/` and `crates/tools/carosign-fence-inspect/` |
| Packaging, build, and test scripts | `packaging/` and `tools/` |

## Build and test

Run these commands from the repository root.

```sh
cargo build --offline
cargo test --offline --workspace
cargo clippy --offline --workspace --all-targets
CAROSIGN_SYSROOT=/path/to/aarch64-sysroot sh tools/build-arch.sh /path/to/new-package.pkg.tar.zst
```
