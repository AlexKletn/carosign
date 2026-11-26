# 24. Stable package identity and platform-managed encrypted templates

## Stable identifiers

Manifest `package.id` is a normalized reverse-DNS ASCII identifier (1..128 bytes), no whitespace; canonical lower-case. On first **approved** installation `ProviderManager` allocates a random, nonzero 16-byte `PackageId` and a family `ProviderId`, and stores mapping in `packages` table. When package is uninstalled, mapping persists. Reinstall binds to same IDs **only** after system-controlled package/provenance and profile validation. Worker-controlled `WorkerHello` never assigns these IDs. `ProfileId` likewise allocated when first approved profile is registered and persisted under a tuple `(PackageId, normalized profile name)` in provider binding metadata; profile strings alone are not IDs. If conflicting name or provenance occurs, explicit admin review, no automatic reassignment.

`AuthenticatorId` is allocated when trusted hardware identity+approved provider profile first bind; persisted under that device identity. Device location path and chip type alone cannot prove same authenticator. Registry preserves ID if package removed; hardware identity ambiguity blocks reconnection with old templates.

## Platform-managed template storage baseline

For GXFP and libfprint templates stored by the platform, use `template_blobs` redb table in the **same DB** as registry+journal. Blob value is `StoredValue.template_blob` from `carosign/storage/v1/storage.proto`; **only ciphertext, AEAD metadata, and non-secret identity mirrors (`enrollment_id`, `generation`) stored**. The 24-byte redb key is canonical, mirrors are mandatory equality checks, not alternative keys. Platform provider's restricted storage adapter performs encryption/decryption. No template bytes in public client protocol, core domain types or event log.

- AEAD: AES-256-GCM, fresh 96-bit nonce per encryption, 256-bit key retrieved from trusted `KeyProvider`. Ciphertext contains 128-bit tag, never stored plaintext.
- AAD exact byte sequence: ASCII `BIO-TEMPLATE-V1` followed by 16B AuthorityId + 16B SubjectId + 16B AuthenticatorId + 16B EnrollmentId + `generation` u64 big-endian + `format_id` byte-length u16 big-endian + UTF-8 format_id + `format_version` u32 big-endian. Reject format_id longer than 128 bytes and zero IDs/generation.
- Store metadata includes non-secret 16B `key_id`, 12B `nonce`, format id/version and ciphertext; key never serialized to DB. Keyfile fallback is separately configured with lower trust; TPM-backed `KeyProvider` is platform adapter and must not be falsely called unextractable if boot isn't trusted.
- Maximum encrypted record payload 8 MiB. For larger sensor datasets, provider must use its own approved internal template abstraction/format or raise `UNSUPPORTED` pending ADR; do not increase public control frame or split templates through arbitrary socket packets.
- During enrollment Runtime requests provider to stage a *protected blob* via scoped template storage port; only restricted storage adapter sees plaintext. The atomic `commit_registry` transaction inserts ciphertext+enrollment entry+index and moves journal phase together. If encryption/key access fails, do **not** send mutation if it has not already been armed; if hardware mutation already occurred, preserve NeedsReconciliation.
- On remove, after provider confirms no longer registered, transaction removes active index and handles blob retention per recovery policy; preserve pending blob until all required recovery conditions met. Physical secure erasure is not guaranteed by logical record deletion.

## Encryption and rollback are separate

SHA-256 storage codec detects accidental corruption, AES-GCM protects blob integrity under secret-key custody, and hardware rollback resistance is an independent verified property. None replaces the others. On corrupt GCM tag, `TemplateUnreadable`, not `NotMatched` or empty enrollment. Key rotation is a journaled migration: decrypt/reencrypt with new key ID, commit atomically, leave old version until verified transition and ability to recover.

## Provider-managed TA templates

For Goodix/Nord 4, `template_blobs` may remain empty. Registry contains only bounded opaque provider registration reference, format/version and subject binding. Do not import TA templates into redb or export from TA unless a separately verified capability and ADR permits it. If TA storage state changes independently of Registry, reconciliation is mandatory.

During any read or migration, compare `template_blobs` key against `ProtectedTemplateRecord.enrollment_id/generation` before decryption. Reconstruct AAD from the matching Registry `SubjectRef`, `AuthenticatorId`, generation, and format fields under a consistent read snapshot. Mismatched value/key/Registry ⇒ `CorruptRecord`, not an empty enrollment or a retry. See [23](23-storage-codec-and-index.md) and [25](25-v061-conflict-resolutions.md).
