use crate::{Result, vm::Command};
use serde::Deserialize;
#[derive(Clone, Deserialize)]
pub struct Palette {
    current: Vec<u8>,
    target: Vec<u8>,
    dirty: u32,
}
impl Palette {
    pub fn current_bytes(&self) -> &[u8] {
        &self.current
    }
    pub fn validate(&self) -> Result<()> {
        if self.current.len() != 1024 || self.target.len() != 1024 {
            return Err("Invalid palette state extent".into());
        }
        Ok(())
    }
    pub fn load(&mut self, start: u8, colors: &[u8]) -> Result<()> {
        self.validate()?;
        if colors.is_empty() || !colors.len().is_multiple_of(3) || colors.len() > 768 {
            return Err("Invalid resource colors".into());
        }
        for (index, rgb) in colors
            .as_chunks::<3>()
            .0
            .iter()
            .take(256 - usize::from(start))
            .enumerate()
        {
            let at = (usize::from(start) + index) * 4;
            self.current[at..at + 3].copy_from_slice(rgb);
        }
        Ok(())
    }
    pub fn mark_dirty(&mut self) {
        self.dirty = 1;
    }
    pub fn apply(&mut self, command: &Command) -> Result<u32> {
        self.validate()?;
        let b = &command.bytes;
        let (start, count, length) = match b.as_slice() {
            [0x3b, start, count, _] => (*start, *count, 4),
            [0x38, 2, start, count, ..] if b.len() == 8 => (*start, *count, 8),
            [0x39, 1 | 2, start, count, ..] if b.len() == 8 => (*start, *count, 8),
            _ => return Err("Unsupported palette command".into()),
        };
        let indices = usize::from(start)..usize::from(start.wrapping_add(count));
        match b[0] {
            0x3b => {
                for i in indices {
                    self.target[i * 4..i * 4 + 3].copy_from_slice(&self.current[i * 4..i * 4 + 3]);
                }
            }
            0x39 => {
                for i in indices {
                    let palette = if b[1] == 1 {
                        &mut self.current
                    } else {
                        &mut self.target
                    };
                    palette[i * 4..i * 4 + 3].fill(b[4]);
                }
                if b[1] == 1 {
                    self.dirty = 1;
                }
            }
            0x38 => {
                for _ in 0..b[4] {
                    for i in indices.clone() {
                        for c in i * 4..i * 4 + 3 {
                            self.current[c] = match self.current[c].cmp(&self.target[c]) {
                                std::cmp::Ordering::Less => self.current[c] + 1,
                                std::cmp::Ordering::Greater => self.current[c] - 1,
                                std::cmp::Ordering::Equal => self.current[c],
                            };
                        }
                    }
                    self.dirty = 1;
                }
            }
            _ => unreachable!(),
        }
        Ok(length)
    }
}
