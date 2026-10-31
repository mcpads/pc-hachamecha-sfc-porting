use crate::{
    Result, SOURCE,
    import::Files,
    original::{Exe, Originals},
    script::decode_resource_script,
    sha,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
const DISPLAYS: [(u8, u32); 4] = [
    (13, 0x46e2c8),
    (14, 0x44fe7c),
    (15, 0x42d044),
    (16, 0x4a2e20),
];
const BODY_SLOT: u16 = 38;
const GEM_SLOT: u16 = 39;
const FRAMES: [u32; 8] = [0, 1, 2, 3, 14, 15, 16, 17];
fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("literal hex"))
        .collect()
}
fn u16_at(raw: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([raw[at], raw[at + 1]])
}
fn fields(raw: &[u8]) -> Result<(BTreeMap<u8, u32>, usize)> {
    if raw[0] != 0x08 {
        return Err("Not an assignment list".into());
    }
    let (mut out, mut at) = (BTreeMap::new(), 4);
    while raw[at] != 0xff {
        match raw[at] {
            2 => {
                out.insert(raw[at + 1], u32::from(u16_at(raw, at + 2)));
                at += 4;
            }
            3 => {
                out.insert(
                    raw[at + 1],
                    u32::from_le_bytes(raw[at + 4..at + 8].try_into()?),
                );
                at += 8;
            }
            _ => return Err("Unexpected assignment kind".into()),
        }
    }
    Ok((out, at + 4))
}
struct Script<'a> {
    exe: &'a Exe,
    root: u32,
}
impl Script<'_> {
    fn cmd(&self, offset: u32, n: usize) -> Result<&[u8]> {
        self.exe.read(self.root + offset, n)
    }
    fn link(&self, offset: u32) -> Result<u32> {
        self.exe.u32(self.root + offset + 4)
    }
    fn expect(&self, offset: u32, bytes: &[u8]) -> Result<()> {
        if self.cmd(offset, bytes.len())? != bytes {
            return Err(format!("Unexpected guardian command at {:#x}", self.root + offset).into());
        }
        Ok(())
    }
    fn draw(&self, offset: u32) -> Result<u16> {
        self.expect(offset, &[0x2b, 0])?;
        Ok(u16_at(self.cmd(offset, 4)?, 2))
    }
    fn show(&self, start: u32, stop: u8) -> Result<(Vec<[u16; 3]>, u32)> {
        let (mut frames, mut pc) = (Vec::new(), start);
        while self.exe.read(pc, 1)?[0] == 0x21 {
            let raw = self.exe.read(pc + 2, 6)?;
            frames.push([u16_at(raw, 2), u16_at(raw, 0), u16_at(raw, 4)]);
            pc += 8;
        }
        if self.exe.read(pc, 1)?[0] != stop {
            return Err("Unexpected SHOW end".into());
        }
        Ok((frames, pc))
    }
}
fn check(ok: bool, what: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("Unexpected guardian {what}").into())
    }
}
fn held(frames: &[[u16; 3]]) -> Vec<[u16; 2]> {
    frames.iter().map(|[f, h, _]| [*f, *h]).collect()
}
fn guardian(originals: &Originals, files: &mut Files, map_id: u8, root: u32) -> Result<Value> {
    let exe = originals.exe()?;
    let s = Script { exe, root };
    let at = |offset: u32| root + offset;
    s.expect(0, &[0x3f, 0, 0, 0])?;
    let loads: Vec<_> = decode_resource_script(exe, s.link(0)?)?
        .into_iter()
        .filter(|c| [2, 4].contains(&c.opcode))
        .collect();
    let body = loads
        .iter()
        .find(|c| c.slot == BODY_SLOT as i16 && c.opcode == 4)
        .ok_or("Missing guardian body")?;
    let gem = loads
        .iter()
        .find(|c| c.slot == GEM_SLOT as i16 && c.opcode == 2)
        .ok_or("Missing guardian gem")?;
    let (init, size) = fields(s.cmd(8, 0x60)?)?;
    let expected: BTreeMap<u8, u32> = [
        (0, 0x40000000),
        (0x1c, 176 << 16),
        (0x20, 208 << 16),
        (0x80, 176 << 16),
        (0x84, 208 << 16),
        (0x94, 2),
        (0x2c, 2),
        (0x28, u32::from(BODY_SLOT) << 16),
    ]
    .into();
    check(size == 0x44 && init == expected, "body initializer")?;
    let (gem_root, emitter) = (s.link(0x4c)?, s.link(0x54)?);
    s.expect(0x4c, &[0x77, 1, 0, 0])?;
    s.expect(0x54, &[0x77, 1, 0, 0])?;
    s.expect(0x5c, &[0x20, 0, 0, 0])?;
    check(gem_root == at(0xb0) && emitter == at(0x14c), "children")?;
    s.expect(0x64, &hex("11818e0000080000"))?;
    s.expect(0x6c, &hex("2d0a00000100000074000000"))?;
    check(s.link(0x74)? == at(0x64), "bobbing loop")?;
    s.expect(0x7c, &[0x3f, 0, 0, 0])?;
    s.expect(0x84, &[0; 4])?;
    let (body_show, tail) = s.show(s.link(0x5c)?, 0x03)?;
    check(
        body_show == (0..4).map(|f| [f, 8, BODY_SLOT]).collect::<Vec<_>>()
            && exe.u32(tail + 4)? == s.link(0x5c)?,
        "body SHOW",
    )?;
    let (mut gem_init, size) = fields(s.cmd(0xb0, 0x60)?)?;
    check(size == 0x40, "gem initializer")?;
    let place = [
        gem_init.remove(&0x1c).ok_or("Gem x")? >> 16,
        gem_init.remove(&0x20).ok_or("Gem y")? >> 16,
    ];
    let expected: BTreeMap<u8, u32> = [
        (0, 0x40000000),
        (0x24, 1 << 16),
        (0x2c, 2),
        (0x28, u32::from(GEM_SLOT) << 16),
        (0x14, 4),
    ]
    .into();
    check(gem_init == expected, "gem initializer")?;
    s.expect(0xf0, &[0x20, 0, 0, 0])?;
    s.expect(0xf8, &[1, 0, 0, 0])?;
    s.expect(0xfc, &[3, 0, 0, 0])?;
    check(s.link(0xfc)? == at(0xf8), "gem yield")?;
    let (gem_show, tail) = s.show(s.link(0xf0)?, 0x03)?;
    check(
        gem_show == (0..8).map(|f| [f, 4, GEM_SLOT]).collect::<Vec<_>>()
            && exe.u32(tail + 4)? == s.link(0xf0)?,
        "gem SHOW",
    )?;
    let e = 0x14c;
    s.expect(e, &hex("1280140004000000"))?;
    s.expect(e + 8, &[0x77, 1, 0, 0])?;
    s.expect(e + 0x10, &[0x77, 1, 0, 0])?;
    let children = [s.link(e + 8)?, s.link(e + 0x10)?];
    check(children == [at(0x1b0), at(0x1c0)], "particles")?;
    s.expect(e + 0x18, &hex("118058000600"))?;
    s.expect(e + 0x20, &[1, 0, 0, 0])?;
    s.expect(e + 0x24, &hex("1182580001000000"))?;
    s.expect(e + 0x2c, &hex("1480580000000000"))?;
    check(s.link(e + 0x30)? == at(e + 0x20), "emitter wait")?;
    let emitter_draw = s.draw(e + 0x38)?;
    s.expect(e + 0x3c, &hex("1181580001000000"))?;
    s.expect(e + 0x44, &[1, 0, 0, 0])?;
    s.expect(e + 0x48, &hex("1182580001000000"))?;
    s.expect(e + 0x50, &hex("1480580000000000"))?;
    check(s.link(e + 0x54)? == at(e + 0x44), "emitter wait")?;
    s.expect(e + 0x5c, &[3, 0, 0, 0])?;
    check(s.link(e + 0x5c)? == emitter, "emitter loop")?;
    let emitter_wait = u16_at(s.cmd(e + 0x18, 8)?, 4);
    let mut depths = Vec::new();
    for child in children {
        let raw = exe.read(child, 8)?;
        check(
            raw[..4] == hex("12802400")
                && exe.read(child + 8, 4)? == [3, 0, 0, 0]
                && exe.u32(child + 12)? == at(0x1d0),
            "particle",
        )?;
        depths.push(u32::from_le_bytes(raw[4..8].try_into()?));
    }
    let p = 0x1d0;
    let (part, size) = fields(s.cmd(p, 0x60)?)?;
    let expected: BTreeMap<u8, u32> = [
        (0, 0x40000000),
        (0x80, 160 << 16),
        (0x84, 182 << 16),
        (0x2c, 2),
        (0x14, 4),
    ]
    .into();
    check(size == 0x30 && part == expected, "particle initializer")?;
    let x_draw = s.draw(p + 0x30)?;
    s.expect(p + 0x34, &hex("1283580000000100"))?;
    s.expect(p + 0x3c, &hex("12a18058"))?;
    let path_draw = s.draw(p + 0x40)?;
    s.expect(p + 0x44, &hex("0a580200"))?;
    let (arc, spread) = (exe.u32(at(p + 0x48))?, exe.u32(at(p + 0x4c))?);
    check(arc == at(0x220) && spread == at(0x2e0), "paths")?;
    let a = 0x220;
    s.expect(a, &[0x20, 0, 0, 0])?;
    let arc_show = s.link(a)?;
    s.expect(a + 8, &hex("11809200"))?;
    s.expect(a + 0x10, &hex("11809400"))?;
    let amplitudes = [u16_at(s.cmd(a + 8, 8)?, 4), u16_at(s.cmd(a + 0x10, 8)?, 4)];
    let amplitude_x_draw = s.draw(a + 0x18)?;
    s.expect(a + 0x1c, &hex("11a19258"))?;
    let amplitude_y_draw = s.draw(a + 0x20)?;
    s.expect(a + 0x24, &hex("11a19458"))?;
    let side_draw = s.draw(a + 0x28)?;
    s.expect(a + 0x2c, &hex("0a580200"))?;
    let sides = [exe.u32(at(a + 0x30))?, exe.u32(at(a + 0x34))?];
    check(sides == [at(0x258), at(0x29c)], "arc sides")?;
    let mut arcs = Vec::new();
    for (side, (mode, sign)) in sides.into_iter().zip([(0xa1u8, 0x81u8), (0xa2, 0x82)]) {
        let o = side - root;
        s.expect(o, &hex("11808c00"))?;
        s.expect(o + 8, &hex("11808e00"))?;
        s.expect(o + 0x10, &hex("11a05892"))?;
        s.expect(o + 0x14, &hex("1283580000000100"))?;
        s.expect(o + 0x1c, &[0x12, mode, 0x80, 0x58])?;
        s.expect(o + 0x20, &[0x11, sign, 0x8c, 0])?;
        s.expect(o + 0x28, &[0x11, sign, 0x8e, 0])?;
        s.expect(o + 0x30, &hex("2d0b000001000000"))?;
        s.expect(o + 0x38, &hex("06002000"))?;
        check(s.link(o + 0x38)? == side + 0x20, "arc loop")?;
        s.expect(o + 0x40, &[0; 4])?;
        let phases = [u16_at(s.cmd(o, 8)?, 4), u16_at(s.cmd(o + 8, 8)?, 4)];
        let step = |offset| -> Result<i32> {
            let value = i32::from(u16_at(s.cmd(offset, 8)?, 4));
            Ok(if sign == 0x81 { value } else { -value })
        };
        arcs.push(json!(
            { "phases" : phases, "steps" : [step(o + 0x20) ?, step(o + 0x28) ?],
            "base_moves" : if mode == 0xa1 { 1 } else { - 1 }, "repeats" : 32 }
        ));
    }
    let o = 0x2e0;
    s.expect(o, &[0x20, 0, 0, 0])?;
    let spread_show = s.link(o)?;
    s.expect(o + 8, &hex("11808c0000c0000011808e0000c00000"))?;
    s.expect(o + 0x18, &hex("11809200010000001180940001000000"))?;
    let phase_draw = s.draw(o + 0x28)?;
    s.expect(o + 0x2c, &hex("11a18c5811a18e58"))?;
    let step_draw = s.draw(o + 0x34)?;
    s.expect(o + 0x38, &hex("1181580002000000"))?;
    s.expect(o + 0x40, &hex("2d0b00000100000011a1925811a19458"))?;
    s.expect(o + 0x50, &hex("06002000"))?;
    check(s.link(o + 0x50)? == at(o + 0x40), "spread loop")?;
    s.expect(o + 0x58, &[0; 4])?;
    let mut shows = Vec::new();
    for start in [arc_show, spread_show] {
        let (frames, tail) = s.show(start, 0x01)?;
        check(
            exe.read(tail + 4, 4)? == [3, 0, 0, 0] && exe.u32(tail + 8)? == tail,
            "particle SHOW",
        )?;
        shows.push(frames);
    }
    let particle_show: Vec<[u16; 3]> = [(14, 4), (15, 16), (16, 8), (17, 4)]
        .map(|(f, h)| [f, h, BODY_SLOT])
        .to_vec();
    check(
        shows[0] == particle_show && shows[1] == particle_show,
        "particle SHOW",
    )?;
    let mut frames = serde_json::Map::new();
    for f in FRAMES {
        let raw = exe.read(body.arguments[0] + 24 * f, 24)?;
        let v: Vec<i32> = raw
            .chunks(4)
            .map(|c| i32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        frames.insert(
            f.to_string(),
            json!({ "rectangle" : & v[..4], "anchor" : & v[4..] }),
        );
    }
    let grid = [gem.arguments[0] & 0xffff, gem.arguments[0] >> 16];
    let file = gem.file.as_deref().ok_or("Gem sheet")?;
    let sheet = originals.cns(file)?;
    check(
        [sheet.width, sheet.height] == [grid[0] as usize * 4, grid[1] as usize * 2],
        "gem sheet",
    )?;
    files.push((
        format!("guardian-display/{file}.indices").into(),
        sheet.pixels.clone(),
    ));
    files.push((
        format!("guardian-display/{file}.rgb").into(),
        sheet.rgb.clone(),
    ));
    Ok(json!(
        { "map_id" : map_id, "entry_va" : root, "body" : { "file" : body.file,
        "palette_base" : body.flags, "frames" : frames, "base" : [176, 208],
        "amplitude" : init[& 0x94], "phase_step" : 0x800, "show" : held(& body_show)
        }, "gem" : { "file" : file, "palette_base" : gem.flags, "key" : gem.flags,
        "grid" : grid, "place" : place, "depth" : 1 << 16, "show" : held(& gem_show),
        "width" : sheet.width, "height" : sheet.height, "pixels_sha256" : sha(& sheet
        .pixels), "palette_sha256" : sha(& sheet.rgb) }, "emitter" : { "wait" :
        emitter_wait, "draw" : emitter_draw, "depths" : depths }, "particle" : {
        "base" : [160, 182], "x_draw" : x_draw, "path_draw" : path_draw, "show" :
        held(& shows[0]), "arc" : { "amplitudes" : amplitudes, "draws" :
        [amplitude_x_draw, amplitude_y_draw], "side_draw" : side_draw, "sides" : arcs
        }, "spread" : { "phase" : 0xc000, "phase_draw" : phase_draw, "amplitude" : 1,
        "step_base" : 2, "step_draw" : step_draw, "repeats" : 32 } } }
    ))
}
pub fn import(originals: &Originals) -> Result<Files> {
    let mut files = Files::new();
    let mut rows = Vec::new();
    for (map_id, root) in DISPLAYS {
        rows.push(guardian(originals, &mut files, map_id, root)?);
    }
    let shared = |row: &Value| {
        let mut row = row.clone();
        for key in ["map_id", "entry_va", "body", "gem"] {
            row.as_object_mut().unwrap().remove(key);
        }
        row
    };
    if rows.iter().any(|r| shared(r) != shared(&rows[0])) {
        return Err("Guardian displays differ beyond their sheets".into());
    }
    let display = json!({ "schema" : 1, "source_sha256" : SOURCE, "guardians" : rows });
    let mut bytes = serde_json::to_vec_pretty(&display)?;
    bytes.push(b'\n');
    files.push(("guardian-display/guardian-display.json".into(), bytes));
    circle_warps(originals.exe()?, &mut files)?;
    Ok(files)
}
const WARP_SHAPE: [[u8; 4]; 10] = [
    [0x24, 0x01, 0x00, 0x1b],
    [0x27, 0x00, 0x00, 0x05],
    [0x07, 0x01, 0x00, 0x00],
    [0x01, 0x00, 0x00, 0x00],
    [0x32, 0x00, 0x40, 0x01],
    [0x24, 0x01, 0x00, 0x28],
    [0x07, 0x01, 0x00, 0x00],
    [0x02, 0x00, 0x19, 0x00],
    [0x31, 0x00, 0x40, 0x01],
    [0x05, 0x00, 0x00, 0x00],
];
const WARP_DONE_FLAG: u32 = 0x140;
const HUM_PLAYED: usize = 80;
fn shake(
    exe: &Exe,
    table: &[u16],
    entry: u32,
    playing: bool,
    limit: usize,
) -> Result<(Vec<Option<i32>>, bool)> {
    let mut object = [0u8; 256];
    let mut shared = [0u8; 256];
    let read = |b: &[u8], at: usize, width: usize| -> u32 {
        b[at..at + width]
            .iter()
            .rev()
            .fold(0u32, |v, &x| (v << 8) | u32::from(x))
    };
    let write = |b: &mut [u8], at: usize, width: usize, value: u32| {
        b[at..at + width].copy_from_slice(&value.to_le_bytes()[..width]);
    };
    let (mut pc, mut offsets, mut ended) = (entry, Vec::new(), false);
    for _ in 0..limit {
        let mut written = None;
        for _ in 0..256 {
            let node = crate::script::decode_node(exe, pc, false)?
                .ok_or_else(|| format!("Shaker command at {pc:#x}"))?;
            let b = &node.raw;
            let next = pc + b.len() as u32;
            let (op, mode) = (b[0], b[1]);
            match op {
                0x00 => {
                    ended = true;
                    break;
                }
                0x01 => {
                    pc = next;
                    break;
                }
                0x03 => pc = node.pointer(4),
                0x4b | 0x27 | 0x31 => {
                    if op == 0x27 && mode == 0x0d {
                        object[0x58] = u8::from(playing);
                    }
                    pc = next;
                }
                0x06 => {
                    let target = node.pointer(4);
                    if target == read(&object, 0x3c, 4) {
                        let count = (read(&object, 0x60, 2) as u16).wrapping_sub(1);
                        write(&mut object, 0x60, 2, count.into());
                        if count == 0 {
                            write(&mut object, 0x3c, 4, 0);
                            pc = next;
                        } else {
                            pc = target;
                        }
                    } else {
                        write(&mut object, 0x3c, 4, target);
                        write(&mut object, 0x60, 2, read(b, 2, 2));
                    }
                }
                0x08 => {
                    let mut k = 4;
                    while b[k] != 0xff {
                        let at = usize::from(b[k + 1]);
                        match b[k] {
                            1 => object[at] = b[k + 2],
                            2 => object[at..at + 2].copy_from_slice(&b[k + 2..k + 4]),
                            3 => object[at..at + 4].copy_from_slice(&b[k + 4..k + 8]),
                            _ => return Err("Unsupported shaker assignment".into()),
                        }
                        k += if b[k] == 3 { 8 } else { 4 };
                    }
                    pc = next;
                }
                0x2d => {
                    write(&mut object, 0x40, 4, pc);
                    let command = crate::vm::Command {
                        va: pc,
                        bytes: b.clone(),
                    };
                    crate::motion::apply(&mut object, &command, table)?;
                    pc = next;
                }
                0x11 | 0x12 | 0x14 | 0x15 => {
                    let width = if op & 1 == 1 { 2 } else { 4 };
                    let source = (mode >> 4) & 3;
                    let operand = |m: u8, at: u8, object: &[u8], shared: &[u8]| -> Result<u32> {
                        match m {
                            2 => Ok(read(object, at.into(), width)),
                            3 => Ok(read(shared, at.into(), width)),
                            _ => Err("Unsupported shaker operand".into()),
                        }
                    };
                    let lhs = operand(mode >> 6, b[2], &object, &shared)?;
                    let rhs = if source == 0 {
                        read(b, 4, width)
                    } else {
                        operand(source, b[3], &object, &shared)?
                    };
                    if op >= 0x14 {
                        let taken = match mode & 15 {
                            0 | 8 => lhs != rhs,
                            1 => lhs == rhs,
                            6 => lhs & rhs != 0,
                            _ => return Err("Unsupported shaker comparison".into()),
                        };
                        pc = if taken {
                            node.pointer(b.len() - 4)
                        } else {
                            next
                        };
                    } else {
                        let value = match mode & 15 {
                            0 => rhs,
                            1 => lhs.wrapping_add(rhs),
                            4 => lhs.checked_div(rhs).ok_or("Shaker divide by zero")?,
                            7 => lhs | rhs,
                            _ => return Err("Unsupported shaker arithmetic".into()),
                        };
                        let value = if width == 4 { value } else { value & 0xffff };
                        match mode >> 6 {
                            2 => write(&mut object, b[2].into(), width, value),
                            3 => {
                                write(&mut shared, b[2].into(), width, value);
                                if b[2] == 0x0c {
                                    written = Some(value as i32);
                                }
                            }
                            _ => return Err("Unsupported shaker destination".into()),
                        }
                        pc = next;
                    }
                }
                _ => {
                    return Err(format!("Unsupported shaker command {op:#x} at {pc:#x}").into());
                }
            }
        }
        if ended {
            break;
        }
        offsets.push(written);
    }
    Ok((offsets, ended))
}
pub fn circle_warps(exe: &Exe, files: &mut Files) -> Result<()> {
    let mut commands: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    for map in crate::import_world::maps(exe)? {
        let scopes = crate::import_world::scopes(exe, &map)?;
        for graph in [&scopes.entry, &scopes.exit, &scopes.events] {
            for node in graph.nodes.values() {
                commands.insert(node.va, node.raw.clone());
            }
        }
    }
    let (mut sites, mut hums, mut quakes) = (
        Vec::new(),
        std::collections::BTreeSet::new(),
        std::collections::BTreeSet::new(),
    );
    let mut parts = Vec::new();
    for (&va, c) in &commands {
        if c[..4] != WARP_SHAPE[1]
            || commands
                .get(&(va - 4))
                .is_none_or(|p| p[..4] != WARP_SHAPE[0])
        {
            continue;
        }
        let mut at = va - 4;
        parts.clear();
        for want in WARP_SHAPE {
            let c = commands.get(&at).ok_or("Circle warp command")?;
            if c[..4] != want {
                return Err(format!("Circle warp at {:#x} differs at {at:#x}", va - 4).into());
            }
            parts.push(c.clone());
            at += c.len() as u32;
        }
        let pointer = |i: usize| u32::from_le_bytes(parts[i][4..8].try_into().unwrap());
        if pointer(4) != va - 4 + 16 {
            return Err(format!("Circle warp at {:#x} waits elsewhere", va - 4).into());
        }
        let (hum, quake) = (pointer(2), pointer(6));
        sites.push(json!({ "subroutine_va" : va - 4, "hum_va" : hum, "quake_va" : quake }));
        hums.insert(hum);
        quakes.insert(quake);
    }
    if sites.is_empty() {
        return Err("No circle warps".into());
    }
    let table: Vec<u16> = exe
        .read(0x45b3c8, 2048)?
        .chunks(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let mut period: Option<Vec<i32>> = None;
    for &hum in &hums {
        let (offsets, _) = shake(exe, &table, hum, true, HUM_PLAYED)?;
        let offsets: Vec<i32> = offsets
            .into_iter()
            .collect::<Option<_>>()
            .ok_or("Hum skips a tick")?;
        let cycle = offsets[..32].to_vec();
        if offsets.iter().enumerate().any(|(i, v)| *v != cycle[i % 32])
            || period.as_ref().is_some_and(|p| *p != cycle)
        {
            return Err(format!("Hum shaker {hum:#x} is not one 32-tick cycle").into());
        }
        if !shake(exe, &table, hum, false, 4)?.1 {
            return Err(format!("Hum shaker {hum:#x} does not end with its sound").into());
        }
        period = Some(cycle);
    }
    let mut runs = serde_json::Map::new();
    for &quake in &quakes {
        let (offsets, ended) = shake(exe, &table, quake, false, 128)?;
        let offsets: Vec<i32> = offsets
            .into_iter()
            .collect::<Option<_>>()
            .ok_or("Quake skips a tick")?;
        if !ended {
            return Err(format!("Quake shaker {quake:#x} does not end").into());
        }
        runs.insert(format!("{quake:#x}"), json!(offsets));
    }
    let warps = json!(
        { "schema" : 1, "source_sha256" : SOURCE, "sites" : sites, "hum" : { "objects" :
        hums, "offsets" : period, "done_flag" : WARP_DONE_FLAG, "effect" : parts[0] [3],
        "played_ticks" : HUM_PLAYED }, "quakes" : runs, "quake_effect" : parts[5] [3],
        "screen_offset_global" : 0x0c }
    );
    let mut bytes = serde_json::to_vec_pretty(&warps)?;
    bytes.push(b'\n');
    files.push(("circle-warps.json".into(), bytes));
    Ok(())
}
