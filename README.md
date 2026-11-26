# CaroSign

> Hail, Meat-Bioform!

CaroSign регистрирует, проверяет и удаляет отпечатки через отдельные
аппаратные провайдеры. Клиенты Linux подключаются через `carosignctl`,
PAM или D-Bus-интерфейс fprintd. Состояние службы, провайдеров и устройств
можно посмотреть в [CaroSign](crates/clients/carosign-ui/README.md).

[Спецификация v0.6.6](docs/specs/README.md) ·
[Документация](docs/README.md) ·
[Терминальные инструменты](docs/guides/headless-clients.md) ·
[Установка на Arch Linux](packaging/arch/README.md)

Язык и стиль клиентских подсказок выбираются отдельно. Каталоги
устанавливаются пакетами. См. [локализацию клиентов](docs/contracts/client-localization.md).

## Текущий статус

**Платформа ещё не принята к выпуску.** Linux-состав обслуживает клиентский
сокет и запускает отдельные процессы посредника и провайдера. В испытательном
составе с моком проверены регистрация, проверка и удаление, включая сценарии
через PAM и D-Bus-адаптер fprintd; отмена и потеря клиента при изменении
оставляют неопределённую область закрытой для сверки. Положительный программный
результат не означает аппаратной аттестации.

Платформа также испытана на Linux с отдельным аппаратным провайдером TB371FC:
регистрация, проверка и вход через PAM работали на устройстве. Это не
подтверждает аппаратную аттестацию или готовность всех сценариев к выпуску;
восстановление ресурсов и интеграция графических сред требуют дальнейшей работы.

Для первого выпуска Linux действует упрощённая системная модель доверия из
[профиля выпуска](docs/specs/engineering/implementation-v06/35-first-release-scope.md).
Формат профиля провайдера описан в [linux-provider-profile.md](docs/contracts/linux-provider-profile.md),
подготовленный unit и пример конфигурации — в [packaging/systemd](packaging/systemd/README.md).

Создать новую базу можно командой `carosignd init-store /абсолютный/путь`;
существующий файл при этом не перезаписывается. Чтобы указать расположение
базы при проверке запуска, передайте `carosignd --config /абсолютный/путь`
с файлом, содержащим строку `store=/абсолютный/путь/metadata.redb`.
Для запуска службы нужны защищённые системные политика и профиль провайдера.
Испытательный установочный состав с systemd и D-Bus подготовлен в
[packaging/arch](packaging/arch/README.md); он не включает провайдера и не
меняет системный PAM автоматически.

## Где что лежит

Общий указатель материалов находится в [docs/README.md](docs/README.md).

| что | где |
|---|---|
| спецификация (нормативная) | `docs/specs/engineering/implementation-v06/` |
| порядок заказов W00…W15 | `docs/specs/engineering/implementation-v06/16-implementation-workorders.md` |
| схемы IPC для сборки (`.proto`) | `contracts/proto/carosign/` |
| доменные типы, переходы и оценка доказательств | `crates/core/` |
| выполнение операций, хранилище и адаптеры портов | `crates/runtime/` |
| межпроцессный протокол и SDK провайдера | `crates/protocol/` |
| службы, ресурсы и транспорт Linux | `crates/linux/` |
| CLI, PAM и D-Bus-адаптер fprintd | `crates/clients/` |
| тестовые реализации и фикстуры | `crates/testing/` |
| диагностические Rust-утилиты | `crates/tools/` |
| испытательный провайдер и утилита просмотра ограждений ресурсов | `crates/testing/carosign-mock-provider/` и `crates/tools/carosign-fence-inspect/`, отдельные Cargo workspace |
| установочные файлы и сценарии сборки и тестирования | `packaging/` и `tools/` |

## Сборка и проверки

Команды выполняются из корня проекта.

```sh
cargo build --offline                                   # хост
cargo test --offline --workspace                        # проверки на хосте
cargo clippy --offline --workspace --all-targets        # замечания линтера
CAROSIGN_SYSROOT=/путь/к/aarch64-sysroot sh tools/build-arch.sh /путь/к/новому-пакету.pkg.tar.zst
```
