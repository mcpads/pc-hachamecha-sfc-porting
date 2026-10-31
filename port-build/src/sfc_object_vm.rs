use crate::{
    Result,
    sfc::{emit, implied, startup, write},
    sfc_object_pool::*,
    sha,
};
use sfc_builder::{
    cartridge::{CartridgeLayout, Section},
    runtime,
};
use w65c816::{AddressingMode as A, Assembler, CodeLocation, CpuMode, Mnemonic as M, Operand as O};
pub struct Program {
    pub start: u32,
    pub end: u32,
    pub bytes: Vec<u8>,
    pub lengths: Vec<u8>,
    pub sine: Vec<u8>,
    pub commands: usize,
}
pub fn compile(commands: &[crate::vm::Command], table: &[u16]) -> Result<Program> {
    if table.len() != 1024 {
        return Err("Missing object sine table".into());
    }
    let table_bytes: Vec<u8> = table.iter().flat_map(|n| n.to_le_bytes()).collect();
    let start = commands
        .iter()
        .map(|c| c.va)
        .min()
        .ok_or("Empty object program")?;
    let end = commands
        .iter()
        .map(|c| c.va + c.bytes.len() as u32)
        .max()
        .unwrap();
    if start >> 16 != (end - 1) >> 16 || end - start > 0x4000 {
        return Err("Object program crosses source bank".into());
    }
    let mut bytes = vec![0; (end - start) as usize];
    let mut lengths = bytes.clone();
    let mut occupied = vec![false; bytes.len()];
    for c in commands {
        if c.bytes.len() >= 4 && matches!(c.bytes[0], 17 | 18) && c.bytes[1] >> 6 == 2 {
            let offset = usize::from(c.bytes[2]);
            let width = if c.bytes[0] == 17 { 2 } else { 4 };
            if offset + width > 230 || (offset < 0x44 && offset + width > 0x40) {
                return Err("Arithmetic destination exceeds work area or overwrites PC".into());
            }
        }
        if c.bytes.first() == Some(&8) {
            validate_assignment(&c.bytes)?;
        }
        let at = (c.va - start) as usize;
        if c.bytes.len() < 4
            || c.bytes.len() > 255
            || occupied[at..at + c.bytes.len()].iter().any(|x| *x)
        {
            return Err("Invalid object command extent".into());
        }
        bytes[at..at + c.bytes.len()].copy_from_slice(&c.bytes);
        occupied[at..at + c.bytes.len()].fill(true);
        lengths[at] = c.bytes.len() as u8;
    }
    Ok(Program {
        start,
        end,
        bytes,
        lengths,
        sine: table_bytes,
        commands: commands.len(),
    })
}
impl Program {
    pub fn emit(&self, p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
        let address = |name: &str, expected: &[u8]| -> Result<u32> {
            let section = layout.section(name)?;
            if section.bytes.is_empty() || section.bytes != expected {
                return Err(
                    format!("Object section does not match compiled program: {name}").into(),
                );
            }
            let first = layout.cpu_address(section.offset)?;
            let last = layout.cpu_address(section.offset + section.bytes.len() - 1)?;
            if first >> 16 != last >> 16 {
                return Err(format!("Object section crosses CPU bank: {name}").into());
            }
            Ok(first)
        };
        let source = address("object_source", &self.bytes)?;
        let lengths = address("object_lengths", &self.lengths)?;
        let sine = address("object_sine", &self.sine)?;
        emit_routines(p);
        emit_vm(p, self.start, self.end, source, lengths);
        crate::sfc_object_motion::emit_routine(p, sine);
        Ok(())
    }
}
pub fn emit_reset(p: &mut Assembler) {
    for at in 0x3a0..0x430 {
        write(p, at, 0);
    }
    write(p, 0x3a0, 1);
    call(p, "pool_tick");
    write(p, 0x3b2, 0x42);
    write(p, 0x403, 0x42);
    bank(p);
    m16(p);
    word(p, M::Ldx, 0x7e00);
    word(p, M::Lda, 0);
    p.label("object_game_clear");
    ix(p, M::Sta, 0);
    implied(p, M::Inx);
    implied(p, M::Inx);
    word(p, M::Cpx, 0x7f00);
    branch(p, M::Bne, "object_game_clear");
    m8(p);
    implied(p, M::Plb);
}
pub fn build(commands: &[crate::vm::Command], table: &[u16]) -> Result<Vec<(String, Vec<u8>)>> {
    let program = compile(commands, table)?;
    let layout = CartridgeLayout::new(
        0x10000,
        0x7000,
        vec![
            Section {
                name: "object_sine".into(),
                offset: 0xe000,
                bytes: program.sine.clone(),
            },
            Section {
                name: "object_source".into(),
                offset: 0x8000,
                bytes: program.bytes.clone(),
            },
            Section {
                name: "object_lengths".into(),
                offset: 0xc000,
                bytes: program.lengths.clone(),
            },
        ],
    )?;
    let mut p = Assembler::new();
    startup(&mut p);
    emit_reset(&mut p);
    abs(&mut p, M::Lda, 0x4210);
    write(&mut p, 0x4200, 0x80);
    p.label("idle");
    implied(&mut p, M::Wai);
    call(&mut p, "pool_tick");
    call(&mut p, "object_tick");
    branch(&mut p, M::Bra, "idle");
    p.label("nmi");
    runtime::emit_interrupt_entry(&mut p);
    abs(&mut p, M::Lda, 0x4210);
    runtime::emit_interrupt_exit(&mut p);
    p.label("irq").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(&mut p, M::Rti);
    program.emit(&mut p, &layout)?;
    let code =
        crate::sfc_width_check::assemble(&p, CodeLocation::new(0, 0x8000), CpuMode::Emulation)?;
    let addr = |s| code.label_location(s).unwrap().program_counter();
    let rom = layout.build(code.bytes(), addr("reset"), addr("nmi"), addr("irq"))?;
    let report = serde_json::json!(
        { "rom_sha256" : sha(& rom), "code_bytes" : code.bytes().len(), "source_start" :
        program.start, "source_end" : program.end, "source_commands" : program.commands,
        "scope" :
        "Object control flow and pool operations with automatic group traversal; unsupported instructions stop before execution",
        "wram" : { "request" : 0x400, "state" : 0x401, "error" : 0x402, "ready" : 0x403,
        "busy" : 0x404, "instructions" : 0x406, "object" : 0x408, "command" : 0x410 },
        "supported_opcodes" : [0, 1, 2, 3, 4, 5, 6, 7, 8, 16, 17, 18, 19, 31, 40, 41, 45,
        75, 111], "motion_table_sha256" : sha(& program.sine), "limits" :
        ["Arithmetic modes are limited to source assignment/add/subtract/multiply/OR with own/reference operands; rendering, resource, message and audio instructions remain unsupported.",
        "No full prologue or B1 execution proof."] }
    );
    Ok(vec![
        ("object-vm.sfc".into(), rom),
        (
            "object-vm-build.json".into(),
            serde_json::to_vec_pretty(&report)?,
        ),
    ])
}
fn select(p: &mut Assembler, n: u8, label: &str) {
    let next = format!("object_not_{n}");
    byte(p, M::Cmp, n);
    branch(p, M::Bne, &next);
    jump(p, label);
    p.label(next);
}
fn pool(p: &mut Assembler, n: u8) {
    write(p, 0x3a0, n);
    call(p, "pool_tick");
    abs(p, M::Lda, 0x3a1);
    let ok = format!("object_pool_ok_{n}");
    branch(p, M::Beq, &ok);
    jump(p, "object_fault");
    p.label(ok);
}
fn emit_vm(p: &mut Assembler, start: u32, end: u32, source: u32, lengths: u32) {
    p.label("object_tick").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, 0x400);
    branch(p, M::Bne, "object_request");
    implied(p, M::Rts);
    p.label("object_request");
    abs(p, M::Sta, 0x405);
    bank(p);
    write(p, 0x400, 0);
    write(p, 0x404, 1);
    write(p, 0x402, 0);
    abs(p, M::Lda, 0x405);
    byte(p, M::Cmp, 1);
    branch(p, M::Beq, "object_start");
    jump(p, "object_fault");
    p.label("object_start");
    pool(p, 8);
    p.label("object_selected");
    abs(p, M::Lda, 0x3f0);
    byte(p, M::Cmp, 2);
    branch(p, M::Bne, "object_pending");
    write(p, 0x401, 1);
    jump(p, "object_exit");
    p.label("object_pending");
    byte(p, M::Cmp, 1);
    branch(p, M::Beq, "object_callback_check");
    jump(p, "object_fault");
    p.label("object_callback_check");
    m16(p);
    abs(p, M::Lda, 0x3f8);
    word(p, M::Cmp, 0xc891);
    m8(p);
    branch(p, M::Bne, "object_unsupported");
    m16(p);
    abs(p, M::Lda, 0x3fa);
    word(p, M::Cmp, 0x41);
    m8(p);
    branch(p, M::Bne, "object_unsupported");
    m16(p);
    abs(p, M::Lda, 0x3f2);
    abs(p, M::Sta, 0x408);
    word(p, M::Lda, 0);
    abs(p, M::Sta, 0x406);
    m8(p);
    jump(p, "object_dispatch");
    p.label("object_unsupported");
    write(p, 0x401, 2);
    jump(p, "object_exit");
    p.label("object_dispatch");
    m16(p);
    abs(p, M::Inc, 0x406);
    abs(p, M::Lda, 0x406);
    word(p, M::Cmp, 10001);
    m8(p);
    branch(p, M::Bcc, "object_budget_ok");
    jump(p, "object_fault");
    p.label("object_budget_ok");
    m16(p);
    abs(p, M::Ldx, 0x408);
    ix(p, M::Lda, 0x42);
    word(p, M::Cmp, (start >> 16) as u16);
    m8(p);
    branch(p, M::Beq, "object_pc_bank");
    jump(p, "object_fault");
    p.label("object_pc_bank");
    m16(p);
    ix(p, M::Lda, 0x40);
    word(p, M::Cmp, start as u16);
    m8(p);
    branch(p, M::Bcs, "object_pc_low");
    jump(p, "object_fault");
    p.label("object_pc_low");
    m16(p);
    word(p, M::Cmp, end as u16);
    m8(p);
    branch(p, M::Bcc, "object_pc_high");
    jump(p, "object_fault");
    p.label("object_pc_high");
    m16(p);
    implied(p, M::Sec);
    word(p, M::Sbc, start as u16);
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(lengths));
    abs(p, M::Sta, 0x40a);
    branch(p, M::Bne, "object_boundary");
    jump(p, "object_fault");
    p.label("object_boundary");
    abs(p, M::Stx, 0x40c);
    for i in 0..8 {
        emit(p, M::Lda, A::AbsoluteLongX, O::Long(source + i));
        abs(p, M::Sta, 0x410 + i as u16);
    }
    abs(p, M::Ldx, 0x408);
    abs(p, M::Lda, 0x410);
    for (op, label) in [
        (0, "object_delete"),
        (1, "object_yield"),
        (2, "object_wait"),
        (3, "object_jump"),
        (4, "object_call"),
        (5, "object_return"),
        (6, "object_loop"),
        (7, "object_spawn"),
        (8, "object_assignment"),
        (16, "object_reference_byte"),
        (17, "object_arithmetic"),
        (18, "object_arithmetic"),
        (19, "object_reference_condition"),
        (75, "object_input_reference"),
        (111, "object_game_reference"),
        (31, "object_link"),
        (40, "object_registry"),
        (41, "object_registered_delete"),
        (45, "object_motion"),
    ] {
        select(p, op, label);
    }
    jump(p, "object_unsupported");
    p.label("object_advance");
    m16(p);
    abs(p, M::Ldx, 0x408);
    abs(p, M::Lda, 0x40a);
    word(p, M::And, 255);
    implied(p, M::Clc);
    ix(p, M::Adc, 0x40);
    ix(p, M::Sta, 0x40);
    m8(p);
    jump(p, "object_dispatch");
    p.label("object_yield");
    m16(p);
    ix(p, M::Lda, 0x40);
    implied(p, M::Clc);
    word(p, M::Adc, 4);
    ix(p, M::Sta, 0x40);
    m8(p);
    jump(p, "object_callback_done");
    p.label("object_wait");
    m16(p);
    ix(p, M::Lda, 0x5e);
    branch(p, M::Bne, "object_wait_count");
    abs(p, M::Lda, 0x412);
    ix(p, M::Sta, 0x5e);
    m8(p);
    jump(p, "object_callback_done");
    p.label("object_wait_count")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    emit(p, M::Dec, A::Accumulator, O::None);
    ix(p, M::Sta, 0x5e);
    m8(p);
    branch(p, M::Bne, "object_wait_yield");
    jump(p, "object_advance");
    p.label("object_wait_yield");
    jump(p, "object_callback_done");
    p.label("object_jump");
    m16(p);
    copy_pointer(p, 0x40, 0x414, false);
    m8(p);
    jump(p, "object_dispatch");
    p.label("object_call");
    m16(p);
    ix(p, M::Lda, 0x5c);
    word(p, M::Cmp, 6);
    m8(p);
    branch(p, M::Bcc, "object_call_depth");
    jump(p, "object_fault");
    p.label("object_call_depth");
    m16(p);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, 0x408);
    implied(p, M::Tay);
    ix(p, M::Lda, 0x40);
    implied(p, M::Clc);
    word(p, M::Adc, 8);
    iy(p, M::Sta, 0x44);
    ix(p, M::Lda, 0x42);
    iy(p, M::Sta, 0x46);
    ix(p, M::Inc, 0x5c);
    m8(p);
    jump(p, "object_jump");
    p.label("object_return");
    m16(p);
    ix(p, M::Lda, 0x5c);
    branch(p, M::Beq, "object_return_bad");
    word(p, M::Cmp, 7);
    branch(p, M::Bcc, "object_return_depth");
    p.label("object_return_bad");
    m8(p);
    jump(p, "object_fault");
    p.label("object_return_depth")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    emit(p, M::Dec, A::Accumulator, O::None);
    ix(p, M::Sta, 0x5c);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, 0x408);
    implied(p, M::Tay);
    for i in [0, 2] {
        iy(p, M::Lda, 0x44 + i);
        ix(p, M::Sta, 0x40 + i);
    }
    m8(p);
    jump(p, "object_dispatch");
    p.label("object_loop");
    m16(p);
    ix(p, M::Lda, 0x3c);
    abs(p, M::Cmp, 0x414);
    branch(p, M::Bne, "object_loop_init");
    ix(p, M::Lda, 0x3e);
    abs(p, M::Cmp, 0x416);
    branch(p, M::Bne, "object_loop_init");
    ix(p, M::Lda, 0x60);
    emit(p, M::Dec, A::Accumulator, O::None);
    ix(p, M::Sta, 0x60);
    branch(p, M::Beq, "object_loop_end");
    m8(p);
    jump(p, "object_jump");
    p.label("object_loop_end")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    ix(p, M::Stz, 0x3c);
    ix(p, M::Stz, 0x3e);
    m8(p);
    jump(p, "object_advance");
    p.label("object_loop_init")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    copy_pointer(p, 0x3c, 0x414, false);
    abs(p, M::Lda, 0x412);
    ix(p, M::Sta, 0x60);
    m8(p);
    jump(p, "object_dispatch");
    p.label("object_spawn");
    abs(p, M::Lda, 0x3fc);
    abs(p, M::Sta, 0x3a2);
    abs(p, M::Lda, 0x411);
    abs(p, M::Sta, 0x3a3);
    for i in 0..4 {
        abs(p, M::Lda, 0x414 + i);
        abs(p, M::Sta, 0x3a8 + i);
    }
    pool(p, 2);
    jump(p, "object_advance");
    p.label("object_delete");
    abs(p, M::Lda, 0x3fc);
    abs(p, M::Sta, 0x3a5);
    pool(p, 3);
    jump(p, "object_callback_done");
    p.label("object_link");
    abs(p, M::Lda, 0x3fc);
    abs(p, M::Sta, 0x3a2);
    pool(p, 5);
    jump(p, "object_advance");
    p.label("object_registry");
    abs(p, M::Lda, 0x3fc);
    abs(p, M::Sta, 0x3a2);
    abs(p, M::Lda, 0x412);
    abs(p, M::Sta, 0x3a6);
    abs(p, M::Lda, 0x411);
    branch(p, M::Beq, "object_register");
    byte(p, M::Cmp, 1);
    branch(p, M::Beq, "object_reference");
    jump(p, "object_fault");
    p.label("object_register");
    pool(p, 4);
    jump(p, "object_advance");
    p.label("object_reference");
    pool(p, 7);
    jump(p, "object_advance");
    p.label("object_registered_delete");
    abs(p, M::Lda, 0x411);
    abs(p, M::Sta, 0x3a6);
    pool(p, 6);
    jump(p, "object_advance");
    p.label("object_callback_done");
    pool(p, 9);
    jump(p, "object_selected");
    p.label("object_fault");
    write(p, 0x402, 1);
    write(p, 0x401, 255);
    p.label("object_exit");
    write(p, 0x404, 0);
    implied(p, M::Plb);
    implied(p, M::Rts);
    emit_assignment(p, source);
    emit_references(p);
    crate::sfc_object_arithmetic::emit_routine(p);
}
fn validate_assignment(bytes: &[u8]) -> Result<()> {
    let mut at = 4;
    loop {
        let row = bytes.get(at..at + 4).ok_or("Truncated assignment row")?;
        if row[0] == 255 {
            if at + 4 != bytes.len() {
                return Err("Assignment has trailing bytes".into());
            }
            return Ok(());
        }
        let width = match row[0] {
            1 => 1,
            2 => 2,
            3 => 4,
            _ => return Err("Unknown assignment width".into()),
        };
        let offset = usize::from(row[1]);
        if offset + width > 230 || (offset < 0x44 && offset + width > 0x40) {
            return Err("Assignment exceeds work area or overwrites PC".into());
        }
        at += if width == 4 { 8 } else { 4 };
        if at > bytes.len() {
            return Err("Truncated assignment value".into());
        }
    }
}
fn emit_assignment(p: &mut Assembler, source: u32) {
    p.label("object_assignment")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, 0x40c);
    implied(p, M::Clc);
    word(p, M::Adc, 4);
    implied(p, M::Tax);
    m8(p);
    p.label("object_assignment_row");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(source));
    byte(p, M::Cmp, 255);
    branch(p, M::Bne, "object_assignment_value");
    jump(p, "object_advance");
    p.label("object_assignment_value");
    abs(p, M::Sta, 0x40e);
    m16(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(source + 1));
    word(p, M::And, 255);
    implied(p, M::Clc);
    abs(p, M::Adc, 0x408);
    implied(p, M::Tay);
    m8(p);
    abs(p, M::Lda, 0x40e);
    byte(p, M::Cmp, 3);
    branch(p, M::Beq, "object_assignment_dword");
    byte(p, M::Cmp, 2);
    branch(p, M::Beq, "object_assignment_word");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(source + 2));
    iy(p, M::Sta, 0);
    m16(p);
    branch(p, M::Bra, "object_assignment_short_next");
    p.label("object_assignment_word")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(source + 2));
    iy(p, M::Sta, 0);
    p.label("object_assignment_short_next");
    implied(p, M::Txa);
    implied(p, M::Clc);
    word(p, M::Adc, 4);
    branch(p, M::Bra, "object_assignment_next");
    p.label("object_assignment_dword")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    for i in [0, 2] {
        emit(
            p,
            M::Lda,
            A::AbsoluteLongX,
            O::Long(source + 4 + u32::from(i)),
        );
        iy(p, M::Sta, i);
    }
    implied(p, M::Txa);
    implied(p, M::Clc);
    word(p, M::Adc, 8);
    p.label("object_assignment_next");
    implied(p, M::Tax);
    m8(p);
    jump(p, "object_assignment_row");
}
fn emit_references(p: &mut Assembler) {
    for (label, address) in [
        ("object_input_reference", 0x4c46a0u32),
        ("object_game_reference", 0x4b47c0),
    ] {
        p.label(label).assume_mode(CpuMode::NATIVE_M8_X16);
        for (i, value) in address.to_le_bytes().iter().enumerate() {
            byte(p, M::Lda, *value);
            ix(p, M::Sta, 0xa8 + i as u16);
        }
        jump(p, "object_advance");
    }
    p.label("object_reference_byte")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, 0x411);
    byte(p, M::Cmp, 0xc0);
    branch(p, M::Beq, "object_reference_byte_mode");
    jump(p, "object_unsupported");
    p.label("object_reference_byte_mode");
    abs(p, M::Lda, 0x412);
    abs(p, M::Sta, 0x424);
    write(p, 0x425, 1);
    call(p, "object_reference_address");
    branch(p, M::Bcc, "object_reference_byte_store");
    jump(p, "object_fault");
    p.label("object_reference_byte_store");
    abs(p, M::Lda, 0x413);
    iy(p, M::Sta, 0);
    jump(p, "object_advance");
    p.label("object_reference_condition");
    abs(p, M::Lda, 0x411);
    byte(p, M::And, 0xfe);
    byte(p, M::Cmp, 0xc0);
    branch(p, M::Beq, "object_reference_condition_mode");
    jump(p, "object_unsupported");
    p.label("object_reference_condition_mode");
    abs(p, M::Lda, 0x412);
    abs(p, M::Sta, 0x424);
    write(p, 0x425, 1);
    call(p, "object_reference_address");
    branch(p, M::Bcc, "object_reference_condition_test");
    jump(p, "object_fault");
    p.label("object_reference_condition_test");
    iy(p, M::Lda, 0);
    abs(p, M::Cmp, 0x413);
    branch(p, M::Beq, "object_reference_equal");
    abs(p, M::Lda, 0x411);
    byte(p, M::Cmp, 0xc0);
    branch(p, M::Beq, "object_reference_taken");
    jump(p, "object_advance");
    p.label("object_reference_equal");
    abs(p, M::Lda, 0x411);
    byte(p, M::Cmp, 0xc1);
    branch(p, M::Beq, "object_reference_taken");
    jump(p, "object_advance");
    p.label("object_reference_taken");
    abs(p, M::Ldx, 0x408);
    jump(p, "object_jump");
    p.label("object_reference_address")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Ldx, 0x408);
    copy_pointer(p, 0xa8, 0x3c0, true);
    abs(p, M::Lda, 0x3c2);
    word(p, M::Cmp, 0x4b);
    branch(p, M::Bne, "object_reference_input_check");
    abs(p, M::Lda, 0x3c0);
    word(p, M::Cmp, 0x47c0);
    branch(p, M::Bne, "object_reference_address_bad16");
    word(p, M::Ldx, 0x7e00);
    word(p, M::Ldy, 256);
    branch(p, M::Bra, "object_reference_offset");
    p.label("object_reference_input_check");
    word(p, M::Cmp, 0x4c);
    branch(p, M::Bne, "object_reference_address_bad16");
    abs(p, M::Lda, 0x3c0);
    word(p, M::Cmp, 0x46a0);
    branch(p, M::Bne, "object_reference_work");
    word(p, M::Ldx, 0x7c70);
    word(p, M::Ldy, 144);
    branch(p, M::Bra, "object_reference_offset");
    p.label("object_reference_work");
    m8(p);
    call(p, "pool_pointer");
    branch(p, M::Bcs, "object_reference_address_bad");
    word(p, M::Cpx, 0x7000);
    branch(p, M::Bcs, "object_reference_address_bad");
    m16(p);
    word(p, M::Ldy, 230);
    p.label("object_reference_offset");
    abs(p, M::Sty, 0x420);
    abs(p, M::Stx, 0x422);
    abs(p, M::Lda, 0x424);
    word(p, M::And, 255);
    abs(p, M::Sta, 0x43c);
    abs(p, M::Lda, 0x425);
    word(p, M::And, 255);
    implied(p, M::Clc);
    abs(p, M::Adc, 0x43c);
    abs(p, M::Cmp, 0x420);
    branch(p, M::Bcc, "object_reference_range_ok");
    branch(p, M::Beq, "object_reference_range_ok");
    jump(p, "object_reference_address_bad16");
    p.label("object_reference_range_ok");
    abs(p, M::Lda, 0x43c);
    implied(p, M::Clc);
    abs(p, M::Adc, 0x422);
    implied(p, M::Tay);
    m8(p);
    implied(p, M::Clc);
    implied(p, M::Rts);
    p.label("object_reference_address_bad16")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    p.label("object_reference_address_bad")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Sec);
    implied(p, M::Rts);
}
