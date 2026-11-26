# Linux adapter: службы, PAM, D-Bus, устройства

## Физические процессы

`carosignd` (core/runtime) + `bio-provider-*` workers + optional hardware brokers/listeners + clients PAM/CLI/GUI. Желательно запускать ProviderManager через systemd service, но Core не зависит от systemd.

## Дистрибуция

- Отдельный системный пакет `carosign`.
- Независимые `carosign-provider-libfprint`, `carosign-provider-gxfp5130`, `carosign-provider-goodix-qsee`, и др.
- Kernel DKMS может быть отдельным пакетом либо зависимостью hardware provider; обновление kernel module не обязано менять Core.
- Provider manifests в root-owned системных каталогах, persistent state в `/var/lib/carosign/`, конфигурация в `/etc/carosign/`.
- Пустая установка провайдеров не ломает демон.

## UNIX IPC

Primary client API — системный seqpacket socket. На уровне Linux проверить SO_PEERCRED, policy, service identity и request owner binding. Не считать UID-имя, присланное CLI, достоверным. Обновлённая политика для root-потребителей PAM должна учитывать trusted service identity, иначе произвольный root-процесс и PAM indistinguishable: при root compromise абсолютную изоляцию не обещать.

## PAM

`pam_carosign.so` инициирует `BeginVerification`, получает ограниченный поток progress/terminal outcome. `PAM_SUCCESS` только по accepted factor для текущего PAM-request; `NoMatch` → `PAM_AUTH_ERR`, недоступность/отмена → соответствующий fallback, без lockout из-за отсутствия устройства. Рекомендуемые PAM stacks поставляются как примеры, не перезаписывают `/etc/pam.d` автоматически. Защиту от повторного использования proof обеспечивает Runtime/auth authority.

## fprintd compatibility

Обособленный optional D-Bus adapter выдаёт существующим приложениям совместимую подмножину API. Он не должен конфликтовать с установленным системным `fprintd` за имя D-Bus и оборудование; установка/включение требует явного выбора ownership. Более богатые semantics аппаратных tokens через fprintd API не раскрываются, если API их не поддерживает.

## GUI / CLI

`carosignctl` — отдельный низкопривилегированный клиент. GUI подписывается на scoped operation events, не использует глобальный одноадресный сокет как в старом GXFP plaque. UI не получает raw fingerprints и не сообщает authoritative match. Для подэкранных датчиков взаимодействие с системной подсветкой/тачем идёт через отдельный `InteractionController` с lease.

## Device broker

Linux udev/sysfs/ACPI/DT/USB/HID для обнаружения. Worker получает только разрешённые ресурсы. `SCM_RIGHTS` имеет limitation на revocation. Root-only hardware operations целесообразно выполнять через broker, не раздавая полностью привилегированные raw device FDs.

## Recovery/security

Перезапуск службы должен до начала mutating операций сверить persistent journal; распознавание может быть заблокировано для impacted authenticator. Расширенная диагностика доступна только администратору; исключить импорт/экспорт нешифрованных отпечатков по обычному клиентскому API.
