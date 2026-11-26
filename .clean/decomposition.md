# Аудит декомпозиции Rust

Срез рабочего дерева на 19 ноября 2026 года по структурным правилам AGENTS.md.

Синтаксически разобраны все 1951 Rust-файлов из инвентаризации рабочего дерева.

- Именованные структуры, перечисления, трейты и объединения учитываются, включая приватные и тестовые типы.
- После ручного прохода по группам свободных функций самостоятельные кодеки, обработчики и операции вынесены. Ниже приведён синтаксический перечень оставленных групп, в том числе пар encode/decode, обёрток над одним переходом, граничных проверок и вспомогательных функций.
- Реализации сопоставлены с однозначно именованным типом в том же крейте. Макросы не раскрывались. Сгенерированные результаты сборки исключены.
- Интеграционные сценарии сохраняют формат Cargo. Многотиповые тестовые фикстуры вынесены в отдельные файлы.

## Итог структурной проверки

- Исходники с несколькими именованными типами — 0.
- Тестовые файлы с несколькими вспомогательными именованными типами — 0.
- Объявления кода вместо деклараций модулей и реэкспортов в lib.rs и mod.rs — 0.
- Тестовые функции внутри производственного исходника — 0.

## Несколько Rust-типов в исходнике (0)

По проверяемому признаку файлов нет.


## Реализации отдельно от объявленного типа (0)

По проверяемому признаку файлов нет.


## Код в lib.rs или mod.rs (0)

По проверяемому признаку файлов нет.


## Тестовые функции внутри производственных исходников (0)

По проверяемому признаку файлов нет.


## Несколько вспомогательных типов в тестовом файле (0)

По проверяемому признаку файлов нет.


## Смежные функции и алиасы (170)

### crates/clients

- [crates/clients/carosign-fprintd-compat/src/dbus/server.rs](/home/alex/projects/carosign/crates/clients/carosign-fprintd-compat/src/dbus/server.rs:21) — serve_system (21), serve (28), serve_with_backend (33).
- [crates/clients/carosign-fprintd-compat/src/ownership/owner.rs](/home/alex/projects/carosign/crates/clients/carosign-fprintd-compat/src/ownership/owner.rs:4) — Owner (4), claim (15), release (27).
- [crates/clients/carosign-fprintd-compat/src/sender.rs](/home/alex/projects/carosign/crates/clients/carosign-fprintd-compat/src/sender.rs:8) — ObservedSender (8), owner (50), observe_sender (60).
- [crates/clients/carosign-fprintd-compat/src/system_backend/fingerprint_inventory.rs](/home/alex/projects/carosign/crates/clients/carosign-fprintd-compat/src/system_backend/fingerprint_inventory.rs:4) — authenticator (4), authenticator_for (8), listed_fingerprint (15), select_fingerprint (21).
- [crates/clients/carosignctl/src/socket/inventory.rs](/home/alex/projects/carosign/crates/clients/carosignctl/src/socket/inventory.rs:7) — subject (7), devices (16), enrollments (29), list_enrollments (54), list_enrollments_connected (59).
- [crates/clients/carosignctl/src/socket/verification.rs](/home/alex/projects/carosign/crates/clients/carosignctl/src/socket/verification.rs:10) — verify (10), verify_connected (20).
- [crates/clients/pam-carosign/src/client/observation.rs](/home/alex/projects/carosign/crates/clients/pam-carosign/src/client/observation.rs:10) — observe_system_verification (10), observe_system_verification_with_acceptance (22), observe_system_verification_with_control (35), observe_system_verification_for_modality (58), observe_system_verification_inner (70), observe_system_verification_inner_with_progress (89), observe_on_new_connection (153), observe_verification (166).
- [crates/clients/pam-carosign/src/client/selection.rs](/home/alex/projects/carosign/crates/clients/pam-carosign/src/client/selection.rs:3) — select_authenticator (3), select_for_account (17).
- [crates/clients/pam-carosign/src/pam_ffi/authentication.rs](/home/alex/projects/carosign/crates/clients/pam-carosign/src/pam_ffi/authentication.rs:5) — authenticate_user (5), retry_no_sample (15), retry_mismatch (21), authenticate_attempt (31).
- [crates/clients/pam-carosign/src/pam_ffi/entrypoints.rs](/home/alex/projects/carosign/crates/clients/pam-carosign/src/pam_ffi/entrypoints.rs:7) — authenticate_outcome (7), pam_sm_authenticate (15), pam_sm_setcred (29), authenticate (42).
### crates/core

- [crates/core/carosign-engine/src/decision/decide.rs](/home/alex/projects/carosign/crates/core/carosign-engine/src/decision/decide.rs:15) — decide (15), mutation_input_of (408), mutation_receipt_matches (541), receipt_to_mutation (573).
- [crates/core/carosign-engine/src/machines/device/device_state.rs](/home/alex/projects/carosign/crates/core/carosign-engine/src/machines/device/device_state.rs:4) — DeviceState (4), next_state (37), can_reserve (61).
- [crates/core/carosign-engine/src/machines/enrollment/mutation_step.rs](/home/alex/projects/carosign/crates/core/carosign-engine/src/machines/enrollment/mutation_step.rs:4) — MutationStep (4), phase_for (29), effects_for (45).
- [crates/core/carosign-engine/src/machines/enrollment/step.rs](/home/alex/projects/carosign/crates/core/carosign-engine/src/machines/enrollment/step.rs:10) — step (10), reconciliation_outcome (360).
- [crates/core/carosign-engine/src/machines/verification/dispatch_outcome.rs](/home/alex/projects/carosign/crates/core/carosign-engine/src/machines/verification/dispatch_outcome.rs:7) — dispatch_not_sent (7), dispatch_unknown (19).
- [crates/core/carosign-engine/src/machines/verification/step.rs](/home/alex/projects/carosign/crates/core/carosign-engine/src/machines/verification/step.rs:14) — step (14), unexpected (204).
- [crates/core/carosign-engine/src/machines/verification/tests_support.rs](/home/alex/projects/carosign/crates/core/carosign-engine/src/machines/verification/tests_support.rs:8) — context (8), initial (29).
- [crates/core/carosign-engine/src/machines/worker/worker_phase.rs](/home/alex/projects/carosign/crates/core/carosign-engine/src/machines/worker/worker_phase.rs:4) — WorkerPhase (4), next_phase (34), phase_timeout_s (50).
### crates/linux

- [crates/linux/carosign-broker/src/external_body/body.rs](/home/alex/projects/carosign/crates/linux/carosign-broker/src/external_body/body.rs:9) — ExternalBody (9), check_exclusive (78), check_fd_count (95).
- [crates/linux/carosign-broker/src/main.rs](/home/alex/projects/carosign/crates/linux/carosign-broker/src/main.rs:5) — id (5), main (16).
- [crates/linux/carosign-broker/src/owner_channel/owner_session.rs](/home/alex/projects/carosign/crates/linux/carosign-broker/src/owner_channel/owner_session.rs:25) — OwnerSession (25), invalid (105), check_channel (112), wait (132), prepare_once (150).
- [crates/linux/carosign-broker/src/process/configured_resource.rs](/home/alex/projects/carosign/crates/linux/carosign-broker/src/process/configured_resource.rs:6) — ConfiguredResource (6), validate_resource_parents (13), open_resource (49), validate_owned_resource (89).
- [crates/linux/carosign-broker/src/process/owner_revocation.rs](/home/alex/projects/carosign/crates/linux/carosign-broker/src/process/owner_revocation.rs:5) — revoke_owner (5), await_owner_revoke (23).
- [crates/linux/carosign-broker/src/process/packet_deadline.rs](/home/alex/projects/carosign/crates/linux/carosign-broker/src/process/packet_deadline.rs:5) — wait_packet (5), send_packet (19).
- [crates/linux/carosign-broker/src/process/request_validation.rs](/home/alex/projects/carosign/crates/linux/carosign-broker/src/process/request_validation.rs:8) — valid_id (8), checked_scope (12), valid_mutation (30).
- [crates/linux/carosign-broker/src/process/run.rs](/home/alex/projects/carosign/crates/linux/carosign-broker/src/process/run.rs:15) — run (15), run_with_resource (20), run_with_resources (30), run_with_resource_owner (40), run_with_control (53), queue_reply (130).
- [crates/linux/carosign-broker/src/recovery/after_crash.rs](/home/alex/projects/carosign/crates/linux/carosign-broker/src/recovery/after_crash.rs:5) — AfterCrash (5), after_crash (14), uncertain_leases (26).
- [crates/linux/carosign-broker/src/relay/hello_gate.rs](/home/alex/projects/carosign/crates/linux/carosign-broker/src/relay/hello_gate.rs:7) — id (7), strict_hello (13), HelloGate (43), request_id (101).
- [crates/linux/carosign-broker/src/resources/binding.rs](/home/alex/projects/carosign/crates/linux/carosign-broker/src/resources/binding.rs:6) — Binding (6), role_fits (23), validate (38).
- [crates/linux/carosign-broker/src/transport/packet_socket.rs](/home/alex/projects/carosign/crates/linux/carosign-broker/src/transport/packet_socket.rs:12) — io_error (12), PacketSocket (21), unnamed_unix (211).
- [crates/linux/carosign-linux-ipc/src/client/transport/client_transport.rs](/home/alex/projects/carosign/crates/linux/carosign-linux-ipc/src/client/transport/client_transport.rs:10) — Transport (10), require_peer_uid (423), check_event_envelope (445), queue_event (463), random_id (476), frame (483), send (493), receive (512), receive_packet (521), check_one_payload (543).
- [crates/linux/carosign-linux-ipc/src/client/verification_events/observation_loop.rs](/home/alex/projects/carosign/crates/linux/carosign-linux-ipc/src/client/verification_events/observation_loop.rs:12) — observe_verification_with_acceptance (12), observe_verification_with_progress (26).
- [crates/linux/carosign-linux-ipc/src/codec/client_request.rs](/home/alex/projects/carosign/crates/linux/carosign-linux-ipc/src/codec/client_request.rs:11) — ClientRequest (11), decode (121), check_payload_assignment (195), take_delimited (217), check_subject_selector (228).
- [crates/linux/carosign-linux-ipc/src/enrollment_authority/scope.rs](/home/alex/projects/carosign/crates/linux/carosign-linux-ipc/src/enrollment_authority/scope.rs:4) — canonical_scope (4), parse_scope (26).
- [crates/linux/carosign-linux-ipc/src/enrollment_authority/validation.rs](/home/alex/projects/carosign/crates/linux/carosign-linux-ipc/src/enrollment_authority/validation.rs:4) — valid_action (4), response_matches (41).
- [crates/linux/carosign-linux-ipc/src/frame.rs](/home/alex/projects/carosign/crates/linux/carosign-linux-ipc/src/frame.rs:17) — FrameError (17), check (53), check_descriptors (73).
- [crates/linux/carosign-linux-ipc/src/mapping/client/domain_request.rs](/home/alex/projects/carosign/crates/linux/carosign-linux-ipc/src/mapping/client/domain_request.rs:8) — DomainRequest (8), authenticator (92), key (96), operation (100), to_domain (104).
- [crates/linux/carosign-linux-ipc/src/provider/tunnel/result_validation.rs](/home/alex/projects/carosign/crates/linux/carosign-linux-ipc/src/provider/tunnel/result_validation.rs:7) — strict_command_result (7), strict_sent_result (11), strict_result (15).
- [crates/linux/carosign-linux/src/discovery/archive/inspect.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/discovery/archive/inspect.rs:23) — inspect_bytes (23), inspect (111).
- [crates/linux/carosign-linux/src/discovery/archive/inspect_and_stage_measured.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/discovery/archive/inspect_and_stage_measured.rs:16) — inspect_and_stage_measured (16), inspect_policy_and_stage_measured (30).
- [crates/linux/carosign-linux/src/discovery/archive/stage.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/discovery/archive/stage.rs:13) — directory_empty (13), same_staged_file (54), stage (61).
- [crates/linux/carosign-linux/src/discovery/installed/pin_candidate.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/discovery/installed/pin_candidate.rs:10) — pin_candidate (10), protected (82).
- [crates/linux/carosign-linux/src/discovery/launch/prepared_launch.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/discovery/launch/prepared_launch.rs:7) — PreparedLaunch (7), prepare (16), root_owned_protected (31), inspect_root_owned_path (45).
- [crates/linux/carosign-linux/src/discovery/policy_file/read.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/discovery/policy_file/read.rs:13) — protected (13), read (24).
- [crates/linux/carosign-linux/src/discovery/publication_owner/prepared_publication.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/discovery/publication_owner/prepared_publication.rs:30) — PreparedPublication (30), retained_profile_path (47), check_watch (191), prepare_publication (198).
- [crates/linux/carosign-linux/src/discovery/publication_owner/provenance_snapshot.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/discovery/publication_owner/provenance_snapshot.rs:11) — ReopenedProvenance (11), reopen_provenance (18), reopen_provenance_with (32).
- [crates/linux/carosign-linux/src/discovery/publication_owner/recovery.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/discovery/publication_owner/recovery.rs:13) — ReverifiedContents (13), reverify_contents (27), check_recovery_roles (85).
- [crates/linux/carosign-linux/src/discovery/publication_owner/stage_policy.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/discovery/publication_owner/stage_policy.rs:5) — stage_policy (5), policy_matches_snapshot (58).
- [crates/linux/carosign-linux/src/discovery/signature_policy/rules.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/discovery/signature_policy/rules.rs:4) — digest (4), runtime_path (16), Rules (29).
- [crates/linux/carosign-linux/src/discovery/signature_status/check.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/discovery/signature_status/check.rs:3) — fingerprint (3), check (10).
- [crates/linux/carosign-linux/src/discovery/supervisor_policy.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/discovery/supervisor_policy.rs:6) — SupervisorPolicy (6), path (23), ident (35).
- [crates/linux/carosign-linux/src/discovery/verified_installation/checked_files.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/discovery/verified_installation/checked_files.rs:15) — CheckedFiles (15), verified_file (95), text (114), validate (126), system_validate (269).
- [crates/linux/carosign-linux/src/discovery/verified_installation/recovery_checked_files.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/discovery/verified_installation/recovery_checked_files.rs:11) — RecoveryCheckedFiles (11), check_recovery_files (55), check_recovery_files_with (64).
- [crates/linux/carosign-linux/src/discovery/verity/measure.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/discovery/verity/measure.rs:11) — measure (11), matches_approved_digest (31).
- [crates/linux/carosign-linux/src/ids.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/ids.rs:18) — KernelIds (18), nonzero_bytes (37), available (84).
- [crates/linux/carosign-linux/src/policy/protected_policy_snapshot.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/policy/protected_policy_snapshot.rs:10) — ProtectedPolicySnapshot (10), kernel_error (85), check_protected (89).
- [crates/linux/carosign-linux/src/process/cgroup/prepared_cgroup.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/process/cgroup/prepared_cgroup.rs:13) — PreparedCgroup (13), errno (20), duplicate (27), open_at (37), write_control (58).
- [crates/linux/carosign-linux/src/process/fd_cleanup/retain_only.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/process/fd_cleanup/retain_only.rs:5) — retain_only (5), close_range (54).
- [crates/linux/carosign-linux/src/process/helper/enter.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/process/helper/enter.rs:17) — enter (17), after_gate (43), after_gate_with (62).
- [crates/linux/carosign-linux/src/process/helper/enter_owned.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/process/helper/enter_owned.rs:19) — enter_owned (19), after_gate_owned (51).
- [crates/linux/carosign-linux/src/process/helper/fail.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/process/helper/fail.rs:5) — terminate (5), fail (11).
- [crates/linux/carosign-linux/src/process/privileges/drop_identity.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/process/privileges/drop_identity.rs:9) — failure (9), drop_identity (16).
- [crates/linux/carosign-linux/src/process/root_switch/enter.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/process/root_switch/enter.rs:9) — last (9), enter (16), enter_prepared (23), enter_inner (29).
- [crates/linux/carosign-linux/src/process/runtime_mount/attach_file.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/process/runtime_mount/attach_file.rs:8) — attach_file (8), attach_file_for_identity (21), attach_file_inner (40).
- [crates/linux/carosign-linux/src/process/runtime_mount/attach_tree_inner.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/process/runtime_mount/attach_tree_inner.rs:23) — error (23), attach_tree_for_identity (30), attach_tree_inner (46).
- [crates/linux/carosign-linux/src/process/seccomp/install.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/process/seccomp/install.rs:18) — instruction (18), install (22).
- [crates/linux/carosign-linux/src/process/tmpfs_root/create.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/process/tmpfs_root/create.rs:23) — error (23), create (30).
- [crates/linux/carosign-linux/src/sys.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/sys.rs:29) — errno (29), boottime_ns (39), timespec_ns (57), boot_id (74), parse_boot_id (85), open_read (120), open_launch_candidate (134), open_launch_component (143), open_launch_at (161), launch_metadata (187), reopen_pinned_regular (199), open_device (250), stat (269), file_identity (284), PidFd (298), socketpair_seqpacket (477), eventfd (501), memfd (512), seal_read_only (537), is_sealed (552), send_fd (574), send_fds (582), recv_fds (631), accept_connection (689), peer_credentials (712), checked_peer_credentials (735), listen_unix (751), random_16 (799), fill_random (822), random_available (842), close_all (847), into_file (852), c_str (858).
- [crates/linux/carosignd/src/accept/enrollment_lookup.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/accept/enrollment_lookup.rs:3) — read_candidates (3), read_enrollment_records (13).
- [crates/linux/carosignd/src/accept/request_scope.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/accept/request_scope.rs:3) — scope_of (3), operation_id (16), selector_is_current (23).
- [crates/linux/carosignd/src/accept/verification_admission.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/accept/verification_admission.rs:7) — prepare_client_verification (7), prepare_verification (43), prepare_authorized_verification (64).
- [crates/linux/carosignd/src/answer/mutation_replay.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/answer/mutation_replay.rs:3) — completed_mutation_snapshot (3), completed_mutation_event (36), completed_mutation_replay_frames (116).
- [crates/linux/carosignd/src/answer/verification_reply.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/answer/verification_reply.rs:3) — terminal_frame (3), verification_progress_frame (51), scope_of (112).
- [crates/linux/carosignd/src/bootstrap.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/bootstrap.rs:17) — recover (17), recover_guarded (27), recover_opened (59).
- [crates/linux/carosignd/src/broker_control.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/broker_control.rs:8) — BrokerControl (8), wait (28), revoke (50).
- [crates/linux/carosignd/src/config.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/config.rs:21) — absolute (21), socket_path (30), bounded (40).
- [crates/linux/carosignd/src/cycle/verification_cycle.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/cycle/verification_cycle.rs:3) — handle (3), verification (16), terminal_if_complete (82).
- [crates/linux/carosignd/src/install_quarantine/publication.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/install_quarantine/publication.rs:8) — prepare_and_commit (8), records (24), commit (79).
- [crates/linux/carosignd/src/installation.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/installation.rs:15) — trusted_store_path (15), quarantine_command (45), prepare (146).
- [crates/linux/carosignd/src/inventory.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/inventory.rs:9) — IncompleteInventory (9), from_linux_scan (13), convert (23).
- [crates/linux/carosignd/src/linux_dispatch/verification.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/linux_dispatch/verification.rs:17) — flow_failure_reason (17), verify (36), deliver_cancelled (317).
- [crates/linux/carosignd/src/linux_service/device_readiness.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/linux_service/device_readiness.rs:3) — device_readiness (3), probe_declaration (18), probe_capabilities (134).
- [crates/linux/carosignd/src/linux_service/serve.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/linux_service/serve.rs:7) — serve (7), serve_worker (74).
- [crates/linux/carosignd/src/main.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/main.rs:12) — StartupEvent (12), startup_diagnostic (38), read_config (50), main (85).
- [crates/linux/carosignd/src/mutation_delivery.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/mutation_delivery.rs:15) — match_record (15), terminal (54).
- [crates/linux/carosignd/src/mutation_execute/actor.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/mutation_execute/actor.rs:7) — actor_effect (7), actor_advance (35).
- [crates/linux/carosignd/src/mutation_execute/enrollment_challenge.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/mutation_execute/enrollment_challenge.rs:4) — enrollment_challenge_frame (4), enrollment_challenge_observation (37).
- [crates/linux/carosignd/src/mutation_execute/observed_mutation.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/mutation_execute/observed_mutation.rs:10) — ObservedMutation (10), finish_inner (18), finish_guarded (151), finish (173).
- [crates/linux/carosignd/src/mutation_execute/start.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/mutation_execute/start.rs:12) — start_inner (12), start (659).
- [crates/linux/carosignd/src/operations/operation_registry.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/operations/operation_registry.rs:19) — cancel_unsent_positive_reservation (19), same_verification_frame_context (28), OperationRegistry (43), mutation_binding_snapshot (3272).
- [crates/linux/carosignd/src/provider_supervisor.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/provider_supervisor.rs:30) — pair (30), inherit (48), restrict_worker (64), hex (95), ProviderSupervisor (104), exit_cleanup (124).
- [crates/linux/carosignd/src/provider_supervisor/profile.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/provider_supervisor/profile.rs:9) — parse_extra_resource (9), parse_id (39), protected_executable (70).
- [crates/linux/carosignd/src/run/effect_runner.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/run/effect_runner.rs:7) — run (7), run_inner (21), waits_for_provider (238), steps_to_send (246), next_effect (260).
- [crates/linux/carosignd/src/run/observed_runner.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/run/observed_runner.rs:7) — publish_observed_mutation_terminal (7), run_observed (53), run_observed_with_storage (78).
- [crates/linux/carosignd/src/serve.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/serve.rs:20) — actor_for (20), admit (30).
- [crates/linux/carosignd/src/server/connection_admission.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/server/connection_admission.rs:6) — accept_counted (6), fresh_connection_id (25), register_counted (38), accept_one (53), connection_from_peer (63), new_connection (77).
- [crates/linux/carosignd/src/server/frames.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/server/frames.rs:3) — check_frame_size (3), read_frame (10), write_frame (47).
- [crates/linux/carosignd/src/supervised_verification/tunnel_preparation.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/supervised_verification/tunnel_preparation.rs:6) — scope_is_live (6), prepare_tunnel (12), prepare_mutation_tunnel (31), prepare_tunnel_until (47).
- [crates/linux/carosignd/src/supervised_verification/verification_exchange.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/supervised_verification/verification_exchange.rs:7) — verify (7), exchange (36).
- [crates/linux/carosignd/src/talk/conversation.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/talk/conversation.rs:7) — converse (7), converse_with_health (31), converse_configured (55), converse_counted_with_dispatcher (85), serve_counted_once (117), serve_counted_once_with_dispatcher (149), converse_with_timeout (208).
- [crates/linux/carosignd/src/talk/frame_io.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/talk/frame_io.rs:3) — read_one (3), send_one (18).
- [crates/linux/carosignd/src/talk/response.rs](/home/alex/projects/carosign/crates/linux/carosignd/src/talk/response.rs:6) — unsupported (6), answer (26).
### crates/protocol

- [crates/protocol/carosign-protocol/src/resource_owner/prepare_validation.rs](/home/alex/projects/carosign/crates/protocol/carosign-protocol/src/resource_owner/prepare_validation.rs:5) — validate_prepare (5), validate_response (50).
- [crates/protocol/carosign-protocol/src/resource_owner/quiescence.rs](/home/alex/projects/carosign/crates/protocol/carosign-protocol/src/resource_owner/quiescence.rs:8) — Quiescence (8), id (22), validate_quiescence_request (26), validate_quiescence_observation (42).
- [crates/protocol/carosign-protocol/src/validation/broker.rs](/home/alex/projects/carosign/crates/protocol/carosign-protocol/src/validation/broker.rs:7) — check_broker_frame (7), check_broker_error (67), check_external_body_metadata (88).
- [crates/protocol/carosign-protocol/src/validation/client.rs](/home/alex/projects/carosign/crates/protocol/carosign-protocol/src/validation/client.rs:7) — check_client_frame (7), check_client_payload (19), check_required_subject (107), check_enrollment_ref (113), check_subject_selector (124).
- [crates/protocol/carosign-protocol/src/validation/field.rs](/home/alex/projects/carosign/crates/protocol/carosign-protocol/src/validation/field.rs:5) — check_frame_size (5), require_id (15), optional_id (23), bounded (31), bounded_str (43), specified (48), known_enum (57), check_frame_sequence (73), check_version (84).
- [crates/protocol/carosign-protocol/src/validation/provider/envelope.rs](/home/alex/projects/carosign/crates/protocol/carosign-protocol/src/validation/provider/envelope.rs:6) — check_provider_envelope (6), check_enrollment_transaction_ids (212).
- [crates/protocol/carosign-provider-sdk/src/command/decoded_command.rs](/home/alex/projects/carosign/crates/protocol/carosign-provider-sdk/src/command/decoded_command.rs:10) — Command (10), authenticator (169), subject (183), enrollment (192), binding_kind (208), parse (241), operation (551), mutation (556), mutation_version (561), bounded_str (572).
- [crates/protocol/carosign-provider-sdk/src/events/event.rs](/home/alex/projects/carosign/crates/protocol/carosign-provider-sdk/src/events/event.rs:11) — Event (11), check_error (143), check_enrollment_reference (163), check_query_result (174), check_mutation_outcome (195), parse (223).
- [crates/protocol/carosign-provider-sdk/src/handshake/phase.rs](/home/alex/projects/carosign/crates/protocol/carosign-provider-sdk/src/handshake/phase.rs:7) — Phase (7), message_name (37), accepts (75), accepts_with_features (84), transition (133).
- [crates/protocol/carosign-provider-sdk/src/wire/channel.rs](/home/alex/projects/carosign/crates/protocol/carosign-provider-sdk/src/wire/channel.rs:13) — Channel (13), check_oneofs (111), payload_name (158), check_payload_bounds (195).
### crates/runtime

- [crates/runtime/carosign-adapters/src/identity.rs](/home/alex/projects/carosign/crates/runtime/carosign-adapters/src/identity.rs:14) — PeerIdentity (14), subject_id (43), subject_number (53).
- [crates/runtime/carosign-adapters/src/mutation/authorized_completed_outcome.rs](/home/alex/projects/carosign/crates/runtime/carosign-adapters/src/mutation/authorized_completed_outcome.rs:5) — authorized_completed_outcome (5), authorized_completed_outcome_by_operation (33).
- [crates/runtime/carosign-adapters/src/mutation/removal_recovery.rs](/home/alex/projects/carosign/crates/runtime/carosign-adapters/src/mutation/removal_recovery.rs:5) — restore_removal_for_reconciliation (5), restore_removal_for_reconciliation_v4 (28), restore_removal_from_plan (48).
- [crates/runtime/carosign-adapters/src/policy/file_policy.rs](/home/alex/projects/carosign/crates/runtime/carosign-adapters/src/policy/file_policy.rs:13) — FilePolicy (13), action_of (148), uid_bytes (159), grant_uid (170).
- [crates/runtime/carosign-adapters/src/policy/subject_encoding.rs](/home/alex/projects/carosign/crates/runtime/carosign-adapters/src/policy/subject_encoding.rs:6) — subject_bytes (6), local_subject (13), subject_of (22).
- [crates/runtime/carosign-adapters/src/provider/binding_encoding.rs](/home/alex/projects/carosign/crates/runtime/carosign-adapters/src/provider/binding_encoding.rs:3) — enrollment_ref (3), to_binding (14).
- [crates/runtime/carosign-adapters/src/provider/mutation_frame.rs](/home/alex/projects/carosign/crates/runtime/carosign-adapters/src/provider/mutation_frame.rs:5) — mutation_frame (5), continuation_frame (48).
- [crates/runtime/carosign-persistence/src/enrollment_codec/envelope_codec.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/enrollment_codec/envelope_codec.rs:3) — unenvelope (3), unenvelope_with (8), check_stored_value_assignment (45), envelope (66), envelope_with (71).
- [crates/runtime/carosign-persistence/src/enrollment_codec/record_codec.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/enrollment_codec/record_codec.rs:8) — decode (8), decode_v3 (14), decode_payload (22), encode (111), encode_v3 (117), encode_payload (125).
- [crates/runtime/carosign-persistence/src/idempotency.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/idempotency.rs:9) — stage (9), lookup (29).
- [crates/runtime/carosign-persistence/src/installation_ledger.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/installation_ledger.rs:19) — key (19), valid_path (27), encode (39), take (76), decode (85).
- [crates/runtime/carosign-persistence/src/installation_records/installation_codec.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/installation_records/installation_codec.rs:11) — path (11), validate (22), encode (79), decode (95).
- [crates/runtime/carosign-persistence/src/installation_records/migration.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/installation_records/migration.rs:9) — prepare_v3_package (9), MigratableRecord (29), prepare_v3_record (40), checked_v4_record_as_v3 (82), preflight_v3_packages (102).
- [crates/runtime/carosign-persistence/src/installation_records/publication.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/installation_records/publication.rs:11) — PublicationFailure (11), publish_initial_package_and_installation (27), publish_initial_with_policy_check (42), publish_initial_with_provenance (65), stage_publication (177), stage_publication_v5 (199), stage_publication_for_schema (221).
- [crates/runtime/carosign-persistence/src/installation_records/record_fields.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/installation_records/record_fields.rs:1) — id (1), profile_name (5), version (13).
- [crates/runtime/carosign-persistence/src/installation_records/revocation.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/installation_records/revocation.rs:9) — RevokeFailure (9), stage_revocation (19), stage_uninstall (79).
- [crates/runtime/carosign-persistence/src/installation_records/snapshot.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/installation_records/snapshot.rs:10) — normal_v4_meta (10), read_active_snapshot (28), read_quarantined_snapshot (67), revalidate_quarantined_snapshot (134), revalidate_snapshot (154), validate_v4_snapshot (172).
- [crates/runtime/carosign-persistence/src/journal_store/current_scope.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/journal_store/current_scope.rs:6) — check_current_scope (6), check_current_scope_v3 (15), read_current_scope_v3 (23), check_current_scope_with (30).
- [crates/runtime/carosign-persistence/src/journal_store/read_schema.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/journal_store/read_schema.rs:4) — ReadSchema (4), read_schema (11), require_legacy_schema (25).
- [crates/runtime/carosign-persistence/src/migration/cleanup_staging.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/migration/cleanup_staging.rs:5) — cleanup_committed_staging (5), cleanup_committed_staging_inner (13).
- [crates/runtime/carosign-persistence/src/migration/publish_pending.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/migration/publish_pending.rs:3) — publish_pending (3), publish_pending_inner (15).
- [crates/runtime/carosign-persistence/src/migration/snapshot_preflight.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/migration/snapshot_preflight.rs:6) — preflight_v3_snapshot (6), preflight_v3_snapshot_inner (16).
- [crates/runtime/carosign-persistence/src/migration/stage_pending.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/migration/stage_pending.rs:6) — stage_pending (6), stage_pending_inner (19).
- [crates/runtime/carosign-persistence/src/migration/switch_staged.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/migration/switch_staged.rs:5) — switch_staged (5), switch_staged_inner (19), is_committed_v4_journal (98).
- [crates/runtime/carosign-persistence/src/mutation_plan/codec.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/mutation_plan/codec.rs:10) — encode_plan (10), decode_plan (55).
- [crates/runtime/carosign-persistence/src/mutation_plan/digest.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/mutation_plan/digest.rs:5) — canonical_digest (5), canonical_request_digest (65).
- [crates/runtime/carosign-persistence/src/mutation_records/codec/idempotency_codec.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/mutation_records/codec/idempotency_codec.rs:6) — encode_idempotency (6), decode_idempotency (26).
- [crates/runtime/carosign-persistence/src/mutation_records/codec/mutation_codec.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/mutation_records/codec/mutation_codec.rs:9) — reference (9), parse_reference (17), subject (26), parse_subject (33), wire_plan (40), parse_plan (66), validate_v3 (104), encode_mutation (145), encode_record_for_schema (150), encode_mutation_version (157), decode_mutation (221).
- [crates/runtime/carosign-persistence/src/mutation_records/codec/outbox_codec.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/mutation_records/codec/outbox_codec.rs:6) — encode_outbox (6), decode_outbox (38), validate_mutation_outbox (85).
- [crates/runtime/carosign-persistence/src/mutation_records/durable_arm.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/mutation_records/durable_arm.rs:9) — arm_durable_v3 (9), arm_durable_for_schema (27).
- [crates/runtime/carosign-persistence/src/mutation_records/observation.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/mutation_records/observation.rs:14) — ObservedMutation (14), stage_observation (37), stage_enrollment_rollback (103).
- [crates/runtime/carosign-persistence/src/mutation_records/ownership.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/mutation_records/ownership.rs:8) — check_journal_owner (8), check_idempotency_owner (26).
- [crates/runtime/carosign-persistence/src/mutation_records/preparation.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/mutation_records/preparation.rs:11) — stage_prepared (11), stage_prepared_for_schema (25).
- [crates/runtime/carosign-persistence/src/mutation_records/reconciliation.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/mutation_records/reconciliation.rs:10) — stage_needs_reconciliation (10), persist_needs_reconciliation (69).
- [crates/runtime/carosign-persistence/src/mutation_store/completion.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/mutation_store/completion.rs:9) — stage_completed_v4 (9), complete_v4 (22), read_completed_v4 (37).
- [crates/runtime/carosign-persistence/src/mutation_store/enrollment.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/mutation_store/enrollment.rs:7) — encode_enrollment_v4 (7), decode_enrollment_v4 (20), decode_enrollment_in_snapshot (30).
- [crates/runtime/carosign-persistence/src/mutation_store/no_change.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/mutation_store/no_change.rs:7) — stage_pre_dispatch_no_change_v4 (7), stage_pre_dispatch_no_change_v5 (17), stage_pre_dispatch_no_change_with_schema (28).
- [crates/runtime/carosign-persistence/src/mutation_store/preparation.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/mutation_store/preparation.rs:9) — stage_prepared_v4 (9), stage_prepared_v5 (21), stage_prepared_with_schema (32).
- [crates/runtime/carosign-persistence/src/mutation_store/registry_update.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/mutation_store/registry_update.rs:8) — stage_registry_update_v4 (8), commit_registry_v4 (21).
- [crates/runtime/carosign-persistence/src/mutation_store/schema.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/mutation_store/schema.rs:7) — corrupt (7), kind (13).
- [crates/runtime/carosign-persistence/src/mutation_store/snapshot.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/mutation_store/snapshot.rs:7) — scan_prepared_v4_snapshot (7), inspect_prepared_v4 (16), scan_prepared_view (48).
- [crates/runtime/carosign-persistence/src/projection.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/projection.rs:26) — build (26), check_scope (97), checked_old (133).
- [crates/runtime/carosign-persistence/src/recovery/completed_outcome.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/recovery/completed_outcome.rs:6) — read_completed_mutation_v3 (6), read_completed_by_operation_v3 (30), completed_outcome_in_snapshot (61).
- [crates/runtime/carosign-persistence/src/recovery/read_mutation_snapshot.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/recovery/read_mutation_snapshot.rs:6) — read_mutation_snapshot_v3 (6), read_mutation_snapshot_v3_in_snapshot (20).
- [crates/runtime/carosign-persistence/src/recovery/recovery_policy.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/recovery/recovery_policy.rs:6) — decide_for (6), may_replay (44), scopes_to_block (53).
- [crates/runtime/carosign-persistence/src/recovery/registry_checks.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/recovery/registry_checks.rs:3) — check_idempotency_for_mutation (3), check_registry_v3 (28).
- [crates/runtime/carosign-persistence/src/redb/backend/backup.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/redb/backend/backup.rs:3) — backup_entry_path (3), backup_identity (8).
- [crates/runtime/carosign-persistence/src/redb/codec.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/redb/codec.rs:23) — id_key (23), scope_lock_key (31), enrollment_index_key (36), outbox_key (51), idempotency_key (64), template_key (75), Envelope (89), checksum (169).
- [crates/runtime/carosign-persistence/src/redb/record.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/redb/record.rs:26) — phase_code (26), phase_from (38), encode_mutation (55), decode_mutation (163), corrupt (312), Cursor (319).
- [crates/runtime/carosign-persistence/src/store/table_data.rs](/home/alex/projects/carosign/crates/runtime/carosign-persistence/src/store/table_data.rs:1) — Entry (1), TableMap (3).
- [crates/runtime/carosign-runtime/src/flow/cycle/cycle_driver.rs](/home/alex/projects/carosign/crates/runtime/carosign-runtime/src/flow/cycle/cycle_driver.rs:6) — step_actor (6), operation_of (34).
- [crates/runtime/carosign-runtime/src/provider/packages/manifest/manifest.rs](/home/alex/projects/carosign/crates/runtime/carosign-runtime/src/provider/packages/manifest/manifest.rs:7) — Manifest (7), parse (22), TomlTable (151), table (153), section (160), string (168), number (181), no_unknown (197).
- [crates/runtime/carosign-runtime/src/scheduler/cleanup/quiesced_lease.rs](/home/alex/projects/carosign/crates/runtime/carosign-runtime/src/scheduler/cleanup/quiesced_lease.rs:5) — QuiescedLease (5), confirm_quiesced (18), confirm_quiesced_for (32).
### crates/testing

- [crates/testing/carosign-mock-provider/src/enrollments/state_file.rs](/home/alex/projects/carosign/crates/testing/carosign-mock-provider/src/enrollments/state_file.rs:5) — load_mock_state (5), save_mock_state (42).
- [crates/testing/carosign-mock-provider/src/packet_channel.rs](/home/alex/projects/carosign/crates/testing/carosign-mock-provider/src/packet_channel.rs:3) — validate_channel (3), send (36), ReceivedPacket (45), receive (47).
- [crates/testing/carosign-mock-provider/src/session/request_validation.rs](/home/alex/projects/carosign/crates/testing/carosign-mock-provider/src/session/request_validation.rs:3) — valid_id (3), valid_verification (7).
- [crates/testing/carosign-mock-provider/src/ssh_decision.rs](/home/alex/projects/carosign/crates/testing/carosign-mock-provider/src/ssh_decision.rs:4) — wait_ssh_decision (4), wait_ssh_decision_at (9).
### crates/tools

- [crates/tools/carosign-fence-inspect/src/main.rs](/home/alex/projects/carosign/crates/tools/carosign-fence-inspect/src/main.rs:4) — count (4), main (11).

## Особые случаи

- [crates/linux/carosign-linux/src/sys.rs](/home/alex/projects/carosign/crates/linux/carosign-linux/src/sys.rs) сохраняет прямые системные вызовы и PidFd в одном файле согласно его архитектурному правилу. Независимые безопасные типы вынесены за фасадом sys.
- В контрактах Protobuf группировка привязана к пакетам. Контракты перечислены для справки и сохранены в составе существующих пакетов.

- [contracts/proto/carosign/broker/v1/broker.proto](/home/alex/projects/carosign/contracts/proto/carosign/broker/v1/broker.proto) — 6 объявлений.
- [contracts/proto/carosign/broker_relay/v1/relay.proto](/home/alex/projects/carosign/contracts/proto/carosign/broker_relay/v1/relay.proto) — 17 объявлений.
- [contracts/proto/carosign/client/v1/client.proto](/home/alex/projects/carosign/contracts/proto/carosign/client/v1/client.proto) — 30 объявлений.
- [contracts/proto/carosign/common/v1/common.proto](/home/alex/projects/carosign/contracts/proto/carosign/common/v1/common.proto) — 17 объявлений.
- [contracts/proto/carosign/provider/v1/provider.proto](/home/alex/projects/carosign/contracts/proto/carosign/provider/v1/provider.proto) — 43 объявлений.
- [contracts/proto/carosign/resource_owner/v1/owner.proto](/home/alex/projects/carosign/contracts/proto/carosign/resource_owner/v1/owner.proto) — 6 объявлений.
- [contracts/proto/carosign/storage/v1/storage.proto](/home/alex/projects/carosign/contracts/proto/carosign/storage/v1/storage.proto) — 23 объявлений.
- [docs/specs/contracts/proto/carosign/broker/v1/broker.proto](/home/alex/projects/carosign/docs/specs/contracts/proto/carosign/broker/v1/broker.proto) — 6 объявлений.
- [docs/specs/contracts/proto/carosign/client/v1/client.proto](/home/alex/projects/carosign/docs/specs/contracts/proto/carosign/client/v1/client.proto) — 30 объявлений.
- [docs/specs/contracts/proto/carosign/common/v1/common.proto](/home/alex/projects/carosign/docs/specs/contracts/proto/carosign/common/v1/common.proto) — 17 объявлений.
- [docs/specs/contracts/proto/carosign/provider/v1/provider.proto](/home/alex/projects/carosign/docs/specs/contracts/proto/carosign/provider/v1/provider.proto) — 36 объявлений.
- [docs/specs/contracts/proto/carosign/storage/v1/storage.proto](/home/alex/projects/carosign/docs/specs/contracts/proto/carosign/storage/v1/storage.proto) — 19 объявлений.

Скрипты и C/JS-фикстуры инвентаризированы отдельно. Rust-правила о фасадах и соседних тестовых файлах к ним не применялись.
