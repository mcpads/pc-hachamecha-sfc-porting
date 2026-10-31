use crate::{Manifest, Result, SOURCE, sha};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
};
#[derive(Clone)]
pub struct Content {
    layers: Arc<Layers>,
    prefix: PathBuf,
}
struct Layers {
    directory: Option<PathBuf>,
    generated: BTreeMap<PathBuf, Vec<u8>>,
}
impl Content {
    pub fn new(directory: Option<&Path>, generated: Vec<(PathBuf, Vec<u8>)>) -> Result<Content> {
        let mut files = BTreeMap::new();
        for (name, bytes) in generated {
            if files.insert(relative(&name)?, bytes).is_some() {
                return Err(format!("Duplicate generated input {}", name.display()).into());
            }
        }
        Ok(Content {
            layers: Arc::new(Layers {
                directory: directory.map(Path::to_path_buf),
                generated: files,
            }),
            prefix: PathBuf::new(),
        })
    }
    pub fn directory(directory: &Path) -> Content {
        Content::new(Some(directory), Vec::new()).expect("no generated inputs")
    }
    pub fn sub(&self, name: impl AsRef<Path>) -> Content {
        Content {
            layers: self.layers.clone(),
            prefix: self.prefix.join(name),
        }
    }
    pub fn read(&self, name: impl AsRef<Path>) -> Result<Vec<u8>> {
        let path = relative(&self.prefix.join(name))?;
        if let Some(bytes) = self.layers.generated.get(&path) {
            return Ok(bytes.clone());
        }
        let directory = self
            .layers
            .directory
            .as_ref()
            .ok_or_else(|| format!("Missing build input {}", path.display()))?;
        fs::read(directory.join(&path))
            .map_err(|e| format!("Build input {}: {e}", path.display()).into())
    }
    pub fn json<T: serde::de::DeserializeOwned>(&self, name: impl AsRef<Path>) -> Result<T> {
        Ok(serde_json::from_slice(&self.read(name)?)?)
    }
}
fn relative(path: &Path) -> Result<PathBuf> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => {
                return Err(
                    format!("Build input outside the content root: {}", path.display()).into(),
                );
            }
        }
    }
    if out.as_os_str().is_empty() {
        return Err("Empty build input name".into());
    }
    Ok(out)
}
pub fn load(root: &Content, manifest: &mut Manifest) -> Result<BTreeMap<String, String>> {
    let mut hashes = BTreeMap::new();
    let mut read = |name: &str| -> Result<serde_json::Value> {
        let bytes = root.read(name)?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)?;
        if value["schema"] != 1 || value["source_sha256"] != SOURCE {
            return Err(format!("Unsupported content definition: {name}").into());
        }
        hashes.insert(name.to_string(), sha(&bytes));
        Ok(value)
    };
    let b1 = read("b1-dialogue.json")?;
    let prologue = read("prologue-dialogue.json")?;
    let prologue_messages: Vec<crate::messages::Message> =
        serde_json::from_value(prologue["messages"].clone())?;
    crate::messages::validate_prologue_controls(&manifest.messages, &prologue_messages)?;
    manifest.messages = serde_json::from_value(b1["messages"].clone())?;
    manifest.messages.extend(prologue_messages);
    let images = read("images.json")?;
    manifest.resources = serde_json::from_value(images["resources"].clone())?;
    manifest.images = serde_json::from_value(images["images"].clone())?;
    manifest.objects = serde_json::from_value(read("objects.json")?["objects"].clone())?;
    Ok(hashes)
}
