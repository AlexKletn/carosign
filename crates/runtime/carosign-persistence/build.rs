//! assembling an internal storage scheme from a common platform contract.

fn main() {
    let root = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("../../../contracts/proto");
    let common = root.join("carosign/common/v1/common.proto");
    let storage = root.join("carosign/storage/v1/storage.proto");
    println!("cargo:rerun-if-changed={}", common.display());
    println!("cargo:rerun-if-changed={}", storage.display());
    prost_build::Config::new()
        .compile_protos(&[common, storage], &[root])
        .expect("схема хранения обязана собираться");
}
