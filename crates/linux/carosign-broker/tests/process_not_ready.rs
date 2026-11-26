//! As long as the contractor and the trusted vehicle are not connected, the process cannot
//! report a successful launch.

#[test]
fn broker_process_fails_closed_without_runtime() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_carosign-broker"))
        .output()
        .expect("запуск посредника");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("не готов"));
}
