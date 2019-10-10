// Copyright 2019 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! This crate simplifies writing build.rs for exonum and exonum services.

#![deny(unsafe_code, bare_trait_objects)]
#![warn(missing_docs, missing_debug_implementations)]

use proc_macro2::{Ident, Span};
use protoc_rust::Customize;
use quote::{quote, ToTokens};
use walkdir::WalkDir;

use std::{
    env,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// Enum represents various sources of protobuf files.
#[derive(Debug, Copy, Clone)]
pub enum ProtoSources<'a> {
    /// Path to exonum core protobuf files.
    Exonum,
    /// Path to exonum crypto protobuf files.
    Crypto,
    /// Path to common protobuf files.
    Common,
    /// Path to manually specified protobuf sources.
    Path(&'a str),
}

impl<'a> ProtoSources<'a> {
    /// Returns path to protobuf files.
    pub fn path(&self) -> String {
        match self {
            ProtoSources::Exonum => get_exonum_protobuf_files_path(),
            ProtoSources::Common => get_exonum_protobuf_common_files_path(),
            ProtoSources::Crypto => get_exonum_protobuf_crypto_files_path(),
            ProtoSources::Path(path) => path.to_string(),
        }
    }

    /// Most frequently used combination of proto dependencies.
    /// TODO: maybe find a better name.
    pub fn frequently_used() -> Vec<Self> {
        vec![
            ProtoSources::Exonum,
            ProtoSources::Crypto,
            ProtoSources::Path("src/proto"),
        ]
    }
}

impl<'a> From<&'a str> for ProtoSources<'a> {
    fn from(path: &'a str) -> Self {
        ProtoSources::Path(path)
    }
}

/// Finds all .proto files in `path` and subfolders and returns a vector of their paths.
fn get_proto_files<P: AsRef<Path>>(path: &P) -> Vec<PathBuf> {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| {
            let e = e.ok()?;
            if e.path().extension()?.to_str() == Some("proto") {
                Some(e.path().into())
            } else {
                None
            }
        })
        .collect()
}

/// Includes all .proto files with their names into generated file as array of tuples,
/// where tuple content is (file_name, file_content).
fn include_proto_files(proto_files: &[PathBuf]) -> impl ToTokens {
    let proto_files_len = proto_files.len();
    // TODO Think about syn crate and token streams instead of dirty strings.
    let proto_files = proto_files.iter().map(|path| {
        let name = path
            .file_name()
            .unwrap()
            .to_str()
            .expect(".proto file name is not convertible to &str");

        let mut content = String::new();
        File::open(path)
            .expect("Unable to open .proto file")
            .read_to_string(&mut content)
            .expect("Unable to read .proto file");

        quote! {
            (#name, #content),
        }
    });

    quote! {
        /// Original proto files which were be used to generate this module.
        /// First element in tuple is file name, second is proto file content.
        #[allow(dead_code)]
        pub const PROTO_SOURCES: [(&str, &str); #proto_files_len] = [
            #( #proto_files )*
        ];
    }
}

/// Collects .rs files generated by the rust-protobuf into single module.
///
/// - If module name is `tests` it adds `#[cfg(test)]` to declaration.
/// - Also this method includes source files as `PROTO_SOURCES` constant.
fn generate_mod_rs<P: AsRef<Path>, Q: AsRef<Path>>(
    out_dir: P,
    proto_files: &[PathBuf],
    mod_file: Q,
) {
    let mod_files = {
        proto_files.iter().map(|f| {
            let mod_name = f
                .file_stem()
                .unwrap()
                .to_str()
                .expect(".proto file name is not convertible to &str");

            let mod_name = Ident::new(mod_name, Span::call_site());
            if mod_name == "tests" {
                quote! {
                    #[cfg(test)] pub mod #mod_name;
                }
            } else {
                quote! {
                    pub mod #mod_name;
                }
            }
        })
    };
    let proto_files = include_proto_files(proto_files);

    let content = quote! {
        #( #mod_files )*
        #proto_files
    };

    let dest_path = out_dir.as_ref().join(mod_file);
    let mut file = File::create(dest_path).expect("Unable to create output file");
    file.write_all(content.into_token_stream().to_string().as_bytes())
        .expect("Unable to write data to file");
}

///TODO: add doc
#[derive(Debug)]
pub struct ProtobufGenerator<'a> {
    sources: Vec<ProtoSources<'a>>,
    mod_name: &'a str,
    input_dir: &'a str,
}

impl<'a> ProtobufGenerator<'a> {
    ///TODO: add doc
    pub fn with_mod_name(mod_name: &'a str) -> Self {
        assert!(!mod_name.is_empty(), "Mod name is not specified");
        Self {
            sources: Vec::new(),
            input_dir: "",
            mod_name,
        }
    }

    ///TODO: add doc
    pub fn input_dir(mut self, path: &'a str) -> Self {
        self.input_dir = path;
        self
    }

    ///TODO: add doc
    pub fn add_path(mut self, path: &'a str) -> Self {
        self.sources.push(ProtoSources::Path(path));
        self
    }

    ///TODO: add doc / maybe find the better name
    pub fn frequently_used(mut self) -> Self {
        self.sources.extend(ProtoSources::frequently_used());
        self
    }

    ///TODO: add doc
    pub fn common(mut self) -> Self {
        self.sources.push(ProtoSources::Common);
        self
    }

    ///TODO: add doc
    pub fn crypto(mut self) -> Self {
        self.sources.push(ProtoSources::Crypto);
        self
    }

    ///TODO: add doc
    pub fn exonum(mut self) -> Self {
        self.sources.push(ProtoSources::Exonum);
        self
    }

    ///TODO: add doc / maybe find the better name
    pub fn includes(mut self, includes: &'a [ProtoSources]) -> Self {
        self.sources.extend_from_slice(includes);
        self
    }

    ///TODO: add doc
    pub fn generate(self) {
        assert!(!self.input_dir.is_empty(), "Input dir is not specified");
        assert!(!self.sources.is_empty(), "Includes are not specified");
        protobuf_generate(self.input_dir, &self.sources, self.mod_name);
    }
}

/// Generates .rs files from .proto files.
///
/// `protoc` executable from protobuf should be in `$PATH`
///
/// # Examples
///
/// In `build.rs`
/// ```no_run
/// extern crate exonum_build;
///
/// use exonum_build::protobuf_generate;
///
/// // Includes usually should contain input_dir.
/// protobuf_generate("src/proto", &["src/proto"], "example_mod.rs")
/// ```
/// After successful run `$OUT_DIR` will contain \*.rs for each \*.proto file in
/// "src/proto/\*\*/" and example_mod.rs which will include all generated .rs files
/// as submodules.
///
/// To use generated protobuf structs.
///
/// In `src/proto/mod.rs`
/// ```ignore
/// extern crate exonum;
///
/// include!(concat!(env!("OUT_DIR"), "/example_mod.rs"));
///
/// // If you use types from `exonum` .proto files.
/// use exonum::proto::schema::*;
/// ```
fn protobuf_generate<P, T>(input_dir: P, includes: &[ProtoSources], mod_file_name: T)
where
    P: AsRef<Path>,
    T: AsRef<str>,
{
    let out_dir = env::var("OUT_DIR")
        .map(PathBuf::from)
        .expect("Unable to get OUT_DIR");

    let proto_files = get_proto_files(&input_dir);
    generate_mod_rs(&out_dir, &proto_files, &mod_file_name.as_ref());

    let includes = includes.iter().collect::<Vec<_>>();
    // Converts paths to strings and adds input dir to includes.
    let mut includes = includes.iter().map(|s| s.path()).collect::<Vec<_>>();
    includes.push(
        input_dir
            .as_ref()
            .to_str()
            .expect("Input dir name is not convertible to &str")
            .into(),
    );

    let includes: Vec<&str> = includes.iter().map(|s| &**s).collect();

    protoc_rust::run(protoc_rust::Args {
        out_dir: out_dir
            .to_str()
            .expect("Out dir name is not convertible to &str"),
        input: &proto_files
            .iter()
            .map(|s| s.to_str().expect("File name is not convertible to &str"))
            .collect::<Vec<_>>(),
        includes: &includes,
        customize: Customize {
            serde_derive: Some(true),
            ..Default::default()
        },
    })
    .expect("protoc");
}

/// Get path to the folder containing `exonum` protobuf files.
///
/// Needed for code generation of .proto files which import `exonum` provided .proto files.
///
/// # Examples
///
/// ```no_run
/// extern crate exonum_build;
///
/// use exonum_build::{protobuf_generate, get_exonum_protobuf_files_path};
///
/// let exonum_protos = get_exonum_protobuf_files_path();
/// protobuf_generate(
///    "src/proto",
///    &["src/proto", &exonum_protos],
///    "protobuf_mod.rs",
/// );
/// ```
fn get_exonum_protobuf_files_path() -> String {
    env::var("DEP_EXONUM_PROTOBUF_PROTOS").expect("Failed to get exonum protobuf path")
}

/// Get path to the folder containing `exonum-crypto` protobuf files.
fn get_exonum_protobuf_crypto_files_path() -> String {
    env::var("DEP_EXONUM_PROTOBUF_CRYPTO_PROTOS")
        .expect("Failed to get exonum crypto protobuf path")
}

/// Get path to the folder containing `exonum-proto` protobuf files.
fn get_exonum_protobuf_common_files_path() -> String {
    env::var("DEP_EXONUM_PROTOBUF_COMMON_PROTOS")
        .expect("Failed to get exonum common protobuf path")
}
