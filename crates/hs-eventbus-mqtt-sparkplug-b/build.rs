fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc not found");
    std::env::set_var("PROTOC", protoc);

    prost_build::Config::new()
        .compile_protos(&["proto/sparkplug_b.proto"], &["proto/"])
        .expect("failed to compile sparkplug_b.proto");
}
