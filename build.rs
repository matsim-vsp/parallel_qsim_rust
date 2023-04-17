extern crate prost_build;
fn main() {
    // we use the protobuf-src which provides the protoc compiler. This line makes it available
    // to prost-build
    std::env::set_var("PROTOC", protobuf_src::protoc());

    // this line comes from the prost-build example and compiles items.proto into corresponding types.
    // the generated code is under ./target/<goal, e.g. debug>/build/<project-name>-<some-hash>/out
    prost_build::compile_protos(
        &[
            "src/simulation/messaging/messages.proto",
            "src/simulation/messaging/events.proto",
            "src/simulation/performance_profiling/profiling.proto",
        ],
        &["src/"],
    )
    .unwrap();
}
