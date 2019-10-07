use exonum_build::{get_exonum_protobuf_files_path, protobuf_generate, get_exonum_protobuf_deps_files_path};

fn main() {
    let exonum_protos = get_exonum_protobuf_files_path();

    let exonum_add_protos = get_exonum_protobuf_deps_files_path();
    let exonum_add_protos: Vec<&str> = exonum_add_protos.iter().map(|s| s.as_str()).collect();

    let mut includes = vec![exonum_protos.as_str(), "src/proto"];
    includes.extend(exonum_add_protos);

    protobuf_generate(
        "src/proto",
        includes,
        "protobuf_mod.rs",
    );
}
