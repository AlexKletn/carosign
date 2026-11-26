use super::*;

#[test]
fn device_must_stay_closed_until_bound() {
    assert_eq!(
        Boot {
            channel: true,
            device_closed: false
        }
        .check(),
        Err(BootError::DeviceOpenTooEarly)
    );
    assert_eq!(
        Boot {
            channel: false,
            device_closed: true
        }
        .check(),
        Err(BootError::NoChannel)
    );
    assert_eq!(
        Boot {
            channel: true,
            device_closed: true
        }
        .check(),
        Ok(())
    );
}
