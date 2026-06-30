extern crate prost_build;
extern crate protobuf_src;

use prost_build::Config;
use std::path::PathBuf;

/// This compiles the protobuf files declared in this repository. The matsim schema repository is used to provide the includes.
fn main() {
    let proto_files = [
        PathBuf::from("src/simulation/io/proto/types/ids.proto"),
        PathBuf::from("src/external_services/routing/routing.proto"),
    ];

    println!(
        "cargo:rerun-if-changed={}",
        matsim_schemas::proto_dir().display()
    );
    for proto in &proto_files {
        println!("cargo:rerun-if-changed={}", proto.display());
    }

    let mut config = Config::new();
    matsim_schemas::configure_extern_path(&mut config);
    // we use the protobuf-src which provides the protoc compiler. This line makes it available
    // to prost-build
    config.protoc_executable(protobuf_src::protoc());

    let include_dirs = [PathBuf::from("src/"), matsim_schemas::proto_dir()];

    tonic_prost_build::configure()
        .build_client(true)
        .compile_with_config(config, &proto_files, &include_dirs)
        .unwrap();
}
