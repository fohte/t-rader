use std::{fs, path::PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

pub(crate) fn write_json<T: Serialize>(path: &PathBuf, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|_| "manifest output directory could not be created".to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|_| "manifest output could not be serialized".to_string())?;
    fs::write(path, bytes).map_err(|_| "manifest output could not be written".to_string())
}

pub(crate) fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn hash_serialized<T: Serialize + ?Sized>(value: &T) -> Result<String, String> {
    let bytes =
        serde_json::to_vec(value).map_err(|_| "canonical hash serialization failed".to_string())?;
    Ok(sha256(&bytes))
}
