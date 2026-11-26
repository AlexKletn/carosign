# Документация CaroSign

| Задача | Где читать |
|---|---|
| Узнать текущий статус и собрать проект | [README проекта](../README.md) |
| Найти нормативные требования | [Спецификация v0.6.6](specs/README.md) |
| Установить систему | [Arch Linux](../packaging/arch/README.md), [настройка PAM](guides/ARCH_PAM_MANUAL.md), [службы systemd](../packaging/systemd/README.md) |
| Настроить интеграцию | [Профиль провайдера Linux](contracts/linux-provider-profile.md), [политика supervisor](contracts/supervisor-policy-v1.md), [источник подтверждения установки](contracts/system-installation-authority.md) |
| Сверить интерфейсы интеграции | [fprintd](contracts/FPRINTD_WIRE_BASELINE.md), [канал владельца ресурсов](contracts/resource-owner-wire.md) |
| Добавить перевод или стиль | [Локализация клиентов](contracts/client-localization.md) |
| Проверить первый выпуск | [Профиль и критерии приёмки](specs/engineering/implementation-v06/35-first-release-scope.md), [проверки отказов](release/first-release-fault-checks.md), [проверочный комплект](guides/REVIEW_BUNDLE.md) |
| Найти решение или предложение | [Принятые уточнения](specs/engineering/implementation-v06/README.md#принятые-уточнения), [проекты ADR](adr/README.md) |
| Посмотреть историю испытаний | [История реализации и проверок](history/README.md), [журналы GNOME](evidence/gnome-shell-51.1/README.md) |

Экспериментальная интеграция GNOME Shell описана рядом с
[патчем и сценариями воспроизведения](../compat/gnome-shell-51.1/README.md).
