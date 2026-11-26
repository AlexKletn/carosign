use carosign_adapters::boot::{BootError, Builder};

#[test]
fn empty_builder_is_refused_before_assembly() {
    // ⚠️ Port assembly does not begin unless something is done.
    let error = Builder::new().compose().unwrap_err();
    match error {
        BootError::MissingPorts { missing } => assert_eq!(missing.len(), 5),
    }
}
