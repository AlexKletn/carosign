# VelesOS adapter: повторное использование без зависимости от платформы

## Принцип

Новый продукт не является подсистемой Veles. `veles-auth`, Поводник, Явь, Uzel, реестр устройств, диспетчер и storage owners — **внешние инфраструктурные компоненты**. Core и Provider Protocol не импортируют Veles crates.

## Карта связей

| Независимая абстракция | Конкретная Veles-реализация |
|---|---|
| `SubjectRef` | идентичность профиля через владельца сессии |
| `OperationGrant` | bounded capability + проверка принципала/generation |
| `Authority` | `veles-auth` и полномочия управления факторами |
| `EvidenceVerifier` | доверенный verifier/anchor owner, принимающий соответствующий тип proof |
| `ResourceLease` | реестр устройств и платформенные аренды Veles |
| `InteractionController` | Явь, доверенная поверхность/ввод, дисплей |
| `ProviderWorker` | запускаемая ячейка под Поводником; реализация не обязана быть Rust |
| `Transport` | adapter Uzel/локальный wire, с сохранением semantics |
| `Secrets` | внешний владелец ключей, не runtime biometric core |

## Главное разграничение

Biometric Runtime владеет фактической **биометрической операцией** и состоянием аппаратной регистрации. `veles-auth` владеет пользовательским фактором, привязкой к профилю, политикой и подтверждением достаточности аутентификации. Их persistent state согласовываются через `EnrollmentRef`, generations, durable mutation/reconciliation — не через два независимых владельца единого binding.

## Реальный статус из исходного документа

`trust-anchor-auth-biometrics.md` показывает работающие тестовые элементы grants, journals, Uzel handlers, QTEE transport и экспериментальную stock TA работу; это ещё не полная защищённая биометрическая цепочка Veles. Нет подтверждённого готового Trusted Input/KeyMint final flow. Не присваивать приложению Veles недоказанные свойства доверенной поверхности либо hardware tokens.

## Особые cases

- Беспарольный профиль может существовать; вход в него не является strict auth и не создаёт hardware key authorization.
- Для enrollment нужен explicit grant на управление факторами, а не сам факт активной session.
- Для Nord4 FOD сенсор использует touch/display illumination; подготовка interaction lease и его отзыв — необходимая часть flow. Координаты и геометрия относятся к профилю устройства.
- При потере сессии/поверхности grant отзывается, но уже начатая hardware mutation требует journal reconciliation.
- `veles-auth` не должен получать raw templates, sensor images или команды stock TA.

## Рекомендация интеграции

Отдельный `veles-carosign-adapter` в репозитории Veles, который переводит grants/identity/device leases/UI/Evidence в независимые контракты и не требует запуска Linux `carosignd`/PAM/fprintd. Начать со simulated provider и только затем включать реальную Nord4 TA под подтверждёнными аппаратными ограничениями.
