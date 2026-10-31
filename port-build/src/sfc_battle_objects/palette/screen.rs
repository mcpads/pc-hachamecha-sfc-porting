use super::*;
pub(super) fn emit(p: &mut Assembler) {
    use crate::sfc_ending::{SCREEN_KEPT, SCREEN_WHITE};
    use crate::sfc_field_events::SCREEN_DARK;
    p.label("bvm_screen_range")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    dp(p, M::Lda, ATA);
    imm8(p, M::Cmp, 48);
    br(p, M::Bcs, "bvm_screen_range_no");
    implied(p, M::Clc);
    dp(p, M::Adc, ATA + 1);
    br(p, M::Bcs, "bvm_screen_range_yes");
    imm8(p, M::Cmp, 240);
    implied(p, M::Rts);
    p.label("bvm_screen_range_no");
    implied(p, M::Clc);
    p.label("bvm_screen_range_yes");
    implied(p, M::Rts);
    p.label("bvm_screen_read")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, SCREEN_DARK);
    imm16(p, M::And, 0xff);
    dp(p, M::Sta, TMP);
    m8(p);
    abs(p, M::Lda, SCREEN_WHITE);
    br(p, M::Bne, "bvm_screen_read_white");
    m16(p);
    imm16(p, M::Lda, 248);
    implied(p, M::Sec);
    dp(p, M::Sbc, TMP);
    implied(p, M::Rts);
    p.label("bvm_screen_read_white")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    dp(p, M::Lda, TMP);
    implied(p, M::Clc);
    imm16(p, M::Adc, 248);
    implied(p, M::Rts);
    p.label("bvm_screen_write")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    imm16(p, M::Cmp, 248);
    br(p, M::Bcc, "bvm_screen_write_dark");
    implied(p, M::Sec);
    imm16(p, M::Sbc, 248);
    m8(p);
    abs(p, M::Sta, SCREEN_DARK);
    imm8(p, M::Lda, 128);
    abs(p, M::Sta, SCREEN_WHITE);
    implied(p, M::Rts);
    p.label("bvm_screen_write_dark")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    dp(p, M::Sta, TMP);
    imm16(p, M::Lda, 248);
    implied(p, M::Sec);
    dp(p, M::Sbc, TMP);
    m8(p);
    abs(p, M::Sta, SCREEN_DARK);
    abs(p, M::Stz, SCREEN_WHITE);
    implied(p, M::Rts);
    p.label("bvm_screen_keep")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    jsr(p, "bvm_screen_range");
    br(p, M::Bcc, "bvm_screen_keep_done");
    jsr(p, "bvm_screen_read");
    p.assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Sta, SCREEN_KEPT);
    m8(p);
    p.label("bvm_screen_keep_done");
    implied(p, M::Rts);
    p.label("bvm_screen_restore")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    jsr(p, "bvm_screen_range");
    br(p, M::Bcc, "bvm_screen_restore_done");
    m16(p);
    abs(p, M::Lda, SCREEN_KEPT);
    jsr(p, "bvm_screen_write");
    p.assume_mode(CpuMode::NATIVE_M8_X16);
    p.label("bvm_screen_restore_done");
    implied(p, M::Rts);
    p.label("bvm_screen_restore_fade")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    jsr(p, "bvm_screen_range");
    br(p, M::Bcc, "bvm_screen_fade_done");
    jsr(p, "bvm_screen_read");
    p.assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Cmp, SCREEN_KEPT);
    br(p, M::Beq, "bvm_screen_fade_store");
    br(p, M::Bcs, "bvm_screen_fade_down");
    implied(p, M::Clc);
    dp(p, M::Adc, ATB);
    abs(p, M::Cmp, SCREEN_KEPT);
    br(p, M::Bcc, "bvm_screen_fade_store");
    br(p, M::Bra, "bvm_screen_fade_clamp");
    p.label("bvm_screen_fade_down");
    implied(p, M::Sec);
    dp(p, M::Sbc, ATB);
    br(p, M::Bcc, "bvm_screen_fade_clamp");
    abs(p, M::Cmp, SCREEN_KEPT);
    br(p, M::Bcs, "bvm_screen_fade_store");
    p.label("bvm_screen_fade_clamp");
    abs(p, M::Lda, SCREEN_KEPT);
    p.label("bvm_screen_fade_store");
    jsr(p, "bvm_screen_write");
    p.assume_mode(CpuMode::NATIVE_M8_X16);
    p.label("bvm_screen_fade_done");
    implied(p, M::Rts);
}
