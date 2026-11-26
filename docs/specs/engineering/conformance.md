# Conformance suite и проверяемые свойства

## Задача

Языково-независимый Provider Protocol означает, что эталоном является wire spec, а **не** Rust SDK. Один и тот же набор сценариев должен работать с workers на Rust, C/C++, Go, Python, с учётом платформенных особенностей.

## Protocol conformance

1. Hello: совместимые версии; несопоставимые major; неизвестные features.
2. Envelope: неверный sequence, повтор command_id, неверный worker generation, truncated packet/ancillary.
3. Binding: worker до BindDevice не может выполнять operations; invalid lease отклоняется.
4. Idempotency: retry same payload не создаёт новую operation; different payload с тем же key — conflict.
5. Exactly one logical final outcome (когда соединение живое); поздний outcome не принимается.
6. Cancellation accepted vs confirmed quiesce.
7. Unsupported commands должны давать явный error, не `success`.
8. Resource FD slot validation, no leaks on protocol error.
9. Event sequence gap/reconnect semantics.
10. Bounded queues/large messages/CPU and memory controls.

## State/crash conformance

| Инъекция | Требуемый результат |
|---|---|
| Worker killed before mutating dispatch | NoChange при доказанном отсутствии dispatch |
| Worker killed after dispatch before response | `NeedsReconciliation` |
| Runtime crash after provider success before Registry commit | recover by journal, no duplicate hardware mutation |
| Crash after Registry commit before journal terminal | independently verify and finish, no duplicate enrollment |
| Device disappeared during verify | reject, revoke, resource fenced |
| Device disappeared during enroll | safe uncertainty state |
| Revocation during matching | suppress accepted result |
| UI disconnect observer | no arbitrary cancellation |
| Owner disconnect | cancel/revoke according to operation class |
| Provider package upgrade with incompatible format | fail activation or explicit migration, never silent data loss |

## Security conformance

Check: spoofed UID/SubjectRef; unauthorized enrollment; fake hardware assurance claim; proof replay; stale challenge; wrong key receiver; shared device race; untrusted manifest path; unexpected FD type; diagnostics attempt to expose templates. Hardware assurance additionally requires device-specific evidence, не только тестовый simulator.

## libfprint bridge checks

On supported representative devices: open/close, enroll, verify, unsupported enumerate/delete, cancel via GCancellable, serialized FpPrint roundtrip/compatibility, coexistence policy with fprintd.

## Goodix-specific checks

Exact command bodies and reply statuses, loaded TA, FS/GPFS/RPMB listeners, IRQ progress, durable enrollment and match after reboot, poisoned listener behaviour. Отличать реальную измеренную работу от имитации транспорта.

## Veles integration checks

Revocation on session/surface change, profile binding, generation invalidation, no raw biometric data to apps, token acceptance by trusted consumer (если оно заявлено). Synthetic tests не считаются доказательством аппаратного защищённого пути.
