use crate::errors::FileError;
use crate::identifier::Name;
use crate::path::{ParentPathBuf, SourcePathBuf};
use crate::sound_effects::{SfxFlags, SfxSubroutinesMml, SoundEffectInput, SoundEffectText};
use crate::textfile::{load_text_file_with_limit, TextFile};
use std::path::PathBuf;
const NEW_SFX_TOKEN_NO_NEWLINE: &str = "===";
const NEW_SFX_TOKEN: &str = "\n===";
const COMMENTED_NEW_SFX_TOKEN: &str = "\n;===";
const OLD_MML_SFX_IDENTIFIER: &str = "MML\n";
const OLD_MML_SFX_IDENTIFIER_NO_NEWLINE: &str = "MML";
const MML_ATTR: &str = "mml";
const INTERRUPTIBLE_ATTR: &str = "interruptible";
const UNINTERRUPTIBLE_ATTR: &str = "uninterruptible";
const ONE_CHANNEL_ATTR: &str = "one";
const ONE_CHANNEL_FALSE_ATTR: &str = "both";
#[derive(Debug, PartialEq)]
pub struct SoundEffectFileSfx {
    pub name: String,
    pub line_no: u32,
    pub flags: SfxFlags,
    pub sfx: SoundEffectText,
}
#[derive(Debug, PartialEq)]
pub struct SoundEffectsFile {
    pub path: Option<PathBuf>,
    pub file_name: String,
    pub subroutines: SfxSubroutinesMml,
    pub sound_effects: Vec<SoundEffectFileSfx>,
}
pub fn convert_sfx_inputs_lossy(sound_effects: Vec<SoundEffectFileSfx>) -> Vec<SoundEffectInput> {
    sound_effects
        .into_iter()
        .map(|s| SoundEffectInput {
            name: Name::new_lossy(s.name),
            flags: s.flags,
            sfx: s.sfx,
        })
        .collect()
}
fn count_lines_including_end(s: &str) -> usize {
    if s.is_empty() {
        return 0;
    }
    s.bytes().filter(|&c| c == b'\n').count() + 1
}
fn sfx_file_from_text_file(tf: TextFile) -> SoundEffectsFile {
    let input = tf.contents;
    let (header, after_header) = match input.strip_prefix(NEW_SFX_TOKEN_NO_NEWLINE) {
        Some(after_prefix) => (String::new(), after_prefix),
        None => match input.split_once(NEW_SFX_TOKEN) {
            Some((header, after_header)) => (header.to_owned(), after_header),
            None => (input, ""),
        },
    };
    let mut sound_effects = Vec::new();
    let mut line_no = count_lines_including_end(&header) + 1;
    for sfx_block in after_header.split(NEW_SFX_TOKEN) {
        let (name_and_attrs, sfx_lines) = match sfx_block.split_once('\n') {
            Some((a, b)) => (a, b),
            None => (sfx_block, ""),
        };
        let mut flags = SfxFlags::default();
        let mut attrs = name_and_attrs
            .trim_matches('=')
            .split(|c: char| c.is_whitespace() || c == '=')
            .filter(|s| !s.is_empty());
        let mut mml_sfx = false;
        let mut name = match attrs.next() {
            Some(s) => s.to_owned(),
            None => String::new(),
        };
        for a in attrs {
            if a.eq_ignore_ascii_case(MML_ATTR) {
                mml_sfx = true;
            } else if a.eq_ignore_ascii_case(INTERRUPTIBLE_ATTR) {
                flags.interruptible = Some(true)
            } else if a.eq_ignore_ascii_case(UNINTERRUPTIBLE_ATTR) {
                flags.interruptible = Some(false)
            } else if a.eq_ignore_ascii_case(ONE_CHANNEL_ATTR) {
                flags.one_channel = Some(true)
            } else if a.eq_ignore_ascii_case(ONE_CHANNEL_FALSE_ATTR) {
                flags.one_channel = Some(false)
            } else {
                name.push(' ');
                name.push_str(a);
            }
        }
        let (sfx_lines, mml_sfx) = if mml_sfx {
            (sfx_lines.to_owned(), true)
        } else {
            match sfx_lines.strip_prefix(OLD_MML_SFX_IDENTIFIER) {
                Some(s) => (s.to_owned(), true),
                None => match sfx_lines {
                    OLD_MML_SFX_IDENTIFIER_NO_NEWLINE => (String::new(), true),
                    _ => (sfx_lines.to_owned(), false),
                },
            }
        };
        sound_effects.push(SoundEffectFileSfx {
            name,
            line_no: line_no.try_into().unwrap(),
            flags,
            sfx: match mml_sfx {
                true => SoundEffectText::Mml(sfx_lines),
                false => SoundEffectText::BytecodeAssembly(sfx_lines),
            },
        });
        line_no += count_lines_including_end(sfx_block);
    }
    SoundEffectsFile {
        path: tf.path,
        file_name: tf.file_name,
        subroutines: SfxSubroutinesMml(header),
        sound_effects,
    }
}
pub fn load_sound_effects_file(
    source: &SourcePathBuf,
    parent_path: &ParentPathBuf,
) -> Result<SoundEffectsFile, FileError> {
    let text_file = load_text_file_with_limit(source, parent_path)?;
    Ok(sfx_file_from_text_file(text_file))
}
pub fn build_sound_effects_file<'a>(
    subroutines: &'a SfxSubroutinesMml,
    sound_effects: impl Iterator<Item = &'a SoundEffectInput>,
) -> String {
    let mut out = String::with_capacity(32 * 1024);
    out.push_str(&subroutines.0);
    for (i, sfx) in sound_effects.enumerate() {
        let (sfx_lines, is_mml) = match &sfx.sfx {
            SoundEffectText::BytecodeAssembly(s) => (s, false),
            SoundEffectText::Mml(s) => (s, true),
        };
        let sfx_lines = sfx_lines.replace(NEW_SFX_TOKEN, COMMENTED_NEW_SFX_TOKEN);
        if i == 0 && subroutines.0.is_empty() {
            out.push_str("=== ");
        } else {
            out.push_str("\n=== ");
        }
        out.push_str(sfx.name.as_str());
        out.push_str(" ===");
        let mut add_attr = |attr| {
            out.push(' ');
            out.push_str(attr);
        };
        if is_mml {
            add_attr(MML_ATTR);
        }
        match sfx.flags.one_channel {
            None => {}
            Some(true) => add_attr(ONE_CHANNEL_ATTR),
            Some(false) => add_attr(ONE_CHANNEL_FALSE_ATTR),
        }
        match sfx.flags.interruptible {
            None => {}
            Some(true) => add_attr(INTERRUPTIBLE_ATTR),
            Some(false) => add_attr(UNINTERRUPTIBLE_ATTR),
        }
        out.push('\n');
        out.push_str(&sfx_lines);
    }
    out
}
