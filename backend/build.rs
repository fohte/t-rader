#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "build script はビルド失敗を panic で伝えるのが cargo の想定する流儀"
)]

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let src = "../agent/openapi.json";
    println!("cargo:rerun-if-changed={src}");

    let file = fs::File::open(src).unwrap_or_else(|e| panic!("failed to open {src}: {e}"));
    let spec = serde_json::from_reader(file)
        .unwrap_or_else(|e| panic!("failed to parse {src} as OpenAPI document: {e}"));

    let mut generator = progenitor::Generator::default();
    let tokens = generator
        .generate_tokens(&spec)
        .unwrap_or_else(|e| panic!("failed to generate agent client from {src}: {e}"));
    let ast = syn::parse2(tokens).expect("generated agent client tokens must parse as Rust");
    let content = prettyplease::unparse(&ast);

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is set by cargo during build");
    let out_file = Path::new(&out_dir).join("agent_internal_api_client.rs");
    fs::write(&out_file, content)
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", out_file.display()));
}
