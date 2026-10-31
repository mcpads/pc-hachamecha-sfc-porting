use super::*;
pub(super) fn emit(
    p: &mut Assembler,
    loader: u32,
    driver: u32,
    loader_size: u16,
    driver_size: u16,
) {
    label(p, "sound_boot");
    word(p, M::Ldx, STATE_END - STATE_START - 1);
    label(p, "tad_clear");
    ax(p, M::Stz, STATE_START);
    imp(p, M::Dex);
    op(p, M::Bpl, A::Relative8, O::Relative8(-6));
    for a in [
        MUSIC,
        PENDING,
        RETURN,
        WANTED,
        PENDING_CLIP,
        LOADED,
        CURRENT,
        PENDING_CLIP + SLOT_STRIDE,
        LOADED + SLOT_STRIDE,
        CURRENT + SLOT_STRIDE,
    ] {
        write(p, a, 0xff);
    }
    byte(p, M::Lda, 0xaa);
    call(p, "tad_wait_0");
    br(p, M::Bcs, "tad_boot_done");
    byte(p, M::Lda, 0xbb);
    call(p, "tad_wait_1");
    br(p, M::Bcs, "tad_boot_done");
    write(p, 0x2142, 0);
    write(p, 0x2143, 2);
    write(p, 0x2141, 1);
    write(p, 0x2140, 0xcc);
    call(p, "tad_wait_0");
    br(p, M::Bcs, "tad_boot_done");
    word(p, M::Ldx, 0);
    label(p, "tad_ipl_byte");
    longx(p, M::Lda, loader);
    abs(p, M::Sta, 0x2141);
    imp(p, M::Txa);
    abs(p, M::Sta, 0x2140);
    call(p, "tad_wait_0");
    br(p, M::Bcs, "tad_boot_done");
    imp(p, M::Inx);
    word(p, M::Cpx, loader_size);
    br(p, M::Bne, "tad_ipl_byte");
    write(p, 0x2141, 0);
    write(p, 0x2142, 0);
    write(p, 0x2143, 2);
    imp(p, M::Txa);
    imp(p, M::Clc);
    byte(p, M::Adc, 2);
    abs(p, M::Sta, 0x2140);
    call(p, "tad_wait_0");
    br(p, M::Bcs, "tad_boot_done");
    pointer(p, driver, PTR);
    m16(p);
    word(p, M::Lda, driver_size);
    abs(p, M::Sta, SIZE);
    m8(p);
    write(p, TYPE, 0);
    write(p, READY, 1);
    call(p, "tad_begin_upload");
    label(p, "tad_boot_driver");
    call(p, "tad_upload_chunk");
    abs(p, M::Lda, READY);
    br(p, M::Beq, "tad_boot_done");
    m16(p);
    abs(p, M::Lda, OFFSET);
    abs(p, M::Cmp, SIZE);
    m8(p);
    br(p, M::Bcc, "tad_boot_driver");
    label(p, "tad_boot_done");
    imp(p, M::Rts);
    for port in 0..4 {
        label(p, &format!("tad_wait_{port}"));
        imp(p, M::Phx);
        word(p, M::Ldx, 0xffff);
        label(p, &format!("tad_poll_{port}"));
        abs(p, M::Cmp, 0x2140 + port);
        p.emit_label_ref(M::Beq, A::Relative8, "tad_wait_acked");
        imp(p, M::Dex);
        p.emit_label_ref(M::Bne, A::Relative8, format!("tad_poll_{port}"));
        jump(p, "tad_wait_timeout");
    }
    label(p, "tad_wait_timeout");
    imp(p, M::Plx);
    write(p, FAULT, 1);
    abs(p, M::Stz, READY);
    imp(p, M::Sec);
    imp(p, M::Rts);
    label(p, "tad_wait_acked");
    imp(p, M::Plx);
    imp(p, M::Clc);
    imp(p, M::Rts);
    label(p, "tad_upload_ack");
    word(p, M::Ldx, 0xffff);
    label(p, "tad_upload_poll");
    abs(p, M::Cmp, 0x2143);
    p.emit_label_ref(M::Beq, A::Relative8, "tad_upload_acked");
    imp(p, M::Dex);
    p.emit_label_ref(M::Bne, A::Relative8, "tad_upload_poll");
    write(p, FAULT, 1);
    abs(p, M::Stz, READY);
    imp(p, M::Sec);
    imp(p, M::Rts);
    label(p, "tad_upload_acked");
    imp(p, M::Clc);
    imp(p, M::Rts);
    label(p, "tad_begin_upload");
    byte(p, M::Lda, 0x44);
    call(p, "tad_wait_3");
    br(p, M::Bcs, "tad_upload_return");
    byte(p, M::Lda, 0x4c);
    call(p, "tad_wait_2");
    br(p, M::Bcs, "tad_upload_return");
    abs(p, M::Lda, TYPE);
    abs(p, M::Sta, 0x2141);
    write(p, 0x2142, 0x4c);
    write(p, 0x2143, 0x44);
    byte(p, M::Lda, 0);
    call(p, "tad_wait_3");
    abs(p, M::Stz, COUNTER);
    m16(p);
    abs(p, M::Stz, OFFSET);
    m8(p);
    imp(p, M::Rts);
    label(p, "tad_upload_chunk");
    abs(p, M::Lda, READY);
    br(p, M::Beq, "tad_upload_return");
    for i in 0..3 {
        abs(p, M::Lda, PTR + i);
        op(
            p,
            M::Sta,
            A::DirectPage,
            O::Byte(TRANSFER_POINTER + i as u8),
        );
    }
    m16(p);
    abs(p, M::Ldy, OFFSET);
    m8(p);
    abs(p, M::Stz, BUDGET);
    label(p, "tad_upload_word");
    op(
        p,
        M::Lda,
        A::DirectPageIndirectLongIndexedY,
        O::Byte(TRANSFER_POINTER),
    );
    abs(p, M::Sta, 0x2141);
    imp(p, M::Iny);
    op(
        p,
        M::Lda,
        A::DirectPageIndirectLongIndexedY,
        O::Byte(TRANSFER_POINTER),
    );
    abs(p, M::Sta, 0x2142);
    imp(p, M::Iny);
    imp(p, M::Tya);
    byte(p, M::And, 7);
    byte(p, M::Ora, 1);
    abs(p, M::Sta, 0x2143);
    call(p, "tad_upload_ack");
    br(p, M::Bcs, "tad_upload_return");
    abs(p, M::Cpy, SIZE);
    br(p, M::Beq, "tad_upload_end");
    abs(p, M::Dec, BUDGET);
    p.emit_label_ref(M::Bne, A::Relative8, "tad_upload_word");
    abs(p, M::Sty, OFFSET);
    label(p, "tad_upload_return");
    imp(p, M::Rts);
    label(p, "tad_upload_end");
    abs(p, M::Sty, OFFSET);
    write(p, 0x2143, 0x80);
    imp(p, M::Rts);
    label(p, "tad_command");
    abs(p, M::Lda, READY);
    br(p, M::Beq, "tad_command_done");
    abs(p, M::Lda, TOKEN);
    call(p, "tad_wait_0");
    br(p, M::Bcs, "tad_command_done");
    abs(p, M::Lda, PARAM);
    abs(p, M::Sta, 0x2141);
    abs(p, M::Lda, PARAM + 1);
    abs(p, M::Sta, 0x2142);
    abs(p, M::Lda, TOKEN);
    byte(p, M::Eor, 1);
    byte(p, M::And, 1);
    abs(p, M::Ora, COMMAND);
    abs(p, M::Sta, TOKEN);
    abs(p, M::Sta, 0x2140);
    call(p, "tad_wait_0");
    label(p, "tad_command_done");
    imp(p, M::Rts);
}
