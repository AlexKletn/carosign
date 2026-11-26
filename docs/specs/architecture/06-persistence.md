# 06. Хранение, форматы, целостность

## Разделение ownership

| Слой | Что хранит | Владелец |
|---|---|---|
| `EnrollmentRegistry` | binding, SubjectRef, AuthenticatorId, provider refs, generation, state | Runtime |
| `MutationJournal` | durable intent/stages/reconcile refs | Runtime |
| `TemplateStorage` | программные шаблоны либо opaque hardware state | Provider или delegated store |
| `KeyProvider` | cryptographic key usage/sealing | отдельная инфраструктура |
| `SecretStore` | приложения/ключницы и их секреты | внешняя служба, не Core |

Не требовать от Goodix TA экспортировать шаблон. Для GXFP redb может хранить ciphertext AES-256-GCM; root-owned keyfile — допустимый fallback с честно низкой границей защиты, а hardware-backed provider — отдельная реализация.

## Пример Registry Record

```rust
struct EnrollmentRecord {
    enrollment_id: EnrollmentId,
    subject: SubjectRef,
    authenticator: AuthenticatorId,
    provider_id: ProviderId,
    provider_enrollment_ref: OpaqueReference,
    template_format: Option<TemplateFormatId>,
    generation: u64,
    state: EnrollmentState,
}
```

`OpaqueReference` — bounded, не сырые шаблоны; provider определяет смысл. Метаданные нельзя автоматически считать достоверным подтверждением TA state.

## Optional software template vault (provider-owned matching remains mandatory)

Authenticated encryption с уникальными nonce и AAD, включающими subject/authenticator/enrollment IDs, версию формата и metadata generation; nonce management и rotation по утверждённому crypto profile. Не публиковать plaintext в IPC. Locking/zeroization в runtime помогают, но не гарантируют защиту от привилегированного локального администратора.

Поддерживать минимум: schema versions, key rotation, write-then-atomic-rename/transactional commit, fsync/flush durability, corruption handling, export/import только с явным privileged intent, миграцию шаблонов с подтверждением compatibility.

## Provider-owned storage

Goodix/QSEE хранит шаблон в системе, обслуживаемой TA и listener. Runtime хранит только durable binding и recovery state. Не считать успешный `SAVE` достаточным подтверждением, если аппаратный API требует повторного чтения/перечисления.

## Journal durability

Должна быть определена последовательность: durable intent → аппаратный dispatch → durable provider observation → registry commit → durable journal terminal. Конкретный backend обеспечивает fencing, атомарность записи и durable storage semantics, включая `fsync` каталога при rename там, где это требуется. Каждый journal record версионирован и содержит `boot_id`/generation/reference для сверки.

## Защита от отката

AEAD защищает целостность записи, но не защищает от возврата к старому корректно зашифрованному blob. Для anti-rollback нужен отдельный доверенный monotonic source/secure storage и подтверждённая связка. Если аппаратный anchor не обеспечивает это, assessment должен отражать отсутствие свойства.

## Удаление

Логический `Removed` не означает физического невозможного восстановления из flash. Для TEE требуется фактическая semantics TA. Удаление provider package не удаляет шаблоны. Для полного purge нужны отдельный привилегированный workflow и явные обещания соответствующие устройству.
