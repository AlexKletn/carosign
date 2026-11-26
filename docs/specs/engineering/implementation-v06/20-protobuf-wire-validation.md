# 20. Wire-level validation and encoding rules (both channels)

The checked-in `.proto` files are the baseline schema. These are not casual sketches. Protobuf compilation and independent C/Rust round-trip are required before calling the external ABI finalized.

## Mandatory semantic validations

- All IDs with suffix `_id`, `_ref` identifying an entity: **16 nonzero bytes**, except opaque `provider_opaque_ref` and `recovery_ref` (0..4096) and `authority_request_ref` (0..256). `subject_selector.opaque_authority_selector` 1..256 bytes.
- `frame_seq`: per-direction monotonically increments from 1 on each connection; no wrap; close on gaps/duplicates. `event_sequence` starts 1 per operation; client replay may explicitly report `EVENT_GAP`.
- Provider `WorkerHello` MUST be first worker frame; the *envelope* `worker_instance_id` is empty **only** for WorkerHello. Manager `Initialize` carries trusted instance ID; after initialized, every frame's worker instance ID matches exactly.
- Provider envelope `operation_id` required on `Start*`, `ProviderProgress`, `*Outcome`, `RequestCancel` (target ID also in payload), optional/empty on Hello/Init/Bind/Describe. `command_id` required on command request and matching ack/response. `device_epoch` must match BoundAuthenticator after Bind.
- Protobuf `enum` default UNSPECIFIED (zero) is invalid whenever the field is semantically required; unknown enum values rejected. Duplicate field assignment in oneof is rejected by conformance validator to prevent parser-dependent interpretations.
- UTF-8 is guaranteed by protobuf `string`, but byte limits are enforced after decoding. `label<=64`, error safe message<=256, implementation_version<=64, format_id<=128; never log sensitive body on rejection.
- Message count and repeated feature lists bounded (features <=32, devices per response <=128, enrollments per response <=256, `fd_slots<=8`); duplicate features/resource slots are errors.
- Protobuf unknown **optional** fields in established harmless messages may be ignored for forward compatibility, but they confer no authority/capability. Unknown security-critical semantic values are rejected; new mandatory security behavior requires negotiated feature or major version. Never rely on prost silently discarding unknown fields to validate proof or grants.
- `evidence_format_id` is advisory format metadata; proof acceptance comes from trusted verifier, not worker claim. Provider `opaque_token` up to 16 KiB on **private** channel only, never public client.
- For provider `BindAuthenticator`, received `SCM_RIGHTS` count must equal `fd_slots.len`, all indices unique [0,N), accepted roles only, each resource ID/lease verified against actual broker-issued claims. No FD pass in any other provider-v1 message. Broker-v1.1 has a separately specified constrained bulk-body exception.
- Provider major=1, **minor>=2** required for live operations; broker major=1, **minor>=1** for large-body handlers. Earlier minor ranges are historical and cannot activate these baseline operations. Both sides MUST negotiate before sending operations; new security semantics must not be silently downgraded.
- Max serialized envelope 65536B; one message per seqpacket packet, no user-space length prefix. Reject `MSG_TRUNC`/`MSG_CTRUNC` and close received ancillary FDs on error.

## Sender vs receiver obligations

Sender constructs only semantically valid fields. Receiver MUST validate independently; SDK implementation is not trusted. Do not deserialize untrusted protobuf directly into `CallerContext`, `ValidatedGrant`, `AssuranceAssessment` or `DeviceLease`: construct those solely through privileged trusted ports.

## Error handling

Wire parse failure → typed `INVALID_MESSAGE` where safe to respond, then close. Unsupported major/minor → `UNSUPPORTED_VERSION` before hardware. Duplicate or stale worker epoch/sequence → close worker and quarantine affected in-flight operations. Invalid public request → reject only that request unless transport framing is invalid. No `panic!` on malformed wire.

## Conformance fixture requirements

Provide raw packet bytes for successful Hello/Bind/Verify, malformed identifier length, enum 0, out-of-order seq, duplicated FD roles, extra FD, absent FD, truncated seqpacket, duplicate oneof, stale epoch, oversized label and unknown feature. Same serialized bytes must be consumed by Rust and independent C reference decoder. Some language runtimes discard unknown fields; conformance must enforce safe behavior through explicit validators.

### FD-role validator matrix (v0.6.1)

| Role | Expected approved object | Additional validation |
|---|---|---|
| `FD_ROLE_DEVICE` | approved broker-opened device handle | fstat/handle identity compared to delegated resource; generic /dev path not authority |
| `FD_ROLE_EVENT` | eventfd | validated as broker-issued eventfd; no trust in event contents |
| `FD_ROLE_CONFIG` | sealed, read-only memfd | required seals and bounded content schema; no writable mapping |
| `FD_ROLE_MEDIATED_SERVICE` | connected AF_UNIX SOCK_SEQPACKET | separate `carosign.broker.v1` protocol; lease/resource validated **per call** |

Slot indices must exactly cover 0..N-1, maximum 8. Each nonzero 16-byte resource ID must be in current lease and have the exact approved role and FD identity. Duplicate resource IDs require separately approved grants; otherwise reject. Ancillary FD is never allowed on `WorkerHello`, `Initialize`, `QueryMutation`, operations or event packets. Any invalid bind closes all received FDs and fails before device operations. No generic broker passthrough for arbitrary ioctl/TA commands. See [25](25-v061-conflict-resolutions.md).

### v0.6.4 validation additions

Provider minor >=2 is required. Validate `StartVerification.permitted_enrollments`: nonempty; bounded by configured maximum; no duplicate EnrollmentRef; current subject/authenticator/generation; kind and reference consistency; no raw bytes beyond 4096 opaque; matching `AuthenticatorRef` epoch. Validate `StartEnrollment.replace_binding` iff `replace` present and exact same ref. Optional `matched_enrollment` on success must be member of request binding set. An empty `VerificationOutcome.evidence` on Matched is legal but creates ProviderAssertion, **never** verified hardware evidence. Unknown binding kinds fail closed. Do not negotiate minor 0 or 1 for live operations in this baseline. See [29](29-v064-provider-owned-matching.md).

### v0.6.5 additional mandatory validation — provider 1.2, broker 1.1

- `CapabilityDescriptor.enrollment_authorization`: required and nonzero when `PROVIDER_ENROLL`; `ENROLL_AUTH_HAT_CHALLENGE` requires compatible trusted challenge/credential/sink services, else enrollment unavailable. `ENROLL_AUTH_NONE` prohibits authorization ref/digest in `StartEnrollment`.
- `PrepareEnrollmentChallenge` command: valid only for HAT enrollment before `StartEnrollment`, with same enrolled operation/subject/authenticator and prepared MutationId; result carries broker-verified 16B challenge ref, 1..64B challenge, nonzero bounded validity, current worker/epoch/command correlation. Broker-side trust record MUST match; raw worker assertion never establishes authority. `StartEnrollment` requires exact 16B auth ref + 32B digest on HAT path; ref/digest must both be absent on NONE path. Unexpected fields, replayed/expired/refused sessions fail closed.
- `StartRemoval.enrollment_binding`: required and exactly equal to `enrollment` (ID, authenticator, generation), with matching authorized Registry binding and kind; legacy `provider_opaque_ref` must be empty for vault, equal-or-empty for provider-opaque. Never fallback to legacy unscoped deletion.
- Broker minor 1 is mandatory for an external-body handler; `ServiceReady` announces it. `ExternalBody` implies exactly one FD, absent body implies **zero** ancillary FD. Only `Call` and `CallResult` may carry one. Check seals READ-ONLY + WRITE/GROW/SHRINK/SEAL, memfd type, no writable mappings, offset==0, file_size==fstat st_size, length==file_size in [1,8388608], 32B SHA-256 matching bytes, trusted handler max and schema. Reject `MSG_CTRUNC`, `MSG_TRUNC`, wrong request/call correlation before using any bytes. No arbitrary QSEE/ioctl handler is implied.
- Stored schema v2 `MutationRecord.plan` must be present from PREPARED, identical across all transitions, with canonical bytes/digest and checked scope; without it an armed record remains blocked. Registry snapshots do not constitute physical-state proof. See [33](33-v065-durable-mutation-plan.md).

### v0.6.6 additional mandatory validation — provider 1.3 and storage v3

Current negotiated provider minor >=3, superseding the historical >=2 text above. Every StartEnrollment (create **or** replace) MUST supply `planned_enrollment` as an exact nonzero `EnrollmentRef` and match `(authenticator, subject, generation, planned stored new ID)` under authenticated mutation scope; it may not equal `replace` or any old snapshot ID. For CREATE both `replace` and `replace_binding` are absent; for REPLACE both are present, refer to the exact authorized old target and match the immutable plan. Never infer from two missing nullable fields. A completed enrollment's `MutationOutcome.enrollment` must exactly echo the planned new ref; wrong/missing echo is not success. Stored schema 3 requires `EnrollmentRecord.binding_kind`, `BindingSnapshot.binding_kind` and `MutationPlanRecord.old_target` iff REPLACE/REMOVE, consistent with `MutationRecord.target_enrollment`. Verify v3 canonical digest including selected old target and binding kind; incomplete v2 remains quarantined. Full decision and B14 fixtures: [34](34-v066-enrollment-command-projection.md).
