use crate::Result;
pub use hachamecha_input::InputManifest;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};
pub fn manifest() -> Result<InputManifest> {
    Ok(hachamecha_input::manifest())
}
pub fn read_kr(game: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    Ok(snapshot_kr(game)?.into_files())
}
pub fn snapshot_kr(game: &Path) -> Result<hachamecha_input::VerifiedInputs> {
    if fs::symlink_metadata(game)?.file_type().is_file() {
        if fs::metadata(game)?.len() > hachamecha_input::MAX_ARCHIVE_BYTES as u64 {
            return Err("ZIP exceeds 128 MiB".into());
        }
        let data = fs::read(game)?;
        return Ok(hachamecha_input::read_zip(&data)?);
    }
    let files = read_verified(game, &manifest()?)?;
    Ok(hachamecha_input::verify_files(files.into_iter().collect())?)
}
fn read_verified(game: &Path, manifest: &InputManifest) -> Result<BTreeMap<String, Vec<u8>>> {
    if manifest.schema != 1 || manifest.files.is_empty() {
        return Err("Unsupported or empty game input contract".into());
    }
    let mut names: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    for entry in fs::read_dir(game)? {
        let entry = entry?;
        if let Some(name) = entry.file_name().to_str() {
            names
                .entry(name.to_ascii_uppercase())
                .or_default()
                .push(entry.path());
        }
    }
    let mut verified = BTreeMap::new();
    for file in &manifest.files {
        let candidates: Vec<_> = file
            .names()
            .flat_map(|name| {
                names
                    .get(name)
                    .into_iter()
                    .flatten()
                    .map(move |path| (name, path))
            })
            .collect();
        let [(name, path)] = candidates.as_slice() else {
            return Err(format!(
                "Expected one KR input {}",
                file.names().collect::<Vec<_>>().join(" / ")
            )
            .into());
        };
        if !fs::symlink_metadata(path)?.file_type().is_file() {
            return Err(format!("KR input must be a regular file, not a link: {name}").into());
        }
        let size = usize::try_from(fs::metadata(path)?.len())?;
        if !file.accepts_size(name, size) {
            return Err(format!("KR input size mismatch: {name}").into());
        }
        let bytes = fs::read(path)?;
        if !file.accepts(name, &bytes) {
            return Err(format!("KR input SHA-256 mismatch: {name}").into());
        }
        if verified.insert((*name).to_owned(), bytes).is_some() {
            return Err("Duplicate input contract entry".into());
        }
    }
    Ok(verified)
}
pub struct BuildPaths {
    pub game: PathBuf,
    pub audio: PathBuf,
    pub output: PathBuf,
    pub font: PathBuf,
    pub dialogue_font: PathBuf,
}
pub struct BuildOptions {
    pub game: PathBuf,
    pub audio: PathBuf,
    pub output: PathBuf,
    pub font: PathBuf,
    pub dialogue_font: PathBuf,
}
impl BuildOptions {
    pub fn parse(args: &[OsString]) -> Result<Self> {
        if !args.len().is_multiple_of(2) {
            return Err(
                "Expected --game DIR_OR_ZIP --audio DIR --output DIR --font FILE --dialogue-font FILE"
                    .into(),
            );
        }
        let paths = BuildPaths::parse(args)?;
        let output = resolved_output(&paths.output)?;
        if output.starts_with(paths.audio.canonicalize()?) {
            return Err("Output must be separate from the audio bundle".into());
        }
        Ok(Self {
            game: paths.game,
            audio: paths.audio,
            output: paths.output,
            font: paths.font,
            dialogue_font: paths.dialogue_font,
        })
    }
}
impl BuildPaths {
    pub fn parse(args: &[OsString]) -> Result<Self> {
        let mut options = BTreeMap::new();
        if !args.len().is_multiple_of(2) {
            return Err(
                "Expected --game DIR --audio FILE --output DIR --font FILE --dialogue-font FILE"
                    .into(),
            );
        }
        for pair in args.as_chunks::<2>().0 {
            let key = pair[0].to_str().ok_or("Invalid option name")?;
            if !["--game", "--audio", "--output", "--font", "--dialogue-font"].contains(&key)
                || options.insert(key, PathBuf::from(&pair[1])).is_some()
            {
                return Err(format!("Unknown or repeated option {key}").into());
            }
        }
        let mut take = |key| {
            options
                .remove(key)
                .ok_or_else(|| format!("Missing required option {key}"))
        };
        let result = Self {
            game: take("--game")?,
            audio: take("--audio")?,
            output: take("--output")?,
            font: take("--font")?,
            dialogue_font: take("--dialogue-font")?,
        };
        result.validate_output()?;
        Ok(result)
    }
    fn validate_output(&self) -> Result<()> {
        let output = resolved_output(&self.output)?;
        let game = self.game.canonicalize()?;
        if output.starts_with(&game) || game.starts_with(&output) {
            return Err("Output must be separate from the original game directory".into());
        }
        for input in [&self.audio, &self.font, &self.dialogue_font] {
            if input.canonicalize()?.starts_with(&output) {
                return Err("Output would replace a required input".into());
            }
        }
        Ok(())
    }
}
fn resolved_output(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        Ok(path.canonicalize()?)
    } else {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        Ok(parent
            .canonicalize()?
            .join(path.file_name().ok_or("Output directory name")?))
    }
}
