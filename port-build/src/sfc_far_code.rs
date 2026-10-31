use crate::Result;
use sfc_builder::cartridge::{CartridgeLayout, Section};
use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
const CODE_SECTION: &str = "far_code";
const CODE_BYTES: usize = 0x1100;
const ENTRIES: [&str; 10] = [
    "b1_menu_compact",
    "b1_palette_window",
    "save_write",
    "save_read",
    "save_scatter",
    "save_slot_map",
    "title_enter",
    "title_scroll_apply",
    "title_tick",
    "title_ppu_restore",
];
const BACK: [&str; 7] = [
    "b1_enqueue",
    "b1_stream_all",
    "sound_music",
    "sound_effect",
    "sound_stop",
    "text_plane_reset",
    "restart",
];
const TRAMPOLINE_BYTES: u16 = 4;
pub fn append(sections: &mut Vec<Section>, space: &mut crate::sfc_rom::Space) -> Result<()> {
    let offset = space.reserve_fast(CODE_BYTES)?;
    sections.push(Section {
        name: CODE_SECTION.into(),
        offset,
        bytes: vec![0; CODE_BYTES],
    });
    Ok(())
}
pub fn emit_trampolines(p: &mut w65c816::Assembler) {
    use crate::sfc::implied;
    for name in BACK {
        p.assume_mode(CpuMode::NATIVE_M8_X16);
        p.emit_label_ref(M::Jsr, A::Absolute, name);
        implied(p, M::Rtl);
    }
}
pub fn emit(p: &mut w65c816::Assembler, layout: &CartridgeLayout) -> Result<()> {
    use crate::sfc::{emit, implied};
    let program = code(layout)?;
    let placed = &layout.section(CODE_SECTION)?.bytes;
    if placed[..program.bytes().len()] != *program.bytes() {
        return Err("Far code differs from its placed section".into());
    }
    for name in ENTRIES {
        let far = program
            .label_location(&format!("{name}_far"))
            .ok_or("Missing far code entry")?;
        p.label(name).assume_mode(CpuMode::NATIVE_M8_X16);
        emit(p, M::Jsl, A::AbsoluteLong, O::Long(far.to_u32()));
        implied(p, M::Rts);
    }
    Ok(())
}
pub fn place_code(sections: &mut [Section], layout: &CartridgeLayout) -> Result<()> {
    let mut bytes = code(layout)?.bytes().to_vec();
    bytes.resize(CODE_BYTES, 0);
    sections
        .iter_mut()
        .find(|s| s.name == CODE_SECTION)
        .ok_or("Missing far code section")?
        .bytes = bytes;
    Ok(())
}
pub fn code_bytes(layout: &CartridgeLayout) -> Result<usize> {
    Ok(code(layout)?.bytes().len())
}
fn code(layout: &CartridgeLayout) -> Result<w65c816::AssembledProgram> {
    use crate::sfc::{emit, implied};
    let at = layout.cpu_address(layout.section(CODE_SECTION)?.offset)?;
    let first = if layout.mapping() == sfc_builder::cartridge::Mapping::HiRom {
        0x80_8000u32
    } else {
        0x8000
    };
    let mut p = w65c816::Assembler::new();
    let p = &mut p;
    for name in ENTRIES {
        p.label(format!("{name}_far"))
            .assume_mode(CpuMode::NATIVE_M8_X16);
        p.emit_label_ref(M::Jsr, A::Absolute, name);
        implied(p, M::Rtl);
    }
    for (i, name) in BACK.iter().enumerate() {
        p.label(*name).assume_mode(CpuMode::NATIVE_M8_X16);
        let trampoline = first + u32::from(TRAMPOLINE_BYTES) * i as u32;
        emit(p, M::Jsl, A::AbsoluteLong, O::Long(trampoline));
        implied(p, M::Rts);
    }
    crate::sfc_b1_menu::emit_compact(p);
    crate::sfc_b1_stone::emit_palette_window(p);
    crate::sfc_save::emit(p, layout)?;
    let pressed = sfc_builder::runtime::PadStorage::new(0x300)?.pressed();
    crate::sfc_title::emit(p, layout, pressed)?;
    crate::sfc_battle_objects::emit_vertical_far(p);
    crate::sfc_battle_objects::emit_orbit_far(p, layout)?;
    crate::sfc_battle_objects::emit_falling_far(p, layout)?;
    crate::sfc_battle_objects::emit_initializers_far(p, layout)?;
    crate::sfc_battle_objects::emit_arithmetic_block_far(p, layout)?;
    crate::sfc_battle_objects::emit_inward_far(p);
    let program = crate::sfc_width_check::assemble(
        p,
        w65c816::CodeLocation::new((at >> 16) as u8, at as u16),
        CpuMode::NATIVE_M8_X16,
    )?;
    if program.bytes().len() > CODE_BYTES {
        return Err(format!("Far code takes {} bytes", program.bytes().len()).into());
    }
    check_banks(&program)?;
    Ok(program)
}
fn check_banks(program: &w65c816::AssembledProgram) -> Result<()> {
    for span in program.instruction_spans() {
        let i = &span.instruction;
        let data = matches!(i.mode, A::Absolute | A::AbsoluteX | A::AbsoluteY)
            && !matches!(i.mnemonic, M::Jsr | M::Jmp);
        let rom = matches!(i.operand, O::Word(w) if w >= 0x8000);
        if matches!(i.mnemonic, M::Phk | M::Plb) || (data && rom) {
            return Err(format!(
                "Far code at {:?} assumes its own data bank: {:?}",
                span.location, i
            )
            .into());
        }
    }
    Ok(())
}
pub(crate) fn vertical_turn_entry(layout: &CartridgeLayout) -> Result<u32> {
    code(layout)?
        .label_location("bvm_vertical_turn_far")
        .map(|at| at.to_u32())
        .ok_or_else(|| "Missing vertical turn helper".into())
}
pub(crate) fn orbit_turn_entry(layout: &CartridgeLayout) -> Result<u32> {
    code(layout)?
        .label_location("bvm_orbit_turn_far")
        .map(|at| at.to_u32())
        .ok_or_else(|| "Missing orbit turn helper".into())
}
pub(crate) fn falling_turn_entry(layout: &CartridgeLayout) -> Result<u32> {
    code(layout)?
        .label_location("bvm_falling_turn_far")
        .map(|at| at.to_u32())
        .ok_or_else(|| "Missing falling turn helper".into())
}
pub(crate) fn initializer_entry(layout: &CartridgeLayout) -> Result<u32> {
    code(layout)?
        .label_location("bvm_initializer_far")
        .map(|at| at.to_u32())
        .ok_or_else(|| "Missing compiled initializer helper".into())
}
pub(crate) fn arithmetic_block_entry(layout: &CartridgeLayout) -> Result<u32> {
    code(layout)?
        .label_location("bvm_arithmetic_block_far")
        .map(|at| at.to_u32())
        .ok_or_else(|| "Missing far arithmetic block".into())
}
pub(crate) fn inward_turn_entry(layout: &CartridgeLayout) -> Result<u32> {
    code(layout)?
        .label_location("bvm_inward_turn_far")
        .map(|at| at.to_u32())
        .ok_or_else(|| "Missing inward turn helper".into())
}
