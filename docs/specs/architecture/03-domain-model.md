# 03. Доменная модель и идентификаторы

## Сущности

| Сущность | Назначение | Владелец |
|---|---|---|
| `ProviderPackage` | установленный исполняемый provider и описания | package registry |
| `ProviderInstance` | запущенный worker с generation | Provider Manager |
| `Device` | физическое оборудование и его locator | platform discovery |
| `Authenticator` | логический механизм enrollment/verification | Runtime + provider |
| `SubjectRef` | идентичность в пространстве внешней authority | identity authority |
| `EnrollmentRef` | устойчивая привязка биометрического фактора | registry |
| `Operation` | один начатый запрос с уникальным context | Runtime |
| `Session` | открытая сессия с аутентификатором | worker/Runtime |
| `ResourceLease` | ограниченное владение одним/несколькими ресурсами | broker |
| `EvidenceMaterial` | opaque result/claim/token | provider |
| `EvidenceAssessment` | проверенные свойства и совместимость | verifier |
| `GrantHandle` | ограниченное разрешение на операцию | authority |
| `MutationRecord` | durable intent, commit/reconcile | Runtime journal |

## Типовые структуры (семантические, не ABI)

```rust
struct SubjectRef { authority: AuthorityId, opaque_id: SubjectId }
struct EnrollmentRef {
    authenticator: AuthenticatorId,
    subject: SubjectRef,
    enrollment_id: EnrollmentId,
    generation: u64,
}
struct OperationContext {
    operation_id: OperationId,
    subject: SubjectRef,
    authenticator_id: AuthenticatorId,
    authenticator_generation: u64,
    enrollment_generation: u64,
    purpose: VerificationPurpose,
    challenge_ref: Option<ChallengeRef>,
    deadline: Deadline,
}
```

## Идентификаторы, которые нельзя смешивать

- `PackageId`: стабильное имя поставки, не криптографическая подлинность.
- `WorkerInstanceId`: случайный идентификатор одного запуска процесса.
- `DeviceLocator`: `/dev/...`, sysfs path и пр.; непостоянен.
- `HardwareIdentity`: подтверждённые идентификаторы железа, могут быть неуникальными.
- `AuthenticatorId`: устойчивый идентификатор механизма, формируется при безопасной привязке устройства.
- `OperationId`: случайная ID операции, НЕ bearer capability.
- `RequestId`: correlation одного wire request/response.
- `IdempotencyKey`: связь повторных requests с одной operation, scoped по actor/scope/payload.
- `SessionId`: идентификатор аппаратного сеанса, живёт не дольше одного instance/epoch.
- `MutationId`: долговечный ID попытки изменить постоянное состояние.
- `EnrollmentGeneration`, `DeviceEpoch`, `WorkerGeneration`: независимые пространства версий.

Рекомендуемые непрозрачные ID в wire — ровно 16 байт, проверяемые при десериализации. Durable generation — целое без переполнения, сравнение выполняется только внутри своего scope.

## Неизвестность и необратимость

`EnrollmentState = Active | RemovalPending | NeedsReconciliation | Unavailable | Orphaned`. Не смешивать `Unavailable` (нет backend) с `Removed` (отсутствие подтверждено).

`VerificationOutcome = Matched(EvidenceMaterial) | NotMatched | NoSample | Cancelled | Failed`. Это **не** системная успешная аутентификация.

`MutationOutcome = Committed | NoChange | Indeterminate | ReconciliationRequired | FailedBeforeDispatch`. Для мутации повтор после `Indeterminate` никогда не происходит автоматически, пока recovery не подтвердит безопасность.

## Связи

`SubjectRef` принадлежит пространству identity authority; может быть несколько enrollment на разных authenticator и несколько пальцев на одном authenticator. Label вроде `right_index` — UX metadata, **не биометрическая идентичность** и не security evidence.

`ProviderPackage` может поддерживать несколько profile, instance и authenticator. Два аутентификатора могут делить физический ресурс или TA, поэтому конфликт определяется ResourceSet, а не только AuthenticatorId.

## Целостность

Регистрация ссылается на конкретные `ProviderId`, `AuthenticatorId`, template format и поколения. Registry создаёт binding только после подтверждённого outcome. Неподтверждённое удаление не трактуется как гарантированное уничтожение шаблона.
