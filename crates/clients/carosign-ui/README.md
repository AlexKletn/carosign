# CaroSign UI

An Obscur Kit v2 desktop client for the local CaroSign service. The platform
panel shows service readiness, device counts and scopes awaiting reconciliation.
The inventory groups devices by provider and shows reachability, availability,
declared capabilities and presentation attack detection.

Cargo fetches Obscur Kit v2 from GitHub at the revision pinned in `Cargo.toml`.
Use Rust 1.96.1 and a Vulkan-capable Linux display session. The UI uses three
layers under `src/ui/`.

- `app/` composes the window, routes between pages and applies the desktop theme.
- `widgets/` contains the sidebar, platform status and inventory. Each component
  keeps its Rust, OKUI and SCSS together.
- `shared/` contains slot projection, common SCSS, palette variables and mixins.
  `shared/ui/` styles the native Kit controls used directly in OKUI, such as
  `<button>`, `<input>` and `<textarea>`. Widgets add styles for their own variants.

The snapshot and query errors live in `src/domain/`. The Linux socket client
and polling loop live in `src/ipc/` and feed that snapshot to the app.

The app composes widgets and shared code. Widgets can use shared code.
Cargo compiles and embeds the stylesheets from the app and shared Sass entry points.

```sh
cargo +1.96.1 run --locked --manifest-path crates/clients/carosign-ui/Cargo.toml
```

The client refreshes through `/run/carosignd/control.sock`. Provider and device
details require inventory permission for the desktop user. Localization uses
the standard CaroSign locale directories.

Obscur Kit applies and refreshes the desktop color scheme, palette, accent,
fonts, text scale, contrast, and reduced-motion preference. CaroSign supplies
fallback colors and font sizes for settings the desktop omits. Window
decorations use the platform window backend.

Edit the system fallback values in `src/ui/shared/styles/_variables.scss`.
The build generates Rust constants from the three `$system-*` maps.
Use six-digit hex colors, quoted font families, point sizes, and a radius in pixels.

The Policies page loads and saves system access rules through
`/usr/bin/carosign-config policy-admin`. The system package installs
`org.carosign.policy.manage`. An active desktop session needs a polkit
authentication agent. Restart `carosignd` after saving to apply the new policy.

## Headless UI snapshots

Run the snapshot test on a host with a Vulkan adapter.

```sh
cargo test --locked --manifest-path crates/clients/carosign-ui/Cargo.toml \
  render_dashboard -- --ignored
```

The test navigates through Platform, Devices, Policies and Fingerprints using
synthetic pointer events. It captures service states and interactions with
demo fingerprint entries. It writes Lyon-rendered PNGs to
`crates/clients/carosign-ui/target/ui-snapshots`. Set
`CAROSIGN_UI_SNAPSHOT_DIR` to choose a different output directory.

## FPS diagnostics

Launch **FPS diagnostics** from the CaroSign launcher menu, or run
`OBSCUR_FRAME_STATS=each carosign-ui` in a terminal. Reproduce the lag by
navigating between pages or resizing the window. CaroSign redraws on demand, so
collect measurements while the window is active.

For launcher sessions that send application output to the journal, read the
presentation samples with:

```sh
journalctl --user -b -f _COMM=carosign-ui --grep='window-performance|window-frame'
```

`shown_fps` is the rate of frames shown by a Wayland compositor that supports
presentation feedback. `[window-frame]` lines describe frame submission and
timing. These lines also appear when presentation feedback is unavailable.
