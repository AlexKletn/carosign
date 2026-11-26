# 22. Security invariants of the Rust implementation

## Trust boundaries

1. Public IPC accepts untrusted input; authenticated OS identity + policy owner grant required to request operation.
2. Installed provider worker is semi-trusted and isolated; its declared capabilities are *not* independent proof of security.
3. Privileged broker owns narrow hardware operations and resource claims; a provider cannot simply request root.
4. Software match result cannot be promoted to protected key authorization by a type conversion.
5. TEE token validity can be established only by a compatible trusted verifier; opaque bytes are not proof.
6. Persistent mutations must have durable intent before potentially mutating dispatch.
7. Revocation prevents future actions and result release; it does not magically undo in-flight hardware mutation.
8. Key material and raw images do not cross public IPC; diagnostics require explicit bounded permission.
9. Package uninstall/reinstall does not implicitly delete user templates or reset sensor provisioning.
10. `Uncertain` device states require proven safe recovery or remain blocked.

## Rust-specific choices

- `#![forbid(unsafe_code)]` in core/engine; unsafe localized in Linux syscalls/PAM/QSEE FFI.
- Opaque newtypes enforce ID and context separation; secrets use zeroizing buffers when feasible.
- Avoid `#[repr(C)]` shared Rust↔provider ABI. Wire messages own bytes/FDs; no cross-process Rust references.
- Never run unbounded tasks or allocation for user-controlled requests; bounded queues and parser limits.
- Avoid holding locks during external callbacks or awaiting worker/device calls.
- `Drop` may request cleanup but **must not be the sole guarantee** of durable mutation resolution. Explicit supervisor/journal lifecycle required.
- Panic in worker does not imply hardware quiesce. Runtime handles restart as uncertain when appropriate.

## Model gaps requiring validation

Root compromise on insecure boot can patch software pipeline; TPM sealing alone doesn't prove trusted UI; TEE presence alone doesn't prove secure sensor path or unspoofable results. Report assessed properties conditionally, not as blanket trust levels.
