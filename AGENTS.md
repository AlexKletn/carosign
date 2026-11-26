# Code Guidelines

- **One entity per file.** Keep a type with its implementations, methods, and internal helpers. Give an independent type, state, resource owner, or handler with its own responsibility a separate file.
- Put tests in a separate `<name>.tests.rs` file next to `<name>.rs`.
- Group related entities in a module. Give each transport its own module. Use `mod.rs` to declare submodules and control their visibility through re-exports. If a `lib.rs` exists, use it only for re-exports. The kernel module entry point selects the transport and handles registration.
- Apply clean-code practices. Write self-documenting code. Keep rationale, hardware details, and correctness requirements in documentation. Omit implementation comments. Preserve required `SAFETY` explanations next to unsafe code and keep license notices.
