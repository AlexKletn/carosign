use super::*;

fn good() -> Preconditions {
    Preconditions {
        trusted_root: true,
        regular_file: true,
        root_owned: true,
        not_world_writable: true,
        manifest_ok: true,
    }
}

#[test]
fn all_conditions_required() {
    assert!(check(&good()).is_ok());
    let mutable = Preconditions {
        not_world_writable: false,
        ..good()
    };
    assert_eq!(check(&mutable), Err(LaunchError::WorldWritable));
    let symlink = Preconditions {
        regular_file: false,
        ..good()
    };
    assert_eq!(check(&symlink), Err(LaunchError::NotRegularFile));
}
