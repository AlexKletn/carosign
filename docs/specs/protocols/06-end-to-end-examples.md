# P6. Сквозные сценарии взаимодействия

Диаграммы Mermaid намеренно простые, без HTML и нестандартных токенов.

## A. Запуск стороннего провайдера

```mermaid
sequenceDiagram
    participant PM as ProviderManager
    participant PS as Supervisor
    participant W as Worker
    participant B as ResourceBroker
    PM->>PM: Resolve package and device profile
    PM->>PS: Spawn worker with private IPC
    PS-->>PM: Worker process handle
    W->>PM: WorkerHello
    PM->>W: Initialize
    W-->>PM: Initialized
    PM->>B: Acquire resource set
    B-->>PM: Lease granted
    PM->>W: BindDevice
    W-->>PM: DeviceReady
    PM->>PM: Publish authenticator
```

## B. Linux PAM verification

```mermaid
sequenceDiagram
    participant C as PAMClient
    participant R as Runtime
    participant A as PolicyAuthority
    participant W as ProviderWorker
    participant V as EvidenceVerifier
    C->>R: BeginVerification
    R->>A: Authorize request
    A-->>R: Scoped grant
    R->>W: StartVerification
    W-->>R: PlaceFinger
    R-->>C: Progress
    W-->>R: Matched plus evidence
    R->>A: Revalidate context
    A-->>R: Valid
    R->>V: Evaluate evidence
    V-->>R: Accepted assessment
    R-->>C: Accepted factor for this request
```

Этот сценарий не обещает hardware evidence GXFP: Policy может принять software factor при соответствующей конфигурации.

## C. Enrollment и durable commit

```mermaid
sequenceDiagram
    participant A as Authority
    participant R as Runtime
    participant J as Journal
    participant W as Provider
    participant G as Registry
    A->>R: Authorized enrollment
    R->>J: Persist intent
    J-->>R: Durable
    R->>W: StartEnrollment
    W-->>R: Progress events
    R->>W: Commit enrollment
    W-->>R: Outcome or unknown
    R->>W: Reconcile if required
    W-->>R: Confirmed state
    R->>G: Commit enrollment binding
    G-->>R: Durable
    R->>J: Mark completed
    J-->>R: Durable
    R-->>A: Enrollment completed
```

При потере worker до подтверждения outcome применяется `NeedsReconciliation`, а не повтор `Commit enrollment`.

## D. Сбой worker при мутирующей операции

```mermaid
sequenceDiagram
    participant R as Runtime
    participant J as Journal
    participant W as OldWorker
    participant S as Supervisor
    participant N as NewWorker
    R->>J: Persist mutation intent
    J-->>R: Durable
    R->>W: Mutating command
    W-->>R: IPC connection lost
    S->>S: Fence old worker and resources
    R->>J: Mark NeedsReconciliation
    S->>N: Start replacement
    N-->>S: Device bound after recovery
    R->>N: Query mutation state
    N-->>R: Verified or Unknown
    R->>J: Persist reconciliation result
```

Если `Unknown`, конфликтующая регистрация остаётся заблокированной. Живость нового worker не доказывает безопасного состояния TA.

## E. libfprint bridge

```mermaid
sequenceDiagram
    participant R as Runtime
    participant W as LibfprintWorker
    participant L as Libfprint
    R->>W: StartVerification
    W->>L: Verify async
    W-->>R: CommandAccepted
    L-->>W: Progress callback
    W-->>R: Progress
    L-->>W: Verify result
    W-->>R: VerificationOutcome
```

`GCancellable` может запросить отмену, но quiesce требует подтверждённого завершения операции.
