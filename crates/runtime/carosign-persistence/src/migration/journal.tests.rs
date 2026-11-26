use super::*;

fn pending() -> Journal {
    Journal {
        id: [1; 16],
        backup_length: 123,
        backup_digest: [2; 32],
        backup_device: 0,
        backup_inode: 3,
        stage: Stage::Pending,
    }
}

fn resign(bytes: &mut [u8]) {
    let digest = sha256(&bytes[8..136]);
    bytes[136..].copy_from_slice(&digest);
}

#[test]
fn all_stages_have_exact_envelope_and_round_trip() {
    let empty = Snapshot {
        count: 0,
        digest: sha256(&[]),
    };
    let full = Snapshot {
        count: 3,
        digest: [4; 32],
    };
    for stage in [Stage::Pending, Stage::Staged(empty), Stage::Committed(full)] {
        let value = Journal { stage, ..pending() };
        let bytes = value.encode().unwrap();
        assert_eq!(bytes.len(), ENVELOPE_LEN);
        assert_eq!(&bytes[..8], b"BIO1\x80\0\0\0");
        assert_eq!(Journal::decode(&bytes), Ok(value));
    }
}

#[test]
fn rejects_truncation_corruption_and_extra_data() {
    let bytes = pending().encode().unwrap();
    for n in 0..bytes.len() {
        assert_eq!(Journal::decode(&bytes[..n]), Err(InvalidJournal));
    }
    let mut extra = bytes.clone();
    extra.push(0);
    assert_eq!(Journal::decode(&extra), Err(InvalidJournal));
    for pos in [0, 4, 8, 24, 136] {
        let mut damaged = bytes.clone();
        damaged[pos] ^= 1;
        assert_eq!(Journal::decode(&damaged), Err(InvalidJournal));
    }
}

#[test]
fn rejects_well_checksummed_noncanonical_fields() {
    let bytes = pending().encode().unwrap();
    for pos in [8, 12, 13, 14, 16, 20, 24, 40, 48, 96] {
        let mut value = bytes.clone();
        match pos {
            24 => value[24..40].fill(0),
            40 => value[40..48].fill(0),
            48 => value[48..80].fill(0),
            _ => value[pos] ^= 1,
        }
        resign(&mut value);
        assert_eq!(
            Journal::decode(&value),
            Err(InvalidJournal),
            "смещение {pos}"
        );
    }
    let mut inode = bytes.clone();
    inode[88..96].fill(0);
    resign(&mut inode);
    assert_eq!(Journal::decode(&inode), Err(InvalidJournal));
    let mut stage = bytes.clone();
    stage[13] = 2;
    resign(&mut stage);
    assert_eq!(Journal::decode(&stage), Err(InvalidJournal));
}

#[test]
fn transitions_preserve_backup_and_snapshot() {
    let first = pending();
    let snapshot = Snapshot {
        count: 2,
        digest: [7; 32],
    };
    let staged = Journal {
        stage: Stage::Staged(snapshot),
        ..first
    };
    let committed = Journal {
        stage: Stage::Committed(snapshot),
        ..first
    };
    assert!(first.may_advance_to(&staged));
    assert!(staged.may_advance_to(&committed));
    assert!(!first.may_advance_to(&committed));
    assert!(!committed.may_advance_to(&staged));
    assert!(!staged.may_advance_to(&Journal {
        backup_inode: 4,
        ..committed
    }));
    assert!(!staged.may_advance_to(&Journal {
        stage: Stage::Committed(Snapshot {
            count: 3,
            ..snapshot
        }),
        ..committed
    }));
    assert!(
        !Journal {
            stage: Stage::Staged(Snapshot {
                count: 0,
                digest: [1; 32]
            }),
            ..first
        }
        .valid()
    );
}

#[test]
fn recovery_never_resumes_unfinished_or_accepts_mismatched_marker() {
    let p = pending();
    let snapshot = Snapshot {
        count: 1,
        digest: [7; 32],
    };
    let pending_bytes = p.encode().unwrap();
    let staged_bytes = Journal {
        stage: Stage::Staged(snapshot),
        ..p
    }
    .encode()
    .unwrap();
    let committed_bytes = Journal {
        stage: Stage::Committed(snapshot),
        ..p
    }
    .encode()
    .unwrap();
    assert_eq!(classify(3, None), Ok(RecoveryState::Unstarted));
    assert_eq!(
        classify(3, Some(&pending_bytes)),
        Ok(RecoveryState::AdministrativeRecovery(Stage::Pending))
    );
    assert_eq!(
        classify(3, Some(&staged_bytes)),
        Ok(RecoveryState::AdministrativeRecovery(Stage::Staged(
            snapshot
        )))
    );
    assert_eq!(
        classify(4, Some(&committed_bytes)),
        Ok(RecoveryState::Committed(snapshot))
    );
    assert_eq!(classify(3, Some(&committed_bytes)), Err(InvalidJournal));
    assert_eq!(classify(4, Some(&pending_bytes)), Err(InvalidJournal));
    assert_eq!(classify(4, Some(&staged_bytes)), Err(InvalidJournal));
    assert_eq!(classify(4, None), Err(InvalidJournal));
    assert_eq!(classify(5, Some(&committed_bytes)), Err(InvalidJournal));
    let mut corrupt = committed_bytes;
    corrupt[30] ^= 1;
    assert_eq!(classify(4, Some(&corrupt)), Err(InvalidJournal));
}
