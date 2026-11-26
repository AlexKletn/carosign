# Открытые вопросы и доработки спецификации

## Протоколы

1. Утвердить окончательные protobuf schemas, strict enums и `oneof`; текущие `.md` содержат sketches.
2. Решить, где hosted authority/evidence verifier process-boundary, а где Rust traits.
3. Зафиксировать FD slot protocol: типы, count/order, transport limits, blob ownership.
4. Установить максимальные размеры, deadlines и quotas per connection/op/device.
5. Согласовать feature negotiation и правила отказа при неизвестных критичных полях.
6. Определить journaled outcome retention и reconnect cursor policy.

## Аппаратные и platform semantics

7. Как идентифицировать устройства без serial и безопасно связывать старые enrollments?
8. Как разрешать несколько независимых providers на одном physical sensor/TA без конфликтов?
9. Как избежать блокировки QSEE shared listeners и обеспечить подтверждённый reset?
10. Какие реализации hardware broker должны оставаться в ядре, какие — в userspace?
11. Где проходит граница `SecureEnvironmentPort` для сторонних провайдеров?
12. Какая проверка у TA действительно подтверждает `Removed/Enrolled`?
13. Какие поддерживаемые конфигурации Linux дают реальный protected UI, а какие — только обычный GUI?

## Безопасность

14. Какой фактор требуется при `Enroll` и `Remove` для разных policy contexts?
15. Что допускается при открытом загрузчике и невозможной независимой аттестации?
16. Где хранить software template encryption keys, можно ли предложить TPM2 provider без зависимости на systemd?
17. Каким способом связывать hardware token с конкретным trusted key receiver?
18. Что считать приемлемым assurance для software-matcher в обычном PAM?
19. Как исключить подмену worker при обновлении пакета и миграцию неавторизованного state?

## Производство

20. Кто подписывает provider package, профили устройств и device assessments?
21. Нужно ли bundleировать подписанные TA/firmware assets или ссылаться на OEM-installed resources?
22. Как устроены версии storage migrations и доказуемый rollback?
23. Как поддерживать разные init systems без привязки Core к systemd?
24. Какие API и модель UI нужны для advanced modality и FOD координации?

## Исследовательские пробелы

25. Goodix TB371FC: повторные IRQ в native Rust прямом пути и полный verified match.
26. Nord4: защищённый путь fingerprint token → KeyMint, trust UI и профиль/TA binding.
27. GXFP: статистическая оценка thresholds и model security для production use.

## Рекомендованный следующий пакет

Превратить `protocols/` в полностью определённые `.proto` и семантические state tables с test vectors; затем зафиксировать `Provider Package Specification v1` и сделать один C/Go sample provider, доказывающий реальную языковую нейтральность.
