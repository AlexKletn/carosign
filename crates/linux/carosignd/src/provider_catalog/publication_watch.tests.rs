use super::*;

#[test]
fn filters_unrelated_names_and_reports_publication_replacement() {
    let path = std::env::temp_dir().join(format!(
        "carosign-publication-watch-{}",
        std::process::id()
    ));
    std::fs::create_dir(&path).unwrap();
    let mut watch = PublicationWatch::new().unwrap();
    watch.add(&path, b"control.sock").unwrap();
    std::fs::write(path.join("other"), b"").unwrap();
    assert_eq!(watch.changed(Duration::from_millis(100)).unwrap(), None);
    std::fs::rename(path.join("other"), path.join("control.sock")).unwrap();
    assert_eq!(
        watch.changed(Duration::from_millis(100)).unwrap(),
        Some(false)
    );
    std::fs::remove_file(path.join("control.sock")).unwrap();
    assert_eq!(
        watch.changed(Duration::from_millis(100)).unwrap(),
        Some(false)
    );
    std::fs::remove_dir(path).unwrap();
}

#[test]
fn queue_overflow_requires_a_full_rescan() {
    let watch = PublicationWatch::new().unwrap();
    let mut event = Vec::new();
    event.extend((-1i32).to_ne_bytes());
    event.extend(libc::IN_Q_OVERFLOW.to_ne_bytes());
    event.extend(0u32.to_ne_bytes());
    event.extend(0u32.to_ne_bytes());
    assert_eq!(watch.decode(&event).unwrap(), Some(true));
}
