use crate::Result;
use sfc_builder::cartridge::{BatteryRam, CartridgeLayout, Mapping, RomSpeed, Section};
pub const SIZE: usize = 0x800000;
pub const HIROM_SIZE: usize = 0x400000;
const CODE_CAPACITY: usize = 0x7400;
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    ExHiRom,
    HiRom,
}
impl Target {
    pub fn layout(self, sections: Vec<Section>) -> Result<CartridgeLayout> {
        Ok(match self {
            Target::ExHiRom => CartridgeLayout::new_exhirom(SIZE, CODE_CAPACITY, sections)?,
            Target::HiRom => {
                let mut layout = CartridgeLayout::new_hirom(HIROM_SIZE, CODE_CAPACITY, sections)?;
                layout.set_rom_speed(RomSpeed::Fast);
                layout
            }
        })
    }
    pub fn battery(self, bytes: usize) -> Result<BatteryRam> {
        Ok(match self {
            Target::ExHiRom => BatteryRam::new_exhirom(bytes)?,
            Target::HiRom => BatteryRam::new_hirom(bytes)?,
        })
    }
}
const SHADOWED: [usize; 2] = [0x7e0000, 0x7f0000];
pub const MAPPING: Mapping = Mapping::ExHiRom;
const SLOW: usize = 0x400000;
const KEPT: (usize, usize) = (crate::sfc_battle_objects::IMAGE_OFFSET, 0x410000);
const IMAGE_END: usize = crate::sfc_battle_objects::IMAGE_OFFSET
    + (crate::sfc_battle_objects::SOURCE_END - crate::sfc_battle_objects::SOURCE_BASE) as usize;
#[derive(Clone, Copy)]
enum Rule {
    Window,
    Bank,
    UpperWindow,
}
impl Rule {
    fn start(self, at: usize, bytes: usize) -> usize {
        let within = |unit: usize| {
            if at / unit == (at + bytes - 1) / unit {
                at
            } else {
                at.next_multiple_of(unit)
            }
        };
        match self {
            Rule::Window => within(0x8000),
            Rule::Bank => within(0x10000),
            Rule::UpperWindow if at & 0xffff <= 0x8000 => (at & !0xffff) + 0x8000,
            Rule::UpperWindow => (at & !0xffff) + 0x18000,
        }
    }
}
pub struct Space {
    free: Vec<(usize, usize)>,
    shared: std::collections::HashMap<Vec<u8>, u32>,
    pass: Pass,
    target: Target,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Item,
    Table,
    Sample,
    UpperWindow,
    Fast,
}
#[derive(Default)]
enum Pass {
    #[default]
    Direct,
    Record(Vec<(Kind, usize)>, Option<Vec<(usize, usize)>>),
    Replay(Vec<(Kind, usize, usize)>, usize),
}
impl Space {
    pub fn new(target: Target) -> Space {
        let free = match target {
            Target::ExHiRom => {
                let mut free = vec![(0x10000, KEPT.0), (KEPT.1, SHADOWED[0])];
                for bank in SHADOWED {
                    free.push((bank + 0x8000, bank + 0x10000));
                }
                free
            }
            Target::HiRom => {
                vec![
                    (0, 0x8000),
                    (0x10000, KEPT.0),
                    (IMAGE_END, HIROM_SIZE),
                    (KEPT.1, SHADOWED[0]),
                ]
            }
        };
        Space {
            free,
            shared: Default::default(),
            pass: Pass::Direct,
            target,
        }
    }
    pub fn target(&self) -> Target {
        self.target
    }
    pub fn recording(target: Target) -> Space {
        Space {
            pass: Pass::Record(Vec::new(), None),
            ..Space::new(target)
        }
    }
    pub fn plan(self) -> Result<Space> {
        let Pass::Record(requests, Some(free)) = self.pass else {
            return Err("No recorded placement".into());
        };
        let mut packer = Space {
            free: free.clone(),
            shared: Default::default(),
            pass: Pass::Direct,
            target: self.target,
        };
        let mut order: Vec<usize> = (0..requests.len()).collect();
        let class = |k: Kind| match k {
            Kind::UpperWindow => 0,
            Kind::Table => 1,
            Kind::Fast => 2,
            Kind::Sample => 3,
            Kind::Item => 4,
        };
        order.sort_by_key(|&i| (class(requests[i].0), std::cmp::Reverse(requests[i].1), i));
        let mut offsets = vec![0; requests.len()];
        for i in order {
            let (kind, bytes) = requests[i];
            offsets[i] = packer.place(kind, bytes)?;
        }
        Ok(Space {
            free: packer.free,
            shared: Default::default(),
            target: self.target,
            pass: Pass::Replay(
                requests
                    .iter()
                    .zip(offsets)
                    .map(|(&(k, b), o)| (k, b, o))
                    .collect(),
                0,
            ),
        })
    }
    pub fn finished(&self) -> Result<()> {
        match &self.pass {
            Pass::Replay(plan, given) if *given != plan.len() => {
                Err("Placement passes made different requests".into())
            }
            _ => Ok(()),
        }
    }
    fn request(&mut self, kind: Kind, bytes: usize) -> Result<usize> {
        match &mut self.pass {
            Pass::Record(list, start) => {
                if start.is_none() {
                    *start = Some(self.free.clone());
                }
                list.push((kind, bytes));
            }
            Pass::Replay(plan, given) => {
                let &(k, b, offset) = plan
                    .get(*given)
                    .ok_or("Placement passes made different requests")?;
                if (k, b) != (kind, bytes) {
                    return Err("Placement passes made different requests".into());
                }
                *given += 1;
                return Ok(offset);
            }
            Pass::Direct => {}
        }
        self.place(kind, bytes)
    }
    fn place(&mut self, kind: Kind, bytes: usize) -> Result<usize> {
        match kind {
            Kind::Item => self
                .take(bytes, Rule::Window, SIZE)
                .ok_or_else(|| "SFC section exceeds 8 MiB".into()),
            Kind::Table => self
                .take(bytes, Rule::Bank, SHADOWED[0])
                .ok_or_else(|| "SFC table section reaches the WRAM-shadowed banks".into()),
            Kind::Sample if self.target == Target::HiRom => self
                .take(bytes, Rule::Bank, SHADOWED[0])
                .ok_or_else(|| "SFC sample section exceeds bank-addressable space".into()),
            Kind::Sample => self
                .take_from(bytes, Rule::Bank, SLOW, SHADOWED[0])
                .or_else(|| self.take(bytes, Rule::Bank, SHADOWED[0]))
                .ok_or_else(|| "SFC sample section exceeds bank-addressable space".into()),
            Kind::Fast => self
                .take(bytes, Rule::Window, SLOW)
                .ok_or_else(|| "No FastROM space left".into()),
            Kind::UpperWindow if self.target == Target::HiRom => self
                .take(bytes, Rule::UpperWindow, SIZE)
                .ok_or_else(|| "SFC section exceeds 8 MiB".into()),
            Kind::UpperWindow => self
                .take_from(bytes, Rule::UpperWindow, SLOW, SIZE)
                .or_else(|| self.take(bytes, Rule::UpperWindow, SIZE))
                .ok_or_else(|| "SFC section exceeds 8 MiB".into()),
        }
    }
    pub fn release(&mut self, runs: &[(usize, usize)]) -> Result<()> {
        if matches!(self.pass, Pass::Replay(..)) {
            return Ok(());
        }
        for &(start, end) in runs {
            if start >= end || start < KEPT.0 || end > KEPT.1 {
                return Err("Released space outside the kept image".into());
            }
            let at = self.free.partition_point(|&(from, _)| from < start);
            let after = self.free.get(at).is_none_or(|&(from, _)| end <= from);
            let before = at == 0 || self.free[at - 1].1 <= start;
            if !after || !before {
                return Err("Released space is already free".into());
            }
            self.free.insert(at, (start, end));
        }
        Ok(())
    }
    pub fn shared(&self, bytes: &[u8]) -> Option<u32> {
        self.shared.get(bytes).copied()
    }
    pub fn share(&mut self, bytes: Vec<u8>, source: u32) {
        self.shared.insert(bytes, source);
    }
    fn take(&mut self, bytes: usize, rule: Rule, limit: usize) -> Option<usize> {
        self.take_from(bytes, rule, 0, limit)
    }
    fn take_from(&mut self, bytes: usize, rule: Rule, floor: usize, limit: usize) -> Option<usize> {
        let fits = self.free.iter().enumerate().filter_map(|(i, &(from, to))| {
            let start = rule.start(from.max(floor), bytes);
            (start + bytes <= to.min(limit)).then(|| (i, start, to.min(limit) - from))
        });
        let (index, start, _) = match rule {
            Rule::UpperWindow => fits.min_by_key(|&(i, ..)| i)?,
            _ => fits.min_by_key(|&(i, start, run)| (start >= SLOW, run, i))?,
        };
        let (from, to) = self.free[index];
        let rest: Vec<_> = [(from, start), (start + bytes, to)]
            .into_iter()
            .filter(|(a, b)| a < b)
            .collect();
        self.free.splice(index..=index, rest);
        Some(start)
    }
    pub fn reserve(&mut self, bytes: usize) -> Result<usize> {
        if bytes == 0 || bytes > 0x8000 {
            return Err("SFC stream section extent".into());
        }
        self.request(Kind::Item, bytes)
    }
    pub fn reserve_table(&mut self, bytes: usize) -> Result<usize> {
        if bytes == 0 || bytes > 0x10000 {
            return Err("SFC table section extent".into());
        }
        self.request(Kind::Table, bytes)
    }
    pub fn reserve_sample(&mut self, bytes: usize) -> Result<usize> {
        if bytes == 0 || bytes > 0x10000 {
            return Err("SFC sample section extent".into());
        }
        self.request(Kind::Sample, bytes)
    }
    pub fn reserve_fast(&mut self, bytes: usize) -> Result<usize> {
        if bytes == 0 || bytes > 0x8000 {
            return Err("SFC fast section extent".into());
        }
        self.request(Kind::Fast, bytes)
    }
    pub fn reserve_upper_window(&mut self, bytes: usize) -> Result<usize> {
        if bytes == 0 || bytes > 0x8000 {
            return Err("SFC upper-window section extent".into());
        }
        self.request(Kind::UpperWindow, bytes)
    }
}
impl Space {
    fn free_below(&self, end: usize) -> usize {
        self.free
            .iter()
            .map(|&(from, to)| to.min(end).saturating_sub(from))
            .sum()
    }
}
pub fn report(sections: &[sfc_builder::cartridge::Section], space: &Space) -> serde_json::Value {
    let total: usize = sections.iter().map(|s| s.bytes.len()).sum();
    let above: usize = sections
        .iter()
        .filter(|s| s.offset >= SLOW)
        .map(|s| s.bytes.len())
        .sum();
    let end = sections
        .iter()
        .map(|s| s.offset + s.bytes.len())
        .max()
        .unwrap_or(0);
    let free = space.free_below(SLOW);
    serde_json::json!(
        { "section_bytes" : total, "data_end" : end, "above_4mib" : above,
        "free_below_4mib" : free, "over_4mib" : above.saturating_sub(free) }
    )
}
pub fn cpu(offset: usize) -> Result<u32> {
    MAPPING.cpu_address(offset)
}
