//! Wire: generated messages from `.proto` And check them out.
//!
//! ⚠️ There are no policy decisions, no "evidence" type. It's just the presentation of
//! messages (document 02: "`carosign-protocol` generated `.proto`It does not store
//! auth-domain logic.
//!
//! ⚠️ Checks (`validation`The code generated does not trust the sender: everything that the
//! fields mean is checked after parsing.

// ⚠️ The code generated warnings are not ours: `prost` maker `oneof` It's big, and rustc says
// it's a size difference, and you can't edit the generated, and you can jam it here and these
// alone.
#[allow(clippy::large_enum_variant)]
pub mod generated {
    //! Generated. `prost` from `contracts/proto` during assembly.
    pub mod carosign {
        pub mod common {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/carosign.common.v1.rs"));
            }
        }
        pub mod provider {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/carosign.provider.v1.rs"));
            }
        }
        pub mod client {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/carosign.client.v1.rs"));
            }
        }
        pub mod storage {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/carosign.storage.v1.rs"));
            }
        }
        pub mod broker {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/carosign.broker.v1.rs"));
            }
        }
        pub mod broker_relay {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/carosign.broker_relay.v1.rs"));
            }
        }
        pub mod provider_rendezvous {
            pub mod v1 {
                include!(concat!(
                    env!("OUT_DIR"),
                    "/carosign.provider_rendezvous.v1.rs"
                ));
            }
        }
        pub mod resource_owner {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/carosign.resource_owner.v1.rs"));
            }
        }
    }
}

pub mod limits;
pub mod resource_owner;
mod unavailability_reason;
pub mod validation;
pub mod version;

pub use generated::carosign;
