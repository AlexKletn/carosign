# 04. Установочный пакет провайдера

## Требование

Аппаратные адаптеры устанавливаются отдельно от Core/Runtime, не требуют Rust, не загружаются в адресное пространство `carosignd`. Установка нулевого числа провайдеров — штатная конфигурация.

## Package / Profile / Runtime Instance

- `Package`: исполняемый файл, manifest, зависимости, версии, владельцы и файлы.
- `Profile`: правила совместимости и параметры конкретной модели/варианта устройства.
- `Instance`: один запущенный worker с отдельным IPC, поколением и аппаратными ресурсами.

Пример предлагаемой структуры для Linux:

```text
/usr/libexec/biometric/providers/goodix-qsee
/usr/lib/carosign/providers/org.example.goodix-qsee/package.toml
/usr/lib/carosign/providers/org.example.goodix-qsee/profiles/tb371fc.toml
/usr/share/doc/carosign-provider-goodix-qsee/
/var/lib/carosign/providers/org.example.goodix-qsee/   # persistent state
```

## Пример package.toml (предлагаемый формат)

```toml
schema = 1
[package]
id = "org.example.goodix-qsee"
version = "1.0.0"
[worker]
executable = "/usr/libexec/biometric/providers/goodix-qsee"
instance_model = "per-device"
[protocol]
name = "carosign.provider"
major = 1
min_minor = 2
max_minor = 2
[[profiles]]
id = "tb371fc"
path = "profiles/tb371fc.toml"
```

Это **пример**, не готовое утверждение об идентификаторах TB371FC. В рабочем пакете запрещать paths, выходящие за доверенный prefix. Package manifest не может сам выдать себе root/device permissions или hardware security level.

## Пример profile.toml

```toml
schema = 1
[profile]
id = "tb371fc"
revision = 1
[match]
platform = "linux"
rules = ["device-rule-tb371fc"] # symbolic; concrete ID requires validation
[requirements]
resources = ["fingerprint-device", "qsee-session", "trusted-storage-listener"]
```

## Идентичность пакета

Проверять права на каталоги и происхождение установленного исполняемого файла. При запуске не допускать TOCTOU между проверкой и исполнением. Подпись/происхождение пакета **не равна** сертификату безопасности аппаратного результата.

## Установка/удаление

Добавление пакета должно быть видно после refresh registry и сопоставления устройств; Core не пересобирается. Удаление пакета выключает provider, но **не удаляет** enrollments и state по умолчанию. Отдельная подтверждённая операция purging user data необходима для удаления.

## Совместимость

Независимо учитывать `provider_version`, `provider_protocol_major/minor`, `profile_revision`, `template_format_version`, `persistent_state_schema`. Обновление программы ≠ изменение формата шаблона.

## Доверенные и сторонние поставщики

Системная политика определяет разрешённых publishers, hardware profiles, ресурсы и допустимые assurance classes. Провайдер без подписанной/одобренной оценки всё ещё может работать в ограниченном режиме, но не получает фиктивный аппаратный trust.

### Current provider protocol requirement v0.6.6

The previous example `min_minor=2` is historical. For the v0.6.6 implementation use **min_minor=3, max_minor>=3** and reject workers without `StartEnrollment.planned_enrollment` support, according to [34](../engineering/implementation-v06/34-v066-enrollment-command-projection.md).
