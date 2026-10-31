use crate::{
    Result,
    sfc::{emit, implied, startup, write},
    sha,
};
use sfc_builder::{cartridge::CartridgeLayout, runtime};
use w65c816::{AddressingMode as A, Assembler, CodeLocation, CpuMode, Mnemonic as M, Operand as O};
const SIZE: u16 = 230;
pub(super) fn abs(p: &mut Assembler, m: M, at: u16) {
    emit(p, m, A::Absolute, O::Word(at));
}
pub(super) fn ix(p: &mut Assembler, m: M, at: u16) {
    emit(p, m, A::AbsoluteX, O::Word(at));
}
pub(super) fn iy(p: &mut Assembler, m: M, at: u16) {
    emit(p, m, A::AbsoluteY, O::Word(at));
}
pub(super) fn byte(p: &mut Assembler, m: M, n: u8) {
    emit(p, m, A::Immediate, O::Byte(n));
}
pub(super) fn word(p: &mut Assembler, m: M, n: u16) {
    emit(p, m, A::Immediate, O::Word(n));
}
pub(super) fn m16(p: &mut Assembler) {
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
}
pub(super) fn m8(p: &mut Assembler) {
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
}
pub(super) fn call(p: &mut Assembler, s: &str) {
    p.emit_label_ref(M::Jsr, A::Absolute, s);
}
pub(super) fn jump(p: &mut Assembler, s: &str) {
    p.emit_label_ref(M::Brl, A::Relative16, s);
}
pub(super) fn branch(p: &mut Assembler, m: M, s: &str) {
    p.emit_label_ref(m, A::Relative8, s);
}
pub(super) fn bank(p: &mut Assembler) {
    implied(p, M::Phb);
    byte(p, M::Lda, 0x7e);
    implied(p, M::Pha);
    implied(p, M::Plb);
}
fn store32(p: &mut Assembler, at: u16, value: u32) {
    for (i, b) in value.to_le_bytes().iter().enumerate() {
        write(p, at + i as u16, *b);
    }
}
pub(super) fn copy_pointer(p: &mut Assembler, field: u16, temp: u16, load: bool) {
    for i in [0, 2] {
        if load {
            ix(p, M::Lda, field + i);
            abs(p, M::Sta, temp + i);
        } else {
            abs(p, M::Lda, temp + i);
            ix(p, M::Sta, field + i);
        }
    }
}
fn checked_index(p: &mut Assembler, argument: u16, output: u16, name: &str) {
    abs(p, M::Lda, argument);
    call(p, "pool_index");
    branch(p, M::Bcc, name);
    jump(p, "pool_bad_argument");
    p.label(name);
    abs(p, M::Stx, output);
}
fn checked_pointer(p: &mut Assembler, output: u16, name: &str) {
    call(p, "pool_pointer");
    branch(p, M::Bcc, name);
    jump(p, "pool_bad_pointer");
    p.label(name);
    abs(p, M::Stx, output);
}
pub fn build() -> Result<Vec<(String, Vec<u8>)>> {
    let layout = CartridgeLayout::new(0x10000, 0x7000, vec![])?;
    let mut p = Assembler::new();
    startup(&mut p);
    for at in 0x3a0..0x400 {
        write(&mut p, at, 0);
    }
    write(&mut p, 0x3a0, 1);
    call(&mut p, "pool_tick");
    write(&mut p, 0x3b2, 0x42);
    abs(&mut p, M::Lda, 0x4210);
    write(&mut p, 0x4200, 0x80);
    p.label("idle");
    implied(&mut p, M::Wai);
    call(&mut p, "pool_tick");
    branch(&mut p, M::Bra, "idle");
    p.label("nmi");
    runtime::emit_interrupt_entry(&mut p);
    abs(&mut p, M::Lda, 0x4210);
    runtime::emit_interrupt_exit(&mut p);
    p.label("irq").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(&mut p, M::Rti);
    emit_routines(&mut p);
    let code =
        crate::sfc_width_check::assemble(&p, CodeLocation::new(0, 0x8000), CpuMode::Emulation)?;
    let addr = |s| code.label_location(s).unwrap().program_counter();
    let rom = layout.build(code.bytes(), addr("reset"), addr("nmi"), addr("irq"))?;
    let report = serde_json::json!(
        { "scope" :
        "Source-layout allocation, registry and parent links; no object scripts or root consumer",
        "rom_sha256" : sha(& rom), "rom_bytes" : rom.len(), "code_bytes" : code.bytes()
        .len(), "pool" : { "base" : 24576, "stride" : 230, "count" : 16, "source_base" :
        0x4c4770 }, "groups" : { "base" : 28672, "stride" : 460, "count" : 5,
        "source_base" : 0x4f1640 }, "registry" : { "base" : 0x7900, "count" : 256,
        "stride" : 4, "source_base" : 0x4c4330 }, "wram" : { "request" : 928, "error" :
        929, "owner" : 930, "group" : 931, "target" : 933, "script" : 936, "result" :
        940, "sequence" : 942, "free_head" : 944, "active_count" : 945, "ready" : 946,
        "busy" : 947, "active" : 976, "free_next" : 992, "registry_slot" : 0x3a6,
        "scan_state" : 0x3f0, "scan_group" : 0x3f1, "scan_object" : 0x3f2, "scan_tail" :
        0x3f4, "scan_visited" : 0x3f6, "scan_callback" : 0x3f8, "scan_slot" : 0x3fc },
        "requests" : { "reset" : 1, "allocate" : 2, "free" : 3, "register" : 4, "link" :
        5, "delete_registered" : 6, "reference_registered" : 7, "begin_group_scan" : 8,
        "resume_group_scan" : 9 }, "errors" : { "invalid_argument" : 1, "exhausted" : 2,
        "invalid_pointer" : 3 }, "limits" :
        ["16-slot capacity is checked; original PC pool has 800 slots.",
        "Original virtual addresses remain data, never CPU pointers.",
        "No full prologue or B1 playback."] }
    );
    Ok(vec![
        ("object-pool.sfc".into(), rom),
        (
            "object-pool-build.json".into(),
            serde_json::to_vec_pretty(&report)?,
        ),
    ])
}
pub fn emit_routines(p: &mut Assembler) {
    p.label("pool_tick").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, 0x3a0);
    branch(p, M::Bne, "pool_request");
    implied(p, M::Rts);
    p.label("pool_request");
    bank(p);
    write(p, 0x3b3, 1);
    write(p, 0x3a1, 0);
    abs(p, M::Lda, 0x3a0);
    abs(p, M::Sta, 0x3ad);
    write(p, 0x3a0, 0);
    jump(p, "pool_dispatch_saved");
    p.label("pool_reset");
    for at in 0x3f0..0x400 {
        write(p, at, 0);
    }
    m16(p);
    word(p, M::Ldx, 0x6000);
    word(p, M::Lda, 0);
    p.label("pool_clear");
    ix(p, M::Sta, 0);
    implied(p, M::Inx);
    implied(p, M::Inx);
    word(p, M::Cpx, 0x7d00);
    branch(p, M::Bne, "pool_clear");
    m8(p);
    for i in 0..16u16 {
        write(p, 0x3d0 + i, u8::from(i == 0));
        write(p, 0x3e0 + i, if i == 15 { 255 } else { (i + 1) as u8 });
        store32(
            p,
            0x6000 + i * SIZE + 12,
            if i == 15 {
                0
            } else {
                0x4c4770 + u32::from((i + 1) * SIZE)
            },
        );
    }
    for group in 0..5u16 {
        let head = 0x7000 + group * SIZE * 2;
        let source = 0x4f1640 + u32::from(group * SIZE * 2);
        store32(p, head + 8, source + u32::from(SIZE));
        store32(p, head + SIZE + 4, source);
    }
    store32(p, 0x7008, 0x4c4770);
    store32(p, 0x70ea, 0x4c4770);
    store32(p, 0x6004, 0x4f1640);
    store32(p, 0x6008, 0x4f1726);
    store32(p, 0x6010, 0x41c891);
    store32(p, 0x6040, 0x46e89c);
    write(p, 0x3b0, 1);
    write(p, 0x3b1, 1);
    write(p, 0x3ac, 0);
    jump(p, "pool_success");
    p.label("pool_allocate");
    abs(p, M::Lda, 0x3a3);
    byte(p, M::Cmp, 5);
    branch(p, M::Bcc, "pool_group_valid");
    jump(p, "pool_bad_argument");
    p.label("pool_group_valid");
    checked_index(p, 0x3a2, 0x3b4, "pool_owner_index");
    m16(p);
    abs(p, M::Lda, 0x3a2);
    word(p, M::And, 255);
    implied(p, M::Tay);
    m8(p);
    iy(p, M::Lda, 0x3d0);
    branch(p, M::Bne, "pool_owner_live");
    jump(p, "pool_bad_argument");
    p.label("pool_owner_live");
    abs(p, M::Lda, 0x3b0);
    byte(p, M::Cmp, 255);
    branch(p, M::Bne, "pool_has_free");
    write(p, 0x3a1, 2);
    jump(p, "pool_exit");
    p.label("pool_has_free");
    abs(p, M::Sta, 0x3ac);
    checked_index(p, 0x3b0, 0x3b6, "pool_free_index");
    for group in 0..5u16 {
        let next = format!("pool_group_not_{group}");
        abs(p, M::Lda, 0x3a3);
        byte(p, M::Cmp, group as u8);
        branch(p, M::Bne, &next);
        m16(p);
        word(p, M::Ldx, 0x7000 + group * SIZE * 2 + SIZE);
        abs(p, M::Stx, 0x3bc);
        m8(p);
        jump(p, "pool_tail_ready");
        p.label(next);
    }
    p.label("pool_tail_ready");
    m16(p);
    copy_pointer(p, 4, 0x3c0, true);
    m8(p);
    checked_pointer(p, 0x3b8, "pool_previous_valid");
    m16(p);
    abs(p, M::Lda, 0x3ac);
    word(p, M::And, 255);
    implied(p, M::Tay);
    m8(p);
    iy(p, M::Lda, 0x3e0);
    abs(p, M::Sta, 0x3b0);
    byte(p, M::Lda, 1);
    iy(p, M::Sta, 0x3d0);
    abs(p, M::Inc, 0x3b1);
    m16(p);
    abs(p, M::Lda, 0x3b6);
    implied(p, M::Sec);
    word(p, M::Sbc, 0x1890);
    abs(p, M::Sta, 0x3c4);
    word(p, M::Lda, 0x4c);
    abs(p, M::Sta, 0x3c6);
    abs(p, M::Ldx, 0x3b8);
    copy_pointer(p, 8, 0x3c4, false);
    abs(p, M::Ldx, 0x3b6);
    copy_pointer(p, 4, 0x3c0, false);
    abs(p, M::Lda, 0x3bc);
    implied(p, M::Sec);
    word(p, M::Sbc, 0x59c0);
    ix(p, M::Sta, 8);
    word(p, M::Lda, 0x4f);
    ix(p, M::Sta, 10);
    word(p, M::Lda, 0xc891);
    ix(p, M::Sta, 0x10);
    word(p, M::Lda, 0x41);
    ix(p, M::Sta, 0x12);
    abs(p, M::Lda, 0x3a3);
    word(p, M::And, 255);
    ix(p, M::Sta, 0x1a);
    copy_pointer(p, 0x40, 0x3a8, false);
    abs(p, M::Ldx, 0x3bc);
    copy_pointer(p, 4, 0x3c4, false);
    abs(p, M::Ldx, 0x3b4);
    copy_pointer(p, 0x58, 0x3c4, false);
    m8(p);
    jump(p, "pool_success");
    p.label("pool_free");
    checked_index(p, 0x3a5, 0x3b6, "pool_target_index");
    m16(p);
    abs(p, M::Lda, 0x3a5);
    word(p, M::And, 255);
    implied(p, M::Tay);
    m8(p);
    iy(p, M::Lda, 0x3d0);
    branch(p, M::Bne, "pool_target_live");
    jump(p, "pool_bad_argument");
    p.label("pool_target_live");
    m16(p);
    copy_pointer(p, 4, 0x3c0, true);
    m8(p);
    checked_pointer(p, 0x3b8, "pool_free_prev");
    m16(p);
    abs(p, M::Ldx, 0x3b6);
    copy_pointer(p, 8, 0x3c4, true);
    copy_pointer(p, 8, 0x3c0, true);
    m8(p);
    checked_pointer(p, 0x3ba, "pool_free_next");
    m16(p);
    abs(p, M::Ldx, 0x3b6);
    copy_pointer(p, 4, 0x3c0, true);
    abs(p, M::Ldx, 0x3ba);
    copy_pointer(p, 4, 0x3c0, false);
    abs(p, M::Ldx, 0x3b8);
    copy_pointer(p, 8, 0x3c4, false);
    abs(p, M::Ldx, 0x3b6);
    word(p, M::Ldy, SIZE / 2);
    word(p, M::Lda, 0);
    p.label("pool_zero_object");
    ix(p, M::Sta, 0);
    implied(p, M::Inx);
    implied(p, M::Inx);
    implied(p, M::Dey);
    branch(p, M::Bne, "pool_zero_object");
    abs(p, M::Ldx, 0x3b6);
    copy_pointer(p, 4, 0x3c0, false);
    copy_pointer(p, 8, 0x3c4, false);
    m8(p);
    abs(p, M::Lda, 0x3b0);
    byte(p, M::Cmp, 255);
    branch(p, M::Beq, "pool_free_null");
    call(p, "pool_index");
    m16(p);
    implied(p, M::Txa);
    implied(p, M::Sec);
    word(p, M::Sbc, 0x1890);
    abs(p, M::Ldx, 0x3b6);
    ix(p, M::Sta, 12);
    word(p, M::Lda, 0x4c);
    ix(p, M::Sta, 14);
    m8(p);
    p.label("pool_free_null");
    m16(p);
    abs(p, M::Lda, 0x3a5);
    word(p, M::And, 255);
    implied(p, M::Tay);
    m8(p);
    abs(p, M::Lda, 0x3b0);
    iy(p, M::Sta, 0x3e0);
    byte(p, M::Lda, 0);
    iy(p, M::Sta, 0x3d0);
    abs(p, M::Lda, 0x3a5);
    abs(p, M::Sta, 0x3b0);
    abs(p, M::Sta, 0x3ac);
    abs(p, M::Dec, 0x3b1);
    jump(p, "pool_success");
    p.label("pool_registered_delete");
    registry_index(p);
    m16(p);
    for i in [0, 2] {
        iy(p, M::Lda, 0x7900 + i);
        abs(p, M::Sta, 0x3c0 + i);
    }
    abs(p, M::Lda, 0x3c0);
    abs(p, M::Ora, 0x3c2);
    m8(p);
    branch(p, M::Bne, "pool_registered_present");
    write(p, 0x3ac, 255);
    jump(p, "pool_success");
    p.label("pool_registered_present");
    call(p, "pool_live_pointer");
    branch(p, M::Bcc, "pool_registered_live");
    jump(p, "pool_bad_pointer");
    p.label("pool_registered_live");
    implied(p, M::Tya);
    abs(p, M::Sta, 0x3a5);
    jump(p, "pool_free");
    p.label("pool_reference_operation");
    checked_index(p, 0x3a2, 0x3b4, "pool_reference_owner_index");
    m16(p);
    abs(p, M::Lda, 0x3a2);
    word(p, M::And, 255);
    implied(p, M::Tay);
    m8(p);
    iy(p, M::Lda, 0x3d0);
    branch(p, M::Bne, "pool_reference_owner_live");
    jump(p, "pool_bad_argument");
    p.label("pool_reference_owner_live");
    abs(p, M::Lda, 0x3ad);
    byte(p, M::Cmp, 7);
    branch(p, M::Bne, "pool_last_child");
    registry_index(p);
    m16(p);
    abs(p, M::Ldx, 0x3b4);
    for i in [0, 2] {
        iy(p, M::Lda, 0x7900 + i);
        ix(p, M::Sta, 0xa8 + i);
    }
    m8(p);
    write(p, 0x3ac, 255);
    jump(p, "pool_success");
    p.label("pool_last_child");
    m16(p);
    abs(p, M::Ldx, 0x3b4);
    copy_pointer(p, 0x58, 0x3c0, true);
    m8(p);
    call(p, "pool_live_pointer");
    branch(p, M::Bcc, "pool_child_live");
    jump(p, "pool_bad_pointer");
    p.label("pool_child_live");
    abs(p, M::Stx, 0x3b6);
    implied(p, M::Tya);
    abs(p, M::Sta, 0x3ac);
    abs(p, M::Lda, 0x3ad);
    byte(p, M::Cmp, 5);
    branch(p, M::Beq, "pool_parent_link");
    registry_index(p);
    m16(p);
    for i in [0, 2] {
        abs(p, M::Lda, 0x3c0 + i);
        iy(p, M::Sta, 0x7900 + i);
    }
    m8(p);
    jump(p, "pool_success");
    p.label("pool_parent_link");
    m16(p);
    abs(p, M::Ldx, 0x3b4);
    copy_pointer(p, 0xa4, 0x3c0, false);
    abs(p, M::Lda, 0x3b4);
    implied(p, M::Sec);
    word(p, M::Sbc, 0x1890);
    abs(p, M::Ldx, 0x3b6);
    ix(p, M::Sta, 0xa0);
    word(p, M::Lda, 0x4c);
    ix(p, M::Sta, 0xa2);
    m8(p);
    jump(p, "pool_success");
    p.label("pool_bad_argument");
    write(p, 0x3a1, 1);
    jump(p, "pool_exit");
    p.label("pool_bad_pointer");
    write(p, 0x3a1, 3);
    jump(p, "pool_exit");
    p.label("pool_success");
    abs(p, M::Lda, 0x3ad);
    byte(p, M::Cmp, 6);
    branch(p, M::Bne, "pool_success_count");
    registry_index(p);
    m16(p);
    word(p, M::Lda, 0);
    iy(p, M::Sta, 0x7900);
    iy(p, M::Sta, 0x7902);
    m8(p);
    p.label("pool_success_count");
    m16(p);
    abs(p, M::Inc, 0x3ae);
    m8(p);
    p.label("pool_exit");
    write(p, 0x3b3, 0);
    implied(p, M::Plb);
    implied(p, M::Rts);
    p.label("pool_dispatch_saved");
    abs(p, M::Lda, 0x3ad);
    for (value, label) in [
        (1, "pool_reset"),
        (2, "pool_allocate"),
        (3, "pool_free"),
        (4, "pool_reference_operation"),
        (5, "pool_reference_operation"),
        (6, "pool_registered_delete"),
        (7, "pool_reference_operation"),
        (8, "pool_scan_begin"),
        (9, "pool_scan_resume"),
    ] {
        let next = format!("pool_not_request_{value}");
        byte(p, M::Cmp, value);
        branch(p, M::Bne, &next);
        jump(p, label);
        p.label(next);
    }
    jump(p, "pool_bad_argument");
    emit_scheduler(p);
    p.label("pool_index").assume_mode(CpuMode::NATIVE_M8_X16);
    byte(p, M::Cmp, 16);
    branch(p, M::Bcc, "pool_index_ok");
    implied(p, M::Sec);
    implied(p, M::Rts);
    p.label("pool_index_ok");
    m16(p);
    word(p, M::And, 255);
    implied(p, M::Tay);
    word(p, M::Ldx, 0x6000);
    word(p, M::Cpy, 0);
    branch(p, M::Beq, "pool_index_done");
    p.label("pool_index_loop");
    implied(p, M::Txa);
    implied(p, M::Clc);
    word(p, M::Adc, SIZE);
    implied(p, M::Tax);
    implied(p, M::Dey);
    branch(p, M::Bne, "pool_index_loop");
    p.label("pool_index_done");
    m8(p);
    implied(p, M::Clc);
    implied(p, M::Rts);
    p.label("pool_pointer").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, 0x3c2);
    word(p, M::Cmp, 0x4c);
    branch(p, M::Bne, "pool_pointer_group");
    abs(p, M::Lda, 0x3c0);
    word(p, M::Cmp, 0x4770);
    branch(p, M::Bcc, "pool_pointer_bad");
    word(p, M::Cmp, 0x55d0);
    branch(p, M::Bcs, "pool_pointer_bad");
    implied(p, M::Sec);
    word(p, M::Sbc, 0x4770);
    word(p, M::Ldx, 0x6000);
    branch(p, M::Bra, "pool_pointer_loop");
    p.label("pool_pointer_group");
    word(p, M::Cmp, 0x4f);
    branch(p, M::Bne, "pool_pointer_bad");
    abs(p, M::Lda, 0x3c0);
    word(p, M::Cmp, 0x1640);
    branch(p, M::Bcc, "pool_pointer_bad");
    word(p, M::Cmp, 0x1f3c);
    branch(p, M::Bcs, "pool_pointer_bad");
    implied(p, M::Sec);
    word(p, M::Sbc, 0x1640);
    word(p, M::Ldx, 0x7000);
    p.label("pool_pointer_loop");
    word(p, M::Cmp, 0);
    branch(p, M::Beq, "pool_pointer_done");
    word(p, M::Cmp, SIZE);
    branch(p, M::Bcc, "pool_pointer_bad");
    implied(p, M::Sec);
    word(p, M::Sbc, SIZE);
    implied(p, M::Pha);
    implied(p, M::Txa);
    implied(p, M::Clc);
    word(p, M::Adc, SIZE);
    implied(p, M::Tax);
    implied(p, M::Pla);
    branch(p, M::Bra, "pool_pointer_loop");
    p.label("pool_pointer_done");
    m8(p);
    implied(p, M::Clc);
    implied(p, M::Rts);
    p.label("pool_pointer_bad")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    m8(p);
    implied(p, M::Sec);
    implied(p, M::Rts);
    p.label("pool_live_pointer")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    call(p, "pool_pointer");
    branch(p, M::Bcs, "pool_live_pointer_bad");
    word(p, M::Cpx, 0x7000);
    branch(p, M::Bcs, "pool_live_pointer_bad");
    m16(p);
    implied(p, M::Txa);
    implied(p, M::Sec);
    word(p, M::Sbc, 0x6000);
    word(p, M::Ldy, 0);
    p.label("pool_live_pointer_loop");
    word(p, M::Cmp, 0);
    branch(p, M::Beq, "pool_live_pointer_check");
    implied(p, M::Sec);
    word(p, M::Sbc, SIZE);
    implied(p, M::Iny);
    branch(p, M::Bra, "pool_live_pointer_loop");
    p.label("pool_live_pointer_check");
    m8(p);
    iy(p, M::Lda, 0x3d0);
    byte(p, M::Cmp, 1);
    branch(p, M::Bne, "pool_live_pointer_bad");
    implied(p, M::Clc);
    implied(p, M::Rts);
    p.label("pool_live_pointer_bad")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Sec);
    implied(p, M::Rts);
}
fn registry_index(p: &mut Assembler) {
    m16(p);
    abs(p, M::Lda, 0x3a6);
    word(p, M::And, 255);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tay);
    m8(p);
}
fn emit_scheduler(p: &mut Assembler) {
    p.label("pool_scan_begin")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, 0x3f0);
    byte(p, M::Cmp, 1);
    branch(p, M::Bne, "pool_scan_start");
    jump(p, "pool_bad_argument");
    p.label("pool_scan_start");
    write(p, 0x3f0, 0);
    write(p, 0x3f1, 0);
    m16(p);
    word(p, M::Lda, 0);
    abs(p, M::Sta, 0x3f6);
    word(p, M::Ldx, 0x7000);
    jump(p, "pool_scan_group");
    p.label("pool_scan_resume")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, 0x3f0);
    byte(p, M::Cmp, 1);
    branch(p, M::Beq, "pool_scan_ack");
    jump(p, "pool_bad_argument");
    p.label("pool_scan_ack");
    m16(p);
    abs(p, M::Ldx, 0x3f2);
    jump(p, "pool_scan_next");
    p.label("pool_scan_group")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Txa);
    implied(p, M::Clc);
    word(p, M::Adc, SIZE);
    abs(p, M::Sta, 0x3f4);
    p.label("pool_scan_next");
    copy_pointer(p, 8, 0x3c0, true);
    m8(p);
    call(p, "pool_pointer");
    branch(p, M::Bcc, "pool_scan_translated");
    jump(p, "pool_scan_fault");
    p.label("pool_scan_translated");
    emit(p, M::Cpx, A::Absolute, O::Word(0x3f4));
    branch(p, M::Bne, "pool_scan_object");
    abs(p, M::Inc, 0x3f1);
    abs(p, M::Lda, 0x3f1);
    byte(p, M::Cmp, 5);
    branch(p, M::Beq, "pool_scan_done");
    m16(p);
    abs(p, M::Lda, 0x3f4);
    implied(p, M::Clc);
    word(p, M::Adc, SIZE);
    implied(p, M::Tax);
    jump(p, "pool_scan_group");
    p.label("pool_scan_done")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    write(p, 0x3f0, 2);
    write(p, 0x3fc, 255);
    jump(p, "pool_success");
    p.label("pool_scan_object");
    call(p, "pool_live_pointer");
    branch(p, M::Bcc, "pool_scan_live");
    jump(p, "pool_scan_fault");
    p.label("pool_scan_live");
    abs(p, M::Stx, 0x3f2);
    implied(p, M::Tya);
    abs(p, M::Sta, 0x3fc);
    m16(p);
    abs(p, M::Inc, 0x3f6);
    abs(p, M::Lda, 0x3f6);
    word(p, M::Cmp, 10001);
    m8(p);
    branch(p, M::Bcc, "pool_scan_budget_ok");
    jump(p, "pool_scan_fault");
    p.label("pool_scan_budget_ok");
    ix(p, M::Lda, 3);
    byte(p, M::And, 0x80);
    branch(p, M::Beq, "pool_scan_callback");
    m16(p);
    jump(p, "pool_scan_next");
    p.label("pool_scan_callback")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    copy_pointer(p, 0x10, 0x3f8, true);
    m8(p);
    write(p, 0x3f0, 1);
    jump(p, "pool_success");
    p.label("pool_scan_fault");
    write(p, 0x3f0, 255);
    jump(p, "pool_bad_pointer");
}
