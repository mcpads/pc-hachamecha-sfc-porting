use crate::Result;
use serde::Deserialize;
use std::collections::BTreeMap;
#[derive(Clone, Deserialize)]
pub struct Command {
    pub(super) va: u32,
    pub(super) bytes: Vec<u8>,
}
#[derive(Deserialize)]
#[cfg_attr(not(test), allow(dead_code))]
struct Case {
    command: Command,
    seed: u8,
    counter: u16,
    loop_active: bool,
    synthetic: bool,
    after_sha256: String,
}
#[derive(Deserialize)]
pub struct Reference {
    #[serde(default)]
    pub text_encoding: Vec<[u32; 2]>,
    #[serde(default)]
    pub scene_windows: Vec<[i32; 4]>,
    #[serde(default)]
    #[cfg_attr(not(test), allow(dead_code))]
    pub blit_cases: Vec<crate::blit::Case>,
    #[serde(default)]
    #[cfg_attr(not(test), allow(dead_code))]
    pub draw_cases: Vec<crate::world::DrawCase>,
    pub commands: Vec<Command>,
    #[serde(default)]
    pub motion: Option<crate::motion::Reference>,
    #[serde(default)]
    pub resources: Option<crate::resources::Reference>,
    #[serde(default)]
    #[cfg_attr(not(test), allow(dead_code))]
    cases: Vec<Case>,
    #[serde(default)]
    #[cfg_attr(not(test), allow(dead_code))]
    sequences: Vec<Sequence>,
    #[serde(default)]
    pub pool_cases: Vec<crate::world::Case>,
}
#[derive(Deserialize)]
#[cfg_attr(not(test), allow(dead_code))]
struct Sequence {
    start_va: u32,
    after_sha256: Vec<String>,
}
pub struct State {
    pub(super) object: [u8; 256],
    pub(super) parent: [u8; 256],
    pub(super) globals: [u8; 1024],
    pub(super) yielded: bool,
    pub(super) reference_address: Option<u32>,
}
fn read(bytes: &[u8], at: usize, width: usize) -> Result<u32> {
    let slice = bytes.get(at..at + width).ok_or("VM read out of bounds")?;
    let mut value = [0; 4];
    value[..width].copy_from_slice(slice);
    Ok(u32::from_le_bytes(value))
}
fn write(bytes: &mut [u8], at: usize, width: usize, value: u32) -> Result<()> {
    bytes
        .get_mut(at..at + width)
        .ok_or("VM write out of bounds")?
        .copy_from_slice(&value.to_le_bytes()[..width]);
    Ok(())
}
impl State {
    fn advance(&mut self, bytes: u32) -> Result<()> {
        let pc = read(&self.object, 0x40, 4)?;
        write(&mut self.object, 0x40, 4, pc.wrapping_add(bytes))
    }
    fn operand(&self, mode: u8, offset: u8, width: usize) -> Result<u32> {
        match mode {
            1 => read(&self.globals, usize::from(offset) * 4, 4),
            2 => read(&self.object, offset.into(), width),
            3 => {
                if Some(read(&self.object, 0xa8, 4)?) != self.reference_address {
                    return Err("Unresolved VM object reference".into());
                }
                read(&self.parent, offset.into(), width)
            }
            _ => Err("Missing VM operand mode".into()),
        }
    }
    fn assign(&mut self, mode: u8, offset: u8, width: usize, value: u32) -> Result<()> {
        let mask = if width == 4 {
            u32::MAX
        } else {
            (1 << (width * 8)) - 1
        };
        match mode {
            1 => write(&mut self.globals, usize::from(offset) * 4, 4, value & mask),
            2 => {
                let at = usize::from(offset);
                if at < 0x44 && at + width > 0x40 {
                    return Err("VM arithmetic writes a control pointer".into());
                }
                write(&mut self.object, at, width, value)
            }
            3 => {
                if Some(read(&self.object, 0xa8, 4)?) != self.reference_address {
                    return Err("Unresolved VM object reference".into());
                }
                write(&mut self.parent, offset.into(), width, value)
            }
            _ => Err("Missing VM destination".into()),
        }
    }
    pub fn step(&mut self, command: &Command) -> Result<()> {
        let b = &command.bytes;
        if b.len() < 4 || read(&self.object, 0x40, 4)? != command.va {
            return Err("Invalid VM command boundary".into());
        }
        match b[0] {
            1 => {
                self.advance(4)?;
                self.yielded = true;
            }
            2 => {
                let counter = read(&self.object, 0x5e, 2)? as u16;
                if counter == 0 {
                    write(&mut self.object, 0x5e, 2, read(b, 2, 2)?)?;
                    self.yielded = true;
                } else {
                    write(&mut self.object, 0x5e, 2, (counter - 1).into())?;
                    if counter == 1 {
                        self.advance(4)?;
                    } else {
                        self.yielded = true;
                    }
                }
            }
            3 => write(&mut self.object, 0x40, 4, read(b, 4, 4)?)?,
            4 => {
                let depth = read(&self.object, 0x5c, 2)? as usize;
                if depth >= 6 {
                    return Err("VM return stack overflow".into());
                }
                let target = read(b, 4, 4)?;
                write(
                    &mut self.object,
                    0x44 + depth * 4,
                    4,
                    command.va.wrapping_add(8),
                )?;
                write(&mut self.object, 0x5c, 2, (depth + 1) as u32)?;
                write(&mut self.object, 0x40, 4, target)?;
            }
            5 => {
                let depth = read(&self.object, 0x5c, 2)? as usize;
                if depth == 0 || depth > 6 {
                    return Err("VM return stack underflow or invalid depth".into());
                }
                let target = read(&self.object, 0x44 + (depth - 1) * 4, 4)?;
                write(&mut self.object, 0x5c, 2, (depth - 1) as u32)?;
                write(&mut self.object, 0x40, 4, target)?;
            }
            6 => {
                let target = read(b, 4, 4)?;
                if target == read(&self.object, 0x3c, 4)? {
                    let count = (read(&self.object, 0x60, 2)? as u16).wrapping_sub(1);
                    write(&mut self.object, 0x60, 2, count.into())?;
                    if count == 0 {
                        self.advance(8)?;
                        write(&mut self.object, 0x3c, 4, 0)?;
                    } else {
                        write(&mut self.object, 0x40, 4, target)?;
                    }
                } else {
                    write(&mut self.object, 0x3c, 4, target)?;
                    write(&mut self.object, 0x60, 2, read(b, 2, 2)?)?;
                }
            }
            8 => {
                let mut at = 4;
                loop {
                    let kind = read(b, at, 1)?;
                    if kind == 255 {
                        at += 4;
                        break;
                    }
                    let width = match kind {
                        1 => 1,
                        2 => 2,
                        3 => 4,
                        _ => return Err("Unknown VM assignment".into()),
                    };
                    let offset = read(b, at + 1, 1)? as u8;
                    let value = read(b, at + if width == 4 { 4 } else { 2 }, width)?;
                    self.assign(2, offset, width, value)?;
                    at += if width == 4 { 8 } else { 4 };
                }
                if at != b.len() {
                    return Err("VM assignment extent mismatch".into());
                }
                self.advance(at as u32)?;
            }
            0x10..=0x13 => {
                let mode = b[1];
                let width = match b[0] {
                    0x11 => 2,
                    0x12 => 4,
                    _ => 1,
                };
                let lhs = self.operand(mode >> 6, b[2], width)?;
                let source = (mode >> 4) & 3;
                let rhs = if source == 0 {
                    if width == 1 {
                        b[3].into()
                    } else {
                        read(b, 4, width)?
                    }
                } else {
                    self.operand(source, b[3], width)?
                };
                if b[0] == 0x13 {
                    if compare(lhs, rhs, mode & 15)? {
                        write(&mut self.object, 0x40, 4, read(b, 4, 4)?)?;
                    } else {
                        self.advance(8)?;
                    }
                } else {
                    self.assign(mode >> 6, b[2], width, arithmetic(lhs, rhs, mode & 15)?)?;
                    self.advance(if width != 1 && source == 0 { 8 } else { 4 })?;
                }
            }
            _ => {
                return Err(format!("Unsupported VM execution opcode {:02x}", b[0]).into());
            }
        }
        Ok(())
    }
}
fn arithmetic(a: u32, b: u32, op: u8) -> Result<u32> {
    Ok(match op {
        0 => b,
        1 => a.wrapping_add(b),
        2 => a.wrapping_sub(b),
        3 => a.wrapping_mul(b),
        4 => a.checked_div(b).ok_or("VM divide by zero")?,
        5 => a.checked_rem(b).ok_or("VM remainder by zero")?,
        6 => a & b,
        7 => a | b,
        8 => a ^ b,
        9 => !b,
        10 => b.wrapping_neg(),
        11 => a.wrapping_shl(b & 31),
        12 => a.wrapping_shr(b & 31),
        _ => return Err("Unsupported VM arithmetic".into()),
    })
}
fn compare(a: u32, b: u32, op: u8) -> Result<bool> {
    Ok(match op {
        0 | 8 => a != b,
        1 => a == b,
        2 => a > b,
        3 => a < b,
        4 => a >= b,
        5 => a <= b,
        6 => a & b != 0,
        7 => a != 0 || b != 0,
        9 => a != 0,
        _ => return Err("Unsupported VM comparison".into()),
    })
}
fn program(reference: &Reference) -> Result<BTreeMap<u32, &Command>> {
    if reference.commands.is_empty() || reference.commands.len() > 65535 {
        return Err("Missing VM program".into());
    }
    let mut program = BTreeMap::new();
    for command in &reference.commands {
        if command.bytes.len() < 4
            || command.bytes.len() > 65535
            || program.insert(command.va, command).is_some()
        {
            return Err("Invalid VM program node".into());
        }
    }
    Ok(program)
}
pub fn compile(reference: &Reference) -> Result<Vec<u8>> {
    let program = program(reference)?;
    let mut bytes = b"HMV1".to_vec();
    bytes.extend((program.len() as u16).to_le_bytes());
    for (va, command) in program {
        bytes.extend(va.to_le_bytes());
        bytes.extend((command.bytes.len() as u16).to_le_bytes());
        bytes.extend(&command.bytes);
    }
    Ok(bytes)
}
