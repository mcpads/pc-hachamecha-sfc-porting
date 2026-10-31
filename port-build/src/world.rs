use crate::{
    Result, sha,
    vm::{Command, State},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
const BASE: u32 = 0x4c2600;
const END: u32 = 0x4f2000;
const FREE: u32 = 0x4c26b0;
const POOL: u32 = 0x4c4770;
const COUNT: u32 = 0x4f1630;
const HEAD: u32 = 0x4f1640;
const TAIL: u32 = 0x4f1726;
const SIZE: u32 = 0xe6;
const YIELD: u32 = 0x4c4094;
const GAME_STATE: u32 = 0x4b47c0;
const MESSAGE_YIELD: u32 = 0x4c3280;
#[derive(Deserialize)]
pub struct Case {
    #[serde(default)]
    pub scene_frames: Vec<crate::scene::Frame>,
    #[serde(default)]
    pub policy: String,
    #[serde(default)]
    pub draw_objects: Vec<Vec<[u32; 6]>>,
    #[serde(default)]
    pub scene_palettes: BTreeMap<String, Vec<u8>>,
    #[serde(default)]
    pub scene_palette_ids: Vec<String>,
    #[serde(default)]
    draw_orders: Vec<Vec<u32>>,
    #[serde(default)]
    palette_initial: Option<crate::palette::Palette>,
    #[serde(default)]
    palette_after_sha256: Vec<String>,
    #[serde(default)]
    pub resource_events: Vec<crate::resources::Event>,
    #[serde(default)]
    pub action_order: Option<Vec<ActionRef>>,
    #[serde(default)]
    pub action_ends: Vec<usize>,
    root: u32,
    root_group: u8,
    commands: Vec<Command>,
    initial_sha256: String,
    after_sha256: Vec<String>,
    #[serde(default)]
    message_commands: Vec<Command>,
    #[serde(default)]
    game_state: Vec<u8>,
    #[serde(default)]
    input_words: Vec<u16>,
    #[serde(default)]
    message_latches: Vec<u8>,
    #[serde(default)]
    game_state_after_sha256: Vec<String>,
    #[serde(default)]
    initial_patches: Vec<Patch>,
    #[serde(default)]
    held_words: Vec<u16>,
    #[serde(default)]
    message_fast: Vec<u32>,
    #[serde(default)]
    pub renderer: Option<Renderer>,
    #[serde(default)]
    pub render_events: Vec<RenderEvent>,
}
#[derive(Deserialize)]
struct Patch {
    address: u32,
    width: usize,
    value: u32,
}
#[derive(Clone, Deserialize)]
pub struct Renderer {
    pub fonts: Vec<[i16; 2]>,
    pub windows: Vec<[i32; 4]>,
    pub palette: Vec<[u8; 3]>,
    banks: BTreeMap<u32, String>,
}
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RenderEvent {
    Music {
        channel: u8,
        bank: String,
        index: u8,
    },
    StopMusic,
    Fill {
        slot: u16,
        color: u8,
        rectangle: [u32; 4],
    },
    Text {
        pc: u32,
        x: i32,
        y: i32,
        font: u16,
        color: u32,
        bytes: Vec<u8>,
    },
    Clear {
        window: u32,
    },
    Audio {
        channel: u16,
        bank: String,
        index: u16,
    },
}
#[derive(Deserialize, Serialize, PartialEq)]
pub struct ActionRef {
    pub kind: String,
    pub index: usize,
}
struct World {
    action_order: Vec<ActionRef>,
    resources: Option<crate::resources::Reference>,
    resource_events: Vec<crate::resources::Event>,
    palette: Option<crate::palette::Palette>,
    motion_table: Option<Vec<u16>>,
    memory: Vec<u8>,
    program: BTreeMap<u32, Command>,
    messages: BTreeMap<u32, Command>,
    game_state: [u8; 256],
    message_latch: u8,
    message_fast: bool,
    renderer: Option<Renderer>,
    render_events: Vec<RenderEvent>,
    now: usize,
    wait_starts: BTreeMap<u32, usize>,
    waits: Vec<(usize, usize)>,
}
impl World {
    fn collect_draw_order(&mut self) -> Result<Vec<u32>> {
        let mut entries = Vec::new();
        let mut visited = std::collections::BTreeSet::new();
        for group in 0..5 {
            let tail = TAIL + group * SIZE * 2;
            let mut object = self.get(HEAD + group * SIZE * 2 + 8, 4)?;
            while object != tail {
                if !(POOL..POOL + 800 * SIZE).contains(&object)
                    || !(object - POOL).is_multiple_of(SIZE)
                    || !visited.insert(object)
                {
                    return Err("Invalid display-list object chain".into());
                }
                let flags = self.get(object, 4)?;
                if flags & 0x4000_0000 != 0 {
                    entries.push((object, self.get(object + 0x24, 4)? as i32));
                } else if flags & 0x2000_0000 != 0 {
                    entries.push((object, self.get(object + 0x24, 4)? as i32));
                    self.put(object, 4, flags & !0x2000_0000)?;
                }
                object = self.get(object + 8, 4)?;
            }
        }
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.1));
        let order: Vec<_> = entries.into_iter().map(|(address, _)| address).collect();
        for (index, address) in order.iter().chain(std::iter::once(&0)).enumerate() {
            self.put(0x4c3408 + index as u32 * 4, 4, *address)?;
        }
        Ok(order)
    }
    fn emit_render(&mut self, event: RenderEvent) {
        self.action_order.push(ActionRef {
            kind: "render".into(),
            index: self.render_events.len(),
        });
        self.render_events.push(event);
    }
    fn range(&self, address: u32, length: usize) -> Result<std::ops::Range<usize>> {
        let start = address
            .checked_sub(BASE)
            .ok_or("Unmapped world reference")? as usize;
        let end = start.checked_add(length).ok_or("World extent overflow")?;
        if end > self.memory.len() {
            return Err("Unmapped world extent".into());
        }
        Ok(start..end)
    }
    fn get(&self, address: u32, width: usize) -> Result<u32> {
        let mut bytes = [0; 4];
        bytes[..width].copy_from_slice(self.bytes(address, width)?);
        Ok(u32::from_le_bytes(bytes))
    }
    fn bytes(&self, address: u32, length: usize) -> Result<&[u8]> {
        if (GAME_STATE..GAME_STATE + 256).contains(&address) {
            let offset = (address - GAME_STATE) as usize;
            return self
                .game_state
                .get(offset..offset + length)
                .ok_or("Game state extent exceeded".into());
        }
        Ok(&self.memory[self.range(address, length)?])
    }
    fn put(&mut self, address: u32, width: usize, value: u32) -> Result<()> {
        if (GAME_STATE..GAME_STATE + 256).contains(&address) {
            let offset = (address - GAME_STATE) as usize;
            self.game_state
                .get_mut(offset..offset + width)
                .ok_or("Game state extent exceeded")?
                .copy_from_slice(&value.to_le_bytes()[..width]);
            return Ok(());
        }
        let range = self.range(address, width)?;
        self.memory[range].copy_from_slice(&value.to_le_bytes()[..width]);
        Ok(())
    }
    fn new(commands: &[Command]) -> Result<Self> {
        let mut world = Self {
            action_order: Vec::new(),
            resources: None,
            resource_events: Vec::new(),
            palette: None,
            motion_table: None,
            memory: vec![0; (END - BASE) as usize],
            program: BTreeMap::new(),
            messages: BTreeMap::new(),
            game_state: [0; 256],
            message_latch: 0,
            message_fast: false,
            renderer: None,
            render_events: Vec::new(),
            now: 0,
            wait_starts: BTreeMap::new(),
            waits: Vec::new(),
        };
        for command in commands {
            if world.program.insert(command.va, command.clone()).is_some() {
                return Err("Duplicate world command".into());
            }
        }
        for index in 0..799 {
            world.put(POOL + index * SIZE + 12, 4, POOL + (index + 1) * SIZE)?;
        }
        world.put(FREE, 4, POOL)?;
        for group in 0..5 {
            world.put(HEAD + group * SIZE * 2 + 8, 4, TAIL + group * SIZE * 2)?;
            world.put(TAIL + group * SIZE * 2 + 4, 4, HEAD + group * SIZE * 2)?;
        }
        Ok(world)
    }
    fn allocate(&mut self, group: u8, script: u32) -> Result<u32> {
        if group >= 5 {
            return Err("Invalid object group".into());
        }
        let object = self.get(FREE, 4)?;
        if object == 0 {
            return Err("Object pool exhausted".into());
        }
        let tail = TAIL + u32::from(group) * SIZE * 2;
        let previous = self.get(tail + 4, 4)?;
        self.put(FREE, 4, self.get(object + 12, 4)?)?;
        self.put(object + 0x10, 4, 0x41c891)?;
        self.put(object + 0x1a, 2, group.into())?;
        self.put(object + 4, 4, previous)?;
        self.put(object + 8, 4, tail)?;
        self.put(previous + 8, 4, object)?;
        self.put(tail + 4, 4, object)?;
        self.put(COUNT, 4, self.get(COUNT, 4)?.wrapping_add(1))?;
        self.put(object + 0x40, 4, script)?;
        Ok(object)
    }
    fn free(&mut self, object: u32) -> Result<()> {
        let previous = self.get(object + 4, 4)?;
        let next = self.get(object + 8, 4)?;
        self.put(next + 4, 4, previous)?;
        self.put(previous + 8, 4, next)?;
        let range = self.range(object, SIZE as usize)?;
        self.memory[range].fill(0);
        self.put(object + 12, 4, self.get(FREE, 4)?)?;
        self.put(FREE, 4, object)?;
        self.put(object + 4, 4, previous)?;
        self.put(object + 8, 4, next)?;
        self.put(COUNT, 4, self.get(COUNT, 4)?.wrapping_sub(1))?;
        Ok(())
    }
    fn advance(&mut self, object: u32, bytes: u32) -> Result<()> {
        self.put(
            object + 0x40,
            4,
            self.get(object + 0x40, 4)?.wrapping_add(bytes),
        )
    }
    fn write_changes(&mut self, address: u32, before: &[u8], after: &[u8]) -> Result<()> {
        for (index, (&old, &new)) in before.iter().zip(after).enumerate() {
            if old != new {
                self.put(address + index as u32, 1, new.into())?;
            }
        }
        Ok(())
    }
    fn execute(&mut self, object: u32, command: &Command) -> Result<()> {
        let b = &command.bytes;
        if b.len() < 4 {
            return Err("Truncated world command".into());
        }
        match b[0] {
            0 => {
                self.free(object)?;
                self.put(YIELD, 4, 1)?;
            }
            7 => {
                let target = u32::from_le_bytes(b.get(4..8).ok_or("Truncated spawn")?.try_into()?);
                let child = self.allocate(b[1], target)?;
                self.put(object + 0x58, 4, child)?;
                self.advance(object, 8)?;
            }
            0x1f => {
                let child = self.get(object + 0x58, 4)?;
                self.put(object + 0xa4, 4, child)?;
                self.put(child + 0xa0, 4, object)?;
                self.advance(object, 4)?;
            }
            0x28 => {
                let slot = 0x4c4330 + u32::from(b[2]) * 4;
                match b[1] {
                    0 => self.put(slot, 4, self.get(object + 0x58, 4)?)?,
                    1 => self.put(object + 0xa8, 4, self.get(slot, 4)?)?,
                    _ => return Err("Unsupported object slot mode".into()),
                }
                self.advance(object, 4)?;
            }
            0x29 => {
                let slot = 0x4c4330 + u32::from(b[1]) * 4;
                let child = self.get(slot, 4)?;
                if child != 0 {
                    self.free(child)?;
                    self.put(slot, 4, 0)?;
                }
                self.advance(object, 4)?;
            }
            0x4b => {
                self.put(object + 0xa8, 4, 0x4c46a0)?;
                self.advance(object, 4)?;
            }
            0x6f => {
                self.put(object + 0xa8, 4, GAME_STATE)?;
                self.advance(object, 4)?;
            }
            0x3f => {
                let start = u32::from_le_bytes(
                    b.get(4..8)
                        .ok_or("Truncated resource handoff")?
                        .try_into()?,
                );
                let (end, events) = self
                    .resources
                    .as_ref()
                    .ok_or("No resource program")?
                    .run(start, self.palette.as_mut().ok_or("No palette state")?)?;
                for event in events {
                    self.action_order.push(ActionRef {
                        kind: "resource".into(),
                        index: self.resource_events.len(),
                    });
                    self.resource_events.push(event);
                }
                self.put(0x4c26b8, 4, end)?;
                self.put(0x4c27c4, 4, 1)?;
                self.advance(object, 8)?;
            }
            0x25 if b[1] == 0 => {
                self.emit_render(RenderEvent::StopMusic);
                self.advance(object, 4)?;
            }
            0x26 if b[1] == 0x30 && b[2] & 0xc0 == 0 => {
                let bank =
                    u32::from_le_bytes(b.get(4..8).ok_or("Truncated music request")?.try_into()?);
                let name = self
                    .renderer
                    .as_ref()
                    .ok_or("No music adapter")?
                    .banks
                    .get(&bank)
                    .ok_or("Unknown music bank")?
                    .clone();
                self.emit_render(RenderEvent::Music {
                    channel: b[2] & 63,
                    bank: name,
                    index: b[3],
                });
                self.advance(object, 8)?;
            }
            0x8d => {
                if b.len() != 12 {
                    return Err("Truncated rectangle fill".into());
                }
                let slot = u16::from_le_bytes([b[2], b[3]]);
                if slot == 0 {
                    return Err("Primary surface fill is not mapped".into());
                }
                let rectangle = std::array::from_fn(|i| {
                    u32::from(u16::from_le_bytes([b[4 + i * 2], b[5 + i * 2]]))
                });
                self.emit_render(RenderEvent::Fill {
                    slot,
                    color: b[1],
                    rectangle,
                });
                self.advance(object, 12)?;
            }
            0x38 | 0x39 | 0x3b => {
                let length = self
                    .palette
                    .as_mut()
                    .ok_or("No palette state")?
                    .apply(command)?;
                self.advance(object, length)?;
            }
            0x2d => {
                let mut object_bytes: [u8; 256] = self.bytes(object, 256)?.try_into()?;
                let before = object_bytes;
                crate::motion::apply(
                    &mut object_bytes,
                    command,
                    self.motion_table.as_deref().ok_or("No motion table")?,
                )?;
                self.write_changes(object, &before, &object_bytes)?;
            }
            0x2f => {
                let message =
                    u32::from_le_bytes(b.get(4..8).ok_or("Truncated message handoff")?.try_into()?);
                match b[1] {
                    0 => {
                        let child = self.allocate(b[2], 0)?;
                        self.put(child + 0x10, 4, 0x414b7b)?;
                        self.put(child, 4, 0x01000000)?;
                        self.put(child + 0xc8, 2, 0)?;
                        self.put(child + 0xb0, 4, message)?;
                        self.put(object + 0x58, 4, child)?;
                    }
                    1 => {
                        self.put(object + 0x10, 4, 0x414b7b)?;
                        self.put(object + 0xb0, 4, message)?;
                        self.put(YIELD, 4, 1)?;
                    }
                    _ => return Err("Unsupported message handoff mode".into()),
                }
                self.advance(object, 8)?;
            }
            _ => {
                let reference = self.get(object + 0xa8, 4)?;
                let referenced_bytes = self.bytes(reference, 256).ok();
                let before_object: [u8; 256] = self.memory[self.range(object, 256)?].try_into()?;
                let before_parent: [u8; 256] = match referenced_bytes {
                    Some(bytes) => bytes.try_into()?,
                    None => [0; 256],
                };
                let reference_mapped = referenced_bytes.is_some();
                let before_globals: [u8; 1024] =
                    self.memory[self.range(0x4c4260, 1024)?].try_into()?;
                let mut state = State {
                    object: before_object,
                    parent: before_parent,
                    globals: before_globals,
                    yielded: false,
                    reference_address: reference_mapped.then_some(reference),
                };
                state.step(command)?;
                self.write_changes(object, &before_object, &state.object)?;
                if reference_mapped {
                    self.write_changes(reference, &before_parent, &state.parent)?;
                }
                self.write_changes(0x4c4260, &before_globals, &state.globals)?;
                self.put(YIELD, 4, u32::from(state.yielded))?;
            }
        }
        Ok(())
    }
    fn invoke(&mut self, object: u32) -> Result<()> {
        self.put(YIELD, 4, 0)?;
        for _ in 0..10000 {
            let pc = self.get(object + 0x40, 4)?;
            let command = self
                .program
                .get(&pc)
                .ok_or("Unknown world script label")?
                .clone();
            self.execute(object, &command)?;
            if self.get(YIELD, 4)? != 0 {
                return Ok(());
            }
        }
        Err("Object invocation exceeded command budget".into())
    }
    fn message_advance(&mut self, object: u32, length: u32) -> Result<()> {
        self.put(
            object + 0xb0,
            4,
            self.get(object + 0xb0, 4)?.wrapping_add(length),
        )
    }
    fn message_invoke(&mut self, object: u32) -> Result<()> {
        self.put(MESSAGE_YIELD, 4, 0)?;
        self.message_fast = false;
        for _ in 0..10000 {
            let pc = self.get(object + 0xb0, 4)?;
            let command = self
                .messages
                .get(&pc)
                .ok_or("Unsupported message command boundary")?
                .clone();
            let b = command.bytes;
            if b.first().is_some_and(|byte| *byte != 64) {
                self.message_text(object)?;
                if self.get(MESSAGE_YIELD, 4)? != 0 {
                    return Ok(());
                }
                continue;
            }
            if b.len() < 4 {
                return Err("Truncated message control".into());
            }
            let value = u16::from_le_bytes([b[2], b[3]]);
            match b[1] {
                0 => {
                    if self.get(object, 4)? & 0x01000000 != 0 {
                        self.free(object)?;
                    } else {
                        self.put(object + 0x10, 4, 0x41c891)?;
                        self.message_advance(object, 4)?;
                    }
                    self.put(MESSAGE_YIELD, 4, 1)?;
                }
                2 => {
                    let font = self.get(object + 0xde, 2)? as usize;
                    let height = self
                        .renderer
                        .as_ref()
                        .ok_or("No text renderer")?
                        .fonts
                        .get(font)
                        .ok_or("Unknown font metrics")?[0] as i32;
                    self.put(object + 0xd6, 4, self.get(object + 0xce, 4)?)?;
                    self.put(
                        object + 0xda,
                        4,
                        self.get(object + 0xda, 4)?.wrapping_add(height as u32),
                    )?;
                    self.message_advance(object, 4)?;
                }
                3 => {
                    self.put(object + 0xde, 2, b[2].into())?;
                    self.message_advance(object, 4)?;
                }
                4 => {
                    self.put(object + 0xca, 2, value.into())?;
                    self.put(object + 0xcc, 2, 0)?;
                    self.message_advance(object, 4)?;
                }
                5 => {
                    if value & 0x8000 != 0 && self.get(0x4c46a2, 2)? & 0x100 != 0 {
                        self.wait_starts.remove(&object);
                        self.put(object + 0x5e, 2, 0)?;
                        self.put(MESSAGE_YIELD, 4, 1)?;
                        self.message_advance(object, 4)?;
                    } else {
                        let count = self.get(object + 0x5e, 2)? as u16;
                        if count == 0 {
                            if value & 0x8000 != 0 {
                                self.wait_starts.insert(object, self.now);
                            }
                            self.put(object + 0x5e, 2, u32::from(value & 0x7fff))?;
                            self.put(MESSAGE_YIELD, 4, 1)?;
                        } else {
                            self.put(object + 0x5e, 2, u32::from(count - 1))?;
                            if count == 1 {
                                if let Some(start) = self.wait_starts.remove(&object) {
                                    self.waits.push((start, self.now));
                                }
                                self.message_advance(object, 4)?;
                            } else {
                                self.put(MESSAGE_YIELD, 4, 1)?;
                            }
                        }
                    }
                }
                6 => {
                    let signal = self.get(0x4c46a2, 2)? & 0x100 != 0;
                    if self.message_latch == 0 {
                        if signal {
                            self.message_latch = 1;
                        }
                    } else if !signal {
                        self.message_latch = 0;
                        self.message_advance(object, 4)?;
                    }
                    self.put(MESSAGE_YIELD, 4, 1)?;
                }
                7 => {
                    let y = u16::from_le_bytes(
                        b.get(4..6)
                            .ok_or("Truncated message position")?
                            .try_into()?,
                    );
                    self.put(object + 0x1c, 4, u32::from(value) << 16)?;
                    self.put(object + 0x20, 4, u32::from(y) << 16)?;
                    self.message_advance(object, 8)?;
                }
                8 if b[2] == 0 => {
                    let color = *self
                        .renderer
                        .as_ref()
                        .ok_or("No text renderer")?
                        .palette
                        .get(b[3] as usize)
                        .ok_or("Unknown text palette color")?;
                    for (i, value) in color.iter().enumerate() {
                        self.put(object + 0xe0 + i as u32 * 2, 2, u32::from(*value))?;
                    }
                    self.message_advance(object, 4)?;
                }
                11 => {
                    if self.renderer.is_none() {
                        return Err("No text renderer".into());
                    }
                    self.emit_render(RenderEvent::Clear {
                        window: self.get(object + 0x28, 4)?,
                    });
                    self.put(object + 0xd6, 4, self.get(object + 0xce, 4)?)?;
                    self.put(object + 0xda, 4, self.get(object + 0xd2, 4)?)?;
                    self.message_advance(object, 4)?;
                }
                13 => {
                    let window = u32::from_le_bytes(
                        b.get(4..8).ok_or("Truncated message window")?.try_into()?,
                    );
                    let rect = *self
                        .renderer
                        .as_ref()
                        .ok_or("No text renderer")?
                        .windows
                        .get((window & 65535) as usize)
                        .ok_or("Unknown text window")?;
                    self.put(object, 4, self.get(object, 4)? | 0x40000000)?;
                    self.put(object + 0x28, 4, window)?;
                    for (offset, value) in [
                        (0xce, rect[0]),
                        (0xd2, rect[1]),
                        (0xd6, rect[0]),
                        (0xda, rect[1]),
                    ] {
                        self.put(object + offset, 4, value as u32)?;
                    }
                    self.message_advance(object, 8)?;
                }
                15 => {
                    self.message_advance(object, 4)?;
                    self.put(MESSAGE_YIELD, 4, 1)?;
                }
                42 => {
                    if self.get(GAME_STATE + 0x48, 1)? != 0 {
                        let bank = u32::from_le_bytes(
                            b.get(4..8).ok_or("Truncated message audio")?.try_into()?,
                        );
                        let name = self
                            .renderer
                            .as_ref()
                            .ok_or("No audio dispatch adapter")?
                            .banks
                            .get(&bank)
                            .ok_or("Unknown message audio bank")?
                            .clone();
                        self.emit_render(RenderEvent::Audio {
                            bank: name,
                            channel: b[2].into(),
                            index: b[3].into(),
                        });
                        self.put(0x4c4324, 2, 0)?;
                    }
                    self.message_advance(object, 8)?;
                }
                _ => {
                    return Err(format!("Unsupported message control {:02x}", b[1]).into());
                }
            }
            if self.get(MESSAGE_YIELD, 4)? != 0 {
                return Ok(());
            }
        }
        Err("Message invocation exceeded command budget".into())
    }
    fn message_text(&mut self, object: u32) -> Result<()> {
        let delay = self.get(object + 0xca, 2)? as u16;
        let held = self.get(0x4c46a0, 2)?;
        let previous = self.get(0x4c46a2, 2)?;
        if delay != 0 {
            if self.message_latch == 0 {
                if !previous & held & 0x100 != 0 {
                    self.message_latch = 1;
                    self.put(MESSAGE_YIELD, 4, 1)?;
                    return Ok(());
                }
            } else {
                if held & 0x100 != 0 {
                    self.put(MESSAGE_YIELD, 4, 1)?;
                    return Ok(());
                }
                self.message_latch = 0;
                self.message_fast = true;
                self.put(object + 0xcc, 2, delay.into())?;
            }
        }
        let pc = self.get(object + 0xb0, 4)?;
        let mut text = Vec::new();
        loop {
            let command = self
                .messages
                .get(&(pc + text.len() as u32))
                .ok_or("Missing message character")?;
            if command.bytes.first() == Some(&64) {
                break;
            }
            if command.bytes.len() != 2 {
                return Err("Invalid source character width".into());
            }
            text.extend(&command.bytes);
            if delay != 0 && !self.message_fast {
                break;
            }
            if text.len() > 65534 {
                return Err("Message text span too large".into());
            }
        }
        if text.is_empty() {
            return Err("Empty text span".into());
        }
        self.put(0x4c3288, 2, text.len() as u32)?;
        self.put(0x4c3284, 4, pc)?;
        let counter = self.get(object + 0xcc, 2)? as u16;
        if counter == 0 && delay != 0 && !self.message_fast {
            self.put(object + 0xcc, 2, delay.into())?;
            self.put(MESSAGE_YIELD, 4, 1)?;
        } else {
            let counter = counter.wrapping_sub(1);
            self.put(object + 0xcc, 2, counter.into())?;
            if counter == 0 || delay == 0 || self.message_fast {
                let font = self.get(object + 0xde, 2)? as u16;
                let width = self
                    .renderer
                    .as_ref()
                    .ok_or("No text renderer")?
                    .fonts
                    .get(font as usize)
                    .ok_or("Unknown font metrics")?[1] as i32;
                let color = self.get(object + 0xe0, 1)?
                    | self.get(object + 0xe2, 1)? << 8
                    | self.get(object + 0xe4, 1)? << 16;
                let count = text.len() as u32;
                self.emit_render(RenderEvent::Text {
                    pc,
                    x: self.get(object + 0xd6, 4)? as i32,
                    y: self.get(object + 0xda, 4)? as i32,
                    font,
                    color,
                    bytes: text,
                });
                self.put(
                    object + 0xd6,
                    4,
                    self.get(object + 0xd6, 4)?
                        .wrapping_add((width as u32).wrapping_mul(count / 2)),
                )?;
                self.message_advance(object, count)?;
            } else {
                self.put(MESSAGE_YIELD, 4, 1)?;
            }
        }
        Ok(())
    }
    fn tick(&mut self) -> Result<()> {
        for group in 0..5 {
            let tail = TAIL + group * SIZE * 2;
            let mut object = self.get(HEAD + group * SIZE * 2 + 8, 4)?;
            let mut visited = 0;
            while object != tail {
                visited += 1;
                if visited > 10000 {
                    return Err("Scheduler traversal exceeded budget".into());
                }
                if self.get(object, 4)? & 0x80000000 == 0 {
                    match self.get(object + 0x10, 4)? {
                        0x41c891 => self.invoke(object)?,
                        0x414b7b => self.message_invoke(object)?,
                        _ => return Err("Unsupported object callback".into()),
                    }
                }
                object = self.get(object + 8, 4)?;
            }
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct DrawCase {
    objects: Vec<DrawObject>,
    passes: Vec<DrawPass>,
}
#[derive(Deserialize)]
#[cfg_attr(not(test), allow(dead_code))]
struct DrawObject {
    group: u8,
    flags: u32,
    z: i32,
}
#[derive(Deserialize)]
#[cfg_attr(not(test), allow(dead_code))]
struct DrawPass {
    order: Vec<u32>,
    arena_sha256: String,
}
fn prepared(
    case: &Case,
    motion: Option<&crate::motion::Reference>,
    resources: Option<&crate::resources::Reference>,
) -> Result<World> {
    let mut world = World::new(&case.commands)?;
    world.motion_table = motion.map(|m| m.table.clone());
    world.resources = resources.cloned();
    world.palette = case.palette_initial.clone();
    if let Some(palette) = &world.palette {
        palette.validate()?;
    }
    world.renderer = case.renderer.clone();
    for patch in &case.initial_patches {
        if ![1, 2, 4].contains(&patch.width) {
            return Err("Invalid initialization patch width".into());
        }
        world.put(patch.address, patch.width, patch.value)?;
    }
    if !case.game_state.is_empty() {
        world
            .game_state
            .copy_from_slice(&<[u8; 256]>::try_from(case.game_state.as_slice())?);
    }
    for command in &case.message_commands {
        if world.messages.insert(command.va, command.clone()).is_some() {
            return Err("Duplicate message command".into());
        }
    }
    for length in [
        case.draw_objects.len(),
        case.scene_palette_ids.len(),
        case.draw_orders.len(),
        case.input_words.len(),
        case.message_latches.len(),
        case.game_state_after_sha256.len(),
        case.palette_after_sha256.len(),
        case.action_ends.len(),
        case.held_words.len(),
        case.message_fast.len(),
    ] {
        if length != 0 && length != case.after_sha256.len() {
            return Err("Incomplete scheduler observations".into());
        }
    }
    world.allocate(case.root_group, case.root)?;
    if !case.initial_sha256.is_empty() && sha(&world.memory) != case.initial_sha256 {
        return Err("Native pool initialization mismatch".into());
    }
    Ok(world)
}
pub fn skippable_waits(
    case: &Case,
    motion: Option<&crate::motion::Reference>,
    resources: Option<&crate::resources::Reference>,
) -> Result<Vec<(usize, usize)>> {
    let mut world = prepared(case, motion, resources)?;
    for tick in 0..case.after_sha256.len() {
        if let Some(word) = case.input_words.get(tick) {
            world.put(0x4c46a2, 2, (*word).into())?;
        }
        if let Some(word) = case.held_words.get(tick) {
            world.put(0x4c46a0, 2, (*word).into())?;
        }
        world.now = tick;
        world.tick()?;
    }
    Ok(world.waits)
}
pub fn record(
    case: &Case,
    motion: Option<&crate::motion::Reference>,
    resources: Option<&crate::resources::Reference>,
    exe: &crate::original::Exe,
) -> Result<serde_json::Value> {
    use serde_json::json;
    let mut world = prepared(case, motion, resources)?;
    let initial = sha(&world.memory);
    let (mut after, mut inputs, mut held, mut ends) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut objects, mut palettes, mut palette_ids) =
        (Vec::new(), serde_json::Map::new(), Vec::new());
    let mut slots: BTreeMap<u16, crate::resources::Event> = BTreeMap::new();
    let (mut frames, mut frame_order) = (BTreeMap::new(), Vec::new());
    let (mut cursor, mut previous) = (0, 0u16);
    for tick in 0..30000 {
        let (current, input) = if case.policy == "skip" {
            let current = if matches!(tick % 8, 2 | 3) { 256 } else { 0 };
            let input = previous;
            previous = current;
            (current, input)
        } else {
            let mut waiting = false;
            for index in 0..800 {
                let object = POOL + index * SIZE;
                if world.get(object + 0x10, 4)? == 0x414b7b {
                    let pc = world.get(object + 0xb0, 4)?;
                    waiting |= pc != 0
                        && world
                            .messages
                            .get(&pc)
                            .is_some_and(|c| c.bytes.starts_with(&[0x40, 0x06]));
                }
            }
            let input = if waiting && world.message_latch == 0 {
                256
            } else {
                0
            };
            (input, input)
        };
        world.put(0x4c46a0, 2, current.into())?;
        world.put(0x4c46a2, 2, input.into())?;
        world.now = tick;
        world.tick()?;
        let order = world.collect_draw_order()?;
        let draws: Vec<[u32; 6]> = order
            .iter()
            .map(|&address| {
                let mut values = [0; 6];
                for (index, offset) in [0x1c, 0x20, 0x28, 0x2c, 0x30, 0x34].iter().enumerate() {
                    values[index] = world.get(address + offset, 4)?;
                }
                Ok(values)
            })
            .collect::<Result<_>>()?;
        let palette = world
            .palette
            .as_ref()
            .ok_or("Recorded scene without a palette")?
            .current_bytes()
            .to_vec();
        let id = sha(&palette);
        palettes.entry(id.clone()).or_insert_with(|| json!(palette));
        palette_ids.push(id);
        for event in &world.resource_events[cursor..] {
            match event.opcode {
                1 | 2 | 4 | 6 => {
                    slots.insert(event.slot, event.clone());
                }
                11 => {
                    slots.remove(&event.slot);
                }
                _ => {}
            }
        }
        cursor = world.resource_events.len();
        for draw in &draws {
            let (slot, clip) = ((draw[2] >> 16) & 255, draw[2] & 0xffff);
            if slot == 3 {
                continue;
            }
            let descriptor = slots.get(&(slot as u16)).ok_or_else(|| {
                format!(
                    "Unregistered draw slot {slot} at {} tick {tick}",
                    case.policy
                )
            })?;
            if descriptor.opcode == 1 {
                let file = descriptor
                    .file
                    .clone()
                    .ok_or("Registration without a file")?;
                let key = (file.clone(), clip);
                if !frames.contains_key(&key) {
                    let raw = exe.read(descriptor.argument + clip * 16, 16)?;
                    let rectangle: Vec<u32> = raw
                        .chunks(4)
                        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                        .collect();
                    frames.insert(
                        key.clone(),
                        json!(
                            { "file" : file, "clip" : clip, "rectangle" : rectangle,
                            "table" : descriptor.argument }
                        ),
                    );
                    frame_order.push(key);
                }
            }
        }
        objects.push(draws);
        after.push(sha(&world.memory));
        inputs.push(input);
        held.push(current);
        ends.push(world.action_order.len());
        if world.get(POOL + 0x40, 4)? == case.root + 12 {
            return Ok(json!(
                { "initial_sha256" : initial, "after_sha256" : after, "input_words" :
                inputs, "held_words" : held, "action_ends" : ends, "action_order" :
                world.action_order, "draw_objects" : objects, "scene_palettes" :
                palettes, "scene_palette_ids" : palette_ids, "scene_frames" :
                frame_order.iter().map(| k | frames[k].clone()).collect::< Vec < _ >>
                (), "resource_events" : world.resource_events, "render_events" :
                world.render_events }
            ));
        }
    }
    Err(format!("Scene did not return to its caller under {}", case.policy).into())
}
