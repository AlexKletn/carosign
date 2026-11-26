use super::*;

#[test]
fn defaults_are_closed() {
    assert!(Sandbox::default().check(&[]).is_ok());
}

#[test]
fn root_is_never_allowed() {
    let sandbox = Sandbox {
        root: true,
        ..Sandbox::default()
    };
    assert_eq!(
        sandbox.check(&[Relaxation {
            field: "root",
            reason: "glib".into()
        }]),
        Err(SandboxError::RootForbidden)
    );
}

#[test]
fn network_needs_a_reason() {
    let sandbox = Sandbox {
        network: true,
        ..Sandbox::default()
    };
    assert_eq!(sandbox.check(&[]), Err(SandboxError::NetworkForbidden));
    assert!(
        sandbox
            .check(&[Relaxation {
                field: "network",
                reason: "согласовано с владельцем".into()
            }])
            .is_ok()
    );
}

#[test]
fn mount_namespace_is_required_even_with_relaxation() {
    let sandbox = Sandbox {
        own_mount_namespace: false,
        ..Sandbox::default()
    };
    assert_eq!(
        sandbox.check(&[]),
        Err(SandboxError::MountNamespaceRequired)
    );
    assert_eq!(
        sandbox.check(&[Relaxation {
            field: "own_mount_namespace",
            reason: "тест".into(),
        }]),
        Err(SandboxError::MountNamespaceRequired)
    );
}
