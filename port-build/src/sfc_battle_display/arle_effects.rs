use super::*;
pub(super) const FIRST_FRAME: u16 = 12;
pub(super) const END_FRAME: u16 = 22;
pub(super) const COLORS: u16 = 180;
const SOURCE_COLORS: [usize; 4] = [3, 4, 5, 15];
pub(super) fn resource(body: &Resource, sheet: &assets::Sheet) -> Result<Resource> {
    if body.file != "ANM.CNS"
        || body.slot != 27
        || body.key != 14
        || body.rects.len() != usize::from(END_FRAME)
        || body.source_base != 176
    {
        return Err("ANM body/effect frame contract changed".into());
    }
    let rects = body.rects[usize::from(FIRST_FRAME)..].to_vec();
    let mut frames = Vec::new();
    for &rect in &rects {
        let mut image = assets::scaled(sheet, rect, body.key);
        for pixel in &mut image.pixels {
            if *pixel != 0 {
                *pixel = SOURCE_COLORS
                    .iter()
                    .position(|&c| c == usize::from(*pixel))
                    .ok_or("ANM spark uses an unreserved colour")? as u8
                    + 4;
            }
        }
        frames.push(
            assets::sprite_frame(&image, (0, 0))?.ok_or("ANM spark exceeds sprite capacity")?,
        );
    }
    let mut palette = vec![0; 32];
    for (to, from) in SOURCE_COLORS.into_iter().enumerate() {
        palette[(to + 4) * 2..(to + 5) * 2].copy_from_slice(&body.palette[from * 2..from * 2 + 2]);
    }
    Ok(Resource {
        file: body.file.clone(),
        slot: body.slot,
        anchors: vec![(0, 0); rects.len()],
        rects,
        used: 0,
        colors: 176,
        palette,
        source_base: body.source_base,
        key: body.key,
        rgb: body.rgb.clone(),
        rgb_upper: None,
        frames: Frames::Sprites(frames),
    })
}
pub(super) fn emit_palette_mirror(p: &mut Assembler) {
    abs(p, M::Lda, CELL);
    word(p, M::Cmp, ARLE_COLORS);
    branch(p, M::Bne, "bdp_no_arle_effects");
    for (source, target, bytes) in [(6, COLORS, 6), (30, COLORS + 3, 2)] {
        abs(p, M::Lda, PALETTE_SENT);
        for _ in 0..3 {
            emit(p, M::Asl, A::Accumulator, O::None);
        }
        implied(p, M::Tax);
        let put = |p: &mut Assembler, at, value| {
            word(p, M::Lda, value);
            emit(p, M::Sta, A::AbsoluteLongX, O::Long(PALETTE_LIST + at));
        };
        put(p, 0, 1);
        put(p, 1, target);
        put(p, 3, bytes);
        abs(p, M::Lda, ROW);
        implied(p, M::Clc);
        word(p, M::Adc, PALETTE_BUFFER as u16 + source);
        emit(p, M::Sta, A::AbsoluteLongX, O::Long(PALETTE_LIST + 5));
        put(p, 7, (PALETTE_BUFFER >> 16) as u16);
        abs(p, M::Inc, PALETTE_SENT);
    }
    p.label("bdp_no_arle_effects");
}
