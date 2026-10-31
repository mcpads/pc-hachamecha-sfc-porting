use super::*;
mod collect;
mod draw;
mod order;
mod publish;
mod tile_cache;
const PAGE: u16 = OBJECT_PAGE;
const FRAME_POINTER: u16 = OBJECT_FRAME_POINTER;
const FRAME_DP: u8 = (FRAME_POINTER - PAGE) as u8;
pub(super) fn emit_objects(
    p: &mut Assembler,
    records: u32,
    slots: u32,
    tables: [u32; 4],
    [extents, firsts, stages, gather, single_block, arle_effect]: [u32; 6],
    oam: u16,
    oam_job: u32,
) -> Result<()> {
    collect::emit_collect(p, slots, extents, firsts);
    order::emit_order(p);
    draw::emit_draw(
        p,
        records,
        slots,
        tables,
        oam,
        single_block,
        arle_effect as u16,
    );
    publish::emit_publish(p, oam, oam_job);
    tile_cache::emit_tile_cache(p, records, stages, gather);
    Ok(())
}
fn abs(p: &mut Assembler, m: M, at: u16) {
    if (PAGE..PAGE + 0x100).contains(&at) {
        emit(p, m, A::DirectPage, O::Byte((at - PAGE) as u8));
    } else {
        crate::sfc_battle_act::abs(p, m, at);
    }
}
fn long(p: &mut Assembler, m: M, at: u32) {
    emit(p, m, A::AbsoluteLong, O::Long(at));
}
fn long_x(p: &mut Assembler, m: M, at: u32) {
    emit(p, m, A::AbsoluteLongX, O::Long(at));
}
fn indirect(p: &mut Assembler, m: M) {
    emit(p, m, A::DirectPageIndirectLongIndexedY, O::Byte(FRAME_DP));
}
fn pool(p: &mut Assembler, m: M, field: u16) {
    emit(p, m, A::AbsoluteLongX, O::Long(0x7e0000 + u32::from(field)));
}
fn br(p: &mut Assembler, m: M, label: &str) {
    p.emit_label_ref(m, A::Relative8, label);
}
fn jmp(p: &mut Assembler, label: &str) {
    p.emit_label_ref(M::Jmp, A::Absolute, label);
}
fn jsr(p: &mut Assembler, label: &str) {
    p.emit_label_ref(M::Jsr, A::Absolute, label);
}
