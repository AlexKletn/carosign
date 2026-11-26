use super::*;

fn record() -> InstallationRecord {
    InstallationRecord {
        package_id: [1; 16],
        provider_id: [2; 16],
        profile_id: [3; 16],
        root_device: 42,
        root_inode: 77,
        fingerprint: "A".repeat(40),
        revision: 1,
        revoked: false,
        files: BTreeMap::from([("bin/worker".into(), [9; 32])]),
    }
}

#[test]
fn strict_codec_rejects_invalid_paths_keys_trailing_and_truncation() {
    let r = record();
    let k = key(&r);
    let encoded = encode(&r).unwrap();
    assert_eq!(decode(&k, &encoded).unwrap(), r);
    assert_eq!(decode(&[8; 48], &encoded), Err(LedgerError::Corrupt));
    for cut in 0..encoded.len() {
        assert_eq!(decode(&k, &encoded[..cut]), Err(LedgerError::Corrupt));
    }
    let mut extra = encoded;
    extra.push(0);
    assert_eq!(decode(&k, &extra), Err(LedgerError::Corrupt));
    let mut invalid = r;
    invalid.files = BTreeMap::from([("../escape".into(), [1; 32])]);
    assert_eq!(encode(&invalid), Err(LedgerError::Invalid));
}

#[test]
fn durable_reopen_conflict_and_revocation() {
    let path = std::env::temp_dir().join(format!(
        "carosign-install-ledger-{}-{}.redb",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _cleanup = Cleanup(path.clone());
    let r = record();
    let k = key(&r);
    {
        let ledger = Ledger::create(&path).unwrap();
        ledger.compare_and_set(&r, None).unwrap();
        assert_eq!(ledger.compare_and_set(&r, None), Err(LedgerError::Conflict));
        let mut wrong = r.clone();
        wrong.revision = 2;
        wrong.fingerprint = "B".repeat(40);
        assert_eq!(
            ledger.compare_and_set(&wrong, Some(1)),
            Err(LedgerError::Conflict)
        );
        assert_eq!(ledger.read(&k).unwrap(), Some(r.clone()));
    }
    let ledger = Ledger::open(&path).unwrap();
    assert_eq!(ledger.read(&k).unwrap(), Some(r.clone()));
    let mut revoked = r.clone();
    revoked.revoked = true;
    revoked.revision = 2;
    ledger.compare_and_set(&revoked, Some(1)).unwrap();
    let mut repeat = r;
    repeat.revision = 3;
    assert_eq!(
        ledger.compare_and_set(&repeat, Some(2)),
        Err(LedgerError::Conflict)
    );
    drop(ledger);
    assert_eq!(
        Ledger::open(&path).unwrap().read(&k).unwrap(),
        Some(revoked)
    );
}
