# Эксплуатация, обновления, пакеты и диагностика

## Жизненный цикл установки

1. Устанавливается `carosign` без hardware packages.
2. Platform Registry читает только доверенные system-level manifests.
3. Hardware discovery сопоставляет устройства с установленными package profiles.
4. При наличии единственного одобренного кандидата Manager запускает worker и проверяет `Hello/Initialize/BindDevice`.
5. Active authenticator становится доступным клиентам; нулевой список — штатное состояние.

Названия Linux packages — примеры, не существующие опубликованные артефакты:

```sh
sudo pacman -S carosign
sudo pacman -S carosign-provider-libfprint
sudo pacman -S carosign-provider-gxfp5130
```

## Package directory и state

Разделить read-only executable/profile, private mutable state provider и platform registry/journal. Удаление пакета не вызывает автоматическое удаление шаблонов. Для полного удаления данных требуется явный privileged purge с проверкой фактических возможностей устройства.

## Обновление

- Stage нового бинарника и профиля, validate package + wire compatibility.
- `Draining`: запретить новые операции, завершить/отозвать действующие.
- Убедиться в безопасном `DeviceState`. Если `Uncertain`, отложить activation до recovery.
- Остановить старый worker, проверить template/persistent format compatibility.
- Выполнить миграцию с durable journal и rollback prerequisites.
- Запустить новый worker, проверить ready + hardware identity.
- Активировать; при отказе откатить только при сохранённой обратной совместимости, иначе quarantine.

## Независимые версии

`PlatformVersion`, `WireProtocolVersion`, `ProviderPackageVersion`, `DeviceProfileRevision`, `TemplateFormatVersion`, `PersistentStateSchema` должны храниться независимо. Обновление Core не заставляет пересобирать Rust/C/C++/Go/Python providers при поддерживаемой wire версии.

## Диагностика

Разделить telemetry событий: `ServiceHealth`, `ProviderLifecycle`, `HardwareHealth`, `OperationLifecycle`, `MutationRecovery`, `ProtocolErrors`. Для каждой записи использовать стабильный код/корреляцию. Нельзя логировать raw images, template bytes, HAT/opaque tokens, PIN/PSK, ключи или keyring secrets. User-level observers получают только допустимый UX-progress, администратор — безопасную расширенную диагностику.

## Alert policy

- `WorkerRestartLoop` → capped backoff и `Quarantined`.
- `PendingMutation` → alert и запрет conflicting mutating requests.
- `ListenerPoisoned` → ресурс unavailable до доказанного восстановления (Goodix-specific).
- `StorageCorrupt` → fail closed, без автоматического создания нового «пустого» store поверх повреждённого.
- `UnmatchedPackage` → информативный статус, не system fault.

## Установка PAM

PAM config не заменять автоматически; предоставить примеры для `sudo`, `polkit`, greeter, lockscreen, с проверяемым fallback на пароль. Отдельно тестировать отсутствие демона/сенсора, чтобы не блокировать вход. Служба secrets/keyring — отдельная интеграция.
