# 32. v0.6.5 — B-11/B-12/B-13: bulk command body from memory to module

**Status:** normative generic broker/memory boundary; **NOT** an invented Goodix/QSEE ioctl specification. This covers the full path `provider-generated typed command bytes → broker IPC → trusted handler → Linux module/DMA → TA`, not fingerprint images/templates in Core.

## 1. Why a new broker wire feature

Provider-control `carosign.provider.v1` frames remain limited to 65536 bytes and **continue to allow SCM_RIGHTS only on BindAuthenticator**. A device provider can generate model/command bodies far exceeding this size: Goodix research includes the roughly `0xc3568`-byte model load and a historical `0xee6a8`-byte body. Those are **hardware research observations**, not generic broker opcodes. Never fragment giant bodies across `ProviderFrame` or smuggle them into `StartEnrollment`.

A separately connected `FD_ROLE_MEDIATED_SERVICE` `carosign.broker.v1` channel negotiates minor **1**. `BrokerFrame.Call` and `.CallResult` now optionally carry `ExternalBody` plus **exactly one ancillary sealed memfd**; other broker frames carry zero FDs. Ordinary typed `Call.payload` remains bounded to 32768B, `CallResult.payload` to 32768B, entire protobuf frame to 65536B. The single bulk region is bounded 1..8388608B (8MiB); this is a policy ceiling, not a default accepted size for every handler.

The broker handler registry must expressly declare `accepts_external_body`, `max_body_length`, alignment, decoder and persistent-effect classification. Without these declarations, deny before opening TA/device handles. Negotiation and FD presence cannot grant a new operation or hardware privilege.

## 2. Memory ownership and wire validation

Sender allocates a private memfd for exactly the body, writes the complete bytes, ensures `F_SEAL_WRITE|F_SEAL_GROW|F_SEAL_SHRINK|F_SEAL_SEAL`, then transfers a **read-only FD** with exactly one `SCM_RIGHTS` entry. No remaining writable mapping or handle may permit modifying the file after sealing. Receiver checks `fstat` file type and exact `file_size`, seals, file position independence, `external_body.offset == 0`, positive length = file_size <= configured/handler max, SHA-256 32B matching actual bytes, no stale/foreign descriptor and no executable mapping. Duplicate/missing/extra FDs, writable/unsealed FD, oversized body, size/digest mismatch, file holes if relevant to handler or parse failure ⇒ reject/close FD without hardware effects. Header/metadata hashes do not replace the handler's typed body decoder.

Memfd lifetime: sender retains its FD until send is committed to the broker socket; receiver owns the transferred FD and retains it through validation and fully completed *synchronous* staging into trusted-owned kernel buffers. It closes on all faults. Never reuse a mutable producer mapping after the hash was checked. No worker-visible DMA/physical address, kernel pointer, QSEE handle or raw listener credential. Both successful and error replies must release the transferred FD. `CallResult.external_body` uses the same rules with producer/consumer roles swapped; a result FD is never a new permission to read arbitrary sensor state.

`ExternalBody` is **not** a generic pass-through TA payload: a per-profile audited broker handler authorizes exact command type/shape and its grammar. Handler must bind the body to `operation_id`, `mutation_id`, `command_id` where applicable. Read-only handlers may omit mutation/command IDs only under the verified policy described in 26. Persistent mutating handlers must independently check durable `DispatchArmed` and exact worker/lease/epoch before first possible hardware side effect. A multi-step persistent operation remains encapsulated in a single audited handler/armed mutation; the presence of an FD does not permit multiple arbitrary mutating calls.

## 3. Trusted module / QSEE hardware memory chain

For a hardware handler that needs a DMA-visible command buffer, its **privileged adapter** must:

1. Validate the operation's vetted typed schema, exact body length and any fixed offsets; do not accept worker-specified arbitrary opcode or native pointer.
2. Allocate an approved kernel/DMA buffer **at least as large as the hardware-declared accessible length** plus documented framing/reply space, with checked integer arithmetic, alignment and upper bounds. Do not advertise a length larger than allocated or initialized accessible memory. Copy only validated bytes, initialize all padding to zero and prevent reading out-of-bounds by TA/module.
3. Check architecture-specific address-width requirements (research suggests a **32-bit address field** in the specific Goodix module). Reject an unrepresentable address rather than truncating. Kernel physical addresses and mappings never cross generic broker wire.
4. Perform documented cache coherency / `DMA_BUF_IOCTL_SYNC` or equivalent begin/end transitions for both sending and receiving, with ownership passed only after sync. Respect full kernel `ioctl` return status, transport status and TA/application status independently.
5. Keep the kernel mapping and DMA allocation valid until the module/TA **actually finishes** with it; timeout, cancellation or worker death are not proof of completion. If completion is uncertain, isolate/quarantine the associated device and buffer and require hardware-specific recovery before reuse or release; do not create an unsafe dangling DMA pointer.
6. On verified completion, validate output bounds and status, copy only typed permitted response bytes to a new broker-owned sealed memfd if large, zero sensitive staging memory and release mappings/FDs on a defined owner thread/context.

The research mentions a `native_guard` kernel-module parameter and particular DMA sizes; **whether the deployed module actually provides this guard and safe API must be established by versioned device evidence**. This document does not authorize installing or patching a live kernel module, and does not promote the old C prototype to normative code. If the address width, kernel UAPI, buffer completion, DMA sync or safe memory retention cannot be proven: `ARCHITECTURE_BLOCKER`, no experimental hardware dispatch as production.

## 4. Error, cancellation and replay

- **Before side-effect boundary:** invalid FD/body/policy fails before hardware; for non-armed read-only calls a typed error is terminal. For an already-armed *mutation*, a locally rejected `Call` is not by itself an independent hardware no-effect proof of a previous or concurrent attempt.
- **During/after side-effect boundary:** ambiguous ioctl/TA completion, `EINTR`, timeout, worker/broker death, truncated response or failed sync ⇒ `ResourceUncertain` and, for armed persistent mutation, `NeedsReconciliation`. Do not resubmit an unknown command, even on a new memfd.
- Cancelling the provider operation does not cancel an in-flight hardware handler. Broker must maintain separate lifecycle/quiescence state, prevent use-after-free, and refuse conflicting commands until hardware safety is independently established.
- A checksum establishes the copied byte identity; it **does not** establish TA acceptance, template durability, trusted hardware evidence or device quiescence.
- Never use a human-facing timeout or process exit as permission to `munmap`/free DMA while firmware may still retain the address. The memory owner must be the trusted driver/broker, not the request actor.

## 5. Scope and limits

Only the mediated broker channel gets this extension. No general `RawQseeCall`, `GenericIoctl`, raw capture stream or unrestricted `memfd` transport is added to Client, Core or Engine. Large body is a typed hardware-command argument, not a generic fingerprint matcher input. Broker's `Call.payload` remains structured arguments of a fixed trusted handler. The optional platform template vault is a separate service with separate authorization/staging and cannot be implemented merely by treating `ExternalBody` as arbitrary template storage.

## 6. Mandatory conformance

L-01 correct sealed memfd + typed handler accepts exactly N bytes; L-02 missing/extra FD or metadata; L-03 writable/unsealed memfd; L-04 digest/size mismatch; L-05 >8MiB or >handler size; L-06 mismatched mutation/command/epoch or no durable arm; L-07 large result FD with read-only seals; L-08 simulated DMA32 overflow rejected without truncation; L-09 declared accessible length > allocated extent rejected; L-10 abort during in-flight DMA retains isolation and no UAF; L-11 broker crash after armed dispatch => no replay; L-12 no raw body permitted over provider-control IPC; L-13 independent TEE statuses separately validated; L-14 memory and FD counts bounded under concurrent calls.
