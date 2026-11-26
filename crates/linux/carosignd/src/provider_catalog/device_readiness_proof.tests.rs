use super::*;
use crate::provider_catalog::publication::Publication;
use crate::provider_catalog::published_device::PublishedDevice;
use carosign_core::{
    device::{AuthenticatorRef, DeviceEpoch},
    ids::AuthenticatorId,
};
use carosign_linux::policy::PolicyChangeWatch;
use std::{path::Path, sync::Mutex};

#[test]
fn readiness_expires_with_publication_epoch_or_profile() {
    let path =
        std::env::temp_dir().join(format!("carosign-readiness-proof-{}", std::process::id()));
    std::fs::write(&path, "profile").unwrap();
    let metadata = std::fs::metadata(&path).unwrap();
    let id = AuthenticatorId::from_bytes([3; 16]).unwrap();
    let identity = EndpointIdentity::of(&metadata, 11);
    let proof = DeviceReadinessProof {
        result: Ok(Default::default()),
        remote: RemoteProvider {
            publication: Publication {
                name: "fixture".into(),
                uid: 12,
                gid: 12,
                devices: [(
                    id,
                    PublishedDevice {
                        profile: Some(path.clone()),
                        disabled: false,
                    },
                )]
                .into(),
            },
            root: Path::new("/run"),
            runtime: "/run/carosignd".into(),
            identity,
            authenticator: AuthenticatorRef {
                id,
                device_epoch: DeviceEpoch::FIRST,
            },
            epochs: Default::default(),
            watches: Mutex::new([
                PolicyChangeWatch::new(&path).unwrap(),
                PolicyChangeWatch::new(&path).unwrap(),
            ]),
        },
    };
    assert!(proof.matches(Some((identity, 1))));
    assert!(!proof.matches(None));
    assert!(!proof.matches(Some((identity, 2))));
    assert!(!proof.matches(Some((EndpointIdentity::of(&metadata, 12), 1))));
    std::fs::write(&path, "changed").unwrap();
    assert!(!proof.matches(Some((identity, 1))));
    std::fs::remove_file(path).unwrap();
}
