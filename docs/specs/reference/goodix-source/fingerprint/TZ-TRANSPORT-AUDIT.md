# TrustZone transport: verification after the hang

⚠️ The analysis specification (stock stack and C version) lives in
[`README.md`](README.md) and its sections; this file is the **measurement log**.

## Direction: no Android userspace

The target client runs on Linux and talks to Goodix and Keymaster inside the TZ
without `libgf_hal.so`, bionic or Android services. The TZ firmware and its
signed apps remain a hardware part of the device. Running the Android HAL is a
diagnostic yardstick for comparison only.

✅ Binary `linux-rom/fingerprint/hal-harness/staging/libgf_hal.so`:
`gf_hal_common_authenticate` @`0x1d42c–0x1d434` sets the length to 120 and the
initial value to −1; @`0x1d54c–0x1d56c` writes the group at `+96`, the challenge
at `+104`, and −1 at `+112`. Preparation of this body was moved into safe Rust;
sending it from Rust on the device has not happened yet.

✅ Binary `goodixfp.flat` @`0x1880` places the command result in reply `+0` and
channel errors in `+4` (for example @`0x1358`). `libgf_ca.so` @`0x2e88` reads
the channel status, then @`0x3074–0x3080` returns the command result. The early
`spinel_tz_smoke` read only `+0`, so it never confirmed a zero channel status on
its own. Later runs through the stock HAL checked both fields.

✅ Measurement: a native Rust program opened `/dev/qseecom`, allocated and
mapped a DMA buffer through `/dev/dma_heap/system`, then freed it. No TZ
command was sent. A separate UAPI call bound the transport to the already
loaded Goodix; no command was sent to the app. The listener service stayed
active and the boot ID did not change.

⚠️ The current `spinel_qseecom.c` source, with `fix_dlina=327592`, substitutes
the declared length for Goodix but sizes the TZ buffer by the DMA object. The
parameter value itself is confirmed by reading sysfs on the tablet. It is not
proven what accesses the TA can actually make beyond the original body; for the
direct client the allocation size and the declared length must agree before the
first send.

A fix for the adapter is prepared: the allocation is at least the declared
length and the added part is zeroed; the Goodix body address is written into the
low 32 bits only, and an address above 4 GiB is rejected. The address width is
confirmed by binary `goodixfp.flat` @`0x12d8`, with neighbouring flags at
@`0x12d0` and @`0x12ec`. The module is built for `7.1.5-spinel` but not
installed and not verified on the tablet; the live module must not be changed
without verification.

A Rust adapter with `DMA_BUF_IOCTL_SYNC` and a 512 KiB buffer is prepared, along
with a session-start machine: power preparation → detect → INIT. A failure at
any step aborts the rest. On the device we verified buffer allocation,
synchronised write/read of test data, and binding to Goodix without sending a
command. The send itself and the later steps are unverified. The adapter
refuses before waking or exchanging if the module does not confirm the fix via
the `native_guard` parameter; the older module version lacks it. The built
module and the Rust binary live in the protected directory
`/root/spinel-fingerprint-rust-stage`; they do not replace the active
components.

✅ Measurement: after a reboot the new module with `native_guard=Y` was loaded
and the direct Rust client ran `DETECT_SENSOR` and `INIT`; both reply statuses
were 0. A repeat with the Rust listener served real FS and GPFS requests, and
the installed C service was restored. The `finger_0_0.so` template matched its
backup byte for byte. Boot ID `2616b5d3-51ff-4d2a-9e2d-5cd2cccf9837` held. The
module was loaded by hand and will not reappear after the next reboot. A zero
body for `INIT` does not prove a full initialisation.

✅ Measurement: the next run, `DOWNLOAD_CFG` (`1004`), returned application
error 1070 on a working channel and the same boot ID. ✅ Binary
`goodixfp.flat`, table @`0x33f260`, routes 1004 to a shared handler @`0x169a4`.
The command is not part of the normal native path. ✅ Binary `libgf_hal.so`
@`0x5da38–0x5db6c`: the Milan AN specific initialisation uses a 344-byte body
for 1001 and passes firmware and OTP information in it; that path is not ported
yet.

✅ Binary `libgf_hal.so`: `gf_hal_reinit` @`0x19294–0x19370` sends 544 zero
bytes to 1000, and @`0x19420–0x194e0` sends 344 zero bytes to 1001;
@`0x19420–0x19444` and @`0x19598–0x195a0` skip 1004 when the configuration
word (reply 1000 `+100`) is non-zero. ✅ Measurement: on the tablet that flag is
non-zero, so the stock branch skips 1004. `INIT_FINISHED` (1005, 100-byte body)
ran directly from Rust. The stock HAL opens an OTP cache file
`/data/media/0/gf_otp_info` @`0x20f28–0x20f68`; ✅ measurement: the file is
absent. The TZ file `gf_otp_info.so` under `/root/spinel-fp` is a different
object.

✅ Measurement: the detection reply reported series 46 and a need for the filter
model, but not for the false-touch model. ✅ Binary `libgf_hal.so`: table
@`0xa96f4` routes series 46 to @`0x63680–0x636d8`, where `0x238ed` 32-bit words
are copied from the symbol `DLWeightLightDubaiB16BIT` @`0x387cb4`, size
`0x8e3b4`. `hal_milan_init_model` @`0x63330–0x63474` prepares a `0xc3568` body,
data from `+96`, the word count at `+0xc3560`, kind 0 at `+0xc3564`, and sends
1111. The extracted weights have SHA-256
`4ce6bd015f35d6bd8c505666aed8b0a425ceee8bad881fbf626a371e5536e879`.
✅ Measurement: the Rust client loaded the model with 1111; both statuses were
0. A repeat with the Rust FS/GPFS listener also passed, after which the C
service was restored. Adapter memory was raised to 1 MiB. This is not yet a
check of IRQ or matching.

⚠️ The next step needs a separate port of the interrupt handler.
✅ Binary `libgf_hal.so` @`0x5cbac–0x5cc24` fills the IRQ block once, and
@`0x5ccec–0x5cd44` resends the same area while the step equals 4. The native
adapter clears the DMA buffer before a command, but writes the full body of the
previous reply back into it. Verifying the chain still needs a real sensor
event and a proven completion sequence.

In Rust we prepared carrying the IRQ reply over as the body of the next command
and the limited steps 4 → 5; on the stand-in transport we verified that nothing
is zeroed a second time. Command 1016 is available only at the matching step.
Group selection 1014, start 1010 and cancel 1009 were also verified on the
stand-in transport. ✅ Binary `libgf_hal.so` @`0x1de28–0x1df24`: group at `+96`
in a 100-byte body; Milan AN @`0x5de88–0x5df30`: cancel 1009 with a zero
100-byte body.

✅ Measurement: the diagnostic Rust client took the netlink number from
`/dev/goodix_fp` and registered with the kernel adapter; the log confirmed the
registration. Two passive 30-second windows produced no interrupt; before the
second one the user confirmed readiness to touch the sensor. The matching mode
was not running at the time, so this does not prove the interrupt delivery is
broken. No IRQ command was sent to the TZ during the observation.

✅ Measurement: after loading the model, the Rust client selected group 0 with
1014, started matching with 1010, received a netlink interrupt and cancelled
matching with 1009. Boot ID `2616b5d3-51ff-4d2a-9e2d-5cd2cccf9837` did not
change, the C listener is active, and the template SHA-256 matched the backup.
The interrupt may have been queued before matching started: this run does not
prove it was tied to the touch. Command 1016 was not sent and no finger was
matched; GNOME integration is off.

✅ Measurement of the next run: after clearing the queue there were no
interrupts for 30 seconds; the user confirmed no finger was placed, so the touch
check never happened. On the next arming, 32 netlink events arrived in about
1.5 seconds before any invitation to touch; observation was stopped and 1009
was issued. The meaning of those interrupts is undetermined: clearing them
blindly is unacceptable for real matching, and the stock stack handles an event
with 1016. After the cancel, IRQ was disabled via `GF_IOC_DISABLE_IRQ`; the
boot ID held and the listener is active. Rust now disables IRQ when the
subscription closes; that change is not yet verified on the tablet.

✅ Measurement: the native Rust client dropped one event before arming, called
1010, received a fresh netlink interrupt and handled it with 1016. The chain
finished with no channel or application error, and the operation field `+100`
was 1; then 1009 cancelled the session, and closing the subscription issued
`DISABLE_IRQ`. Boot ID `2616b5d3-51ff-4d2a-9e2d-5cd2cccf9837` did not change,
the listener is active, and the template hash matches the backup. This is one
interrupt handled, not a completed match: the match result has not been
extracted. It ran without any UI attached. The two earlier windows without a
touch do not test the sensor.

✅ Binary `libgf_hal.so` @`0x5ce54–0x5ce78`: `+96` is the IRQ mask, `+100` is
the operation; the earlier log label "event type" for `+100` was a mistake.
@`0x5f8a0–0x5f95c`: branches by mask bits 1, 7 and 2. On bit 7, the handler
@`0x5fad4–0x5fcfc` distinguishes the command result: zero with operation 1/2
leads to @`0x603bc`, where @`0x60420–0x60458` sends the success notice; status
1006 leads to the no-match handler @`0x5fee0–0x5fef0`. After success the stock
stack also calls command 1086 (@`0x604cc`, @`0x60868–0x608f4`); its name in the
binary's symbol list is `GF_CMD_AUTHENTICATE_FINISH`
([PERECHNI.txt](/home/alex/projects/TB371FC-linux/specs/stock/fingerprint/PERECHNI.txt));
its role and necessity are still under analysis. The native mask/status parsing
and the repeated edge wait were prepared and verified on the stand-in transport;
the full sequence on the device is not finished.

✅ Measurement: in the next run a registered finger produced one event — mask
`0x2`, operation `1`, command result zero. Within a 60-second session neither a
next event nor a match result arrived; the session was cancelled and IRQ
disabled. Boot ID unchanged, listener active, template matching the backup. The
program's "no interrupt received" line on timeout wrongly ignored the event
already handled; it was fixed. ✅ Binary @`0x5f978–0x5faac`: the bit-1 branch
sends notices; the jump table @`0xa8a18` routes operation 1 to @`0x5fab0`,
bypassing timer creation @`0x5fa5c`. So the absence of that timer does not
explain this measurement.

✅ Measurement: a repeat with a registered finger gave one IRQ with mask `0x2`,
operation 1 and step 2; no further IRQ and no final decision appeared in the
window. ✅ Binary `libgf_hal.so` @`0x16628–0x1665c`: on a full open, after
power the stock stack reads `GET_FW_INFO`, then performs `RESET`, and only then
detects the sensor. `gf_hal_reset_chip` @`0x37750–0x37760` calls ioctl `0x6702`.
That step was missing from the native preparation; it was added together with
reading one version byte. ✅ Measurement: after the fix the Rust client detected
the sensor successfully; its effect on matching is still being checked.

✅ Binary: `gf_hal_open` uses the Milan AN callback @`0x5da10`; after
`INIT_FINISHED` @`0x5dd10` it calls @`0x60ae0`. In the latter,
@`0x60b80–0x60b8c` checks the configuration word `+208`, and when it is
non-zero @`0x60bb8–0x60bf4` masks IRQ and sends `1016` with a zero `0x4ffa8`
body and `+0x4ff9c = 2`. The configuration comes from the DETECT_SENSOR reply
starting at `+96`; ✅ measurement: the word on the tablet is 1. This separate
initial call is not handling a sensor event. The Rust client now performs it
after 1005 and before the model, with IRQ masked; ✅ measurement: the command
passed with zero statuses. In the next session IRQs with masks `0` and `2`
arrived, but the match result did not; this does not prove the step fixed the
problem.

✅ Binary @`0x172e0–0x17330` and @`0x5d7ec–0x5d8b0`: the Milan AN matching
handler is @`0x5e3dc`, which calls `gf_hal_common_authenticate` @`0x5e408`. This
confirms the chosen shared command 1010. `gf_hal_get_screen_state`
@`0x1922c–0x1927c` does not change the value passed in, so the initial `-1` in
the body matches the stock behaviour.

The diagnostic matching machine was moved into portable Rust
(`authentication.rs`): ready → wait for the first IRQ → handle each IRQ → wait
for the next, finish on a match or cancel → result. The first and subsequent
timeouts, the overall deadline and the event limit are set separately; errors
in waiting, handling, finishing and cancelling keep their step. The deadlines
bound the netlink wait but do not interrupt a hung synchronous TZ ioctl.
✅ Measurement: on the tablet a SIGTERM was sent during a live session and
`Complete { outcome: Interrupted }` was returned; the log confirmed the cancel
and IRQ disable, and the boot ID and template hash did not change. This is not
a test of matching or of a hang inside the TZ. On the host 36 tests, Clippy and
the cross build passed.

✅ Measurement, 10 November 21:3x: after the continuation fix in the client
(`--native-handle-irq`) the initial `1016` triggered by `+208=1` passed, the
series-46 model loaded, the left-over event was cleared (`dropped before
arming: 1`) and arming succeeded. Before arming a service IRQ had entered the
queue: mask `0x0`, operation `Authentication`, result `Processed`, steps `[7]`.
Then a touch arrived: mask `0x2`, operation `1`, result `Processed`, steps
`[2]` — and no second event came in the remaining 60 s; the session ended with
`NextInterruptTimedOut { handled: 2 }`. Boot ID `2616b5d3-…` unchanged, the TZ
service active, the template hash unchanged.

⚠️ Our rule "all mask bits outside `0x4` are closed" deferred the continuation
until a new edge, while the stock stack does not. The fix: a reply with step
`4` (`Image`) or `5` (`PostImage`) is taken as a request to continue (next
`1016` immediately with the same body), and only a non-`4`/`5` step waits for an
edge. The matching state gained a step `AwaitingContinuation`;
`await_next_interrupt` now accepts `HandlingInterrupt` too. Tests fixed (33 + 4),
Clippy clean, aarch64 build passes. ⚠️ The rule looks at the step but **not at
the mask**: on `[7]` (mask `0x0`, operation `Authentication`) it also waited for
an edge — visible in the `handled` count. Next step: decide from the stock code
and a measurement what counts as a "continuation" for step `7`/`4` when the
mask lacks `0x4`/`0x80`.

⚠️ The measurement did not finish: the next step (the HAL path through the
bionic harness as a yardstick) needed a reboot, and the tablet stopped
answering (address `192.168.1.129` stopped pinging right after `reboot`).
Nothing was written: the shim module sits aside as `spinel_qseecom-new.ko`,
not loaded.

✅ Repeat on the next boot (address changed to `192.168.1.173`, boot ID
`546ee0ff-…`): modules brought up by `tools/spinel-fp-moduli.sh` (proba
**before** the shim: the shim needs its symbols), listener active. The direct
session on a touch again produced one `1016` (`[2]`, mask `0x2`) and
`NextInterruptTimedOut`. The stock yardstick on the same series — `auth_probe`
→ `auth_ok`, `match_score=35`, and three device reads in a row. So **what the
stock stack does after the first reply**: it **reads again**, it does not wait
for an edge. Analysis —
[stok/history/10-event-measurements.md](stok/history/10-event-measurements.md),
section "Stock yardstick".

The `AUTHENTICATE_FINISH` 1086 body was ported from binary @`0x60868–0x608f4`:
operation, mask and two identifier fields are copied from the IRQ reply; the
call is allowed only after the successful finishing branch. The layout and the
no-repeat rule were verified on the stand-in transport; on the tablet this
command has not been run from Rust yet.

⚠️ During a sysrq reboot two freshly copied module files on Btrfs turned out
empty and one was damaged. The modules were restored from verified host copies,
hashes checked, `sync` run. Before the next sysrq reboot, written files must be
synchronised and re-checked.

⚠️ In the historical `specs/stock/fingerprint/RAZBOR.md` the Gatekeeper
description with a flat `SizedBuffer` belongs to another path and does not
prove the format of the last successful call. ✅ Binary
`android.hardware.gatekeeper@1.0-impl-qti.so`: `GatekeeperHalDevice::enroll`
@`0x4af4–0x4b30` picks the legacy path only under branch conditions; @`0x4c28`
sets command `0x1001`, @`0x4ca8` calls `serializeClientEnroll`, and @`0x4cc0` —
`KeymasterUtils::sendCmd`. `serializeClientEnroll` @`0x5294–0x5324` builds CBOR
through QCBOR. So the direct client must reproduce exactly the working branch
that is selected and its wrapper, not assume the old flat format. The binary
contract of the reply and the initialisation route still need cross-checking; do
not send arbitrary Keymaster commands on the device.

✅ Measurement of this built library's serialiser on the tablet with made-up
data, without calling the TZ: the first `enroll` builds a CBOR map with keys
`3,8`, a password change — `3,5,7,8`, `verify` — `3,4,6,9`. All three
byte sequences match the tests in `src/gatekeeper.rs`. Request bodies in Rust
are cleared on release. This is serialisation only: the shared header, reply
parsing, sending to Keymaster and binding to a Linux account are not
implemented yet.

## Porting the listeners and HAL checks to Rust, 10 November

✅ Measurement: a Rust program found W-LUN `0:0:0:49476` and read the geometry:
32 768 sectors, 64 reliable-write frames. A direct RPMB counter read through
its SCSI adapter returned response `0x0200`, result 0; nothing was written and
the reply MAC was not checked. This is **not** a test of an RPMB request from
the TZ through the listener.

✅ Measurement: with the service temporarily replaced by Rust, the stock
`gf_hal_open` returned 0 and file-listener and GPFS requests were served. In a
separate session with the same listener the user placed a registered finger:
after two `auth_fail` results an `auth_ok` came; the service handled GPFS reads
and writes. Then a diagnostic Rust HAL call built against bionic, together with
the Rust listener, again got the success notice on a registered-finger touch.
These checks ran in group 0; they do not establish binding to a Linux user and
do not estimate the false-accept rate. The same boot ID
`c660d02d-9eea-44b3-ac70-9fada92554e7` held. The checks ran with a timer that
restores the installed C service; it was restored. A copy of the TZ file
directory is kept on the tablet at `/root/spinel-fp-before-rust-listener` with
mode `0700`.

After that the Rust probe gained verifiable session steps; its new build passed
HAL open, but a full match with this latest change has not been repeated. Do not
enable PAM/GNOME before a separate check of client rights, account binding and
safe failure handling.

## What is established

* ✅ Measurement from earlier work, log `SLUSHATEL-PLAN.md`, 2–13 September:
  the Goodix INIT was served by file listener 10 through `INCOMPLETE`, and the
  stock HAL received an HMAC from the resident Keymaster. This is evidence for
  one specific sequence, not for any request to any app.
* ✅ Measurement, 10 November: searching for `keymaster64` and
  `qcom.tz.uefisecapp` and registering listeners 10 and 8192 completed; a
  direct `0x200` request to Keymaster hung the system. The previous boot's log
  did not retain enough to establish the hang phase. The cause is unknown.
* ✅ Binary of this tablet's stock kernel
  `source_artifacts/dumps/tb371fc-stock/boot_unpacked/kernel`: at `0x7e1f38`
  result 1 is checked, at `0x7e1f78` the ID is read, at `0x7e1f90` it is looked
  up in the registered-listener list. `0x7e216c` marks the found listener busy,
  `0x7e21b4` wakes the waiting service. The choice of reply command at
  `0x7e2240–0x7e2260` depends on support and on a table being present; the
  constant `0x1f` is not proof of an unconditional empty continuation.
* ✅ Measurement from earlier work: freeing a separate SHM bridge on repeated
  registration was accompanied by resets; a permanent pool helped in three
  runs. This does not prove that arbitrary re-registration is safe.
* ✅ File comparison: the restored `spinel_qsee_listener.c` and
  `spinel_qseecom.c` match the `fingerprint` branch. Matching sources does not
  prove that the live kernel and environment are byte-identical to those used
  for the September measurement.

## Open questions before testing on the tablet

1. The handler was moved outside the global SCM mutex. A separate lock is now
   held from the start of handling until the TZ replies; the `BLOCKED` thread
   waits on the same lock. This is a sequential implementation for our two
   listeners, not the full asynchronous stock QSEE. The build is verified; the
   interaction of several apps on the hardware is not.
2. ✅ Stock kernel `0x7e237c–0x7e2388`, on an unknown listener, sets status
   `0xffffffff`; `0x7e212c–0x7e2154` writes the ID and status into the reply
   structure. Our code used to send an empty continuation `0xfc` instead. The
   source is fixed: a reply with the ID and a FAILURE status is sent; an
   arbitrary Linux errno is mapped to SUCCESS or FAILURE. Not verified on the
   hardware.
3. ✅ Stock kernel `0x7df77c–0x7df7e4` checks whether the listener is busy,
   waits and re-checks; only then `0x7df81c–0x7df844` prepares the continuation
   request. A wait on the shared lock with a listener-ID check is prepared, but
   its behaviour on the tablet is not proven.
4. The daemon no longer maps the TZ buffer: it works on a separate copy. After a
   timeout the state irreversibly moves to `POISONED` until the module is
   unloaded or the device reboots; a late DONE does not complete a new request.
   The table of all allowed transitions is verified on the host. A userspace
   timeout does not interrupt an SMC already inside the TZ. Do not unload the
   module on a live system: reliable re-binding of buffers to the TZ is not
   proven.

## Check after the 10 November reboot

✅ Measurement: `spinel-tz-listener.service` is enabled and became `active` on
its own after a sysrq reboot. The boot ID after the boot is
`582ae65c-aa4a-4394-a33e-00243db69b81`; it held through later runs. The Goodix
components and the diagnostic HAL were started by hand.

✅ Measurement: the stock `gf_hal_open` returned 0, detected the sensor and got
a key from the real `keymaster64`: the Goodix app route = 5, Keymaster = 1.
Goodix requests went through the file listener. The HAL reports the fallback
file `/data/media/0/gf_otp_info` missing; the effect on enrolment is unchecked.

✅ Binary of the tablet's `librpmb.so`, `rpmb_ufs_init` @`0x746c–0x7478`: the
number of 512-byte sectors is the low word of the block count divided by two.
✅ Measurement: W-LUN `0:0:0:49476` reports `logical_block_size=0x08`,
`logical_block_count=0x10000`, `rpmb_rw_size=0x40`. The daemon now reads the
values from sysfs and reports 32 768 sectors and 64 frames. The old value 4096
was a mistake. The bounds check passed on the host.

✅ Measurement: a direct RPMB counter read on `/dev/sg1` finished with response
`0x0200`, result `0x0000`; no RPMB data was written and the reply MAC was not
checked. The TZ service and the tablet stayed active. This does not replace a
real RPMB request from the TZ through the listener.

Shim routing fixed: an unknown app or a missing Keymaster is no longer
redirected to Goodix. The name check ran on the host.

## Gatekeeper and the first enrolment, 10 November

✅ Measurement: the stock Gatekeeper `enroll` and `verify` for a separate random
UID returned 0 and a 69-byte token. No `SetRot` and no user deletion. The TZ
served two RPMB reads and two writes through the listener. A trial Gatekeeper
record was created; its one-time password was not kept. The old conclusion about
an inevitable `-30` refusal on this boot is refuted by measurement.

✅ Measurement: for enrolment, Goodix produced a challenge, Gatekeeper signed
the HAT, `gf_hal_enroll` accepted it and began capture. Two runs accepted 19 of
20 samples; on the last one the HAL returned
`GF_ERROR_WRITE_SECURE_OBJECT_FAILED` (1035). No template was enrolled. The first
fingerprint UID was 2737327482, the second 3057423019; the store shows only read
attempts for the corresponding `asp_template_*`, with no open-for-create in the
log.

✅ Binary `libdrmfs.so` @`0x6698–0x66b8`: the `open` error is kept in a separate
global field; @`0x482c–0x4834` returns it on request `0x21c`. Our daemon used to
return the current `errno`. The fix was verified by a test and on the tablet
(`ENOENT` → `GET_ERRNO` = 2); enrolment still ended with error 1035, so this is
not its established cause.

✅ Binary `goodixfp.flat`: storage mode 2 @`0x337d3c`; opening in write mode
forms `0x242` @`0x4244–0x4258`, then FTS is selected @`0x8dcc–0x8eb0`. On the
last attempt no OPEN with flag `0x242` for templates, and no request to an
unknown listener, was recorded. The next task is to analyse the FTS write
handling and its errors before another finger run.

## GPFS, enrolment and matching, 10 November

✅ Binary `cmnlib64.b02` @`0x18afc–0x18b10`: FTS calls service `0x7000`.
✅ Binary `libdrmfs.so` @`0x5a7c`: the GPFS buffer is `0x7e000`. The first port
of the Nord4 handler understood only commands 4–11 and rejected the tablet's
real request with command 1. ✅ Binary `libdrmfs.so` @`0x5c3c–0x5c50`,
`0x5cd8–0x5cf0`: the stock stack also handles commands 0–3 with full paths.
After adding that variant, real GPFS writes succeeded; the files are visible in
`/root/spinel-fp/data/vendor_de/0/fpdata`.

✅ Measurement: the HAL reported `samples_remaining=0`, group 0, finger ID
3299986489; `finger_0_0.so` and a backup were written, 88 810 bytes each. The
input HAT came from Gatekeeper for a random trial UID; its password was not
kept. A later check of the same finger returned `auth_ok`, match score 47.
After a reboot the template was read and the same finger matched again.

✅ Measurement: the user confirmed a different finger was used; the HAL
repeatedly returned `auth_fail`, score 0. In a new session the registered finger
first gave several refusals, then `auth_ok`, score 28. The user confirmed the
registered finger. Boot ID `c660d02d-9eea-44b3-ac70-9fada92554e7` held; the TZ
service is active. This is a functional check of two fingers, not a statistical
false-accept estimate and not a system-login test.

GPFS and the TZ daemon are enabled at boot; the sensor modules and the stock HAL
were loaded by hand at the time of this measurement. GNOME/PAM login is not
connected.

5. The RPMB handler was verified with modelled requests on the host; the real
   TZ ↔ RPMB operation through the listener, and recovery after errors, are not
   confirmed. A direct counter read was verified separately (above).
6. The `spinel_qseecom` shim modifies the command field at offset 36 when a
   modfd is present (`fix_dlina`). This is now limited to the Goodix ID. The
   value remains part of the earlier verified workaround, not of the general
   QSEE protocol.

## Prepared fixes

On the host the shim now refuses before calling the TZ if the DMA buffer cannot
be mapped, if its size exceeds the limit, or if the physical address offset
falls outside the command. Copy-back errors are no longer masked. The daemon
does not pass an RPMB request to the file handler when the RPMB buffer is
absent. Command, reply and RPMB frame contents are no longer printed to the log.

## Check on the tablet, 10 November

✅ Measurement: the boot image
`out/rebuild-2026-11-09/tz-transport/spinel-boot-tz.img` was written to the
active `boot_b`; the bootloader does not support `fastboot boot`. The DTB
matches `spinel-boot-usb-5v-test.img`, kept for rollback, byte for byte. After
`7.1.5-spinel` booted, listeners 10 and 8192 were registered and the daemon
mapped its buffer copies. Initially the modules and the daemon were started by
hand.

✅ Measurement: `spinel_fp_proba` loaded the signed `goodixfp` as app 5;
`spinel_tz_smoke` sent DETECT_SENSOR (1000) and INIT (1001). Both returned SCM 0
and app status 0. INIT produced seven `INCOMPLETE` for FS 10; all seven requests
were served by the daemon, and the last continuation returned result 0. The
listener actually opened and wrote `fingerprint_version` in the file store.
Boot ID `7e12861e-e2e8-4715-8404-279269b67f46` held after the run.

This confirms one real Goodix ↔ TZ ↔ FS path, including buffer copying.
Parallel calls, `BLOCKED`, a missing listener and a TZ timeout were not
reproduced on the device; RPMB and Keymaster were not checked at this stage. Do
not unload or reboot the live test modules: the bridges are deliberately held
until reboot.

Do not perform a direct `GET_VERSION`, arbitrary TA commands, or bootloader
updates to test these assumptions. First restore the documented sequence and
check its bounds and error handling off-device.
