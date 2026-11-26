# Нормативный пакет v0.6.6

Для первого выпуска Linux сначала прочитайте [документ 35](35-first-release-scope.md).
Он задаёт системную модель доверия, актуальную схему новых баз v4 и критерии
приёмки, а при расхождении имеет приоритет над другими документами пакета и
предложенными ADR. Профиль Veles определяется отдельно.

## Порядок применения

1. При регистрации и замене [документ 34](34-v066-enrollment-command-projection.md)
   устанавливает явное различие Create и Replace и требования к старой привязке.
   Для активных операций Provider wire требуется minor 3 или выше.
2. При расхождении базовых документов с поправками применяйте профиль выпуска,
   затем документ 34 и пакет v0.6.5 (30–33), затем поправки 28, 27, 25 и 26.
   [Документ 29](29-v064-provider-owned-matching.md) закрепляет сопоставление за
   провайдером. Упоминания схем хранения v2 и v3 уточняет документ 35.
3. [Исходные Protobuf-схемы](../../../../contracts/proto/carosign/) задают проводной
   контракт. Примеры из материалов v0.5 и аппаратные справки служат контекстом.
   Публикация стабильного внешнего ABI требует отдельной проверки совместимости.
4. Принятые ADR уточняют соответствующие документы пакета в пределах
   указанной в них области. При расхождении с ранним текстом применяется
   принятое уточнение с учётом профиля первого выпуска. Проекты и непринятые
   части решений остаются в [указателе ADR](../../../adr/README.md).

## Принятые уточнения

| Область | Где закреплён договор |
|---|---|
| Системная граница доверия | [01](01-decisions-and-change-control.md), [35](35-first-release-scope.md) |
| Запрос проверки, идемпотентность, PAM, клиентский транспорт и health | [07](07-public-client-contract.md), [13](13-limits-errors-and-observability.md) |
| Наблюдение провайдера, supervisor и владение ресурсами | [06](06-provider-host-and-wire.md), [09](09-device-and-broker.md) |
| Доказательства, завершение проверки и доставка | [11](11-evidence-and-assurance.md), [04](04-pure-engine.md) |
| Источник идентификаторов и отправка изменения | [21](21-internal-port-signatures.md), [22](22-actor-messages.md) |
| Типы аутентификаторов, события захвата и надёжность | [36](36-authenticator-types-and-events.md) |
| Инициализация, запись установки и условная миграция | [23](23-storage-codec-and-index.md), [10](10-persistence-and-recovery.md), [35](35-first-release-scope.md) |

## Закреплённые здесь решения

- Rust workspace: `carosign-core` → `carosign-engine`; отдельно `carosign-persistence`, `carosign-evidence`, `carosign-runtime`, `carosign-protocol`, `carosign-provider-sdk`, `carosign-linux`, `carosign-linux-ipc`, приложения. Нет обратных зависимостей.
- Core/Engine — чистые детерминированные Rust-библиотеки. Engine реализует `decide(state, input) -> Decision`; Runtime выполняет эффекты, возвращает наблюдения.
- Внутри Runtime один последовательный обработчик на операцию, ограниченные mailboxes, отдельные супервизоры оборудования. Для первой реализации допускается Tokio, **не** распространять Tokio в Core/Engine или межъязыковой ABI.
- Registry, MutationJournal и outbox в **одной** транзакционной метадата-БД; baseline — redb. Template Storage независимо и может принадлежать trustlet.
- Изменяющая команда перед отправкой проходит **устойчивый `DispatchArmed`**. После него обрыв означает `NeedsReconciliation`, пока независимая сверка не докажет обратное. Не отправлять мутацию повторно только из-за потери ACK.
- Провайдер — **отдельный устанавливаемый процесс** с `carosign.provider.v1`; любой язык; Rust SDK необязателен. Схемы `.proto` — первичный wire-контракт. Linux transport baseline: `AF_UNIX/SOCK_SEQPACKET`, raw Protobuf packet + ancillary FD, без Rust ABI.
- Идентификатор операции и публичная ссылка на grant не являются bearer-capabilities; полномочия устанавливаются системным проверяющим на каждой критической границе.
- `Matched` провайдера не равно `AuthenticationSucceeded` и не подтверждает аппаратную защиту. Hardware token передаётся только подходящему независимому проверяющему.
- Core/Runtime не импортируют Goodix/GXFP/QTEE/libfprint/Veles. `libfprint` — самостоятельный C/GLib worker, `fprintd` — независимый клиентский адаптер.
- В Veles используется существующий владелец профиля/сессии/Поводник/Явь и Uzel; **никаких** прямых зависимостей в Core.

## Архитектура и владение

- [01. Зафиксированные решения и контроль изменений](01-decisions-and-change-control.md)
- [02. Точное дерево crates и внутренних модулей](02-crates-and-modules.md)
- [03. `carosign-core`: exact domain contract v1](03-domain-types.md)
- [04. `carosign-engine`: deterministic transitions and effects](04-pure-engine.md)
- [05. `carosign-runtime`: actor ownership, concurrency and effect execution](05-runtime-actors.md)
- [21. Exact internal port contracts and ownership](21-internal-port-signatures.md)
- [22. Runtime actors: concrete command/observation contracts](22-actor-messages.md)

## Интерфейсы, полномочия и ресурсы

- [06. `carosign.provider.v1`: process and wire contract](06-provider-host-and-wire.md)
- [07. `carosign.client.v1`: client-facing service and PAM mapping](07-public-client-contract.md)
- [08. Identity Authority, policy grants and revocation](08-identity-authority.md)
- [09. Device discovery, resource graph and Linux Hardware Broker](09-device-and-broker.md)
- [11. Evidence, proof verification and assurance evaluation](11-evidence-and-assurance.md)
- [12. Independently installable provider packages and updates](12-package-and-update.md)
- [14. Concrete providers and external platform integrations](14-provider-integration-contracts.md)
- [20. Wire-level validation and encoding rules (both channels)](20-protobuf-wire-validation.md)
- [24. Stable package identity and platform-managed encrypted templates](24-package-identity-and-templates.md)
- [26. `carosign.broker.v1` — mediated service protocol (normative v0.6.1)](26-mediated-broker-v1.md)

## Хранилище и восстановление

- [10. Durable metadata, outbox, reconciliation and transactional boundaries](10-persistence-and-recovery.md)
- [23. Exact stable encoding for redb metadata keys and values](23-storage-codec-and-index.md)

## Реализация и приёмка

- [13. Bounded execution, error model and observability](13-limits-errors-and-observability.md)
- [15. Conformance and adversarial acceptance cases](15-conformance-matrix.md)
- [16. Work orders for implementation agent (architecture locked)](16-implementation-workorders.md)
- [17. Differences from v0.5 and verified-unknown boundary](17-delta-and-open-evidence.md)
- [18. Review gates, required reports and acceptance discipline](18-review-gates.md)
- [19. End-to-end message and persistence traces](19-normative-traces.md)

## Поправки и уточнения v0.6.1–v0.6.6

- [25. v0.6.1 — normative reconciliation of four review findings](25-v061-conflict-resolutions.md)
- [27. v0.6.2 — B-01 and B-02: normative Engine, commit and error decisions](27-v062-engine-conflict-resolutions.md)
- [28. v0.6.3 — OperationState discriminant and dispatch isolation (C-04 / E-08..E-14)](28-v063-operation-discriminant-and-send.md)
- [29. v0.6.4 — Matching is exclusively provider-owned (M-01…M-12)](29-v064-provider-owned-matching.md)
- [30. v0.6.5 — independent specification audit and decision precedence](30-v065-audit-and-authority.md)
- [31. v0.6.5 — B-10: challenged enrollment authorization](31-v065-enrollment-authorization.md)
- [32. v0.6.5 — B-11/B-12/B-13: bulk command body from memory to module](32-v065-bulk-command-buffer.md)
- [33. v0.6.5 — durable immutable MutationPlan for Engine recovery](33-v065-durable-mutation-plan.md)
- [34. v0.6.6 — explicit enrollment command projection and provenance (B-14)](34-v066-enrollment-command-projection.md)

## Правило для coding agent

Действие по умолчанию: **реализовать точно по документу**. Если предлагаемая реализация требует альтернативного типа/состояния/протокола/хранилища/привилегии, остановить изменение, вывести `ARCHITECTURE_BLOCKER` с местом, конфликтом и минимальными вариантами, и не писать новый архитектурный код до решения. Hardware-specific неизвестное также является blocker. Не заменять ошибку/отказ на молчаливый fallback.

[Заметка об исходном пробеле enrollment challenge](27-enrollment-challenge-gap.md)
сохранена для истории. Действующий контракт challenge и HAT описан в
[документе 31](31-v065-enrollment-authorization.md).
