#![allow(dead_code, clippy::large_enum_variant, clippy::enum_variant_names)]

pub mod common {
    pub mod v1 {
        include!(concat!(env!("OUT_DIR"), "/carosign.common.v1.rs"));
    }
}
pub mod storage {
    pub mod v1 {
        include!(concat!(env!("OUT_DIR"), "/carosign.storage.v1.rs"));
    }
}
