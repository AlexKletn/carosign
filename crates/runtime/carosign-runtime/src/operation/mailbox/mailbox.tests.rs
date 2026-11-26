use super::super::*;

#[test]
fn control_goes_first() {
    let mut mailbox = Mailbox::new();
    mailbox.push_normal('n').expect("влезает");
    mailbox.push_control('c').expect("влезает");
    assert_eq!(mailbox.take(), Some('c'));
    assert_eq!(mailbox.take(), Some('n'));
}

#[test]
fn normal_never_starves() {
    let mut mailbox = Mailbox::new();
    mailbox.push_normal('n').expect("влезает");
    for _ in 0..CONTROL_BURST {
        mailbox.push_control('c').expect("влезает");
    }
    mailbox.push_control('c').expect("влезает");

    let mut seen = Vec::new();
    while let Some(item) = mailbox.take() {
        seen.push(item);
    }
    // The usual message hit the stream. We're counting before it.
    let first_normal = seen.iter().position(|item| *item == 'n').expect("есть");
    assert!(first_normal <= CONTROL_BURST);
}

#[test]
fn pending_invalidating_control_is_not_overtaken_after_a_burst() {
    let mut mailbox = Mailbox::new();
    mailbox.push_normal('n').unwrap();
    for _ in 0..CONTROL_BURST {
        mailbox.push_control('c').unwrap();
    }
    mailbox.push_control('r').unwrap();
    for _ in 0..CONTROL_BURST {
        assert_eq!(mailbox.take_control_first(), Some('c'));
    }
    assert_eq!(mailbox.take_control_first(), Some('r'));
    assert_eq!(mailbox.take_control_first(), Some('n'));
}

#[test]
fn control_overflow_is_an_error_not_a_drop() {
    let mut mailbox = Mailbox::new();
    for _ in 0..CONTROL_CAPACITY {
        mailbox.push_control(1).expect("влезает");
    }
    assert_eq!(
        mailbox.push_control(2),
        Err(MailboxFull {
            priority: Priority::Control
        })
    );
}

#[test]
fn normal_overflow_is_an_error() {
    let mut mailbox = Mailbox::new();
    for _ in 0..NORMAL_CAPACITY {
        mailbox.push_normal(1).expect("влезает");
    }
    assert!(mailbox.push_normal(2).is_err());
}
