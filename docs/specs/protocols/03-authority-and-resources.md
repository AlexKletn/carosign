# P3. `bio.authority` и `bio.resources` — полномочия и аренды

## Полномочие

Клиент может **запросить** проверку или регистрацию, но право на конкретное действие подтверждает доверенный внешний `PolicyAuthority`. Значение `UID`, имя пользователя или присланный `GrantId` без проверки не имеет властного смысла.

`OperationGrant` семантически связывает: issuer, caller binding, subject, authenticator/scope, тип операции, purpose, срок, поколение authority/session, revocation handle. В wire передаётся только проверяемый непрозрачный handle/ссылка на authority, не самодостаточная сериализованная структура доверия.

### Контрольные точки

`Authorize` перед созданием операции → `Validate/WatchRevocation` до выдачи lease → `Revalidate` перед потенциально необратимым вызовом → `Revalidate` перед выдачей результата и при смене generation. Отзыв уже начатой mutation не может отменить свершившуюся запись и обязывает reconciliation.

Для enrollment/removal требуется более строгая authorization policy, чем простая проверка совпадения peer UID. В Linux возможны privileged polkit+re-auth flows; в Veles — scoped grants и доверенная поверхность.

## Resource Broker

`ResourceSet` позволяет атомарно резервировать несколько связанных ресурсов, например сенсор+QSEE session+listener. Runtime не должен выдавать их по одному, если возможен deadlock или утечка эксклюзивности.

`ResourceClaim = kind, access, sharing, resource_selector, mandatory`. Manifest декларирует требования, системная политика решает, что разрешено. `Lease = ID + owner instance + device epoch + scope + expiry/revocation semantics`.

### Команды

`Describe`, `Acquire`, `Delegate`, `Revoke`, `Release`, `Reset`, `GetState`. Предпочтительно `Acquire(ResourceSet)` целиком (или безопасный prepare-commit reservation). Конфликтующие ресурсы не выдавать одновременно.

### Состояния

`Available → Reserved → Active → Quiescing → Available`.

Fault: `Active → Uncertain → Recovering → Available | Unavailable`.

`Revoked` относится к **разрешению**, а `Available` — к **физическому ресурсу**. Они не эквивалентны.

### Linux implementation

FD через `SCM_RIGHTS` только после explicit grant, валидации происхождения и типа FD. Ограниченный FD не означает поддерживаемый отзыв; если небезопасно выдавать raw ioctl, broker исполняет разрешённые команды самостоятельно. Путь `/dev/tee0` или `/dev/qseecom` не открывается для любого произвольного user plugin.

### Veles implementation

Вместо дублирования реестра/аренды устройство выдаётся через существующий системный механизм. `ResourceLease` остаётся **доменным** понятием, `Uzel`/Поводник — только адаптер реализации.

## Capability assessment

Manifest/provider descriptor декларируют функциональные операции. Независимый assessor подтверждает hardware-origin, spoof resistance, template protection, rate-limiting, rollback, key-bound evidence и ограничения загрузки, каждое отдельно. Отсутствующее или неподтверждённое свойство **не** трактуется как `Verified`.

### v0.6.5 — challenged enrollment versus policy grant

An `Enroll` manage-factor grant is **not** a Gatekeeper Hardware Authentication Token. For a provider advertising HAT requirement, a trusted credential authority must issue a challenge-bound restricted token reference, whose use is revalidated against the mutation's durable arm by a trusted broker/sink. No public client or generic provider-control frame may carry HAT bytes. Provider-owned matching unchanged. See [31](../engineering/implementation-v06/31-v065-enrollment-authorization.md).
