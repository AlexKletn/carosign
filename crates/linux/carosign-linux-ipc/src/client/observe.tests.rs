use super::*;

#[test]
fn gap_is_explicit() {
    assert_eq!(
        start(0, Some(5)),
        ObserveStart::GapThenSnapshot { resume_from: 5 }
    );
    assert_eq!(start(4, Some(5)), ObserveStart::From { sequence: 5 });
    assert_eq!(start(0, None), ObserveStart::From { sequence: 1 });
    assert_eq!(start(u64::MAX, None), ObserveStart::Exhausted);
    assert_eq!(start(u64::MAX, Some(5)), ObserveStart::Exhausted);
}
