use crate::{Result, original::Originals};
use serde_json::Value;
use std::path::{Path, PathBuf};
pub type Files = Vec<(PathBuf, Vec<u8>)>;
type Importer = fn(&Originals) -> Result<Files>;
const IMPORTERS: &[(&str, Importer)] = &[
    ("sheets", crate::import_sheets::import),
    ("screens", crate::import_screens::import),
    ("world", crate::import_world::import),
    ("field", crate::import_field::import),
    ("prologue", crate::import_prologue::import),
    ("portrait", crate::import_portrait::import),
    ("scenes", crate::import_scenes::import),
    ("dialogue", crate::import_dialogue::import),
    ("items", crate::import_items::import),
    ("menu", crate::import_menu::import),
    ("maps", crate::import_maps::import),
    ("floor objects", crate::import_floor_objects::import),
    ("battle", crate::import_battle::import),
    ("combat", crate::import_combat::import),
    ("battle scripts", crate::import_battle::battle_scripts),
    ("records", crate::import_records::import),
];
pub fn run(originals: &Originals) -> Result<Files> {
    run_except(originals, &[])
}
pub fn run_except(originals: &Originals, skip: &[&str]) -> Result<Files> {
    let mut files = Vec::new();
    for (area, importer) in IMPORTERS.iter().filter(|(a, _)| !skip.contains(a)) {
        files.extend(importer(originals).map_err(|e| format!("Importing {area}: {e}"))?);
    }
    Ok(files)
}
pub fn split(files: Files) -> (Files, Files) {
    let (records, content): (Files, Files) = files
        .into_iter()
        .partition(|(name, _)| name.starts_with("records"));
    let records = records
        .into_iter()
        .map(|(name, bytes)| (name.strip_prefix("records").unwrap().to_path_buf(), bytes))
        .collect();
    (records, content)
}
