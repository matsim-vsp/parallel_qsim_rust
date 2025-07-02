extern crate prost_build;
extern crate protobuf_src;

fn main() {
    // we use the protobuf-src which provides the protoc compiler. This line makes it available
    // to prost-build
    std::env::set_var("PROTOC", protobuf_src::protoc());

    let proto_files = [
        "src/simulation/io/proto/types/general.proto",
        "src/simulation/io/proto/types/events.proto",
        "src/simulation/io/proto/types/ids.proto",
        "src/simulation/io/proto/types/network.proto",
        "src/simulation/io/proto/types/population.proto",
        "src/simulation/io/proto/types/vehicles.proto",
    ];

    // tell cargo to rerun this build script if any of the proto files change
    for proto in &proto_files {
        println!("cargo:rerun-if-changed={}", proto);
    }

    // Compiling the protobuf files
    tonic_build::configure()
        .build_client(true)
        .compile_protos(&proto_files, &["src/"])
        .unwrap();
}
