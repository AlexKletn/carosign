use super::super::*;

fn words(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

#[test]
fn parses_known_commands() {
    assert_eq!(parse(&words(&["health"])), Ok(Command::Health));
    assert_eq!(
        parse(&words(&["status", "01010101010101010101010101010101"])),
        Ok(Command::Status {
            operation: "01010101010101010101010101010101".into()
        })
    );
    assert_eq!(
        parse(&words(&["verify", "alex"])),
        Ok(Command::Verify {
            user: "alex".into()
        })
    );
    assert_eq!(
        parse(&words(&["list", "enrollments"])),
        Ok(Command::ListEnrollments { user: None })
    );
    assert_eq!(
        parse(&words(&["list", "enrollments", "alex"])),
        Ok(Command::ListEnrollments {
            user: Some("alex".into())
        })
    );
}

#[test]
fn unknown_and_missing_are_reported() {
    assert!(matches!(
        parse(&words(&["sha"])),
        Err(CommandError::Unknown { .. })
    ));
    assert_eq!(
        parse(&words(&["verify"])),
        Err(CommandError::Missing {
            what: "пользователь"
        })
    );
}

#[test]
fn unknown_subcommand_and_extra_arguments_are_not_ignored() {
    for args in [
        vec!["list", "unexpected"],
        vec!["list", "enrollments", "alex", "unexpected"],
        vec!["health", "unexpected"],
        vec!["verify", "alex", "unexpected"],
        vec!["enroll", "alex", "label", "03030303", "unexpected"],
        vec!["remove", "id", "alex", "unexpected"],
        vec!["watch", "id", "alex", "unexpected"],
        vec!["status", "id", "alex", "unexpected"],
        vec!["cancel", "id", "alex", "unexpected"],
    ] {
        assert!(matches!(
            parse(&words(&args)),
            Err(CommandError::Unknown { .. })
        ));
    }
}
