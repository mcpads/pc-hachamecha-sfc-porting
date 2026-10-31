use crate::Result;
use std::collections::BTreeMap;
use w65c816::{
    AddressingMode as A, AssembledProgram, Assembler, CodeLocation, CpuMode, Mnemonic as M,
    Operand as O, Width, WidthState,
};
const MARK: &str = "@enter=";
fn widths_name(w: WidthState) -> &'static str {
    match (w.accumulator, w.index) {
        (Width::Eight, Width::Eight) => "M8X8",
        (Width::Eight, Width::Sixteen) => "M8X16",
        (Width::Sixteen, Width::Eight) => "M16X8",
        (Width::Sixteen, Width::Sixteen) => "M16X16",
    }
}
#[allow(dead_code)]
pub fn entered_with(p: &mut Assembler, label: &str, mode: CpuMode) {
    p.label(format!("{label}{MARK}{}", widths_name(mode.widths())));
}
fn quoted_after(text: &str, key: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find(key) {
        rest = &rest[at + key.len()..];
        let mut name = String::new();
        let mut chars = rest.char_indices();
        let mut end = rest.len();
        while let Some((i, c)) = chars.next() {
            match c {
                '\\' => {
                    if let Some((_, e)) = chars.next() {
                        name.push(e);
                    }
                }
                '"' => {
                    end = i + 1;
                    break;
                }
                _ => name.push(c),
            }
        }
        found.push(name);
        rest = &rest[end..];
    }
    found
}
fn label_names(p: &Assembler, program: &AssembledProgram) -> Result<Vec<String>> {
    let text = format!("{p:?}");
    let names = quoted_after(&text, "Label(\"");
    let referred = quoted_after(&text, "label: \"");
    let unreadable = || -> Box<dyn std::error::Error> {
        "Label names could not be read from the assembler (its Debug form changed?)".into()
    };
    if names.is_empty() && (!referred.is_empty() || text.contains("Label")) {
        return Err(unreadable());
    }
    if names.iter().any(|n| program.label_location(n).is_none()) {
        return Err(unreadable());
    }
    let known: std::collections::BTreeSet<&str> = names.iter().map(String::as_str).collect();
    if referred.iter().any(|n| !known.contains(n.as_str())) {
        return Err(unreadable());
    }
    Ok(names)
}
fn differing(a: WidthState, b: WidthState) -> u8 {
    let mut bits = 0;
    if a.accumulator != b.accumulator {
        bits |= 0x20;
    }
    if a.index != b.index {
        bits |= 0x10;
    }
    bits
}
fn depends(ins: &w65c816::Instruction, bits: u8) -> Option<bool> {
    let index_only = matches!(
        ins.mnemonic,
        M::Ldx
            | M::Ldy
            | M::Stx
            | M::Sty
            | M::Cpx
            | M::Cpy
            | M::Inx
            | M::Iny
            | M::Dex
            | M::Dey
            | M::Phx
            | M::Phy
            | M::Plx
            | M::Ply
            | M::Txy
            | M::Tyx
    );
    let neutral = matches!(
        ins.mnemonic,
        M::Clc
            | M::Sec
            | M::Cli
            | M::Sei
            | M::Cld
            | M::Sed
            | M::Clv
            | M::Nop
            | M::Phb
            | M::Plb
            | M::Phd
            | M::Pld
            | M::Phk
            | M::Php
            | M::Tcd
            | M::Tcs
            | M::Tdc
            | M::Tsc
            | M::Xba
    );
    let control = matches!(
        ins.mnemonic,
        M::Bcc
            | M::Bcs
            | M::Beq
            | M::Bne
            | M::Bmi
            | M::Bpl
            | M::Bvc
            | M::Bvs
            | M::Bra
            | M::Brl
            | M::Jmp
            | M::Jml
            | M::Jsr
            | M::Jsl
            | M::Rts
            | M::Rtl
            | M::Rti
            | M::Brk
            | M::Cop
            | M::Wai
            | M::Stp
            | M::Xce
            | M::Plp
    );
    if control {
        return None;
    }
    if neutral {
        return Some(false);
    }
    let indexed = matches!(
        ins.mode,
        A::DirectPageX
            | A::DirectPageY
            | A::DirectPageIndexedIndirectX
            | A::DirectPageIndirectIndexedY
            | A::DirectPageIndirectLongIndexedY
            | A::AbsoluteX
            | A::AbsoluteY
            | A::AbsoluteLongX
            | A::AbsoluteIndexedIndirectX
            | A::StackRelativeIndirectIndexedY
            | A::BlockMove
    ) || matches!(
        ins.mnemonic,
        M::Tax | M::Tay | M::Txa | M::Tya | M::Txs | M::Tsx
    );
    let on_m = bits & 0x20 != 0 && !index_only;
    let on_x = bits & 0x10 != 0 && (index_only || indexed);
    Some(on_m || on_x)
}
fn settles(program: &AssembledProgram, k: usize, mut bits: u8) -> bool {
    for span in &program.instruction_spans()[k..] {
        let ins = &span.instruction;
        if let (M::Rep | M::Sep, O::Byte(mask)) = (ins.mnemonic, &ins.operand) {
            bits &= !mask;
            if bits == 0 {
                return true;
            }
            continue;
        }
        if depends(ins, bits) != Some(false) {
            return false;
        }
    }
    false
}
pub struct Mismatch {
    pub site: u32,
    pub site_label: String,
    pub site_widths: &'static str,
    pub target: u32,
    pub target_label: String,
    pub target_widths: &'static str,
}
impl std::fmt::Display for Mismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:06x} in {} ({}) -> {:06x} {} (assembled {})",
            self.site,
            self.site_label,
            self.site_widths,
            self.target,
            self.target_label,
            self.target_widths
        )
    }
}
pub fn mismatches(p: &Assembler, program: &AssembledProgram) -> Result<Vec<Mismatch>> {
    let mut labels: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for name in label_names(p, program)? {
        if let Some(at) = program.label_location(&name) {
            labels.entry(at.to_u32()).or_default().push(name);
        }
    }
    let marked = |at: u32, widths: &str| {
        labels.get(&at).is_some_and(|names| {
            names
                .iter()
                .any(|n| n.split_once(MARK).is_some_and(|(_, w)| w == widths))
        })
    };
    let label_at = |at: u32| -> String {
        labels
            .range(..=at)
            .rev()
            .flat_map(|(a, names)| names.iter().map(move |n| (*a, n)))
            .find(|(_, n)| !n.contains(MARK))
            .map(|(a, n)| {
                if a == at {
                    n.clone()
                } else {
                    format!("{n}+{}", at - a)
                }
            })
            .unwrap_or_else(|| "?".into())
    };
    let widths: BTreeMap<u32, (usize, WidthState)> = program
        .instruction_width_states()
        .iter()
        .enumerate()
        .map(|(k, s)| (s.location.to_u32(), (k, s.widths)))
        .collect();
    let mut found = Vec::new();
    for (span, state) in program
        .instruction_spans()
        .iter()
        .zip(program.instruction_width_states())
    {
        let at = span.location.to_u32();
        let next = at + span.bytes.len().get() as u32;
        let ins = &span.instruction;
        let target = match (ins.mnemonic, ins.mode, &ins.operand) {
            (
                M::Bcc | M::Bcs | M::Beq | M::Bne | M::Bmi | M::Bpl | M::Bvc | M::Bvs | M::Bra,
                A::Relative8,
                O::Relative8(d),
            ) => next.wrapping_add_signed(i32::from(*d)),
            (M::Brl, A::Relative16, O::Relative16(d)) => next.wrapping_add_signed(i32::from(*d)),
            (M::Jmp, A::Absolute, O::Word(w)) => u32::from(*w),
            _ => continue,
        };
        let target = (at & 0xff_0000) | (target & 0xffff);
        let Some(&(k, assembled)) = widths.get(&target) else {
            continue;
        };
        if assembled == state.widths
            || settles(program, k, differing(assembled, state.widths))
            || marked(target, widths_name(state.widths))
        {
            continue;
        }
        found.push(Mismatch {
            site: at,
            site_label: label_at(at),
            site_widths: widths_name(state.widths),
            target,
            target_label: label_at(target),
            target_widths: widths_name(assembled),
        });
    }
    Ok(found)
}
pub fn assemble(p: &Assembler, origin: CodeLocation, mode: CpuMode) -> Result<AssembledProgram> {
    let program = p.assemble(origin, mode)?;
    let found = mismatches(p, &program)?;
    if found.is_empty() {
        return Ok(program);
    }
    let list: Vec<String> = found.iter().map(|m| m.to_string()).collect();
    Err(format!(
        "Branches reach code assembled for other widths (mark an intended entry with \
         sfc_width_check::entered_with):\n  {}",
        list.join("\n  ")
    )
    .into())
}
