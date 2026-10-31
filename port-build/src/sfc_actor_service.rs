use crate::{
    Result,
    sfc::{emit, implied, write},
};
use sfc_builder::{
    publication::Publication,
    transfer::{Destination, WramTransferPlan},
};
use w65c816::{AddressingMode as A, Assembler, CpuMode, Mnemonic as M, Operand as O};
pub struct Piece {
    pub owner: usize,
    pub dx: i16,
    pub dy: i16,
    pub edge: u16,
}
pub struct Actors {
    pub positions: Vec<[i16; 2]>,
    pub pieces: Vec<Piece>,
    pub initial_oam: Vec<u8>,
    pub expression_tiles: Vec<(usize, u16, u16)>,
}
fn publication(pieces: usize) -> Result<Publication> {
    if pieces == 0 || pieces > 128 {
        return Err("Invalid resident actor piece count".into());
    }
    Publication::new(
        0x230,
        WramTransferPlan::new(0x2000, pieces * 4, Destination::Oam(0), 512)?,
    )
}
fn store(p: &mut Assembler, offset: usize) {
    emit(
        p,
        M::Sta,
        A::AbsoluteLong,
        O::Long(0x7e2000 + offset as u32),
    );
}
fn byte(p: &mut Assembler, offset: usize, value: u8) {
    emit(p, M::Lda, A::Immediate, O::Byte(value));
    store(p, offset);
}
impl Actors {
    pub fn reset(&self, p: &mut Assembler) -> Result<()> {
        if self.initial_oam.len() != 544 || self.pieces.len() > 128 || self.positions.len() > 8 {
            return Err("Invalid actor service allocation".into());
        }
        publication(self.pieces.len())?.emit_reset(p);
        write(p, 0x220, 0);
        write(p, 0x221, 0);
        write(p, 0x224, 2);
        for (i, pos) in self.positions.iter().enumerate() {
            for (axis, value) in pos.iter().enumerate() {
                for (b, value) in value.to_le_bytes().into_iter().enumerate() {
                    write(p, 0x240 + (i * 4 + axis * 2 + b) as u16, value);
                }
            }
        }
        for (offset, value) in self.initial_oam.iter().enumerate() {
            byte(p, offset, *value);
        }
        Ok(())
    }
    pub fn routines(&self, p: &mut Assembler) -> Result<()> {
        let publication = publication(self.pieces.len())?;
        publication.emit_begin(p, "actors_begin");
        publication.emit_submit(p, "actors_submit");
        publication.emit_consume(p, 0, "actors_consume_low")?;
        p.label("actors_consume")
            .assume_mode(CpuMode::NATIVE_M8_X16);
        p.emit_label_ref(M::Jsr, A::Absolute, "actors_consume_low");
        p.emit_label_ref(M::Bcs, A::Relative8, "actors_consume_done");
        let high_bytes = self.pieces.len().div_ceil(4).next_multiple_of(2);
        for chunk in WramTransferPlan::new(0x2200, high_bytes, Destination::Oam(512), 32)?.chunks()
        {
            chunk.emit(p, 0)?;
        }
        implied(p, M::Clc);
        p.label("actors_consume_done");
        implied(p, M::Rts);
        p.label("actors_update").assume_mode(CpuMode::NATIVE_M8_X16);
        emit(p, M::Lda, A::Absolute, O::Word(0x220));
        p.emit_label_ref(M::Bne, A::Relative8, "actors_requested");
        implied(p, M::Rts);
        p.label("actors_requested");
        p.emit_label_ref(M::Jsr, A::Absolute, "actors_begin");
        p.emit_label_ref(M::Bcc, A::Relative8, "actors_owned");
        implied(p, M::Rts);
        p.label("actors_owned");
        emit(p, M::Lda, A::Absolute, O::Word(0x224));
        emit(p, M::Cmp, A::Immediate, O::Byte(1));
        p.emit_label_ref(M::Beq, A::Relative8, "actors_frame_valid");
        emit(p, M::Cmp, A::Immediate, O::Byte(2));
        p.emit_label_ref(M::Beq, A::Relative8, "actors_frame_valid");
        write(p, 0x220, 0);
        write(p, 0x221, 2);
        write(p, 0x230, 0);
        implied(p, M::Rts);
        p.label("actors_frame_valid");
        emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
        for i in 0..self.positions.len() * 2 {
            let ok = format!("actor_anchor_{i}");
            emit(p, M::Lda, A::Absolute, O::Word(0x240 + i as u16 * 2));
            implied(p, M::Clc);
            emit(p, M::Adc, A::Immediate, O::Word(1024));
            emit(p, M::Cmp, A::Immediate, O::Word(2048));
            p.emit_label_ref(M::Bcc, A::Relative8, &ok);
            p.emit_label_ref(M::Brl, A::Relative16, "actors_invalid");
            p.label(ok);
        }
        emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
        emit(p, M::Lda, A::Absolute, O::Word(0x224));
        emit(p, M::Cmp, A::Immediate, O::Byte(1));
        p.emit_label_ref(M::Bne, A::Relative8, "actors_frame_two");
        for (slot, one, _) in &self.expression_tiles {
            byte(p, slot * 4 + 2, *one as u8);
            byte(
                p,
                slot * 4 + 3,
                (self.initial_oam[slot * 4 + 3] & !1) | (*one >> 8) as u8,
            );
        }
        p.emit_label_ref(M::Bra, A::Relative8, "actors_frame_set");
        p.label("actors_frame_two");
        for (slot, _, two) in &self.expression_tiles {
            byte(p, slot * 4 + 2, *two as u8);
            byte(
                p,
                slot * 4 + 3,
                (self.initial_oam[slot * 4 + 3] & !1) | (*two >> 8) as u8,
            );
        }
        p.label("actors_frame_set");
        for i in 0..32 {
            let mut value = self.initial_oam[512 + i];
            for slot in i * 4..(i * 4 + 4).min(self.pieces.len()) {
                value &= !(1 << (slot % 4 * 2));
            }
            byte(p, 512 + i, value);
        }
        for (slot, piece) in self.pieces.iter().enumerate() {
            let hidden = format!("actor_piece_{slot}_hidden");
            let done = format!("actor_piece_{slot}_done");
            emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
            for (axis, delta, bound) in [(0, piece.dx, 256), (1, piece.dy, 224)] {
                let negative = format!("actor_piece_{slot}_{axis}_negative");
                let visible = format!("actor_piece_{slot}_{axis}_visible");
                emit(
                    p,
                    M::Lda,
                    A::Absolute,
                    O::Word(0x240 + piece.owner as u16 * 4 + axis * 2),
                );
                implied(p, M::Clc);
                emit(p, M::Adc, A::Immediate, O::Word(delta as u16));
                emit(p, M::Sta, A::Absolute, O::Word(0x280 + axis * 2));
                p.emit_label_ref(M::Bmi, A::Relative8, &negative);
                emit(p, M::Cmp, A::Immediate, O::Word(bound));
                p.emit_label_ref(M::Bcc, A::Relative8, &visible);
                p.emit_label_ref(M::Brl, A::Relative16, &hidden);
                p.label(negative);
                emit(
                    p,
                    M::Cmp,
                    A::Immediate,
                    O::Word(1u16.wrapping_sub(piece.edge)),
                );
                p.emit_label_ref(M::Bcs, A::Relative8, &visible);
                p.emit_label_ref(M::Brl, A::Relative16, &hidden);
                p.label(visible);
            }
            emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
            emit(p, M::Lda, A::Absolute, O::Word(0x280));
            store(p, slot * 4);
            emit(p, M::Lda, A::Absolute, O::Word(0x282));
            store(p, slot * 4 + 1);
            emit(p, M::Lda, A::Absolute, O::Word(0x281));
            emit(p, M::And, A::Immediate, O::Byte(1));
            for _ in 0..slot % 4 * 2 {
                emit(p, M::Asl, A::Accumulator, O::None);
            }
            p.emit_label_ref(M::Bra, A::Relative8, &done);
            p.label(hidden).assume_mode(CpuMode::NATIVE_M16_X16);
            emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
            byte(p, slot * 4, 0);
            emit(p, M::Lda, A::Immediate, O::Byte(1 << (slot % 4 * 2)));
            p.label(done);
            emit(
                p,
                M::Ora,
                A::AbsoluteLong,
                O::Long(0x7e2200 + (slot / 4) as u32),
            );
            store(p, 512 + slot / 4);
        }
        write(p, 0x220, 0);
        write(p, 0x221, 0);
        p.emit_label_ref(M::Jsr, A::Absolute, "actors_submit");
        implied(p, M::Rts);
        p.label("actors_invalid")
            .assume_mode(CpuMode::NATIVE_M16_X16);
        emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
        write(p, 0x220, 0);
        write(p, 0x221, 1);
        write(p, 0x230, 0);
        implied(p, M::Rts);
        Ok(())
    }
}
