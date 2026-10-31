use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Cursor, Read},
};
pub type Result<T> = std::result::Result<T, String>;
pub const MAX_ARCHIVE_BYTES: usize = 128 * 1024 * 1024;
const MAX_ENTRIES: usize = 8192;
const MAX_EXPANDED_BYTES: u64 = 256 * 1024 * 1024;
#[derive(Deserialize, Serialize)]
pub struct InputManifest {
    pub schema: u32,
    pub id: String,
    pub scope: String,
    pub files: Vec<InputFile>,
}
#[derive(Deserialize, Serialize)]
pub struct InputFile {
    pub file: String,
    pub bytes: usize,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<InputVariant>,
}
#[derive(Deserialize, Serialize)]
pub struct InputVariant {
    pub file: String,
    pub bytes: usize,
    pub sha256: String,
}
impl InputFile {
    pub fn names(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.file.as_str()).chain(self.alternatives.iter().map(|v| v.file.as_str()))
    }
    pub fn accepts_size(&self, name: &str, size: usize) -> bool {
        (self.file == name && self.bytes == size)
            || self
                .alternatives
                .iter()
                .any(|v| v.file == name && v.bytes == size)
    }
    pub fn accepts(&self, name: &str, data: &[u8]) -> bool {
        if !self.accepts_size(name, data.len()) {
            return false;
        }
        let digest = hash(data);
        (self.file == name && self.bytes == data.len() && self.sha256 == digest)
            || self
                .alternatives
                .iter()
                .any(|v| v.file == name && v.bytes == data.len() && v.sha256 == digest)
    }
}

pub fn manifest() -> InputManifest {
    serde_json::from_slice(include_bytes!("../../port-build/inputs/kr-game.json"))
        .expect("authored KR input contract")
}
pub struct VerifiedInputs(BTreeMap<String, Vec<u8>>);
impl VerifiedInputs {
    pub fn files(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.0
    }
    pub fn into_files(self) -> BTreeMap<String, Vec<u8>> {
        self.0
    }
}
fn hash(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
pub fn verify_files(files: Vec<(String, Vec<u8>)>) -> Result<VerifiedInputs> {
    verify(files, &manifest())
}
fn verify(files: Vec<(String, Vec<u8>)>, contract: &InputManifest) -> Result<VerifiedInputs> {
    let mut candidates = BTreeMap::new();
    for (name, bytes) in files {
        let upper = name.to_ascii_uppercase();
        if let Some(expected) = contract
            .files
            .iter()
            .find(|f| f.names().any(|n| n == upper))
        {
            if candidates
                .insert(expected.file.clone(), (upper, bytes))
                .is_some()
            {
                return Err(format!("Ambiguous KR input {}", expected.file));
            }
        }
    }
    let mut snapshot = BTreeMap::new();
    for expected in &contract.files {
        let (name, bytes) = candidates
            .remove(&expected.file)
            .ok_or_else(|| format!("Missing KR input {}", expected.file))?;
        if !expected.accepts(&name, &bytes) {
            return Err(format!("KR input size or SHA-256 mismatch: {}", name));
        }
        if snapshot.insert(expected.file.clone(), bytes).is_some() {
            return Err("Duplicate contract entry".into());
        }
    }
    if contract.schema != 1 || snapshot.is_empty() {
        return Err("Unsupported input contract".into());
    }
    Ok(VerifiedInputs(snapshot))
}
pub fn read_zip(bytes: &[u8]) -> Result<VerifiedInputs> {
    zip_with_contract(bytes, &manifest())
}
fn zip_with_contract(bytes: &[u8], contract: &InputManifest) -> Result<VerifiedInputs> {
    if bytes.len() > MAX_ARCHIVE_BYTES {
        return Err("ZIP exceeds 128 MiB".into());
    }
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("Invalid ZIP: {e}"))?;
    if archive.len() > MAX_ENTRIES {
        return Err("ZIP exceeds 8192 entries".into());
    }
    let mut files = Vec::new();
    let mut total = 0_u64;
    let mut game_parent: Option<String> = None;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Unreadable ZIP entry: {e}"))?;
        let name = entry.name().replace('\\', "/");
        let parts: Vec<_> = name.split('/').collect();
        if name.starts_with('/')
            || name.contains(':')
            || name.contains('\0')
            || parts.iter().any(|p| *p == ".." || *p == ".")
        {
            return Err(format!("Unsafe ZIP entry path: {name}"));
        }
        if entry.is_symlink() {
            return Err(format!("ZIP symlink not supported: {name}"));
        }
        total = total.checked_add(entry.size()).ok_or("ZIP size overflow")?;
        if total > MAX_EXPANDED_BYTES {
            return Err("ZIP expanded size exceeds 256 MiB".into());
        }
        if entry.is_dir() {
            continue;
        }
        let basename = parts
            .last()
            .ok_or("Empty ZIP filename")?
            .to_ascii_uppercase();
        let Some(expected) = contract
            .files
            .iter()
            .find(|f| f.names().any(|n| n == basename))
        else {
            continue;
        };
        let parent = parts[..parts.len() - 1].join("/");
        if game_parent.as_ref().is_some_and(|p| p != &parent) {
            return Err(
                "Required KR files must be in one game folder; mixed or duplicate folders found"
                    .into(),
            );
        }
        game_parent = Some(parent);
        if !expected.accepts_size(
            &basename,
            usize::try_from(entry.size()).map_err(|_| "ZIP entry size overflow")?,
        ) {
            return Err(format!("KR input size mismatch: {basename}"));
        }
        let size = entry.size();
        let mut data = Vec::with_capacity(size as usize);
        (&mut entry)
            .take(size + 1)
            .read_to_end(&mut data)
            .map_err(|e| format!("Invalid ZIP payload {basename}: {e}"))?;
        files.push((basename, data));
    }
    verify(files, contract)
}
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn inspect_zip(bytes: &[u8]) -> Result<String> {
    let inputs = read_zip(bytes)?;
    Ok(serde_json::json!(
        { "validated" : true, "files" : inputs.files().len(), "contract" : manifest()
        .id, "kr_only_rom_ready" : false }
    )
    .to_string())
}
