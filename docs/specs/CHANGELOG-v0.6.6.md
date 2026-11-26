# Biometric Platform v0.6.6 — B-14 command projection closure

**Status:** normative architecture/design update, not verified hardware implementation.

- Fix ambiguity in implementation `MutationChange::Enrollment`: use distinct typed `Create(planned_new)` / `Replace(planned_new, validated_old_binding)`.
- Decide that `CommandPlan` **explicitly carries** the checked old binding for REPLACE as an ephemeral projection derived by privileged Runtime from immutable plan and locked Registry. Provider adapter never queries Registry or infers which enrollment was replaced.
- v3 durable plan stores binding kinds, explicit old target, full frozen scope, and v3 canonical digest; Registry enrollment persists binding kind. No raw templates/HAT are stored in a plan.
- Provider protocol minor 3 mandates `StartEnrollment.planned_enrollment`, assigned by system; completed enrollment echoes same identity, not physical proof.
- Update validators, actor/ports/recovery gates and negative acceptance scenarios B14-01..B14-12.
- v2 nonterminal mutations remain quarantined without verified migration. No new trust in worker-supplied physical observations and no automatic mutation retry.

Read `engineering/implementation-v06/34-v066-enrollment-command-projection.md` first; historical v0.6.5 docs remain in package for context with supersession amendments.
