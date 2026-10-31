use super::*;
pub(super) fn emit(p: &mut w65c816::Assembler, tables: &Tables) -> Result<()> {
    use crate::sfc::emit;
    let Tables {
        layout, program, ..
    } = *tables;
    let section = |name: &str| -> Result<u32> { layout.cpu_address(layout.section(name)?.offset) };
    p.label("field_op_07");
    {
        use crate::sfc_b1_portrait::{HUM, HUM_ON, QUAKE, QUAKE_BASE};
        let hums = section("field_warp_hums")?;
        let quakes = section("field_warp_quakes")?;
        m16(p);
        long_x(p, M::Lda, program + 4);
        abs(p, M::Sta, TEMP);
        implied(p, M::Phx);
        emit(p, M::Ldx, A::Immediate, O::Word(0));
        p.label("field_hum_find")
            .assume_mode(CpuMode::NATIVE_M16_X16);
        long_x(p, M::Lda, hums);
        emit(p, M::Cmp, A::Immediate, O::Word(0xffff));
        branch(p, M::Beq, "field_hum_none");
        abs(p, M::Cmp, TEMP);
        branch(p, M::Beq, "field_hum_found");
        implied(p, M::Inx);
        implied(p, M::Inx);
        branch(p, M::Bra, "field_hum_find");
        p.label("field_hum_found")
            .assume_mode(CpuMode::NATIVE_M16_X16);
        m8(p);
        write(p, HUM, HUM_ON);
        implied(p, M::Plx);
        jump(p, "field_next8");
        p.label("field_hum_none")
            .assume_mode(CpuMode::NATIVE_M16_X16);
        emit(p, M::Ldx, A::Immediate, O::Word(0));
        p.label("field_warp_quake_find")
            .assume_mode(CpuMode::NATIVE_M16_X16);
        long_x(p, M::Lda, quakes);
        emit(p, M::Cmp, A::Immediate, O::Word(0xffff));
        branch(p, M::Beq, "field_warp_quake_none");
        abs(p, M::Cmp, TEMP);
        branch(p, M::Beq, "field_warp_quake_found");
        implied(p, M::Txa);
        implied(p, M::Clc);
        emit(p, M::Adc, A::Immediate, O::Word(5));
        implied(p, M::Tax);
        branch(p, M::Bra, "field_warp_quake_find");
        p.label("field_warp_quake_found")
            .assume_mode(CpuMode::NATIVE_M16_X16);
        long_x(p, M::Lda, quakes + 2);
        abs(p, M::Sta, QUAKE_BASE);
        m8(p);
        long_x(p, M::Lda, quakes + 4);
        abs(p, M::Sta, QUAKE);
        implied(p, M::Plx);
        jump(p, "field_next8");
        p.label("field_warp_quake_none")
            .assume_mode(CpuMode::NATIVE_M16_X16);
        implied(p, M::Plx);
        m8(p);
    }
    let root = &layout.section("field_quake_root")?.bytes;
    m16(p);
    long_x(p, M::Lda, program + 4);
    emit(
        p,
        M::Cmp,
        A::Immediate,
        O::Word(u16::from_le_bytes([root[0], root[1]])),
    );
    m8(p);
    branch(p, M::Beq, "field_quake_spawn");
    jump(p, "field_shards_spawn");
    p.label("field_quake_spawn");
    write(p, crate::sfc_b1_portrait::QUAKE, 32);
    abs(p, M::Stz, crate::sfc_b1_portrait::QUAKE_BASE);
    abs(p, M::Stz, crate::sfc_b1_portrait::QUAKE_BASE + 1);
    implied(p, M::Phx);
    emit(p, M::Ldx, A::Immediate, O::Word(128));
    p.label("field_smoke_draw");
    implied(p, M::Phx);
    for (i, divisor) in [0x70u8, 0x80].into_iter().enumerate() {
        emit(p, M::Lda, A::Immediate, O::Byte(divisor));
        call(p, "encounter_rng");
        abs(p, M::Sta, TEMP + i as u16);
    }
    implied(p, M::Plx);
    implied(p, M::Txa);
    emit(
        p,
        M::And,
        A::Immediate,
        O::Byte(crate::sfc_field_smoke::PUFF_EVERY as u8 - 1),
    );
    branch(p, M::Bne, "field_smoke_kept");
    implied(p, M::Phy);
    m16(p);
    implied(p, M::Txa);
    emit(p, M::Eor, A::Immediate, O::Word(0xffff));
    implied(p, M::Sec);
    emit(p, M::Adc, A::Immediate, O::Word(128));
    for _ in 0..3 {
        emit(p, M::Lsr, A::Accumulator, O::None);
    }
    implied(p, M::Tay);
    m8(p);
    for i in 0..2u16 {
        abs(p, M::Lda, TEMP + i);
        emit(
            p,
            M::Sta,
            A::AbsoluteY,
            O::Word(crate::sfc_field_smoke::SMOKE_DRAWS + i),
        );
    }
    implied(p, M::Ply);
    p.label("field_smoke_kept");
    implied(p, M::Dex);
    branch(p, M::Bne, "field_smoke_draw");
    implied(p, M::Plx);
    call(p, "field_smoke_start");
    jump(p, "field_next8");
    p.label("field_shards_spawn");
    let shards = &layout.section("field_shard_root")?.bytes;
    m16(p);
    long_x(p, M::Lda, program + 4);
    emit(
        p,
        M::Cmp,
        A::Immediate,
        O::Word(u16::from_le_bytes([shards[0], shards[1]])),
    );
    m8(p);
    branch(p, M::Bne, "field_spawn_done");
    implied(p, M::Phx);
    implied(p, M::Phy);
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("field_shards_draw");
    implied(p, M::Phy);
    emit(p, M::Lda, A::Immediate, O::Byte(SHARD_DRAW_RANGE));
    call(p, "encounter_rng");
    implied(p, M::Ply);
    emit(
        p,
        M::Sta,
        A::AbsoluteY,
        O::Word(crate::sfc_field_shards::SHARD_DRAWS),
    );
    implied(p, M::Iny);
    emit(
        p,
        M::Cpy,
        A::Immediate,
        O::Word(2 * crate::sfc_field_shards::SHARDS as u16),
    );
    branch(p, M::Bne, "field_shards_draw");
    implied(p, M::Ply);
    implied(p, M::Plx);
    abs(p, M::Lda, INPUT + 0x19);
    call(p, "field_shards_start");
    p.label("field_spawn_done");
    jump(p, "field_next8");
    Ok(())
}
