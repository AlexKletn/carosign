# 05. Обнаружение, разрешение конфликтов, привязка устройств

## Два этапа

1. **Пассивный match:** platform discovery даёт сведения из udev/sysfs/ACPI/DT/USB/HID, Provider Manager ищет соответствующие profiles без запуска стороннего кода.
2. **Ограниченный probe:** только при необходимости и после выбора кандидата; read-only с точки зрения persistent hardware state. Небезопасный probe не должен автоматически исполняться.

## Candidate resolution

`Candidate = PackageId + ProfileId + MatchProof + RequiredResources + CompatibilityStatus`.

Статусы: `Compatible`, `RequiresProbe`, `MissingDependency`, `Incompatible`, `Ambiguous`. Порядок выбора: системное явное назначение → единственный одобренный match → admin policy → `Ambiguous`. Поле priority, написанное в пакете, не может независимо вытеснить одобренный драйвер.

## Устойчивая привязка

`DeviceLocator` (например `/dev/gxfp`) не является стабильной идентичностью. Использовать доступные serial/hardware characteristics/platform position и сохранять связь с `AuthenticatorId`. Если уникальных характеристик нет, отмечать identity confidence и блокировать неочевидную автоматическую привязку шаблонов после смены устройства.

## Изменение железа

- Device hot-unplug: немедленный отзыв прав на новые операции; текущая verification → failure, мутация → reconcile при потенциальном dispatch.
- Reconnect: новый `DeviceEpoch`; старые операции и outcomes отвергаются.
- Дублирующиеся match одного датчика: не позволять параллельно запускать конфликтующие workers.
- Profile revision change: повторная проверка совместимости шаблонов и binding.

## В Veles

Platform discovery реализуется через Device Registry/leases Veles, но нормализованные `DeviceDescriptor`, `HardwareIdentity` и механизм selection остаются независимыми.

## Не предрешено

Конкретная грамматика правил match, сертификаты для производителей, механизм driver blacklisting и UI выбора неоднозначного backend должны быть утверждены отдельно. Пока используется понятие нормализованных правил, а не произвольные shell-команды в manifest.
