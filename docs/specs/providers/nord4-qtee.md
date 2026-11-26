# Nord 4 / Qualcomm QTEE — будущий backend

## Статус

Исходный Veles-документ — план исследования и частично проверенные экспериментальные элементы, не завершённый product-ready provider. В Linux/Nord4 опыте подтверждены отдельные Gatekeeper/QTEE/stock TA операции и распознавание в исследовательской конфигурации; полная сквозная защищённая цепочка с KeyMint **не доказана**.

## Модель backend

`Nord4Provider` координирует stock fingerprint TA, FOD sensor, touch and localized display illumination. TEE bridge, RPMB/FS callbacks и ownership hardware ресурсов предоставляются через infrastructure; provider не получает права менять системную policy.

## FOD interaction

Подэкранная оптика требует временно арендуемой подсветки, синхронизации touch input, корректной геометрии/калибровки и отмены при потере UI/session grant. У Veles это адаптер Яви/дисплея/ввода; у обычного Linux — иной `InteractionController`, с отличающимся assurance. Ни один UI event не является доказательством Match.

## Profile and anchor

Сопоставление UID/профиля Veles, handle и enrollments должно иметь доверенного владельца, устойчивый journal и reconciliation; отсутствие anti-rollback не подменять software-only обещанием. Аппаратные токены передаются совместимому verifier/key consumer, если proof действительно подтверждён.

## Полезные исследования Veles

Повторная проверка grant перед изменением фактора, отделение cancellation от cleanup, `NeedsReconciliation` после uncertainty, раздельные auth и secret authorities и generation-bound scopes — архитектурные требования, применимые к независимой платформе.

## Разделение

Veles Auth policy остаётся во Veles. `Nord4Provider` реализует биометрический сеанс. Системная сцена принадлежит Яви; TEE/ресурсы — уполномоченным сервисам. Никакого прямого импорта `veles-auth` в Provider API.
