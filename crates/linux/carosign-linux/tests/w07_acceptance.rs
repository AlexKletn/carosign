//! Receiving W07: inventory - from the system, launch - on real kernel calls.
//!
//! ⚠️ Checks what breaks quietly: empty inventory is taken for full, the worker is launched
//! from the world where everything is written, and `exec` on the way instead of a file.

use carosign_linux::discovery::inventory::InventoryScan;
use carosign_linux::discovery::launch::{LaunchPlan, LaunchRefusal, prepare};
use carosign_linux::process::verified_exec::{LaunchError, Preconditions, check};

#[test]
fn inventory_scan_is_reported_as_complete_or_not() {
    // ⚠️ An incomplete round is not a complete one.
    let scan = InventoryScan::scan();
    if !scan.is_incomplete() {
        // Full image: Each device has a subsystem.
        for device in scan.devices() {
            assert!(
                device
                    .signals
                    .iter()
                    .any(|(key, value)| key == "subsystem" && value == "hidraw")
            );
        }
    }
}

#[test]
fn empty_inventory_is_a_healthy_state() {
    let scan = InventoryScan::empty();
    assert!(scan.is_empty());
    assert!(!scan.is_incomplete());
    // ⚠️ Zero devices is not a mistake.
    assert_eq!(scan.len(), 0);
}

#[test]
fn launch_requires_verified_trusted_root_and_manifest() {
    // The very existence of the file does not confirm the origin of the packet.
    let program = std::fs::canonicalize("/bin/sh").expect("исполняемый файл");
    let prepared = prepare(program.to_str().expect("UTF-8 путь"));
    assert!(matches!(
        prepared,
        Err(LaunchRefusal::Conditions(LaunchError::UntrustedRoot))
    ));
}

#[test]
fn missing_program_is_refused() {
    assert!(matches!(
        prepare("/nonexistent/definitely-not-here"),
        Err(LaunchRefusal::Kernel(_))
    ));
}

#[test]
fn world_writable_condition_stops_launch() {
    let conditions = Preconditions {
        trusted_root: true,
        regular_file: true,
        root_owned: true,
        not_world_writable: false,
        manifest_ok: true,
    };
    assert_eq!(check(&conditions), Err(LaunchError::WorldWritable));
}

#[test]
fn launch_plan_has_no_inherited_environment() {
    let plan = LaunchPlan::clean("worker");
    assert_eq!(plan.argv.len(), 1);
    assert!(!plan.env.iter().any(|item| item.starts_with("LD_PRELOAD")));
}
