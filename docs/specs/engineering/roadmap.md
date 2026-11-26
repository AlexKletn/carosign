# Дорожная карта проектирования и реализации

## Этап 0 — утвердить нормативный фундамент

- Доменная модель, границы Runtime/Authority/Evidence/ResourceProvider.
- Полные `.proto` с tagged messages, пределами, presence, FD slots, strict error enums.
- Правила version negotiation, compatibility и idempotency.
- Capability descriptors и независимые assessments.
- Исходный threat model.

**Критерий:** независимый разработчик может реализовать simulated provider на C или Go только по спецификации.

## Этап 1 — Runtime + Linux skeleton

- Основной `carosignd`, public socket, Provider Manager, Supervisor.
- Системный inventory/device resolution, process management, private seqpacket.
- Bounded scheduler, leases, cancellation, observer events.
- Registry и persistent journal с fail-closed recovery.
- Simulated provider для проверки без hardware.

**Критерий:** `discover → start verify → events → result → cancel/restart`, плюс restore durable mutation.

## Этап 2 — перенос GXFP5130

- Выделить отдельный provider для захвата, matching и software templates.
- Обновить storage с scoped authenticated metadata, управления ключами и migration.
- PAM + CLI/GUI как клиенты системного API, а не старого монолита.
- Проверить full `enroll/verify/remove/reboot`, user separation и degraded mode.

**Критерий:** Linux использует GXFP без прямых аппаратных обращений clients.

## Этап 3 — libfprint bridge

- C/GLib worker, асинхронный loop, adapters к FpContext/FpDevice/FpPrint.
- Валидация feature sets на конкретных устройствах.
- Избежать конфликтов с fprintd; опциональный fprintd D-Bus client adapter отдельно.
- Миграция только с явным соглашением пользователя и доказанной совместимостью.

**Критерий:** произвольный supported libfprint device работает без изменений Core.

## Этап 4 — Goodix QSEE

- Продуктовый Rust worker, отдельный transport/lifecycle от прототипного C.
- Закрыть нерешённые IRQ и direct native match, затем enroll/remove/reconcile.
- Обслуживание listener/FS/GPFS/RPMB с контролем poisoned-state.
- Проверить persistent templates, поколения, восстановление после сбоев.

**Критерий:** нативный Rust без Android HAL выполняет полный цикл на TB371FC.

## Этап 5 — Veles/Nord4

- Adapter identity/grants/resources/interaction/evidence.
- Реальный доверенный FOD workflow с подсветкой и protected input.
- Проверка токенов в совместимом key consumer, если поддерживается.
- Сверка независимых journal владельца Veles и hardware provider.

**Критерий:** Veles использует независимую platform semantic model без дублирования session/auth authority.

## Этап 6 — production quality

- Conformance suite для всех языков и worker providers.
- Проверки при kill/reboot/hang, миграции и rollback.
- Security audit, documented assurance per configuration, safe packaging and installation UX.
- Отсутствие неограниченных privileged APIs у сторонних providers.

## Приоритет

Ни один новый hardware provider не должен начинаться как исключение из общего IPC. Но ранний GXFP vertical slice допускает внутренние временные реализации, если их явно пометить как не-normative и не привязать к public API.

## Rust execution work breakdown v0.5

Раскладка по пакетам, модулям и контрольным вертикальным срезам: [rust/24-implementation-sequence.md](rust/24-implementation-sequence.md).
