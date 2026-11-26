# 01. Зафиксированные решения и контроль изменений

## N-01: границы

`carosign-core` определяет доменную семантику; `carosign-engine` — чистые переходы; `carosign-runtime` — владелец операций и side effects; `carosignd` — Linux composition root. Единственный экспортируемый контракт внешнего hardware worker — `carosign.provider.v1` (не Rust ABI). Системный consumer, например PAM или veles-auth, **не** получает права на аппаратную команду и не владеет шаблонами.

## N-02: владение

- `OperationId` создаёт Runtime; `CommandId` — ProviderHost; `MutationId` — MutationCoordinator **до** durable intent; `LeaseId` — ResourceBroker; `WorkerInstanceId` — Supervisor.
- `SubjectRef` назначает IdentityAuthority; Provider получает только scoped, pseudonymous `SubjectBinding` и не выбирает Linux uid.
- `AuthenticatorId` — стабильная метадата под контролем платформы; `DeviceLocator` — только временный адрес; `DeviceEpoch` и `WorkerInstanceId` обязательны для fencing.
- Один owner операции, много авторизованных observers; закрытие observer не означает отмены.
- Registry и Journal принадлежат Runtime; provider-owned TA state не переписывается Runtime. Veles владеет привязкой фактора к профилю и политикой входа.

<a id="system-trust-root"></a>

## N-03: безопасность

- Внешний worker не объявляет собственную аппаратную гарантию. `ProviderCapabilities` описывают функции, `AssuranceAssessment` создаётся проверяющим владельцем по источнику доказательства и контексту устройства.
- Linux daemon/worker по умолчанию без сетевого доступа; привилегированный broker выдаёт только policy-approved ресурсы. `SCM_RIGHTS` не считается отзывным полномочием.
- Секреты, raw images, шаблоны и аппаратные токены не передаются публичному UI API. Provider wire может передавать ограниченные непрозрачные proof handles; их трактует verifier.
- Регистрация/удаление требуют отдельного grant, просто `SO_PEERCRED`/UID недостаточно. Отзыв проверяется до dispatch и перед delivery; мутация после dispatch требует сверки.
- Для системного состава Linux доверенной базой являются установленная система,
  администратор root и наделённые им полномочиями процессы, ядро и systemd.
  Сбои и подмена со стороны непривилегированных процессов входят в модель
  защиты; враждебный root, подмена ядра или загрузчика и откат всей системы
  или диска — вне неё. Локальное поколение на том же диске само по себе не
  доказывает защиту от такого отката. Эта граница не отменяет проверок
  полномочий, изоляции, отказа при ошибке и восстановления после сбоя.
  Для первого выпуска применяется [профиль Linux](35-first-release-scope.md).
  [Дополнительные механизмы корня доверия](../../../adr/adr-system-trust-mechanisms-proposed.md)
  остаются проектом.

## N-04: implementation baseline

- Rust 2024 edition, один Cargo workspace платформы и **отдельные** проекты hardware providers. `#![forbid(unsafe_code)]` в core/engine. Указание edition — целевой baseline, не утверждение о наличии компилятора в среде.
- `carosign-protocol` генерируется из `.proto`, не хранит auth-domain logic. Wire IDs — 16 bytes, никаких Rust enum layouts в IPC.
- Linux `SOCK_SEQPACKET`, Protobuf, контроль packet/FD размеров; `SCM_RIGHTS` только в явно разрешённых сообщениях.
- Первая БД метаданных — `redb`, одна транзакционная область для registry+journal+outbox. Никакая local DB транзакция не охватывает TEE.
- Internal async executor — Tokio. Только Runtime/Linux adapter зависит от Tokio; backend worker вправе выбрать любой язык/executor.
- Cargo feature flags не должны менять смысл v1 wire, только наличие необязательных локальных компонентов.

## N-05: модификации

Запрещено без отдельного принятого ADR:

1. Сливать Core/Engine/Runtime в один crate или добавлять зависимость Runtime → конкретный провайдер.
2. Заменять отдельный worker динамической библиотекой в адресном пространстве `carosignd`.
3. Делать `SO_PEERCRED`, `ProviderHello`, имя пакета либо `OperationId` достаточным доказательством полномочий.
4. Переотправлять mutating command по автоматическому retry после `DispatchArmed`.
5. Утверждать `VerifiedHardware` из одного ответа TEE без проверки цепочки доказательства.
6. Вводить зависимость Core от `systemd`, D-Bus, Uzel, PAM, Goodix или libfprint.
7. Переносить template bytes в public IPC в рамках v1.
8. Автоматически очищать Registry при отсутствии провайдера.
9. Заменять корректные различия `Rejected / Failed / Indeterminate` общим Boolean.
10. Изменять уже опубликованные wire tag numbers или семантику существующих полей.

## Допустимые решения исполнителя

Только локальные технические детали, не меняющие публичные сигнатуры и инварианты: имена приватных переменных, расположение вспомогательных чистых функций в том же модуле, аллокационные оптимизации без изменения порядка эффектов, логирование без секретов. Новые публичные типы, фоновые процессы, обработчики команд, хранилища и стратегии security fallback требуют ADR.

## Формат блокады

```
ARCHITECTURE_BLOCKER
Document: engineering/implementation-v06/<file>
Invariant: <N-ID or section>
Missing evidence: <observed absence or conflicting requirement>
Proposed alternatives: <minimal options, no implementation>
Impact: <wire / data / security / lifecycle / packaging>
```

До разрешения не выполнять небезопасный fallback. Если аппаратная функция не подтверждена — сообщить `Unavailable` или `Unsupported` согласно семантике, не выдумывать совместимость.
