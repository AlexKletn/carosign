# P4. `bio.evidence` — сопоставление, доказательство, решение

## Три разные ступени

1. `BiometricMatch`: provider сообщает, что по собственным алгоритмам нашёл совпадение.
2. `EvidenceAssessment`: доверенный verifier проверил применимые свойства evidence (подлинность, challenge, источник, subject binding, freshness).
3. `AuthenticationDecision`: внешняя Authentication Authority решает, хватает ли подтверждённых факторов для конкретной операции. Сам Core не открывает сессию.

## Типы данных

```rust
// Conceptual types, NOT wire ABI.
enum EvidenceMaterial {
    SoftwareAssertion(SoftwareMatchReport),
    ProviderAssertion(OpaqueAssertion),
    HardwareToken(OpaqueToken),
}
struct VerificationContext {
    operation_id: OperationId,
    subject: SubjectRef,
    authenticator: AuthenticatorId,
    enrollment_generation: u64,
    challenge: ChallengeRef,
    purpose: VerificationPurpose,
    deadline: Deadline,
}
```

`HardwareToken` как `bytes` **не гарантирует** аппаратную проверяемость. Для protected key operations нужен токен, который способен принять/проверить именно соответствующий trusted mechanism (например KeyMint), с совместимым challenge и scope.

## Разделение механизмов

- GXFP host matching: программный результат, не аппаратный токен. Применимость для Linux PAM определяется доверием к worker/Runtime и политикой.
- Goodix QSEE: matching выполняет stock TA; фактическая цепочка защищённых гарантий и возможность верификации токена должна проверяться для устройства.
- Nord4 QTEE: имеющиеся исходные исследования не подтверждают сквозную цепочку `fingerprint → token → KeyMint permitted operation`; strict hardware class пока нельзя заявлять.

## Verification contract

`Evaluate(context, EvidenceMaterial, RequiredProperties) → AcceptedAssessment | Rejected | Indeterminate`.

Trusted verifier проверяет актуальный subject/challenge/purpose, поколение, происхождение и область доверия. Для software assertions результат может быть принят по policy software trust, без генерации фиктивного hardware token.

После истечения deadline, смены subject/session/authenticator generation, cancellation или revocation прежнее evidence нельзя применить к новому запросу. Успех provider после отмены подавляется на границе Runtime.

## Необходимые свойства

- Неподменяемость provider result и соответствие одному operation.
- Challenge freshness и impossibility of replay (когда поддерживается trust chain).
- Привязка к нужному subject и enrollment generation.
- Совместимость с требуемым protected key/consumer.
- Проверенная оценка возможностей, не самодекларация worker.
- Отдельная политика для enrollment и повторной регистрации.

## Ошибочные конструкции

Не заменять proof одним `true`. Не считать срабатывание TA подтверждённым trust цепочки при unlocked bootloader. Не передавать неограниченный сырой HAT в публичный UI и не добавлять аппаратную гарантию программной подписью пользователя/root процесса.
