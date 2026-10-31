use crate::Result;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

type Project = (String, String, BTreeSet<&'static str>);

pub(super) fn expand(config: &Value, projects: &mut [Project]) -> Result<()> {
    let mut assigned = BTreeMap::new();
    for group in config.as_array().ok_or("Missing instrument banks")? {
        let names = group.as_array().ok_or("Invalid instrument bank")?;
        if names.is_empty() {
            return Err("Empty instrument bank".into());
        }
        let mut instruments = BTreeSet::new();
        for name in names {
            let name = name.as_str().ok_or("Invalid bank song")?;
            let project = projects
                .iter()
                .find(|p| p.0 == name)
                .ok_or("Unknown bank song")?;
            instruments.extend(project.2.iter().copied());
        }
        for name in names {
            if assigned
                .insert(name.as_str().unwrap(), instruments.clone())
                .is_some()
            {
                return Err("Song assigned to multiple instrument banks".into());
            }
        }
    }
    for (name, _, instruments) in projects {
        *instruments = assigned
            .remove(name.as_str())
            .ok_or("Song has no instrument bank")?;
    }
    Ok(())
}
