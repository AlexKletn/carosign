# Enrollment challenge gap — B-10 (reconstructed review attachment)

**Provenance:** The original document named `27-enrollment-challenge-gap.md` was **not present** in the v0.6.4 archive retrieved for this audit. This file is a newly reconstructed review note from the reported B-10 issue, **not a verbatim copy of the missing original**. It deliberately has a distinct name from normative `27-v062-engine-conflict-resolutions.md`.

## Reported deficiency in v0.6.4

`StartEnrollment` carried `replace_binding` but no definition of where the device's enrollment challenge originates; how a trusted credential authority signs or provides an HAT; how challenge, subject, hardware authenticator and operation are bound; how a token reaches the TA; how expiry, cancellation and crashes are handled; and whether the provider may see credential/token bytes. An enrollment grant alone cannot substitute for a Gatekeeper-signed Hardware Authentication Token.

## Evidence and scope

The included research `reference/goodix-source/fingerprint/TZ-TRANSPORT-AUDIT.md` records a stock Gatekeeper token of **69 bytes**, a Goodix-produced challenge, and `gf_hal_enroll` accepting the token and reaching capture (but ending with secure-object write error 1035). This is device-specific observation, **not proof of successful registration, general token format compatibility, independent verifier reliability or a working production path**.

## Resolution

The generic platform contract, state ordering, reference-only provider wire and trusted enrollment credential authority are specified by [31-v065-enrollment-authorization.md](31-v065-enrollment-authorization.md), provider wire major 1/minor 2 and `common.CapabilityDescriptor.enrollment_authorization`.

B-10 is **contractually specified**, but production Goodix HAT issuance, trusted broker injection and credential-authority integration still require implementation plus device evidence. The implementation agent MUST report `ARCHITECTURE_BLOCKER` for any missing hardware-specific binding; it MUST NOT synthesize an unsigned HAT or use the diagnostic/test-enroll bypass.

**v0.6.6:** This historical B-10 note remains preserved. The current provider enrollment-command projection additionally requires protocol minor >=3 and the `planned_enrollment` field. See [34](34-v066-enrollment-command-projection.md).
