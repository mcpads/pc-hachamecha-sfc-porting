use crate::{Result, SOURCE, sha};
use std::{collections::BTreeMap, path::Path, sync::OnceLock};
pub struct Originals {
    files: BTreeMap<String, Vec<u8>>,
    exe: OnceLock<Exe>,
}
impl Originals {
    pub fn kr_only(game: &Path) -> Result<Self> {
        Ok(Self {
            files: crate::package_inputs::snapshot_kr(game)?.into_files(),
            exe: OnceLock::new(),
        })
    }
    pub fn exe(&self) -> Result<&Exe> {
        if let Some(exe) = self.exe.get() {
            return Ok(exe);
        }
        let bytes = self.file("MADO1.EXE")?;
        if sha(&bytes) != SOURCE && sha(&bytes) != crate::executable_layout::ORIGINAL_SHA256 {
            return Err("Unsupported MADO1.EXE".into());
        }
        let parsed = Exe::parse(&bytes)?;
        Ok(self.exe.get_or_init(|| parsed))
    }
    pub fn file(&self, name: &str) -> Result<Vec<u8>> {
        self.files
            .get(name)
            .cloned()
            .ok_or_else(|| format!("File not declared by input contract: {name}").into())
    }
    pub fn cns(&self, name: &str) -> Result<crate::cns::Sheet> {
        if !name.ends_with(".CNS") {
            return Err("Expected CNS file".into());
        }
        crate::cns::Sheet::decode(&self.file(name)?)
    }
}
pub struct Exe {
    base: u32,
    image: Vec<u8>,
    mapped_ranges: Option<Vec<[usize; 2]>>,
}
impl Exe {
    pub fn parse(bytes: &[u8]) -> Result<Exe> {
        let u16_at = |at: usize| -> Result<u16> {
            Ok(u16::from_le_bytes(
                bytes.get(at..at + 2).ok_or("Truncated PE")?.try_into()?,
            ))
        };
        let u32_at = |at: usize| -> Result<u32> {
            Ok(u32::from_le_bytes(
                bytes.get(at..at + 4).ok_or("Truncated PE")?.try_into()?,
            ))
        };
        if bytes.get(..2) != Some(b"MZ") {
            return Err("Not an MZ executable".into());
        }
        let pe = u32_at(0x3c)? as usize;
        if bytes.get(pe..pe + 4) != Some(b"PE\0\0") {
            return Err("Not a PE executable".into());
        }
        let sections = usize::from(u16_at(pe + 6)?);
        let optional = pe + 24;
        if u16_at(optional)? != 0x10b {
            return Err("Not a PE32 executable".into());
        }
        let base = u32_at(optional + 28)?;
        let size = u32_at(optional + 56)? as usize;
        let headers = u32_at(optional + 60)? as usize;
        let mut image = vec![0; size];
        let header_bytes = bytes
            .get(..headers.min(size))
            .ok_or("Truncated PE headers")?;
        image[..header_bytes.len()].copy_from_slice(header_bytes);
        let table = optional + usize::from(u16_at(pe + 20)?);
        for i in 0..sections {
            let at = table + i * 40;
            let address = u32_at(at + 12)? as usize;
            let raw_size = u32_at(at + 16)? as usize;
            let raw = u32_at(at + 20)? as usize;
            let length = raw_size.min(size.saturating_sub(address));
            let data = bytes
                .get(raw..raw + length.min(bytes.len().saturating_sub(raw)))
                .ok_or("Truncated PE section")?;
            image
                .get_mut(address..address + data.len())
                .ok_or("PE section outside the image")?
                .copy_from_slice(data);
        }
        let mapped_ranges = if sha(bytes) == crate::executable_layout::ORIGINAL_SHA256 {
            let mapped = crate::executable_layout::normalize(&image)?;
            image = mapped.image;
            Some(mapped.ranges)
        } else {
            None
        };
        Ok(Exe {
            base,
            image,
            mapped_ranges,
        })
    }
    pub fn read(&self, va: u32, length: usize) -> Result<&[u8]> {
        let at = va
            .checked_sub(self.base)
            .ok_or_else(|| format!("Address {va:#x} below the image"))? as usize;
        let end = at.checked_add(length).ok_or("Executable read overflow")?;
        if let Some(ranges) = &self.mapped_ranges {
            let mut covered = at;
            for &[lo, hi] in ranges {
                if hi <= covered {
                    continue;
                }
                if lo > covered || covered >= end {
                    break;
                }
                covered = hi;
            }
            if covered < end {
                return Err(format!("Unmapped executable data at {va:#x}+{length}").into());
            }
        }
        self.image
            .get(at..end)
            .ok_or_else(|| format!("Read {va:#x}+{length} outside the image").into())
    }
    pub fn u32(&self, va: u32) -> Result<u32> {
        Ok(u32::from_le_bytes(self.read(va, 4)?.try_into()?))
    }
    pub fn cstring(&self, va: u32, limit: usize) -> Result<&[u8]> {
        let mut length = 0;
        while self.read(va + length as u32, 1)?[0] != 0 {
            length += 1;
            if length >= limit {
                return Err(format!("Unterminated string at {va:#x}").into());
            }
        }
        self.read(va, length)
    }
}
