use crate::{
    Result,
    sfc::{emit, implied, startup, write},
    sha,
};
use sfc_builder::{
    cartridge::{CartridgeLayout, Section},
    runtime,
};
use w65c816::{AddressingMode as A, Assembler, CodeLocation, CpuMode, Mnemonic as M, Operand as O};
const START: u32 = 0x46e89c;
const END: u32 = 0x46ee98;
fn abs(p: &mut Assembler, m: M, at: u16) {
    emit(p, m, A::Absolute, O::Word(at));
}
fn byte(p: &mut Assembler, m: M, n: u8) {
    emit(p, m, A::Immediate, O::Byte(n));
}
fn word(p: &mut Assembler, m: M, n: u16) {
    emit(p, m, A::Immediate, O::Word(n));
}
fn m16(p: &mut Assembler) {
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
}
fn m8(p: &mut Assembler) {
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
}
fn jump(p: &mut Assembler, label: &str) {
    p.emit_label_ref(M::Brl, A::Relative16, label);
}
fn branch(p: &mut Assembler, m: M, label: &str) {
    p.emit_label_ref(m, A::Relative8, label);
}
fn call(p: &mut Assembler, label: &str) {
    p.emit_label_ref(M::Jsr, A::Absolute, label);
}
fn select(p: &mut Assembler, value: u8, label: &str, next: &str) {
    byte(p, M::Cmp, value);
    branch(p, M::Bne, next);
    jump(p, label);
    p.label(next);
}
pub struct Program {
    pub bytes: Vec<u8>,
    pub lengths: Vec<u8>,
    pub commands: usize,
}
pub fn compile(commands: &[crate::vm::Command]) -> Result<Program> {
    let mut program: Vec<_> = commands
        .iter()
        .filter(|c| (START..END).contains(&c.va))
        .collect();
    program.sort_by_key(|c| c.va);
    if program.len() != 249 {
        return Err("Incomplete prologue root".into());
    }
    let mut bytes = Vec::new();
    let mut lengths = vec![0; (END - START) as usize];
    for c in &program {
        if c.bytes.len() < 4 {
            return Err("Truncated root command".into());
        }
        if c.va != START + bytes.len() as u32 {
            return Err("Root command gap or overlap".into());
        }
        let length = match c.bytes[0] {
            1 | 2 | 5 | 0x10 | 0x25 | 0x28 | 0x29 | 0x3b | 0x4b | 0x6f => 4,
            6 | 7 | 0x12 | 0x26 | 0x2f | 0x38 | 0x39 | 0x3f => 8,
            0x8d => 12,
            _ => return Err("Unadmitted root opcode".into()),
        };
        if c.bytes.len() != length {
            return Err("Root command length changed".into());
        }
        if c.bytes[0] == 6 {
            let target = u32::from_le_bytes(c.bytes[4..8].try_into()?);
            if !program.iter().any(|c| c.va == target) {
                return Err("Root loop target is not a command".into());
            }
        }
        if c.bytes[0] == 0x10 && (c.bytes[1] != 0xc0 || ![0x19, 0x4a].contains(&c.bytes[2])) {
            return Err("Unadmitted root byte assignment".into());
        }
        if c.bytes[0] == 0x12 && c.bytes != [0x12, 0x80, 0, 0, 0, 0, 0, 0] {
            return Err("Unadmitted root flags assignment".into());
        }
        if c.bytes[0] == 0x2f && c.bytes[1..4] != [1, 1, 0] {
            return Err("Root message must use the admitted synchronous handoff".into());
        }
        lengths[(c.va - START) as usize] = length as u8;
        bytes.extend(&c.bytes);
    }
    if bytes.len() != (END - START) as usize {
        return Err("Root extent changed".into());
    }
    Ok(Program {
        bytes,
        lengths,
        commands: program.len(),
    })
}
pub fn build(commands: &[crate::vm::Command]) -> Result<Vec<(String, Vec<u8>)>> {
    let program = compile(commands)?;
    let bytes = program.bytes;
    let lengths = program.lengths;
    let layout = CartridgeLayout::new(
        0x10000,
        0x7000,
        vec![
            Section {
                name: "root_source".into(),
                offset: 0x8000,
                bytes: bytes.clone(),
            },
            Section {
                name: "root_lengths".into(),
                offset: 0x8800,
                bytes: lengths,
            },
        ],
    )?;
    let source = layout.cpu_address(0x8000)?;
    let length_source = layout.cpu_address(0x8800)?;
    let mut p = Assembler::new();
    startup(&mut p);
    for at in 0x360..=0x391 {
        write(&mut p, at, 0);
    }
    write(&mut p, 0x390, 0x42);
    abs(&mut p, M::Lda, 0x4210);
    write(&mut p, 0x4200, 0x80);
    p.label("idle");
    implied(&mut p, M::Wai);
    call(&mut p, "root_tick");
    branch(&mut p, M::Bra, "idle");
    p.label("nmi");
    runtime::emit_interrupt_entry(&mut p);
    abs(&mut p, M::Lda, 0x4210);
    abs(&mut p, M::Inc, 0x391);
    runtime::emit_interrupt_exit(&mut p);
    p.label("irq").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(&mut p, M::Rti);
    emit_routine(&mut p, source, length_source);
    let assembled =
        crate::sfc_width_check::assemble(&p, CodeLocation::new(0, 0x8000), CpuMode::Emulation)?;
    let addr = |name| assembled.label_location(name).unwrap().program_counter();
    let rom = layout.build(assembled.bytes(), addr("reset"), addr("nmi"), addr("irq"))?;
    let report = serde_json::json!(
        { "scope" :
        "Source prologue root control kernel with explicit external-action boundaries",
        "rom_sha256" : sha(& rom), "rom_bytes" : rom.len(), "code_bytes" : assembled
        .bytes().len(), "source_start" : START, "source_end" : END, "source_commands" :
        program.commands, "source_bytes_sha256" : sha(& bytes), "sections" : layout
        .section_report(), "wram" : { "request" : 864, "state" : 865, "error" : 866,
        "reference" : 867, "pc_offset" : 868, "wait" : 870, "loop_count" : 872,
        "loop_offset" : 874, "ticks" : 876, "action_sequence" : 878, "ack_sequence" :
        880, "scene_phase" : 882, "game_flag" : 883, "flags" : 884, "action" : 888, "pc"
        : 900, "busy" : 910, "ready" : 912 }, "limits" :
        ["Game actions complete through their runtime handlers.",
        "Standalone control kernel, not composed graphics/audio, original object pool or B1 gameplay.",
        "Logical source ticks are requested independently of NMI; no original real-time playback claim."]
        }
    );
    Ok(vec![
        ("prologue-control.sfc".into(), rom),
        (
            "prologue-control-build.json".into(),
            serde_json::to_vec_pretty(&report)?,
        ),
    ])
}
pub fn emit_routine(p: &mut Assembler, source: u32, length_source: u32) {
    p.label("root_tick").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, 0x360);
    branch(p, M::Bne, "root_vm_request");
    implied(p, M::Rts);
    p.label("root_vm_request");
    abs(p, M::Sta, 0x38f);
    write(p, 0x360, 0);
    write(p, 0x38e, 1);
    abs(p, M::Lda, 0x38f);
    select(p, 1, "root_vm_restart", "root_vm_request_tick");
    select(p, 2, "root_vm_start_tick", "root_vm_request_ack");
    select(p, 3, "root_vm_ack", "root_vm_request_finish");
    select(p, 4, "root_vm_finish_message", "root_vm_request_bad");
    jump(p, "root_vm_invalid");
    p.label("root_vm_restart");
    for at in 0x361..=0x38d {
        write(p, at, 0);
    }
    write(p, 0x36a, 255);
    write(p, 0x36b, 255);
    write(p, 0x361, 1);
    jump(p, "root_vm_start_tick");
    p.label("root_vm_start_tick");
    abs(p, M::Lda, 0x361);
    byte(p, M::Cmp, 1);
    branch(p, M::Beq, "root_vm_begin_tick");
    byte(p, M::Cmp, 3);
    branch(p, M::Beq, "root_vm_message_tick");
    jump(p, "root_vm_invalid");
    p.label("root_vm_message_tick");
    m16(p);
    abs(p, M::Inc, 0x36c);
    m8(p);
    jump(p, "root_vm_return");
    p.label("root_vm_finish_message");
    abs(p, M::Lda, 0x361);
    byte(p, M::Cmp, 3);
    branch(p, M::Beq, "root_vm_finish_valid");
    jump(p, "root_vm_invalid");
    p.label("root_vm_finish_valid");
    m16(p);
    abs(p, M::Lda, 0x370);
    abs(p, M::Cmp, 0x36e);
    branch(p, M::Beq, "root_vm_finish_sequence_valid");
    m8(p);
    jump(p, "root_vm_invalid");
    p.label("root_vm_finish_sequence_valid")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    write(p, 0x361, 1);
    jump(p, "root_vm_message_tick");
    p.label("root_vm_begin_tick");
    m16(p);
    abs(p, M::Inc, 0x36c);
    m8(p);
    write(p, 0x388, 0);
    write(p, 0x389, 0);
    jump(p, "root_vm_dispatch");
    p.label("root_vm_ack");
    abs(p, M::Lda, 0x361);
    byte(p, M::Cmp, 2);
    branch(p, M::Beq, "root_vm_ack_valid");
    jump(p, "root_vm_invalid");
    p.label("root_vm_ack_valid");
    m16(p);
    abs(p, M::Lda, 0x370);
    abs(p, M::Cmp, 0x36e);
    branch(p, M::Beq, "root_vm_ack_sequence_valid");
    m8(p);
    jump(p, "root_vm_invalid");
    p.label("root_vm_ack_sequence_valid")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    write(p, 0x361, 1);
    call(p, "root_vm_advance");
    abs(p, M::Lda, 0x378);
    byte(p, M::Cmp, 0x2f);
    branch(p, M::Bne, "root_vm_ack_immediate");
    write(p, 0x361, 3);
    jump(p, "root_vm_return");
    p.label("root_vm_ack_immediate");
    jump(p, "root_vm_dispatch");
    p.label("root_vm_dispatch")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Inc, 0x388);
    abs(p, M::Lda, 0x388);
    word(p, M::Cmp, 1024);
    branch(p, M::Bcc, "root_vm_budget_ok");
    m8(p);
    jump(p, "root_vm_fault");
    p.label("root_vm_budget_ok")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Lda, 0x364);
    word(p, M::Cmp, (END - START) as u16);
    branch(p, M::Bcc, "root_vm_offset_ok");
    m8(p);
    jump(p, "root_vm_fault");
    p.label("root_vm_offset_ok")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(length_source));
    branch(p, M::Bne, "root_vm_boundary_ok");
    jump(p, "root_vm_fault");
    p.label("root_vm_boundary_ok");
    abs(p, M::Sta, 0x38a);
    write(p, 0x38b, 0);
    for i in 0..12 {
        write(p, 0x378 + i, 0);
    }
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("root_vm_copy");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(source));
    emit(p, M::Sta, A::AbsoluteY, O::Word(0x378));
    implied(p, M::Inx);
    implied(p, M::Iny);
    emit(p, M::Cpy, A::Absolute, O::Word(0x38a));
    branch(p, M::Bne, "root_vm_copy");
    abs(p, M::Lda, 0x378);
    for (op, label) in [
        (1, "root_vm_yield"),
        (2, "root_vm_wait"),
        (5, "root_vm_done"),
        (6, "root_vm_loop"),
        (0x10, "root_vm_assign"),
        (0x12, "root_vm_flags"),
        (0x4b, "root_vm_scene_ref"),
        (0x6f, "root_vm_game_ref"),
    ] {
        select(p, op, label, &format!("root_vm_not_{op}"));
    }
    write(p, 0x361, 2);
    m16(p);
    abs(p, M::Inc, 0x36e);
    m8(p);
    jump(p, "root_vm_return");
    p.label("root_vm_yield");
    call(p, "root_vm_advance");
    jump(p, "root_vm_return");
    p.label("root_vm_done");
    write(p, 0x361, 4);
    jump(p, "root_vm_return");
    p.label("root_vm_scene_ref");
    write(p, 0x363, 1);
    jump(p, "root_vm_next");
    p.label("root_vm_game_ref");
    write(p, 0x363, 0);
    jump(p, "root_vm_next");
    p.label("root_vm_flags");
    for at in 0x374..0x378 {
        write(p, at, 0);
    }
    jump(p, "root_vm_next");
    p.label("root_vm_assign");
    abs(p, M::Lda, 0x37a);
    byte(p, M::Cmp, 0x19);
    branch(p, M::Bne, "root_vm_assign_game");
    abs(p, M::Lda, 0x363);
    byte(p, M::Cmp, 1);
    branch(p, M::Beq, "root_vm_assign_scene_valid");
    jump(p, "root_vm_fault");
    p.label("root_vm_assign_scene_valid");
    abs(p, M::Lda, 0x37b);
    abs(p, M::Sta, 0x372);
    jump(p, "root_vm_next");
    p.label("root_vm_assign_game");
    abs(p, M::Lda, 0x363);
    branch(p, M::Beq, "root_vm_assign_game_valid");
    jump(p, "root_vm_fault");
    p.label("root_vm_assign_game_valid");
    abs(p, M::Lda, 0x37b);
    abs(p, M::Sta, 0x373);
    jump(p, "root_vm_next");
    p.label("root_vm_wait");
    m16(p);
    abs(p, M::Lda, 0x366);
    branch(p, M::Bne, "root_vm_wait_decrement");
    abs(p, M::Lda, 0x37a);
    abs(p, M::Sta, 0x366);
    m8(p);
    jump(p, "root_vm_return");
    p.label("root_vm_wait_decrement")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Dec, 0x366);
    branch(p, M::Beq, "root_vm_wait_done");
    m8(p);
    jump(p, "root_vm_return");
    p.label("root_vm_wait_done")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    jump(p, "root_vm_next");
    p.label("root_vm_loop");
    m16(p);
    abs(p, M::Lda, 0x37c);
    implied(p, M::Sec);
    word(p, M::Sbc, START as u16);
    abs(p, M::Cmp, 0x36a);
    branch(p, M::Beq, "root_vm_loop_decrement");
    abs(p, M::Sta, 0x36a);
    abs(p, M::Lda, 0x37a);
    abs(p, M::Sta, 0x368);
    m8(p);
    jump(p, "root_vm_dispatch");
    p.label("root_vm_loop_decrement")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Dec, 0x368);
    branch(p, M::Beq, "root_vm_loop_done");
    abs(p, M::Lda, 0x36a);
    abs(p, M::Sta, 0x364);
    m8(p);
    jump(p, "root_vm_dispatch");
    p.label("root_vm_loop_done")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    word(p, M::Lda, 65535);
    abs(p, M::Sta, 0x36a);
    m8(p);
    jump(p, "root_vm_next");
    p.label("root_vm_next");
    call(p, "root_vm_advance");
    jump(p, "root_vm_dispatch");
    p.label("root_vm_advance")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, 0x364);
    implied(p, M::Clc);
    abs(p, M::Adc, 0x38a);
    abs(p, M::Sta, 0x364);
    m8(p);
    implied(p, M::Rts);
    p.label("root_vm_fault");
    write(p, 0x361, 255);
    write(p, 0x362, 2);
    jump(p, "root_vm_publish");
    p.label("root_vm_invalid");
    write(p, 0x362, 1);
    jump(p, "root_vm_publish");
    p.label("root_vm_return");
    write(p, 0x362, 0);
    p.label("root_vm_publish");
    m16(p);
    abs(p, M::Lda, 0x364);
    implied(p, M::Clc);
    word(p, M::Adc, START as u16);
    abs(p, M::Sta, 0x384);
    m8(p);
    write(p, 0x386, 0x46);
    write(p, 0x387, 0);
    write(p, 0x38e, 0);
    implied(p, M::Rts);
}
