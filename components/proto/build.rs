extern crate exonum_build;

use exonum_build::{get_exonum_protobuf_files_path, protobuf_generate};

fn main() {
    protobuf_generate("src/proto", &["src/proto"], "protobuf_mod.rs");
}
