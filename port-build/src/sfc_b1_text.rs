use crate::Result;
use crate::font::{DIALOGUE_LINE, DialogueFont};
use std::collections::BTreeMap;
mod cache;
pub(crate) const COLUMNS: usize = 30;
pub(crate) const ROWS: usize = 9;
pub(crate) const PEN: [usize; 2] = [10, 4];
const GLYPH_ROWS: usize = DIALOGUE_LINE;
pub(crate) const GLYPH_BYTES: usize = 32;
pub(crate) const LINES: usize = (ROWS * 8 - PEN[1] - (GLYPH_ROWS + 1)) / DIALOGUE_LINE + 1;
pub(crate) const LAST_LINE_Y: usize = ROWS * 8 - (GLYPH_ROWS + 1);
const _: () = assert!(PEN[1] + (LINES - 1) * DIALOGUE_LINE <= LAST_LINE_Y);
pub(crate) const END: u8 = 0;
pub(crate) const BREAK: u8 = 1;
pub(crate) const COLOR: u8 = 2;
pub(crate) const FIRST_GLYPH: u8 = 8;
pub(crate) const PREFIX: u8 = 0xf0;
const ONE_BYTE: usize = (PREFIX - FIRST_GLYPH) as usize;
const COLORS: u8 = 6;
const DEFAULT_COLOR: u8 = 2;
pub(crate) fn ink_index(color: u8) -> u8 {
    match color {
        5 => 14,
        6 => 15,
        _ => color,
    }
}
const CODE_CHUNK: usize = 0x8000;
pub struct Assets {
    pub glyphs: Vec<u8>,
    pub codes: Vec<Vec<u8>>,
    pub offsets: Vec<(usize, u16)>,
    pub blank: Vec<u8>,
    pub reference: Vec<u8>,
    pub report: serde_json::Value,
    index: BTreeMap<char, usize>,
    cached: Vec<cache::Page>,
}
enum Step {
    Glyph(usize, u8),
    Break,
}
fn steps(page: &str, index: &BTreeMap<char, usize>) -> Result<Vec<Step>> {
    let mut steps = Vec::new();
    let mut color = DEFAULT_COLOR;
    for (line, text) in page.lines().enumerate() {
        if line > 0 {
            steps.push(Step::Break);
        }
        for ch in text.chars() {
            if let Some(next) = crate::b1_dialogue::mark_color(ch) {
                if next > COLORS {
                    return Err("B1 text colour outside the dialogue palette".into());
                }
                color = next;
                continue;
            }
            if let Some(&glyph) = index.get(&ch) {
                steps.push(Step::Glyph(glyph, color));
            }
        }
    }
    Ok(steps)
}
pub(crate) fn draw_glyph(page: &mut [u8], rows: &[u16; GLYPH_ROWS], x: usize, y: usize, color: u8) {
    let mut above = 0u16;
    for r in 0..=GLYPH_ROWS {
        let ink = rows.get(r).copied().unwrap_or(0);
        let shadow = (above >> 1) & !ink;
        above = ink;
        if ink | shadow == 0 {
            continue;
        }
        let (row, line) = ((y + r) / 8, (y + r) % 8);
        let ink = (u32::from(ink) << 8) >> (x % 8);
        let shadow = (u32::from(shadow) << 8) >> (x % 8);
        for k in 0..3 {
            let (i, s) = ((ink >> (16 - 8 * k)) as u8, (shadow >> (16 - 8 * k)) as u8);
            if i | s == 0 {
                continue;
            }
            let at = (row * COLUMNS + x / 8 + k) * 32 + line * 2;
            for (plane, ink_bit, shadow_bit) in
                [(0, 1, true), (1, 2, false), (16, 4, true), (17, 8, false)]
            {
                let byte = &mut page[at + plane];
                *byte &= !(i | s);
                if ink_index(color) & ink_bit != 0 {
                    *byte |= i;
                }
                if shadow_bit {
                    *byte |= s;
                }
            }
        }
    }
}
fn glyph_rows(font: &DialogueFont, ch: char) -> Result<([u16; GLYPH_ROWS], u8)> {
    let glyph = font.glyph(ch)?;
    let mut rows = [0u16; GLYPH_ROWS];
    for &[dx, dy] in &glyph.ink {
        if dx > 14 || dy >= GLYPH_ROWS {
            return Err(format!("B1 glyph {ch:?} exceeds the 15-column cell").into());
        }
        rows[dy] |= 0x8000 >> dx;
    }
    Ok((rows, u8::try_from(glyph.advance)?))
}
fn drawn_rows(rows: &[u16; GLYPH_ROWS]) -> (u8, u8) {
    let mut above = 0u16;
    let drawn: Vec<u8> = (0..=GLYPH_ROWS)
        .filter(|&r| {
            let ink = rows.get(r).copied().unwrap_or(0);
            let shadow = (above >> 1) & !ink;
            above = ink;
            ink | shadow != 0
        })
        .map(|r| r as u8)
        .collect();
    match (drawn.first(), drawn.last()) {
        (Some(&first), Some(&last)) => (first, last - first + 1),
        _ => (0, 0),
    }
}
fn reach(rows: &[u16; GLYPH_ROWS]) -> usize {
    let ink = rows.iter().fold(0, |a, r| a | r);
    if ink == 0 {
        0
    } else {
        17 - ink.trailing_zeros() as usize
    }
}
fn draw_page(blank: &[u8], steps: &[Step], glyphs: &[([u16; GLYPH_ROWS], u8)]) -> Result<Vec<u8>> {
    let mut page = blank.to_vec();
    let [mut x, mut y] = PEN;
    for step in steps {
        match step {
            Step::Break => [x, y] = [PEN[0], y + DIALOGUE_LINE],
            Step::Glyph(glyph, color) => {
                let (rows, advance) = &glyphs[*glyph];
                if y > LAST_LINE_Y || x + reach(rows) > COLUMNS * 8 {
                    return Err("A B1 page leaves the dialogue window".into());
                }
                draw_glyph(&mut page, rows, x, y, *color);
                x += usize::from(*advance);
            }
        }
    }
    Ok(page)
}
impl Assets {
    pub fn append(
        &self,
        sections: &mut Vec<sfc_builder::cartridge::Section>,
        space: &mut crate::sfc_rom::Space,
    ) -> Result<()> {
        use crate::sfc_b1::add;
        add(
            sections,
            space,
            "b1_text_glyphs".into(),
            self.glyphs.clone(),
        )?;
        let mut chunks = Vec::new();
        for (i, chunk) in self.codes.iter().enumerate() {
            chunks.push(add(
                sections,
                space,
                format!("b1_text_codes_{i}"),
                chunk.clone(),
            )?);
        }
        let mut offsets = Vec::new();
        for &(chunk, offset) in &self.offsets {
            let at = chunks[chunk] + u32::from(offset);
            if at >> 16 != chunks[chunk] >> 16 {
                return Err("A page's codes cross a bank".into());
            }
            offsets.extend(at.to_le_bytes());
        }
        add(sections, space, "b1_text_offsets".into(), offsets)?;
        add(sections, space, "b1_text_blank".into(), self.blank.clone())?;
        let mut targets = Vec::new();
        for r in 0..ROWS {
            let address = if r == 8 {
                usize::from(ROW8_HOME)
            } else {
                crate::sfc_b1::TEXT_TILES as usize + (1 + r * COLUMNS) * 32
            };
            targets.extend(((address / 2) as u16).to_le_bytes());
            targets.extend(((BUFFER as usize + r * COLUMNS * 32) as u16).to_le_bytes());
        }
        add(sections, space, "b1_text_targets".into(), targets)?;
        let pool = pools()
            .iter()
            .flatten()
            .flat_map(|&(at, tiles)| [at.to_le_bytes(), tiles.to_le_bytes()].concat())
            .collect();
        add(sections, space, "b1_text_pool".into(), pool)?;
        Ok(())
    }
    pub fn prepare(
        pages: &[String],
        extra: &str,
        font: &DialogueFont,
        reference: impl Fn(&str) -> Result<Vec<Vec<u8>>>,
    ) -> Result<Self> {
        let mut uses: BTreeMap<char, usize> = BTreeMap::new();
        for ch in pages.iter().flat_map(|p| p.chars()).chain(extra.chars()) {
            if ch == '\n' || crate::b1_dialogue::mark_color(ch).is_some() {
                continue;
            }
            if font.glyph(ch)?.ink.is_empty() && font.advance(ch)? == 0 {
                continue;
            }
            *uses.entry(ch).or_default() += 1;
        }
        let mut order: Vec<char> = uses.keys().copied().collect();
        order.sort_by_key(|ch| std::cmp::Reverse(uses[ch]));
        let index: BTreeMap<char, usize> =
            order.iter().enumerate().map(|(i, ch)| (*ch, i)).collect();
        let shapes = order
            .iter()
            .map(|ch| glyph_rows(font, *ch))
            .collect::<Result<Vec<_>>>()?;
        let mut glyphs = Vec::new();
        for (rows, advance) in &shapes {
            glyphs.extend([*advance, u8::try_from(reach(rows))?]);
            glyphs.extend(rows.iter().flat_map(|r| r.to_le_bytes()));
            glyphs.resize(glyphs.len().next_multiple_of(GLYPH_BYTES), 0);
            let (first, count) = drawn_rows(rows);
            let at = glyphs.len() - GLYPH_BYTES + crate::sfc_b1_text_draw::FIRST_AT;
            glyphs[at..at + 2].copy_from_slice(&[first * 2, count]);
        }
        if glyphs.len() > 0x8000 {
            return Err("B1 glyph table exceeds one bank".into());
        }
        let blank = reference("")?.concat();
        let mut codes: Vec<Vec<u8>> = vec![Vec::new()];
        let mut offsets = Vec::new();
        let mut all_codes: Vec<Vec<u8>> = Vec::new();
        let mut laid_out = Vec::new();
        for page in pages {
            let steps = steps(page, &index)?;
            let expected = reference(page)?.concat();
            if draw_page(&blank, &steps, &shapes).map_err(|e| format!("{e}: {page:?}"))? != expected
            {
                return Err(format!("Run-time drawing differs from the B1 page {page:?}").into());
            }
            laid_out.extend(expected);
            let mut page_codes = Vec::new();
            let codes_of = &mut page_codes;
            let mut color = DEFAULT_COLOR;
            for step in &steps {
                match step {
                    Step::Break => codes_of.push(BREAK),
                    Step::Glyph(glyph, next) => {
                        if *next != color {
                            codes_of.push(COLOR + next - 1);
                            color = *next;
                        }
                        if *glyph < ONE_BYTE {
                            codes_of.push(FIRST_GLYPH + *glyph as u8);
                        } else {
                            let rest = glyph - ONE_BYTE;
                            codes_of.push(PREFIX + u8::try_from(rest / 256)?);
                            codes_of.push((rest % 256) as u8);
                        }
                    }
                }
            }
            page_codes.push(END);
            all_codes.push(page_codes);
        }
        let mut longest: Vec<usize> = (0..all_codes.len()).collect();
        longest.sort_by_key(|&i| std::cmp::Reverse(all_codes[i].len()));
        let mut tails: std::collections::HashMap<&[u8], (usize, u16)> =
            std::collections::HashMap::new();
        let mut placed = vec![(0, 0); all_codes.len()];
        for i in longest {
            let page = &all_codes[i];
            if let Some(&at) = tails.get(page.as_slice()) {
                placed[i] = at;
                continue;
            }
            if codes.last().unwrap().len() + page.len() > CODE_CHUNK {
                codes.push(Vec::new());
            }
            let chunk = codes.len() - 1;
            let offset = codes[chunk].len();
            codes[chunk].extend(page);
            for k in 0..page.len() {
                tails
                    .entry(&page[k..])
                    .or_insert((chunk, u16::try_from(offset + k)?));
            }
            placed[i] = (chunk, u16::try_from(offset)?);
        }
        offsets.extend(placed);
        let report = serde_json::json!(
            { "glyphs" : order.len(), "glyph_bytes" : glyphs.len(), "code_bytes" : codes
            .iter().map(Vec::len).sum::< usize > (), "code_chunks" : codes.len(), "pages"
            : pages.len(), "blank_bytes" : blank.len(), "drawn_pages_match_layout" :
            pages.len() }
        );
        Ok(Self {
            glyphs,
            codes,
            offsets,
            blank,
            reference: laid_out,
            report,
            index,
            cached: Vec::new(),
        })
    }
    pub fn metrics(&self, ch: char) -> Option<(usize, usize)> {
        let at = self.index.get(&ch)? * GLYPH_BYTES;
        Some((
            usize::from(self.glyphs[at]),
            usize::from(self.glyphs[at + 1]),
        ))
    }
    pub fn encode(&self, text: &str) -> Result<Vec<u8>> {
        let mut codes = Vec::new();
        for ch in text.chars() {
            let Some(&glyph) = self.index.get(&ch) else {
                continue;
            };
            if glyph < ONE_BYTE {
                codes.push(FIRST_GLYPH + glyph as u8);
            } else {
                let rest = glyph - ONE_BYTE;
                codes.push(PREFIX + u8::try_from(rest / 256)?);
                codes.push((rest % 256) as u8);
            }
        }
        Ok(codes)
    }
}
pub(crate) const RUNTIME_PAGE: u16 = 0xffff;
pub(crate) const RUNTIME_CODES: u32 = 0x7f5400;
pub(crate) const RUNTIME_BYTES: u16 = 0x200;
pub(crate) const SERIAL: u16 = 0x1781;
pub(crate) const ROW8_HOME: u16 = 0x6b40;
const BASE_RUNS: [(u16, u16); 3] = [(0x6f00, 12), (0xde20, 15), (0xbf00, 8)];
const ARLE_RUN: u16 = 0x8000;
const RUNS: usize = 5;
fn pools() -> [[(u16, u16); RUNS]; 4] {
    let arle = crate::sfc_battle_display::ARLE_BUFFERS;
    let run = |(at, tiles): (u16, usize)| (at, tiles as u16 | ARLE_RUN);
    let [a, b, c] = BASE_RUNS;
    [
        [(0xb3c0, 66 | ARLE_RUN), (ARLE_TAIL, 26 | ARLE_RUN), a, b, c],
        [run(arle[0][0]), run(arle[0][1]), a, b, c],
        [run(arle[1][0]), run(arle[1][1]), a, b, c],
        [a, b, c, (0, 0), (0, 0)],
    ]
}
pub(crate) const ARLE_TAIL: u16 = 0x7080;
pub(crate) const BELOW_PORTRAIT: u16 = ARLE_TAIL + 26 * 32;
const _: () = assert!(ROW8_HOME + 30 * 32 == 0x6f00 && 0x6f00 + 12 * 32 == ARLE_TAIL);
const PAUSE_EXTRA: u16 = 900;
pub(crate) const GROUP_KIND: u8 = 5;
pub(crate) const STONE_SKIP: u16 = 0x1888;
pub(crate) const SHARED: u16 = 0x1884;
pub(crate) const TAIL_COUNT: u16 = 0x1882;
const J: u16 = 0x1868;
const SUM: u16 = 0x186a;
const LAST: u16 = 0x186c;
const COST: u16 = 0x186e;
const LIMIT: u16 = 0x1870;
const PAUSE: u16 = 0x1872;
const POOL_TILES: u16 = 0x1874;
const RUN: u16 = 0x1876;
const RUN_LEFT: u16 = 0x1878;
const RUN_AT: u16 = 0x187a;
const ROW: u16 = 0x187c;
const COL: u16 = 0x187e;
const GROUP: u16 = 0x1880;
const SPAN: u16 = 0x1886;
const N: u16 = 0x188a;
const TMP: u16 = 0x188c;
const LP: u16 = 0x188e;
const DV: u16 = 0x1890;
const DN: u16 = 0x1892;
const DS: u16 = 0x1894;
const DK: u16 = 0x1896;
const TAIL_FROM: u16 = 0x1898;
const POOL_BASE: u16 = 0x189e;
const PER_ROW: u16 = 0x18a0;
const LIST: u32 = 0x7f7880;
const LIST_MAX: u16 = 32;
const MAP_BLOCK: u32 = 0x7f7980;
const WINDOW_MAP: u16 = (0xf000 + 18 * 64) / 2;
pub(crate) const DONE: u16 = 0x1782;
pub(crate) const QUEUED: u16 = 0x1783;
pub(crate) const PAGE: u16 = 0x1784;
pub(crate) const EXTENT: u16 = 0x1830;
pub(crate) const SHOWN: u16 = 0x1840;
pub(crate) const POSTED: u16 = 0x1850;
const JOB: u16 = 0x1860;
pub(crate) const TEXT_DP: u16 = 0x1800;
const DP: u16 = TEXT_DP;
pub(crate) const BUFFER: u32 = 0x7f0000;
const CODE: u8 = 0x00;
pub(crate) const GLYPH: u8 = 0x03;
pub(crate) const PEN_X: u8 = 0x06;
pub(crate) const LINE_Y: u8 = 0x08;
pub(crate) const INK_COLOR: u8 = 0x0a;
const CLEAR: u8 = 0x22;
const TEMP: u8 = 0x24;
const DRAWING: u8 = 0x26;
const ROW_INDEX: u8 = 0x28;
const ROW_OFFSET: u8 = 0x2a;
const SKIP: u8 = 0x2c;
const HIDDEN: u16 = 0xffff;
pub fn emit(
    p: &mut w65c816::Assembler,
    layout: &sfc_builder::cartridge::CartridgeLayout,
) -> Result<()> {
    use crate::sfc::{emit, implied};
    use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
    let abs = |p: &mut w65c816::Assembler, m, at: u16| emit(p, m, A::Absolute, O::Word(at));
    let long = |p: &mut w65c816::Assembler, m, at: u16| {
        emit(p, m, A::AbsoluteLong, O::Long(u32::from(at)))
    };
    let dp = |p: &mut w65c816::Assembler, m, at: u8| emit(p, m, A::DirectPage, O::Byte(at));
    let branch = |p: &mut w65c816::Assembler, m, label: &str| {
        p.emit_label_ref(m, A::Relative8, label);
    };
    let call = |p: &mut w65c816::Assembler, label: &str| {
        p.emit_label_ref(M::Jsr, A::Absolute, label);
    };
    let jump = |p: &mut w65c816::Assembler, label: &str| {
        p.emit_label_ref(M::Jmp, A::Absolute, label);
    };
    let m16 = |p: &mut w65c816::Assembler| emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    let m8 = |p: &mut w65c816::Assembler| emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    let section = |name: &str| -> Result<u32> { layout.cpu_address(layout.section(name)?.offset) };
    let glyphs = section("b1_text_glyphs")?;
    let offsets = section("b1_text_offsets")?;
    let blank = section("b1_text_blank")?;
    let targets = section("b1_text_targets")?;
    let pool = section("b1_text_pool")?;
    let text_map = section("b1_text_map")? + 18 * 64;
    let page_slot = 5u16;
    let count = crate::sfc_b1::QUEUE + page_slot + 3;
    p.label("b1_text_request")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, QUEUED);
    abs(p, M::Cmp, SERIAL);
    branch(p, M::Bne, "b1_text_request_shown");
    m16(p);
    abs(p, M::Lda, crate::sfc_b1::QUEUE + page_slot + 3);
    m8(p);
    branch(p, M::Bne, "b1_text_request_shown");
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("b1_text_request_rows");
    emit(p, M::Lda, A::AbsoluteX, O::Word(POSTED));
    emit(p, M::Sta, A::AbsoluteX, O::Word(SHOWN));
    implied(p, M::Inx);
    emit(p, M::Cpx, A::Immediate, O::Word(ROWS as u16));
    branch(p, M::Bne, "b1_text_request_rows");
    p.label("b1_text_request_shown");
    m16(p);
    abs(p, M::Lda, 0x501);
    abs(p, M::Sta, PAGE);
    abs(p, M::Lda, count);
    branch(p, M::Beq, "b1_text_request_drop");
    abs(p, M::Cmp, TAIL_COUNT);
    branch(p, M::Beq, "b1_text_request_kept");
    branch(p, M::Bcc, "b1_text_request_kept");
    p.label("b1_text_request_drop");
    abs(p, M::Stz, count);
    p.label("b1_text_request_kept");
    m8(p);
    abs(p, M::Inc, SERIAL);
    implied(p, M::Rts);
    p.label("b1_text_post").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, DONE);
    abs(p, M::Cmp, SERIAL);
    branch(p, M::Beq, "b1_text_post_drawn");
    implied(p, M::Rts);
    p.label("b1_text_post_drawn");
    abs(p, M::Cmp, QUEUED);
    branch(p, M::Bne, "b1_text_post_new");
    implied(p, M::Rts);
    p.label("b1_text_post_new");
    abs(p, M::Sta, QUEUED);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("b1_text_post_row");
    emit(p, M::Lda, A::AbsoluteX, O::Word(EXTENT));
    emit(p, M::Sta, A::AbsoluteX, O::Word(POSTED));
    emit(p, M::Cmp, A::AbsoluteX, O::Word(SHOWN));
    branch(p, M::Bcs, "b1_text_post_wider");
    emit(p, M::Lda, A::AbsoluteX, O::Word(SHOWN));
    p.label("b1_text_post_wider");
    emit(p, M::Sta, A::AbsoluteX, O::Word(SHOWN));
    implied(p, M::Inx);
    emit(p, M::Cpx, A::Immediate, O::Word(ROWS as u16));
    branch(p, M::Bne, "b1_text_post_row");
    let shl = |p: &mut w65c816::Assembler, n: usize| {
        for _ in 0..n {
            emit(p, M::Asl, A::Accumulator, O::None);
        }
    };
    let shr = |p: &mut w65c816::Assembler, n: usize| {
        for _ in 0..n {
            emit(p, M::Lsr, A::Accumulator, O::None);
        }
    };
    let span_y = |p: &mut w65c816::Assembler| {
        emit(p, M::Lda, A::AbsoluteY, O::Word(SHOWN));
        emit(p, M::And, A::Immediate, O::Word(0xff));
    };
    let times_row = |p: &mut w65c816::Assembler| {
        for _ in 0..6 {
            emit(p, M::Asl, A::Accumulator, O::None);
        }
        emit(p, M::Sta, A::Absolute, O::Word(TMP));
        for _ in 0..4 {
            emit(p, M::Asl, A::Accumulator, O::None);
        }
        implied(p, M::Sec);
        emit(p, M::Sbc, A::Absolute, O::Word(TMP));
    };
    let set = |p: &mut w65c816::Assembler, at: u16, value: u16| {
        emit(p, M::Lda, A::Immediate, O::Word(value));
        emit(p, M::Sta, A::Absolute, O::Word(at));
    };
    let cost = crate::sfc_b1::DESCRIPTOR_COST;
    let buffer = BUFFER as u16;
    let bank = (BUFFER >> 16) as u16;
    m16(p);
    set(p, LAST, 0xffff);
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("b1_text_post_last");
    span_y(p);
    branch(p, M::Beq, "b1_text_post_last_next");
    emit(p, M::Sty, A::Absolute, O::Word(LAST));
    p.label("b1_text_post_last_next");
    implied(p, M::Iny);
    emit(p, M::Cpy, A::Immediate, O::Word(8));
    branch(p, M::Bne, "b1_text_post_last");
    let tiles = |t: &[(u16, u16)]| -> u16 { t.iter().map(|r| r.1 & !ARLE_RUN).sum() };
    let tables = pools();
    let base_tiles = tiles(&BASE_RUNS);
    let battle_tiles = tiles(&tables[1]);
    if tiles(&tables[2]) != battle_tiles {
        return Err("Arle's two buffers differ in size".into());
    }
    abs(p, M::Stz, POOL_BASE);
    emit(p, M::Lda, A::Immediate, O::Word(tiles(&tables[0])));
    abs(p, M::Sta, POOL_TILES);
    abs(p, M::Lda, crate::sfc_battle_field::ACTIVE);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    branch(p, M::Beq, "b1_text_post_pool");
    emit(p, M::Lda, A::Immediate, O::Word(3 * RUNS as u16 * 4));
    abs(p, M::Sta, POOL_BASE);
    emit(p, M::Lda, A::Immediate, O::Word(base_tiles));
    abs(p, M::Sta, POOL_TILES);
    abs(
        p,
        M::Lda,
        crate::sfc_b1::QUEUE + crate::sfc_battle_display::ARLE_SLOT * 5 + 3,
    );
    branch(p, M::Bne, "b1_text_post_pool");
    abs(p, M::Lda, crate::sfc_battle_display::ARLE_BANK);
    emit(p, M::And, A::Immediate, O::Word(1));
    emit(p, M::Eor, A::Immediate, O::Word(1));
    emit(p, M::Inc, A::Accumulator, O::None);
    abs(p, M::Sta, TMP);
    shl(p, 2);
    implied(p, M::Clc);
    abs(p, M::Adc, TMP);
    shl(p, 2);
    abs(p, M::Sta, POOL_BASE);
    emit(p, M::Lda, A::Immediate, O::Word(battle_tiles));
    abs(p, M::Sta, POOL_TILES);
    p.label("b1_text_post_pool");
    abs(p, M::Stz, PAUSE);
    p.label("b1_text_post_pass");
    abs(p, M::Lda, PAUSE);
    branch(p, M::Beq, "b1_text_post_budget");
    emit(
        p,
        M::Lda,
        A::Immediate,
        O::Word(crate::sfc_b1::STREAM_BUDGET + PAUSE_EXTRA),
    );
    branch(p, M::Bra, "b1_text_post_limit");
    p.label("b1_text_post_budget");
    emit(
        p,
        M::Lda,
        A::Immediate,
        O::Word(crate::sfc_b1::STREAM_BUDGET),
    );
    p.label("b1_text_post_limit");
    abs(p, M::Sta, LIMIT);
    abs(p, M::Stz, J);
    abs(p, M::Stz, SUM);
    p.label("b1_text_post_try");
    abs(p, M::Stz, COST);
    abs(p, M::Lda, LAST);
    branch(p, M::Bmi, "b1_text_post_no_main");
    abs(p, M::Cmp, J);
    branch(p, M::Bcc, "b1_text_post_no_main");
    implied(p, M::Sec);
    abs(p, M::Sbc, J);
    times_row(p);
    abs(p, M::Sta, COST);
    emit(p, M::Ldy, A::Absolute, O::Word(LAST));
    span_y(p);
    shl(p, 5);
    implied(p, M::Clc);
    abs(p, M::Adc, COST);
    emit(p, M::Adc, A::Immediate, O::Word(cost));
    abs(p, M::Sta, COST);
    abs(p, M::Stz, PER_ROW);
    abs(p, M::Stz, N);
    emit(p, M::Ldy, A::Absolute, O::Word(J));
    p.label("b1_text_post_rows_cost");
    span_y(p);
    branch(p, M::Beq, "b1_text_post_rows_cost_next");
    shl(p, 5);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(cost));
    implied(p, M::Clc);
    abs(p, M::Adc, N);
    abs(p, M::Sta, N);
    p.label("b1_text_post_rows_cost_next");
    implied(p, M::Iny);
    implied(p, M::Tya);
    abs(p, M::Cmp, LAST);
    branch(p, M::Beq, "b1_text_post_rows_cost");
    branch(p, M::Bcc, "b1_text_post_rows_cost");
    abs(p, M::Lda, N);
    abs(p, M::Cmp, COST);
    branch(p, M::Bcs, "b1_text_post_no_main");
    abs(p, M::Sta, COST);
    abs(p, M::Inc, PER_ROW);
    p.label("b1_text_post_no_main");
    abs(p, M::Lda, J);
    emit(p, M::Cmp, A::Immediate, O::Word(9));
    branch(p, M::Bcs, "b1_text_post_no_8");
    abs(p, M::Lda, SHOWN + 8);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    branch(p, M::Beq, "b1_text_post_no_8");
    shl(p, 5);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(cost));
    abs(p, M::Adc, COST);
    abs(p, M::Sta, COST);
    p.label("b1_text_post_no_8");
    abs(p, M::Lda, J);
    branch(p, M::Beq, "b1_text_post_no_map");
    shl(p, 6);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(cost));
    abs(p, M::Adc, COST);
    abs(p, M::Sta, COST);
    p.label("b1_text_post_no_map");
    abs(p, M::Lda, COST);
    abs(p, M::Cmp, LIMIT);
    branch(p, M::Beq, "b1_text_post_fits");
    branch(p, M::Bcs, "b1_text_post_more");
    p.label("b1_text_post_fits");
    jump(p, "b1_text_post_build");
    p.label("b1_text_post_more")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Lda, J);
    emit(p, M::Cmp, A::Immediate, O::Word(ROWS as u16));
    branch(p, M::Bcs, "b1_text_post_fail");
    implied(p, M::Tay);
    span_y(p);
    implied(p, M::Clc);
    abs(p, M::Adc, SUM);
    abs(p, M::Sta, SUM);
    abs(p, M::Inc, J);
    abs(p, M::Lda, SUM);
    abs(p, M::Cmp, POOL_TILES);
    branch(p, M::Beq, "b1_text_post_again");
    branch(p, M::Bcs, "b1_text_post_fail");
    p.label("b1_text_post_again");
    p.emit_label_ref(M::Brl, A::Relative16, "b1_text_post_try");
    p.label("b1_text_post_fail");
    abs(p, M::Lda, PAUSE);
    branch(p, M::Bne, "b1_text_post_rows");
    abs(p, M::Inc, PAUSE);
    p.emit_label_ref(M::Brl, A::Relative16, "b1_text_post_pass");
    p.label("b1_text_post_rows")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Stz, LP);
    abs(p, M::Stz, TAIL_COUNT);
    abs(p, M::Stz, SHARED);
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("b1_text_post_rows_row");
    span_y(p);
    branch(p, M::Beq, "b1_text_post_rows_next");
    shl(p, 5);
    abs(p, M::Sta, DN);
    implied(p, M::Tya);
    shl(p, 2);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(targets));
    abs(p, M::Sta, DV);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(targets + 2));
    abs(p, M::Sta, DS);
    set(p, DK, bank << 8);
    call(p, "b1_text_descriptor");
    p.label("b1_text_post_rows_next");
    implied(p, M::Iny);
    emit(p, M::Cpy, A::Immediate, O::Word(ROWS as u16));
    branch(p, M::Bne, "b1_text_post_rows_row");
    jump(p, "b1_text_post_job");
    p.label("b1_text_post_build")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Stz, LP);
    abs(p, M::Stz, SHARED);
    abs(p, M::Lda, J);
    branch(p, M::Beq, "b1_text_post_spares_done_far");
    shl(p, 6);
    emit(p, M::Dec, A::Accumulator, O::None);
    implied(p, M::Phb);
    emit(p, M::Ldx, A::Immediate, O::Word(text_map as u16));
    emit(p, M::Ldy, A::Immediate, O::Word(MAP_BLOCK as u16));
    emit(
        p,
        M::Mvn,
        A::BlockMove,
        O::BlockMove {
            destination_bank: (MAP_BLOCK >> 16) as u8,
            source_bank: (text_map >> 16) as u8,
        },
    );
    implied(p, M::Plb);
    set(p, RUN, 0xfffc);
    abs(p, M::Stz, RUN_LEFT);
    abs(p, M::Stz, ROW);
    branch(p, M::Bra, "b1_text_post_spare_row");
    p.label("b1_text_post_spares_done_far");
    jump(p, "b1_text_post_spares_done");
    p.label("b1_text_post_spare_row")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Lda, ROW);
    abs(p, M::Cmp, J);
    branch(p, M::Bcc, "b1_text_post_spare_this");
    jump(p, "b1_text_post_spares_done");
    p.label("b1_text_post_spare_this");
    implied(p, M::Tay);
    span_y(p);
    abs(p, M::Sta, SPAN);
    abs(p, M::Stz, COL);
    p.label("b1_text_post_piece");
    abs(p, M::Lda, COL);
    abs(p, M::Cmp, SPAN);
    branch(p, M::Bcc, "b1_text_post_piece_left");
    jump(p, "b1_text_post_spare_next");
    p.label("b1_text_post_piece_left");
    abs(p, M::Lda, RUN_LEFT);
    branch(p, M::Bne, "b1_text_post_run_held");
    abs(p, M::Lda, RUN);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(4));
    abs(p, M::Sta, RUN);
    implied(p, M::Clc);
    abs(p, M::Adc, POOL_BASE);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(pool));
    abs(p, M::Sta, RUN_AT);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(pool + 2));
    branch(p, M::Bpl, "b1_text_post_unshared");
    abs(p, M::Inc, SHARED);
    emit(p, M::And, A::Immediate, O::Word(!ARLE_RUN));
    p.label("b1_text_post_unshared");
    abs(p, M::Sta, RUN_LEFT);
    p.label("b1_text_post_run_held");
    abs(p, M::Lda, SPAN);
    implied(p, M::Sec);
    abs(p, M::Sbc, COL);
    abs(p, M::Cmp, RUN_LEFT);
    branch(p, M::Bcc, "b1_text_post_n");
    abs(p, M::Lda, RUN_LEFT);
    p.label("b1_text_post_n");
    abs(p, M::Sta, N);
    abs(p, M::Lda, RUN_AT);
    shr(p, 1);
    abs(p, M::Sta, DV);
    abs(p, M::Lda, N);
    shl(p, 5);
    abs(p, M::Sta, DN);
    abs(p, M::Lda, ROW);
    times_row(p);
    abs(p, M::Sta, DS);
    abs(p, M::Lda, COL);
    shl(p, 5);
    implied(p, M::Clc);
    abs(p, M::Adc, DS);
    emit(p, M::Adc, A::Immediate, O::Word(buffer));
    abs(p, M::Sta, DS);
    set(p, DK, bank << 8);
    call(p, "b1_text_descriptor");
    abs(p, M::Lda, ROW);
    shl(p, 6);
    abs(p, M::Sta, TMP);
    abs(p, M::Lda, COL);
    emit(p, M::Inc, A::Accumulator, O::None);
    shl(p, 1);
    implied(p, M::Clc);
    abs(p, M::Adc, TMP);
    implied(p, M::Tax);
    abs(p, M::Lda, RUN_AT);
    implied(p, M::Sec);
    emit(p, M::Sbc, A::Immediate, O::Word(crate::sfc_b1::BG2_TILES));
    shr(p, 5);
    emit(
        p,
        M::Ora,
        A::Immediate,
        O::Word((crate::sfc_b1::TEXT_PALETTE as u16) << 10),
    );
    abs(p, M::Sta, TMP);
    emit(p, M::Ldy, A::Absolute, O::Word(N));
    p.label("b1_text_post_entry");
    abs(p, M::Lda, TMP);
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(MAP_BLOCK));
    abs(p, M::Inc, TMP);
    implied(p, M::Inx);
    implied(p, M::Inx);
    implied(p, M::Dey);
    branch(p, M::Bne, "b1_text_post_entry");
    abs(p, M::Lda, N);
    shl(p, 5);
    implied(p, M::Clc);
    abs(p, M::Adc, RUN_AT);
    abs(p, M::Sta, RUN_AT);
    abs(p, M::Lda, RUN_LEFT);
    implied(p, M::Sec);
    abs(p, M::Sbc, N);
    abs(p, M::Sta, RUN_LEFT);
    abs(p, M::Lda, COL);
    implied(p, M::Clc);
    abs(p, M::Adc, N);
    abs(p, M::Sta, COL);
    jump(p, "b1_text_post_piece");
    p.label("b1_text_post_spare_next")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Inc, ROW);
    jump(p, "b1_text_post_spare_row");
    p.label("b1_text_post_spares_done")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Lda, LP);
    abs(p, M::Sta, GROUP);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(8));
    abs(p, M::Sta, LP);
    abs(p, M::Lda, J);
    branch(p, M::Beq, "b1_text_post_group_map_done");
    shl(p, 6);
    abs(p, M::Sta, DN);
    set(p, DV, WINDOW_MAP);
    set(p, DS, MAP_BLOCK as u16);
    set(p, DK, ((MAP_BLOCK >> 16) as u16) << 8);
    call(p, "b1_text_descriptor");
    p.label("b1_text_post_group_map_done");
    abs(p, M::Lda, LAST);
    branch(p, M::Bpl, "b1_text_post_group_some");
    jump(p, "b1_text_post_group_main_done");
    p.label("b1_text_post_group_some");
    abs(p, M::Cmp, J);
    branch(p, M::Bcs, "b1_text_post_group_from_j");
    jump(p, "b1_text_post_group_main_done");
    p.label("b1_text_post_group_from_j");
    abs(p, M::Lda, PER_ROW);
    branch(p, M::Beq, "b1_text_post_group_joined");
    emit(p, M::Ldy, A::Absolute, O::Word(J));
    p.label("b1_text_post_group_row");
    span_y(p);
    branch(p, M::Beq, "b1_text_post_group_row_next");
    shl(p, 5);
    abs(p, M::Sta, DN);
    implied(p, M::Tya);
    shl(p, 2);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(targets));
    abs(p, M::Sta, DV);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(targets + 2));
    abs(p, M::Sta, DS);
    set(p, DK, bank << 8);
    call(p, "b1_text_descriptor");
    p.label("b1_text_post_group_row_next");
    implied(p, M::Iny);
    implied(p, M::Tya);
    abs(p, M::Cmp, LAST);
    branch(p, M::Beq, "b1_text_post_group_row");
    branch(p, M::Bcc, "b1_text_post_group_row");
    jump(p, "b1_text_post_group_main_done");
    p.label("b1_text_post_group_joined")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Lda, LAST);
    implied(p, M::Sec);
    abs(p, M::Sbc, J);
    times_row(p);
    abs(p, M::Sta, DN);
    emit(p, M::Ldy, A::Absolute, O::Word(LAST));
    span_y(p);
    shl(p, 5);
    implied(p, M::Clc);
    abs(p, M::Adc, DN);
    abs(p, M::Sta, DN);
    abs(p, M::Lda, J);
    shl(p, 2);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(targets));
    abs(p, M::Sta, DV);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(targets + 2));
    abs(p, M::Sta, DS);
    set(p, DK, bank << 8);
    call(p, "b1_text_descriptor");
    p.label("b1_text_post_group_main_done");
    abs(p, M::Lda, J);
    emit(p, M::Cmp, A::Immediate, O::Word(9));
    branch(p, M::Bcs, "b1_text_post_group_8_done");
    abs(p, M::Lda, SHOWN + 8);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    branch(p, M::Beq, "b1_text_post_group_8_done");
    shl(p, 5);
    abs(p, M::Sta, DN);
    set(p, DV, ROW8_HOME / 2);
    set(p, DS, buffer + 8 * (COLUMNS as u16) * 32);
    set(p, DK, bank << 8);
    call(p, "b1_text_descriptor");
    p.label("b1_text_post_group_8_done");
    abs(p, M::Ldx, GROUP);
    abs(p, M::Lda, LP);
    implied(p, M::Sec);
    abs(p, M::Sbc, GROUP);
    shr(p, 3);
    emit(p, M::Dec, A::Accumulator, O::None);
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(LIST + 1));
    abs(p, M::Lda, COST);
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(LIST + 3));
    abs(p, M::Lda, PAUSE);
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(LIST + 5));
    m8(p);
    emit(p, M::Lda, A::Immediate, O::Byte(GROUP_KIND));
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(LIST));
    m16(p);
    abs(p, M::Lda, LP);
    abs(p, M::Sta, TAIL_FROM);
    abs(p, M::Lda, J);
    branch(p, M::Bne, "b1_text_post_back");
    jump(p, "b1_text_post_back_done");
    p.label("b1_text_post_back");
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("b1_text_post_back_row");
    span_y(p);
    branch(p, M::Beq, "b1_text_post_back_next");
    shl(p, 5);
    abs(p, M::Sta, DN);
    implied(p, M::Tya);
    shl(p, 2);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(targets));
    abs(p, M::Sta, DV);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(targets + 2));
    abs(p, M::Sta, DS);
    set(p, DK, bank << 8);
    call(p, "b1_text_descriptor");
    p.label("b1_text_post_back_next");
    implied(p, M::Iny);
    implied(p, M::Tya);
    abs(p, M::Cmp, J);
    branch(p, M::Bcc, "b1_text_post_back_row");
    p.label("b1_text_post_back_map");
    abs(p, M::Lda, J);
    shl(p, 6);
    abs(p, M::Sta, DN);
    set(p, DV, WINDOW_MAP);
    set(p, DS, text_map as u16);
    set(p, DK, ((text_map >> 16) as u16) << 8);
    call(p, "b1_text_descriptor");
    p.label("b1_text_post_back_done");
    abs(p, M::Lda, LP);
    implied(p, M::Sec);
    abs(p, M::Sbc, TAIL_FROM);
    shr(p, 3);
    abs(p, M::Sta, TAIL_COUNT);
    p.label("b1_text_post_job");
    abs(p, M::Lda, LP);
    shr(p, 3);
    abs(p, M::Sta, JOB + 3);
    emit(p, M::Lda, A::Immediate, O::Word(LIST as u16));
    abs(p, M::Sta, JOB);
    emit(p, M::Lda, A::Immediate, O::Word(JOB));
    abs(p, M::Sta, 0xe0);
    m8(p);
    emit(p, M::Lda, A::Immediate, O::Byte((LIST >> 16) as u8));
    abs(p, M::Sta, JOB + 2);
    abs(p, M::Stz, 0xe2);
    emit(p, M::Ldx, A::Immediate, O::Word(page_slot));
    call(p, "b1_enqueue");
    implied(p, M::Rts);
    p.label("b1_text_descriptor")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Ldx, LP);
    abs(p, M::Lda, DK);
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(LIST));
    abs(p, M::Lda, DV);
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(LIST + 1));
    abs(p, M::Lda, DN);
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(LIST + 3));
    abs(p, M::Lda, DS);
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(LIST + 5));
    abs(p, M::Lda, DK);
    implied(p, M::Xba);
    m8(p);
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(LIST + 7));
    m16(p);
    implied(p, M::Txa);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(8));
    abs(p, M::Sta, LP);
    implied(p, M::Rts);
    const _: () = assert!(LIST_MAX * 8 <= (MAP_BLOCK - LIST) as u16);
    p.label("b1_text_work").assume_mode(CpuMode::NATIVE_M8_X16);
    long(p, M::Lda, SERIAL);
    emit(p, M::Cmp, A::AbsoluteLong, O::Long(u32::from(DONE)));
    branch(p, M::Bne, "b1_text_work_start");
    implied(p, M::Rts);
    p.label("b1_text_work_start");
    long(p, M::Lda, count);
    long(p, M::Ora, count + 1);
    branch(p, M::Beq, "b1_text_work_free");
    implied(p, M::Rts);
    p.label("b1_text_work_free");
    implied(p, M::Php);
    implied(p, M::Phd);
    implied(p, M::Phb);
    m16(p);
    emit(p, M::Lda, A::Immediate, O::Word(DP));
    implied(p, M::Tcd);
    m8(p);
    p.label("b1_text_restart")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    long(p, M::Lda, SERIAL);
    dp(p, M::Sta, DRAWING);
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x30));
    dp(p, M::Stz, ROW_INDEX);
    dp(p, M::Stz, ROW_OFFSET);
    p.label("b1_text_restore_row");
    dp(p, M::Ldx, ROW_INDEX);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(u32::from(EXTENT)));
    emit(p, M::And, A::Immediate, O::Word(0xff));
    branch(p, M::Beq, "b1_text_restore_next");
    for _ in 0..5 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    emit(p, M::Dec, A::Accumulator, O::None);
    dp(p, M::Sta, TEMP);
    dp(p, M::Lda, ROW_OFFSET);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(blank as u16));
    implied(p, M::Tax);
    dp(p, M::Ldy, ROW_OFFSET);
    dp(p, M::Lda, TEMP);
    emit(
        p,
        M::Mvn,
        A::BlockMove,
        O::BlockMove {
            destination_bank: (BUFFER >> 16) as u8,
            source_bank: (blank >> 16) as u8,
        },
    );
    m8(p);
    dp(p, M::Ldx, ROW_INDEX);
    emit(p, M::Lda, A::Immediate, O::Byte(0));
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(u32::from(EXTENT)));
    m16(p);
    p.label("b1_text_restore_next");
    dp(p, M::Lda, ROW_OFFSET);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word((COLUMNS * 32) as u16));
    dp(p, M::Sta, ROW_OFFSET);
    dp(p, M::Inc, ROW_INDEX);
    dp(p, M::Lda, ROW_INDEX);
    emit(p, M::Cmp, A::Immediate, O::Word(ROWS as u16));
    branch(p, M::Bne, "b1_text_restore_row");
    p.label("b1_text_restored");
    emit(
        p,
        M::Pea,
        A::Absolute,
        O::Word(((BUFFER >> 16) as u16) << 8 | (BUFFER >> 16) as u16),
    );
    implied(p, M::Plb);
    implied(p, M::Plb);
    long(p, M::Lda, PAGE);
    call(p, "b1_text_cached_page");
    branch(p, M::Bcc, "b1_text_uncached_page");
    m8(p);
    jump(p, "b1_text_publish");
    p.label("b1_text_uncached_page")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    long(p, M::Lda, PAGE);
    emit(p, M::Cmp, A::Immediate, O::Word(RUNTIME_PAGE));
    branch(p, M::Bne, "b1_text_rom_page");
    emit(p, M::Lda, A::Immediate, O::Word(RUNTIME_CODES as u16));
    dp(p, M::Sta, CODE);
    m8(p);
    emit(
        p,
        M::Lda,
        A::Immediate,
        O::Byte((RUNTIME_CODES >> 16) as u8),
    );
    branch(p, M::Bra, "b1_text_page_bank");
    p.label("b1_text_rom_page")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(offsets));
    dp(p, M::Sta, CODE);
    m8(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(offsets + 2));
    p.label("b1_text_page_bank");
    dp(p, M::Sta, CODE + 2);
    m16(p);
    dp(p, M::Stz, SKIP);
    long(p, M::Lda, PAGE);
    emit(p, M::Cmp, A::Immediate, O::Word(RUNTIME_PAGE));
    branch(p, M::Bne, "b1_text_lines_counted");
    dp(p, M::Stz, TEMP);
    dp(p, M::Stz, ROW_INDEX);
    emit(p, M::Lda, A::Immediate, O::Word(PEN[0] as u16));
    dp(p, M::Sta, ROW_OFFSET);
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("b1_text_count")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(CODE));
    implied(p, M::Iny);
    emit(p, M::Cmp, A::Immediate, O::Byte(END));
    branch(p, M::Beq, "b1_text_counted");
    emit(p, M::Cmp, A::Immediate, O::Byte(BREAK));
    branch(p, M::Bne, "b1_text_count_code");
    m16(p);
    p.label("b1_text_count_line")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Inc, TEMP);
    emit(p, M::Lda, A::Immediate, O::Word(PEN[0] as u16));
    dp(p, M::Sta, ROW_OFFSET);
    branch(p, M::Bra, "b1_text_count");
    p.label("b1_text_count_code")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Cmp, A::Immediate, O::Byte(FIRST_GLYPH));
    branch(p, M::Bcc, "b1_text_count");
    emit(p, M::Cmp, A::Immediate, O::Byte(PREFIX));
    branch(p, M::Bcc, "b1_text_count_one");
    implied(p, M::Sec);
    emit(p, M::Sbc, A::Immediate, O::Byte(PREFIX));
    implied(p, M::Xba);
    emit(p, M::Lda, A::DirectPageIndirectLongIndexedY, O::Byte(CODE));
    implied(p, M::Iny);
    m16(p);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(ONE_BYTE as u16));
    branch(p, M::Bra, "b1_text_count_index");
    p.label("b1_text_count_one")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    implied(p, M::Sec);
    emit(p, M::Sbc, A::Immediate, O::Word(u16::from(FIRST_GLYPH)));
    p.label("b1_text_count_index")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    for _ in 0..5 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(glyphs));
    dp(p, M::Sta, CLEAR);
    implied(p, M::Xba);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    implied(p, M::Clc);
    dp(p, M::Adc, ROW_OFFSET);
    emit(p, M::Cmp, A::Immediate, O::Word((COLUMNS * 8 + 1) as u16));
    branch(p, M::Bcc, "b1_text_count_fits");
    dp(p, M::Inc, TEMP);
    emit(p, M::Lda, A::Immediate, O::Word(PEN[0] as u16));
    dp(p, M::Sta, ROW_OFFSET);
    p.label("b1_text_count_fits")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Lda, TEMP);
    dp(p, M::Sta, ROW_INDEX);
    dp(p, M::Lda, CLEAR);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    implied(p, M::Clc);
    dp(p, M::Adc, ROW_OFFSET);
    dp(p, M::Sta, ROW_OFFSET);
    branch(p, M::Bra, "b1_text_count");
    p.label("b1_text_counted")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    dp(p, M::Lda, ROW_INDEX);
    implied(p, M::Sec);
    emit(p, M::Sbc, A::Immediate, O::Word(LINES as u16 - 1));
    branch(p, M::Bcc, "b1_text_lines_counted");
    dp(p, M::Sta, SKIP);
    p.label("b1_text_lines_counted")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    emit(p, M::Lda, A::Immediate, O::Word(PEN[0] as u16));
    dp(p, M::Sta, PEN_X);
    emit(p, M::Lda, A::Immediate, O::Word(PEN[1] as u16));
    dp(p, M::Sta, LINE_Y);
    emit(p, M::Lda, A::Immediate, O::Word(u16::from(DEFAULT_COLOR)));
    call(p, "b1_text_color");
    p.label("b1_text_next").assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    long(p, M::Lda, SERIAL);
    dp(p, M::Cmp, DRAWING);
    branch(p, M::Beq, "b1_text_current");
    call(p, "b1_text_extent");
    jump(p, "b1_text_restart");
    p.label("b1_text_current")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Lda, A::DirectPageIndirectLong, O::Byte(CODE));
    m16(p);
    dp(p, M::Inc, CODE);
    m8(p);
    emit(p, M::Cmp, A::Immediate, O::Byte(END));
    branch(p, M::Bne, "b1_text_not_end");
    jump(p, "b1_text_finish");
    p.label("b1_text_not_end")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Cmp, A::Immediate, O::Byte(BREAK));
    branch(p, M::Bne, "b1_text_not_break");
    call(p, "b1_text_extent");
    m16(p);
    call(p, "b1_text_newline");
    jump(p, "b1_text_next");
    p.label("b1_text_not_break")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Cmp, A::Immediate, O::Byte(FIRST_GLYPH));
    branch(p, M::Bcs, "b1_text_glyph");
    m16(p);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    implied(p, M::Sec);
    emit(p, M::Sbc, A::Immediate, O::Word(u16::from(COLOR) - 1));
    call(p, "b1_text_color");
    branch(p, M::Bra, "b1_text_next");
    p.label("b1_text_glyph").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Cmp, A::Immediate, O::Byte(PREFIX));
    branch(p, M::Bcc, "b1_text_one_byte");
    implied(p, M::Sec);
    emit(p, M::Sbc, A::Immediate, O::Byte(PREFIX));
    implied(p, M::Xba);
    emit(p, M::Lda, A::DirectPageIndirectLong, O::Byte(CODE));
    m16(p);
    dp(p, M::Inc, CODE);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(ONE_BYTE as u16));
    branch(p, M::Bra, "b1_text_index");
    p.label("b1_text_one_byte")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    implied(p, M::Sec);
    emit(p, M::Sbc, A::Immediate, O::Word(u16::from(FIRST_GLYPH)));
    p.label("b1_text_index")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    for _ in 0..5 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(glyphs as u16));
    dp(p, M::Sta, GLYPH);
    m8(p);
    emit(p, M::Lda, A::Immediate, O::Byte((glyphs >> 16) as u8));
    dp(p, M::Sta, GLYPH + 2);
    m16(p);
    dp(p, M::Lda, PEN_X);
    emit(
        p,
        M::Cmp,
        A::Immediate,
        O::Word((COLUMNS * 8 - 16 + 1) as u16),
    );
    branch(p, M::Bcc, "b1_text_glyph_fits");
    emit(p, M::Lda, A::DirectPageIndirectLong, O::Byte(GLYPH));
    implied(p, M::Xba);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    implied(p, M::Clc);
    dp(p, M::Adc, PEN_X);
    emit(p, M::Cmp, A::Immediate, O::Word((COLUMNS * 8 + 1) as u16));
    branch(p, M::Bcc, "b1_text_glyph_fits");
    m8(p);
    call(p, "b1_text_extent");
    m16(p);
    call(p, "b1_text_newline");
    p.label("b1_text_glyph_fits")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Lda, SKIP);
    branch(p, M::Bne, "b1_text_glyph_skipped");
    call(p, "b1_text_glyph_draw");
    p.label("b1_text_glyph_skipped")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    emit(p, M::Lda, A::DirectPageIndirectLong, O::Byte(GLYPH));
    emit(p, M::And, A::Immediate, O::Word(0xff));
    implied(p, M::Clc);
    dp(p, M::Adc, PEN_X);
    dp(p, M::Sta, PEN_X);
    jump(p, "b1_text_next");
    p.label("b1_text_finish")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    call(p, "b1_text_extent");
    p.label("b1_text_publish")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    long(p, M::Lda, SERIAL);
    dp(p, M::Cmp, DRAWING);
    branch(p, M::Beq, "b1_text_drawn");
    jump(p, "b1_text_restart");
    p.label("b1_text_drawn").assume_mode(CpuMode::NATIVE_M8_X16);
    long(p, M::Sta, DONE);
    implied(p, M::Plb);
    implied(p, M::Pld);
    implied(p, M::Plp);
    p.label("b1_text_work_end")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Rts);
    p.label("b1_text_newline")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    emit(p, M::Lda, A::Immediate, O::Word(PEN[0] as u16));
    dp(p, M::Sta, PEN_X);
    dp(p, M::Lda, SKIP);
    branch(p, M::Beq, "b1_text_newline_down");
    emit(p, M::Cmp, A::Immediate, O::Word(HIDDEN));
    branch(p, M::Beq, "b1_text_newline_done");
    dp(p, M::Dec, SKIP);
    implied(p, M::Rts);
    p.label("b1_text_newline_down")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Lda, LINE_Y);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(DIALOGUE_LINE as u16));
    emit(p, M::Cmp, A::Immediate, O::Word(LAST_LINE_Y as u16 + 1));
    branch(p, M::Bcc, "b1_text_newline_line");
    emit(p, M::Lda, A::Immediate, O::Word(HIDDEN));
    dp(p, M::Sta, SKIP);
    implied(p, M::Rts);
    p.label("b1_text_newline_line")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Sta, LINE_Y);
    p.label("b1_text_newline_done")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Rts);
    p.label("b1_text_extent")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    dp(p, M::Lda, PEN_X);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(15));
    for _ in 0..3 {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    emit(p, M::Inc, A::Accumulator, O::None);
    emit(p, M::Cmp, A::Immediate, O::Word(COLUMNS as u16));
    branch(p, M::Bcc, "b1_text_extent_columns");
    emit(p, M::Lda, A::Immediate, O::Word(COLUMNS as u16));
    p.label("b1_text_extent_columns");
    dp(p, M::Sta, TEMP);
    dp(p, M::Lda, LINE_Y);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(DIALOGUE_LINE as u16));
    for _ in 0..3 {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    emit(p, M::Cmp, A::Immediate, O::Word(ROWS as u16));
    branch(p, M::Bcc, "b1_text_extent_last");
    emit(p, M::Lda, A::Immediate, O::Word(ROWS as u16 - 1));
    p.label("b1_text_extent_last");
    dp(p, M::Sta, ROW_INDEX);
    dp(p, M::Lda, LINE_Y);
    for _ in 0..3 {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    emit(p, M::Cmp, A::Immediate, O::Word(ROWS as u16));
    branch(p, M::Bcc, "b1_text_extent_first");
    m8(p);
    implied(p, M::Rts);
    p.label("b1_text_extent_first")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Tax);
    m8(p);
    p.label("b1_text_extent_row");
    dp(p, M::Lda, TEMP);
    emit(p, M::Cmp, A::AbsoluteLongX, O::Long(u32::from(EXTENT)));
    branch(p, M::Bcc, "b1_text_extent_kept");
    emit(p, M::Sta, A::AbsoluteLongX, O::Long(u32::from(EXTENT)));
    p.label("b1_text_extent_kept");
    implied(p, M::Txa);
    dp(p, M::Cmp, ROW_INDEX);
    implied(p, M::Inx);
    branch(p, M::Bcc, "b1_text_extent_row");
    implied(p, M::Rts);
    p.label("b1_text_color")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Sta, INK_COLOR);
    implied(p, M::Rts);
    p.label("b1_text_glyph_draw")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    emit(
        p,
        M::Jsl,
        A::AbsoluteLong,
        O::Long(crate::sfc_b1_text_draw::entry(layout)?),
    );
    implied(p, M::Rts);
    cache::emit_dispatch(p, layout)?;
    Ok(())
}
