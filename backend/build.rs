#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "build script はビルド失敗を panic で伝えるのが cargo の想定する流儀"
)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let migration_source_hash = hash_migration_sources();
    println!("cargo:rustc-env=MIGRATION_SOURCE_HASH={migration_source_hash}");

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

fn hash_migration_sources() -> String {
    let root = Path::new("migration/src");
    let mut files = Vec::new();
    collect_migration_sources(root, &mut files);
    files.sort();

    let mut hash = 0x6c62272e07bb014262b821756295c58d_u128;
    for file in files {
        let relative_path = file
            .strip_prefix(root)
            .expect("migration source must be under migration/src");
        let relative_path = relative_path
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        hash = fnv1a_128(hash, relative_path.as_bytes());
        hash = fnv1a_128(hash, &[0]);
        let contents = fs::read(&file)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", file.display()));
        hash = fnv1a_128(hash, &contents);
        hash = fnv1a_128(hash, &[0xff]);
    }

    format!("{hash:032x}")
}

fn collect_migration_sources(directory: &Path, files: &mut Vec<PathBuf>) {
    println!("cargo:rerun-if-changed={}", directory.display());

    let entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("failed to read migration entry: {error}"));
        let path = entry.path();
        let file_type = entry
            .file_type()
            .unwrap_or_else(|error| panic!("failed to inspect {}: {error}", path.display()));
        if file_type.is_dir() {
            collect_migration_sources(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            println!("cargo:rerun-if-changed={}", path.display());
            files.push(path);
        }
    }
}

fn fnv1a_128(mut hash: u128, bytes: &[u8]) -> u128 {
    const PRIME: u128 = 0x0000000001000000000000000000013b;

    for byte in bytes {
        hash ^= u128::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}
