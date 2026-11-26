use super::super::{OBSERVERS_MAX, Observer, Observers};

#[test]
fn observers_are_bounded_and_unique() {
    let mut observers = Observers::default();
    for id in 0..OBSERVERS_MAX as u32 {
        observers
            .subscribe(Observer {
                id,
                after_sequence: 0,
            })
            .expect("влезает");
    }
    assert!(
        observers
            .subscribe(Observer {
                id: 99,
                after_sequence: 0
            })
            .is_err()
    );
    assert!(
        observers
            .subscribe(Observer {
                id: 0,
                after_sequence: 0
            })
            .is_err()
    );
    assert!(observers.unsubscribe(0));
    assert!(!observers.unsubscribe(0));
}
