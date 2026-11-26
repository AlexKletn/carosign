use crate::generated::carosign::provider::v1 as provider;
use crate::limits;

use super::super::{WireError, bounded_str, check_version};

/// Checking the worker's welcome.
pub fn check_worker_hello(hello: &provider::WorkerHello) -> Result<(), WireError> {
    check_version(hello.major, hello.min_minor)?;
    if hello.min_minor > hello.max_minor {
        return Err(WireError::UnsupportedVersion {
            major: hello.major,
            minor: hello.min_minor,
        });
    }
    bounded_str(
        &hello.implementation_version,
        "implementation_version",
        limits::IMPLEMENTATION_VERSION_MAX,
    )?;
    if hello.features.len() > limits::FEATURES_MAX {
        return Err(WireError::FieldTooLong {
            field: "features",
            actual: hello.features.len(),
            allowed: limits::FEATURES_MAX,
        });
    }
    for (index, feature) in hello.features.iter().enumerate() {
        if *feature == provider::Feature::Unspecified as i32 {
            return Err(WireError::UnspecifiedEnum { field: "features" });
        }
        if hello.features[..index].contains(feature) {
            return Err(WireError::UnexpectedPayload {
                what: "повтор возможности в WorkerHello",
            });
        }
    }
    Ok(())
}
