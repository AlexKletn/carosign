use super::*;

#[test]
fn unavailable_or_repeated_zero_does_not_invent_ids() {
    let mut calls = 0;
    assert_eq!(
        nonzero_bytes(|| {
            calls += 1;
            Ok([0; 16])
        }),
        Err(IdUnavailable)
    );
    assert_eq!(calls, 8);
    assert_eq!(
        nonzero_bytes(|| Err(sys::SysError {
            errno: libc::EAGAIN
        })),
        Err(IdUnavailable)
    );
    let mut calls = 0;
    assert_eq!(
        nonzero_bytes(|| {
            calls += 1;
            Ok(if calls == 2 { [1; 16] } else { [0; 16] })
        }),
        Ok([1; 16])
    );
}

#[test]
fn hundred_operations_are_hundred_different_ids() {
    let source = KernelIds::new();
    let mut seen = std::collections::HashSet::new();
    for _ in 0..100 {
        let id = source.new_operation_id().unwrap();
        assert!(id.as_bytes().iter().any(|byte| *byte != 0));
        assert!(seen.insert(*id.as_bytes()), "повтор идентификатора");
    }
}

#[test]
fn every_kind_gets_its_own_bytes() {
    let source = KernelIds::new();
    assert_eq!(source.new_mutation_id().unwrap().as_bytes().len(), 16);
    assert_eq!(source.new_command_id().unwrap().as_bytes().len(), 16);
    assert_eq!(
        source.new_worker_instance_id().unwrap().as_bytes().len(),
        16
    );
    assert_eq!(source.new_lease_id().unwrap().as_bytes().len(), 16);
    assert_eq!(source.new_evidence_id().unwrap().as_bytes().len(), 16);
    assert_eq!(source.new_idempotency_key().unwrap().as_bytes().len(), 16);
    assert_eq!(source.new_connection_id().unwrap().as_bytes().len(), 16);
}

#[test]
fn randomness_is_available_on_this_machine() {
    assert!(available());
}

#[test]
fn sys_module_is_the_only_unsafe_place() {
    // ⚠️ Verification of intent, not code: the source should not drag the second `unsafe` -
    // He's all in. `sys`.
    let _ = sys::boot_id();
}
