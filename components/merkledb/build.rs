extern crate exonum_build;

use exonum_build::{get_exonum_protobuf_files_path, protobuf_generate, get_exonum_protobuf_deps_files_path};

fn main() {
    protobuf_generate("src/proto", &["src/proto", "../crypto/src/proto"], "protobuf_mod.rs");
}
