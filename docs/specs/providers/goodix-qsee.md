# Goodix QSEE / Lenovo TB371FC — trusted matching backend

## Источники

Актуальные файлы старого проекта: `reference/goodix-source/fingerprint/stok/*`, `reference/goodix-source/fingerprint/tablicy/*`, `reference/goodix-source/fingerprint/TZ-TRANSPORT-AUDIT.md`. C-реализация была быстрым почти рабочим прототипом; целевая Rust-реализация должна стать самостоятельным продуктовым provider, **не** построчным портом C.

## Ответственность provider

- Управление Goodix sensor/IRQ/power/reset и зависимостями стоковых TA.
- Goodix HAL-equivalent orchestration (команды порядка `1000…1111`) и обработка двух status fields.
- Открытие доверенного приложения через совместимый QSEE adapter.
- Coordination со службой слушателей FS/GPFS/RPMB.
- Отправка нормализованных `Enroll`, `Verify`, `Remove`, `Enumerate`, `Cancel`, `Reconcile` в provider wire.

Core **не знает** номера команд, памяти DMA, структуры IRQ body и конкретного netlink number. Для QSEE низкоуровневый доступ ограничивается platform infrastructure.

## Важные аппаратные ограничения

Источники описывают `spinel_fp_proba` как владельца загрузки TA/power/IRQ, `spinel_qseecom` как ioctl/SCM shim, `spinel-lstnrd` как обработчик listeners. Контроль доступа не должен предполагать, что выдача raw `/dev/qseecom` — тонкая, отзывная capability. Listener может оставаться активным между enroll/verify, иметь собственное состояние, ошибку `POISONED` и зависимость от тайминга.

## Состояние разработки

Стоковый HAL успешно использовался как эталон matching, есть подтверждённые измерения QSEE, Goodix protocol и Rust listener/частичного direct client. В актуальном `stok/05-open-questions.md` нерешён следующий IRQ в прямом пути. Нельзя объявлять самостоятельный нативный Rust backend готовым без полного end-to-end `enroll → persistent save → verify`.

## Mutation and storage

Шаблоны TA — opaque provider-owned state, Registry содержит `ProviderEnrollmentRef` и поколение. После `SAVE/REMOVE` возможно требуется независимое перечисление или проверка; при неясном outcome блокируется conflicting mutation до reconciliation. Не повторять команду SAVE вслепую.

## Proof

TEE-based matching не гарантирует автоматической пригодности hardware token для внешнего KeyMint. Отдельно проверить token provenance, challenge binding, secure sensor path, boot configuration и приемник proof. Пока не подтверждено — provider возвращает только заявленный результат и opaque material с `Unknown` assurance.

## План подключения

1. Утвердить `carosign.provider.v1` и resource leases.
2. Оформить отдельный Rust Goodix worker.
3. Не меняя пользовательский Registry, доказать init/IRQ/matching.
4. Реализовать enroll/remove/recovery и protected storage listeners.
5. Подключить verifier/assessment только после аппаратных исследований.
