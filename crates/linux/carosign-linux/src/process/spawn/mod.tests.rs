use crate::process::pidfd::ProcessState;
use crate::process::sandbox::Sandbox;

use super::*;
use crate::process::sandbox::Relaxation;
use crate::process::verified_exec::Preconditions;

fn conditions() -> Preconditions {
    Preconditions {
        trusted_root: true,
        regular_file: true,
        root_owned: true,
        not_world_writable: true,
        manifest_ok: true,
    }
}

#[test]
fn stages_go_in_order_and_no_skipping() {
    let mut launch = Launch::new();
    assert_eq!(
        launch.spawned(),
        Err(SpawnError::WrongStage {
            stage: LaunchStage::Idle,
            requested: LaunchStage::Spawned,
        })
    );
    launch
        .check(&conditions(), &Sandbox::default(), &[])
        .expect("проверено");
    assert_eq!(launch.stage(), LaunchStage::Checked);
    launch.channelled().expect("канал");
    launch.spawned().expect("процесс");
    assert!(launch.handed().is_err());
    launch.isolated().expect("изоляция");
    assert!(launch.ready().is_err());
    launch.executed().expect("exec");
    launch.handed().expect("отдано");
    launch.hello().expect("приветствие");
    launch.initialized().expect("инициализация");
    launch.ready().expect("привязка");
    assert_eq!(launch.stage(), LaunchStage::Ready);
    assert_eq!(launch.process(), ProcessState::Running);
}

#[test]
fn failure_retains_stage_and_cannot_be_restarted() {
    let mut launch = Launch::new();
    launch
        .check(&conditions(), &Sandbox::default(), &[])
        .unwrap();
    launch.channelled().unwrap();
    launch.spawned().unwrap();
    launch.fail();
    launch.fail();
    assert_eq!(launch.failed_at(), Some(LaunchStage::Spawned));
    assert_eq!(launch.stage(), LaunchStage::Failed);
    assert!(launch.isolated().is_err());
    assert!(
        launch
            .check(&conditions(), &Sandbox::default(), &[])
            .is_err()
    );
    // Failure does not prove the completion of the process or the rest of the device.
    assert_eq!(launch.process(), ProcessState::Running);
}

#[test]
fn open_condition_stops_the_launch() {
    let mut launch = Launch::new();
    let mut bad = conditions();
    bad.root_owned = false;
    assert!(launch.check(&bad, &Sandbox::default(), &[]).is_err());
    assert_eq!(launch.stage(), LaunchStage::Idle);
}

#[test]
fn sandbox_stops_the_launch_too() {
    let mut launch = Launch::new();
    let sandbox = Sandbox {
        network: true,
        ..Sandbox::default()
    };
    assert!(launch.check(&conditions(), &sandbox, &[]).is_err());
    launch
        .check(
            &conditions(),
            &sandbox,
            &[Relaxation {
                field: "network",
                reason: "согласовано".into(),
            }],
        )
        .expect("с ослаблением");
}

#[test]
fn missing_mount_namespace_cannot_reach_checked_stage() {
    let mut launch = Launch::new();
    let sandbox = Sandbox {
        own_mount_namespace: false,
        ..Sandbox::default()
    };
    assert_eq!(
        launch.check(&conditions(), &sandbox, &[]),
        Err(SpawnError::Sandbox(
            crate::process::sandbox::SandboxError::MountNamespaceRequired
        ))
    );
    assert_eq!(launch.stage(), LaunchStage::Idle);
    assert!(launch.channelled().is_err());
}
