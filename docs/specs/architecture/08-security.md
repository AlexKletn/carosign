# 08. Модель угроз и границы доверия

## Цель

Не допустить, чтобы сбой/компрометация провайдера, клиента или UI автоматически давали право на системную авторизацию, доступ к чужим биометрическим регистрациям, аппаратным ключам либо управление устройствами за рамками выделенных полномочий.

## Участники и доверие

| Участник | Потенциальные атаки | Граница |
|---|---|---|
| Unprivileged client | чужой SubjectRef, replay operation, flood | IPC peer identity + per-operation policy |
| Compromised user session | enrollment нового пальца, UI spoofing | отдельная re-auth для mutating grants |
| Malicious provider worker | fake Matched, DMA misuse, template exfiltration | sandbox + resource broker + evidence assessment |
| Malicious/buggy firmware | ложные TA results, stale token | проверка реально поддерживаемых proof и boot conditions |
| Root/local admin | чтение keyfile, подмена службы, манипуляции | security boundary в ordinary Linux ограничена; нельзя обещать несуществующий TEE trust |
| Offline disk attacker | запись/откат базы, кража keyfile | AEAD + TPM/secure store только при подтверждённом key binding |

## Permissions

- Установку пакета и resource policies контролирует привилегированная система.
- Процессное имя или текст из manifest не удостоверяет provider.
- Worker не имеет root by default, сети по умолчанию нет, /dev access минимален; C/C++/Python workers получают те же архитектурные ограничения, что Rust.
- Package provenance, device match, runtime function claims и hardware-assurance assessment — разные источники доверия.
- Diagnostics не показывают сырые кадры/шаблоны/секреты в продовом режиме.

## Linux sandbox

Предлагаемое: dedicated UID, mount/user/pid namespaces по необходимости, seccomp, cgroup CPU/memory/pids, capability drop, read-only package assets, writable state только через определённый broker/storage. Для hardware unrevocable FD требуется mediated RPC либо изоляция и device fencing.

## Enrollment policy

Разрешение `Enroll`/`Remove` не основано исключительно на UID. Необходимо подтверждение независимым фактором или политикой доверенного владельца. Grant привязывается к subject+operation+device+purpose+generation; revocation наблюдается и проверяется перед mutation dispatch.

## Keyring integration

Существующий GXFP secret release после match — полезный исторический сценарий, но новая Core не должна выдавать пароль приложения как побочный эффект `Verify`. Key operations и secret release — отдельные полномочия и потребители. Фальсификация software match не должна автоматически разблокировать hardware-bound ключ.

## Assurance properties

Хранить отдельно: template isolation, matching isolation, secure sensor path, proof authenticity, anti-replay, key operation binding, rate limiting, rollback resistance, boot integrity, attestation, revocation. `Absent/Unknown/Verified/ConditionallyVerified` с областью и источником; неизвестность не равна «да».

## Threat-to-test map

- Forged `SubjectRef` → unauthorized.
- Replayed `OperationId`/token → rejected.
- Revoked grant while match runs → result suppressed.
- Crash after mutating command → NeedsReconciliation.
- Misbehaving worker flooding progress → bounded queue/backpressure.
- Truncated `SCM_RIGHTS` message → close all partial FD and fail.
- Old template rebound to new sensor → rejected until compatibility proof.
