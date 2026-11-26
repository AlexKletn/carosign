# 20. `carosign-provider-libfprint`: intentionally non-Rust

This is the architectural demonstration of language neutrality. A provider bridge in **C/GLib** implements the exact same `carosign.provider.v1` wire protocol. No Rust ABI, no dependency on platform crates.

## C project layout (proposal)

```text
providers/libfprint/
  meson.build
  src/
    main.c                # worker entry
    worker.c/.h           # private IPC state machine
    codec.c/.h            # generated protobuf C bindings
    device.c/.h           # FpContext/FpDevice lifecycle
    operations.c/.h       # enroll/verify/delete
    prints.c/.h           # FpPrint serialization/compatibility
    cancel.c/.h           # GCancellable and quiesce
    glib_loop.c/.h        # GMainContext integration
    errors.c/.h           # normalized protocol errors
  data/
    package.toml
```

## Integration rules

`FpContext` discovers, `FpDevice` operates; enumerate/identify/delete only when features available. `FpPrint` uses libfprint serialization where supported, stays in encrypted template storage under suitable policy. `GCancellable` is a request, not immediate quiescence. `GMainContext` continues processing while device async operation in flight.

Direct libfprint access and existing fprintd sharing one sensor is a conflict; Resolver prevents duplicate ownership. `carosign-fprintd-compat` is outbound D-Bus API, not the C bridge.

## External provider conformance

Buildable with Meson/C compiler and protoc-generated bindings; must pass exactly same protocol and lifecycle conformance suite as Rust/Go workers. Canonical tests must not import Rust internals.
