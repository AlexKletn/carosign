//! Constructing wire circuits from `.proto`.
//!
//! ⚠️ The only source of truth on the wire - files in `contracts/proto` The code is not
//! written by hand and cannot be repaired after assembly: the diagram is edited in `.proto`
//! And it's being tested.
//!
//! ⚠️ Pathway `.proto` It is given from the root of the crate assembly and points to the
//! general project directory: the diagrams live **out ** of the krate so that anyone can read
//! them, including the provider written in C.

use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("каталог крейта"));
    let root = manifest.join("../../../contracts/proto");

    let protos = [
        root.join("carosign/common/v1/common.proto"),
        root.join("carosign/provider/v1/provider.proto"),
        root.join("carosign/client/v1/client.proto"),
        root.join("carosign/storage/v1/storage.proto"),
        root.join("carosign/broker/v1/broker.proto"),
        root.join("carosign/broker_relay/v1/relay.proto"),
        root.join("carosign/resource_owner/v1/owner.proto"),
        root.join("carosign/provider_rendezvous/v1/rendezvous.proto"),
    ];

    for proto in &protos {
        println!("cargo:rerun-if-changed={}", proto.display());
    }

    let mut config = prost_build::Config::new();
    config.btree_map(["."]);
    for capture_type in [
        "CapturePrompt",
        "CaptureRetry",
        "AcceptedSamples",
        "CaptureFeedback",
        "CaptureState",
    ] {
        config.type_attribute(
            format!(".carosign.common.v1.{capture_type}"),
            "#[derive(Copy, Eq)]",
        );
    }
    config
        .compile_protos(&protos, &[&root])
        .expect("схемы провода обязаны собираться");
}
