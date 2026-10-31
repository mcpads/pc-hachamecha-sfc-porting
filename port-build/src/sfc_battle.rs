use crate::Result;
use sfc_builder::cartridge::{CartridgeLayout, Section};
use w65c816::{AddressingMode as A, CpuMode, Mnemonic as M, Operand as O};
pub const STATS: u16 = 0xa40;
pub const STATS_BYTES: usize = 0x60;
pub const LEVEL: u16 = STATS + 2;
pub const HP: u16 = STATS + 4;
pub const HP_MAX: u16 = STATS + 6;
pub const MP: u16 = STATS + 8;
pub const MP_MAX: u16 = STATS + 10;
pub const LEARNED: u16 = STATS + 0x26;
const COMPANION: usize = 0x30;
pub const ACTORS: u16 = 0xb00;
pub const ACTOR_BYTES: u16 = 104;
pub const ACTOR_COUNT: u16 = 0xca0;
pub const ORDER: u16 = 0xca2;
pub const ORDER_INDEX: u16 = 0xca8;
const PLACING: u16 = 0xcaa;
const COMPARED: u16 = 0xcac;
pub(crate) const DIVIDEND: u16 = 0xcb0;
pub(crate) const DIVISOR: u16 = 0xcb2;
pub(crate) const QUOTIENT: u16 = 0xcb4;
const BIND: u16 = 0xcb6;
const SCRIPT_PC: u16 = 0xcb8;
pub(crate) const RESULT: u16 = 0xcba;
pub(crate) const CURRENT: u16 = 0xcbc;
pub(crate) const CURRENT_INDEX: u16 = 0xcbe;
pub fn initial_stats(content: &crate::content::Content) -> Result<Vec<u8>> {
    let actors: serde_json::Value = content.json("combat-actors.json")?;
    let mut block = vec![0u8; STATS_BYTES];
    for (slot, at) in [(0, 0), (1, COMPANION)] {
        let party = actors["party_initial"]
            .as_array()
            .and_then(|p| p.iter().find(|r| r["slot"] == slot))
            .ok_or("Missing party record")?;
        let raw = hex(party["raw_hex"].as_str().ok_or("Party record bytes")?)?;
        if raw.len() != 32 {
            return Err("Party record is not 32 bytes".into());
        }
        block[at..at + 32].copy_from_slice(&raw);
    }
    let spells: Vec<u8> =
        serde_json::from_value(actors["player_spells"]["initial_values"].clone())?;
    if spells.len() != 8 {
        return Err("Start spells are not eight slots".into());
    }
    let at = usize::from(LEARNED - STATS);
    block[at..at + 8].copy_from_slice(&spells);
    Ok(block)
}
fn hex(text: &str) -> Result<Vec<u8>> {
    (0..text.len())
        .step_by(2)
        .map(|i| Ok(u8::from_str_radix(&text[i..i + 2], 16)?))
        .collect()
}
pub fn enemy_records(content: &crate::content::Content) -> Result<Vec<u8>> {
    let actors: serde_json::Value = content.json("combat-actors.json")?;
    let entries = actors["entries"]
        .as_array()
        .ok_or("Missing enemy records")?;
    let mut table = Vec::new();
    for (id, entry) in entries.iter().enumerate() {
        if entry["id"] != id {
            return Err("Enemy records out of id order".into());
        }
        let raw = hex(entry["stats_hex"].as_str().ok_or("Enemy record bytes")?)?;
        if raw.len() != 32 {
            return Err("Enemy record is not 32 bytes".into());
        }
        table.extend(raw);
    }
    Ok(table)
}
pub fn sections(
    sections: &mut Vec<Section>,
    space: &mut crate::sfc_rom::Space,
    stats: &[u8],
    enemies: &[u8],
    scripts: &crate::sfc_battle_scripts::Scripts,
) -> Result<()> {
    crate::sfc_b1::add(sections, space, "party_stats".into(), stats.to_vec())?;
    crate::sfc_b1::add(sections, space, "enemy_records".into(), enemies.to_vec())?;
    crate::sfc_b1::add(
        sections,
        space,
        "battle_scripts".into(),
        scripts.code.clone(),
    )?;
    crate::sfc_b1::add(
        sections,
        space,
        "battle_script_roots".into(),
        scripts.roots.clone(),
    )?;
    let offset = space.reserve_table(CODE_BYTES)?;
    sections.push(Section {
        name: CODE_SECTION.into(),
        offset,
        bytes: vec![0; CODE_BYTES],
    });
    Ok(())
}
pub fn emit(p: &mut w65c816::Assembler, layout: &CartridgeLayout) -> Result<()> {
    use crate::sfc::{emit, implied};
    let initial = layout.cpu_address(layout.section("party_stats")?.offset)?;
    p.label("stats_reset").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("stats_reset_byte");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(initial));
    emit(p, M::Sta, A::AbsoluteX, O::Word(STATS));
    implied(p, M::Inx);
    emit(p, M::Cpx, A::Immediate, O::Word(STATS_BYTES as u16));
    p.emit_label_ref(M::Bne, A::Relative8, "stats_reset_byte");
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("stats_reset_actor");
    emit(p, M::Stz, A::AbsoluteX, O::Word(ACTORS));
    implied(p, M::Inx);
    emit(p, M::Cpx, A::Immediate, O::Word(ACTOR_BYTES));
    p.emit_label_ref(M::Bne, A::Relative8, "stats_reset_actor");
    implied(p, M::Rts);
    let rules = code(layout)?;
    let placed = &layout.section(CODE_SECTION)?.bytes;
    if placed[..rules.bytes().len()] != *rules.bytes() {
        return Err("Battle rules bank differs from its placed section".into());
    }
    for entry in ENTRIES {
        let far = rules
            .label_location(&format!("{entry}_far"))
            .ok_or("Missing battle entry")?;
        p.label(entry.to_string())
            .assume_mode(CpuMode::NATIVE_M8_X16);
        emit(p, M::Jsl, A::AbsoluteLong, O::Long(far.to_u32()));
        implied(p, M::Rts);
    }
    Ok(())
}
pub fn code(layout: &CartridgeLayout) -> Result<w65c816::AssembledProgram> {
    let first = assemble_rules(layout, None)?;
    let table = crate::sfc_battle_objects::dispatch_table(&first)?;
    assemble_rules(layout, Some(&table))
}
fn assemble_rules(
    layout: &CartridgeLayout,
    table: Option<&[u8]>,
) -> Result<w65c816::AssembledProgram> {
    use crate::sfc::implied;
    let at = layout.cpu_address(layout.section(CODE_SECTION)?.offset)?;
    let mut p = w65c816::Assembler::new();
    for entry in ENTRIES {
        p.label(format!("{entry}_far"))
            .assume_mode(CpuMode::NATIVE_M8_X16);
        p.emit_label_ref(M::Jsr, A::Absolute, *entry);
        implied(&mut p, M::Rtl);
    }
    crate::sfc_encounter::emit_rng(&mut p);
    rules(&mut p, layout)?;
    crate::sfc_battle_act::emit_rules(&mut p, layout)?;
    crate::sfc_battle_vm::emit_rules(&mut p, layout)?;
    crate::sfc_battle_message::emit_rules(&mut p, layout)?;
    crate::sfc_shop::emit_trade(&mut p, layout)?;
    crate::sfc_item_use::emit_rules(&mut p, layout)?;
    crate::sfc_battle_display::emit_rules(&mut p, layout)?;
    crate::sfc_battle_objects::emit_rules(&mut p, layout, table)?;
    crate::sfc_prologue::emit_skip(&mut p, layout)?;
    crate::sfc_ending::emit_rules(&mut p, layout)?;
    let program = crate::sfc_width_check::assemble(
        &p,
        w65c816::CodeLocation::new((at >> 16) as u8, at as u16),
        CpuMode::NATIVE_M8_X16,
    )
    .map_err(|e| format!("Rules bank: {e:?}"))?;
    if program.bytes().len() > CODE_BYTES {
        return Err(format!("Battle rules take {} bytes", program.bytes().len()).into());
    }
    Ok(program)
}
const ENTRIES: &[&str] = &[
    "encounter_rng",
    "encounter_rng_skip",
    "battle_setup",
    "battle_order",
    "battle_start",
    "battle_growth_start",
    "battle_tick",
    "battle_message_tick",
    "item_use",
    "battle_act",
    "battle_apply",
    "battle_display_start",
    "battle_display_tick",
    "battle_display_end",
    "battle_view_overlay",
    "prologue_skip",
    "prologue_record_loaded",
    "field_shortage",
    "status_prepare",
    "bvm_reset",
    "bvm_spawn",
    "bvm_spawn_root",
    "bvm_update",
    "ending_start",
    "scene_reserved",
    "ending_setup",
    "ending_registers",
    "ending_frame",
    "ending_text_work",
];
pub fn report(layout: &CartridgeLayout) -> Result<serde_json::Value> {
    let rules = code(layout)?;
    let mut entries = serde_json::Map::new();
    for entry in ENTRIES {
        let far = rules
            .label_location(&format!("{entry}_far"))
            .ok_or("Missing battle entry")?;
        entries.insert(entry.to_string(), far.to_u32().into());
    }
    let mut labels = serde_json::Map::new();
    for name in crate::sfc_battle_objects::PROFILE_LABELS
        .iter()
        .map(|s| s.to_string())
        .chain(ENTRIES.iter().map(|s| s.to_string()))
        .chain((0..=255u32).map(|n| format!("bvm_op_{n:02x}")))
        .chain(["battle_run_script".to_string()])
        .chain(
            crate::sfc_battle_display::PROFILE_LABELS
                .iter()
                .chain(crate::sfc_ending::PROFILE_LABELS)
                .map(|s| s.to_string()),
        )
    {
        if let Some(at) = rules.label_location(&name) {
            labels.insert(name, at.to_u32().into());
        }
    }
    Ok(serde_json::json!(
        { "bytes" : rules.bytes().len(), "far_entries" : entries, "bvm_labels" :
        labels, "display_object_abi" : { "direct_page" : crate
        ::sfc_battle_display::OBJECT_PAGE, "frame_pointer" : crate
        ::sfc_battle_display::OBJECT_FRAME_POINTER, "caller_direct_page" : 0,
        "public_frame_pointer" : 240, "borrowed_bytes" : 3, "restores_borrowed_bytes"
        : true }, "compiled_body_blocks" : crate
        ::sfc_battle_objects::compiled_blocks_report(), "radial_velocity" : crate
        ::sfc_battle_objects::radial_velocity_report(), "linked_velocity" : crate
        ::sfc_battle_objects::linked_velocity_report(), "orbit_turn" : crate
        ::sfc_battle_objects::orbit_turn_report(), "inward_turn" : crate
        ::sfc_battle_objects::inward_turn_report(), "inward_projection" : crate
        ::sfc_battle_objects::inward_projection_report(layout) ?, "jugem_density" :
        crate ::sfc_battle_objects::jugem_density_report(), "falling_turn" : crate
        ::sfc_battle_objects::falling_turn_report(), "falling_density" : crate
        ::sfc_battle_objects::falling_density_report(layout) ?, "falling_birth" :
        crate ::sfc_battle_objects::falling_birth_report(layout) ?, "common_radial" :
        crate ::sfc_battle_objects::common_radial_report(layout) ?, "common_blink" :
        crate ::sfc_battle_objects::common_blink_report(layout) ?, "lightning_radius"
        : crate ::sfc_battle_objects::lightning_radius_report(layout) ?,
        "lightning_density" : crate ::sfc_battle_objects::lightning_density_report(),
        "common_density" : crate ::sfc_battle_objects::common_density_report(layout)
        ?, "common_blink_density" : crate
        ::sfc_battle_objects::common_blink_density_report(layout) ?, "common_batch" :
        crate ::sfc_battle_objects::common_batch_report(layout) ?, "common_birth" :
        crate ::sfc_battle_objects::common_birth::report(layout) ?, "common_cohort" :
        crate ::sfc_battle_objects::common_cohort::report(& rules) ?,
        "lightning_turn" : crate ::sfc_battle_objects::lightning_turn::report(&
        rules) ?, "hidden_child" : crate ::sfc_battle_objects::hidden_child::report(&
        rules) ?, "bound_child_copy" : crate
        ::sfc_battle_objects::bound_child_copy_report(layout) ?, "falling_setup" :
        crate ::sfc_battle_objects::falling_setup_report(layout) ?,
        "compiled_initializers" : crate
        ::sfc_battle_objects::compiled_initializers_report(), "vertical_body_motion"
        : crate ::sfc_battle_objects::vertical_motion_report() }
    ))
}
const CODE_SECTION: &str = "battle_code";
const CODE_BYTES: usize = 0xb800;
pub fn place_code(sections: &mut [Section], layout: &CartridgeLayout) -> Result<()> {
    let mut bytes = code(layout)?.bytes().to_vec();
    bytes.resize(CODE_BYTES, 0);
    sections
        .iter_mut()
        .find(|s| s.name == CODE_SECTION)
        .ok_or("Missing battle code section")?
        .bytes = bytes;
    Ok(())
}
fn rules(p: &mut w65c816::Assembler, layout: &CartridgeLayout) -> Result<()> {
    use crate::sfc::{emit, implied};
    let enemies = layout.cpu_address(layout.section("enemy_records")?.offset)?;
    let abs = |p: &mut w65c816::Assembler, m, at: u16| emit(p, m, A::Absolute, O::Word(at));
    let m16 = |p: &mut w65c816::Assembler| emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
    let m8 = |p: &mut w65c816::Assembler| emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
    let branch = |p: &mut w65c816::Assembler, m, label: &str| {
        p.emit_label_ref(m, A::Relative8, label);
    };
    let jump = |p: &mut w65c816::Assembler, label: &str| {
        p.emit_label_ref(M::Jmp, A::Absolute, label);
    };
    p.label("battle_setup").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    emit(p, M::Lda, A::Immediate, O::Word(3));
    abs(p, M::Sta, ACTOR_COUNT);
    m8(p);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("battle_setup_clear");
    emit(p, M::Stz, A::AbsoluteX, O::Word(ACTORS));
    implied(p, M::Inx);
    emit(p, M::Cpx, A::Immediate, O::Word(3 * ACTOR_BYTES));
    branch(p, M::Bne, "battle_setup_clear");
    for (i, (to, from, bytes)) in [
        (ACTORS, STATS, 32u16),
        (ACTORS + 0x20, LEARNED, 8),
        (ACTORS + ACTOR_BYTES, STATS + COMPANION as u16, 32),
    ]
    .into_iter()
    .enumerate()
    {
        emit(p, M::Ldx, A::Immediate, O::Word(0));
        p.label(format!("battle_setup_copy_{i}"));
        emit(p, M::Lda, A::AbsoluteX, O::Word(from));
        emit(p, M::Sta, A::AbsoluteX, O::Word(to));
        implied(p, M::Inx);
        emit(p, M::Cpx, A::Immediate, O::Word(bytes));
        branch(p, M::Bne, &format!("battle_setup_copy_{i}"));
    }
    m16(p);
    abs(p, M::Lda, crate::sfc_encounter::ENEMY);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    for _ in 0..5 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("battle_setup_enemy");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(enemies));
    emit(p, M::Sta, A::AbsoluteY, O::Word(ACTORS + 2 * ACTOR_BYTES));
    implied(p, M::Inx);
    implied(p, M::Iny);
    emit(p, M::Cpy, A::Immediate, O::Word(32));
    branch(p, M::Bne, "battle_setup_enemy");
    abs(p, M::Lda, crate::sfc_encounter::ENEMY);
    emit(p, M::Cmp, A::Immediate, O::Byte(38));
    branch(p, M::Bne, "battle_setup_not_final");
    emit(p, M::Lda, A::Immediate, O::Byte(1));
    abs(p, M::Sta, ACTORS + 0x3e);
    m16(p);
    abs(p, M::Lda, ACTORS + 0x2e);
    emit(p, M::Ora, A::Immediate, O::Word(0x4000));
    abs(p, M::Sta, ACTORS + 0x2e);
    m8(p);
    p.label("battle_setup_not_final");
    for slot in [0u16, 2] {
        emit(p, M::Ldy, A::Immediate, O::Word(slot * ACTOR_BYTES));
        p.emit_label_ref(M::Jsr, A::Absolute, "battle_shortage");
        emit(p, M::Lda, A::Immediate, O::Byte(9));
        implied(p, M::Sec);
        emit(p, M::Sbc, A::AbsoluteY, O::Word(ACTORS + 0x4f));
        emit(p, M::Lsr, A::Accumulator, O::None);
        abs(p, M::Sta, QUOTIENT);
        emit(p, M::Lda, A::Immediate, O::Byte(4));
        implied(p, M::Sec);
        abs(p, M::Sbc, QUOTIENT);
        emit(p, M::Sta, A::AbsoluteY, O::Word(ACTORS + 0x42));
    }
    implied(p, M::Rts);
    p.label("battle_shortage")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    for (current, out) in [(4u16, 0x4fu16), (8, 0x50)] {
        m16(p);
        emit(p, M::Lda, A::AbsoluteY, O::Word(ACTORS + current));
        abs(p, M::Sta, crate::sfc_battle_act::AMOUNT);
        abs(p, M::Stz, crate::sfc_battle_act::AMOUNT + 2);
        emit(p, M::Lda, A::AbsoluteY, O::Word(ACTORS + current + 2));
        abs(p, M::Sta, DIVISOR);
        m8(p);
        p.emit_label_ref(M::Jsr, A::Absolute, "battle_steps");
        emit(p, M::Sta, A::AbsoluteY, O::Word(ACTORS + out));
    }
    implied(p, M::Rts);
    p.label("battle_order").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Stz, ORDER_INDEX);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("battle_order_index");
    implied(p, M::Txa);
    emit(p, M::Lsr, A::Accumulator, O::None);
    emit(p, M::Sta, A::AbsoluteX, O::Word(ORDER));
    implied(p, M::Inx);
    implied(p, M::Inx);
    implied(p, M::Txa);
    emit(p, M::Lsr, A::Accumulator, O::None);
    abs(p, M::Cmp, ACTOR_COUNT);
    branch(p, M::Bcc, "battle_order_index");
    emit(p, M::Lda, A::Immediate, O::Word(1));
    abs(p, M::Sta, PLACING);
    p.label("battle_order_actor");
    abs(p, M::Lda, PLACING);
    abs(p, M::Cmp, ACTOR_COUNT);
    branch(p, M::Bcs, "battle_order_done");
    emit(p, M::Dec, A::Accumulator, O::None);
    abs(p, M::Sta, COMPARED);
    p.label("battle_order_compare");
    abs(p, M::Lda, COMPARED);
    branch(p, M::Bmi, "battle_order_place");
    p.emit_label_ref(M::Jsr, A::Absolute, "battle_order_speed");
    abs(p, M::Sta, DIVIDEND);
    abs(p, M::Lda, PLACING);
    p.emit_label_ref(M::Jsr, A::Absolute, "battle_order_speed");
    abs(p, M::Cmp, DIVIDEND);
    branch(p, M::Beq, "battle_order_place");
    branch(p, M::Bcc, "battle_order_place");
    abs(p, M::Lda, COMPARED);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteX, O::Word(ORDER));
    emit(p, M::Sta, A::AbsoluteX, O::Word(ORDER + 2));
    abs(p, M::Dec, COMPARED);
    branch(p, M::Bra, "battle_order_compare");
    p.label("battle_order_place");
    abs(p, M::Lda, COMPARED);
    emit(p, M::Inc, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    abs(p, M::Lda, PLACING);
    emit(p, M::Sta, A::AbsoluteX, O::Word(ORDER));
    abs(p, M::Inc, PLACING);
    branch(p, M::Bra, "battle_order_actor");
    p.label("battle_order_done");
    m8(p);
    implied(p, M::Rts);
    p.label("battle_order_speed")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    implied(p, M::Phx);
    implied(p, M::Tax);
    emit(p, M::Lda, A::Immediate, O::Word(0));
    p.label("battle_order_speed_mul");
    implied(p, M::Dex);
    branch(p, M::Bmi, "battle_order_speed_at");
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(ACTOR_BYTES));
    branch(p, M::Bra, "battle_order_speed_mul");
    p.label("battle_order_speed_at");
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteX, O::Word(ACTORS + 0x12));
    implied(p, M::Plx);
    implied(p, M::Rts);
    let code = layout.cpu_address(layout.section("battle_scripts")?.offset)?;
    let roots = layout.cpu_address(layout.section("battle_script_roots")?.offset)?;
    let next = |p: &mut w65c816::Assembler, bytes: u16| {
        m16(p);
        abs(p, M::Lda, SCRIPT_PC);
        implied(p, M::Clc);
        emit(p, M::Adc, A::Immediate, O::Word(bytes));
        abs(p, M::Sta, SCRIPT_PC);
        m8(p);
        p.emit_label_ref(M::Jmp, A::Absolute, "battle_script_step");
    };
    p.label("battle_actor_offset")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    emit(p, M::And, A::Immediate, O::Word(3));
    implied(p, M::Tax);
    emit(p, M::Lda, A::Immediate, O::Word(0));
    p.label("battle_actor_offset_add");
    implied(p, M::Dex);
    branch(p, M::Bmi, "battle_actor_offset_done");
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(ACTOR_BYTES));
    branch(p, M::Bra, "battle_actor_offset_add");
    p.label("battle_actor_offset_done");
    m8(p);
    implied(p, M::Rts);
    p.label("battle_script_field")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(code + 2));
    emit(p, M::And, A::Immediate, O::Word(0xff));
    implied(p, M::Clc);
    abs(p, M::Adc, BIND);
    implied(p, M::Tay);
    m8(p);
    implied(p, M::Rts);
    p.label("battle_script").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Stx, A::Absolute, O::Word(SCRIPT_PC));
    p.label("battle_script_step");
    emit(p, M::Ldx, A::Absolute, O::Word(SCRIPT_PC));
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(code));
    emit(p, M::Cmp, A::Immediate, O::Byte(0x05));
    branch(p, M::Bne, "battle_script_not_05");
    implied(p, M::Rts);
    p.label("battle_script_not_05");
    emit(p, M::Cmp, A::Immediate, O::Byte(0x4d));
    branch(p, M::Bne, "battle_script_not_4d");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(code + 1));
    p.emit_label_ref(M::Jsr, A::Absolute, "battle_actor_offset");
    m16(p);
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Word(ACTORS));
    abs(p, M::Sta, BIND);
    m8(p);
    next(p, 4);
    p.label("battle_script_not_4d");
    emit(p, M::Cmp, A::Immediate, O::Byte(0x4b));
    branch(p, M::Bne, "battle_script_not_4b");
    m16(p);
    abs(p, M::Stz, BIND);
    m8(p);
    next(p, 4);
    p.label("battle_script_not_4b");
    emit(p, M::Cmp, A::Immediate, O::Byte(0x10));
    branch(p, M::Bne, "battle_script_not_10");
    p.emit_label_ref(M::Jsr, A::Absolute, "battle_script_field");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(code + 1));
    emit(p, M::Cmp, A::Immediate, O::Byte(0xc7));
    branch(p, M::Bne, "battle_script_not_or");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(code + 3));
    emit(p, M::Ora, A::AbsoluteY, O::Word(0));
    p.emit_label_ref(M::Bra, A::Relative8, "battle_script_store");
    p.label("battle_script_not_or");
    emit(p, M::Cmp, A::Immediate, O::Byte(0xc2));
    branch(p, M::Bne, "battle_script_set");
    emit(p, M::Lda, A::AbsoluteY, O::Word(0));
    implied(p, M::Sec);
    emit(p, M::Sbc, A::AbsoluteLongX, O::Long(code + 3));
    p.emit_label_ref(M::Bra, A::Relative8, "battle_script_store");
    p.label("battle_script_set");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(code + 3));
    p.label("battle_script_store");
    emit(p, M::Sta, A::AbsoluteY, O::Word(0));
    next(p, 4);
    p.label("battle_script_not_10");
    emit(p, M::Cmp, A::Immediate, O::Byte(0x13));
    branch(p, M::Bne, "battle_script_not_13");
    p.emit_label_ref(M::Jsr, A::Absolute, "battle_script_field");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(code + 1));
    emit(p, M::Cmp, A::Immediate, O::Byte(0xc4));
    branch(p, M::Bne, "battle_script_not_ge");
    emit(p, M::Lda, A::AbsoluteY, O::Word(0));
    emit(p, M::Cmp, A::AbsoluteLongX, O::Long(code + 3));
    branch(p, M::Bcs, "battle_script_taken");
    p.emit_label_ref(M::Bra, A::Relative8, "battle_script_not_taken");
    p.label("battle_script_not_ge");
    emit(p, M::Lda, A::AbsoluteY, O::Word(0));
    emit(p, M::Cmp, A::AbsoluteLongX, O::Long(code + 3));
    branch(p, M::Beq, "battle_script_equal");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(code + 1));
    emit(p, M::And, A::Immediate, O::Byte(1));
    branch(p, M::Beq, "battle_script_taken");
    p.emit_label_ref(M::Bra, A::Relative8, "battle_script_not_taken");
    p.label("battle_script_equal");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(code + 1));
    emit(p, M::And, A::Immediate, O::Byte(1));
    branch(p, M::Bne, "battle_script_taken");
    p.label("battle_script_not_taken");
    next(p, 8);
    p.label("battle_script_taken");
    m16(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(code + 4));
    abs(p, M::Sta, SCRIPT_PC);
    m8(p);
    jump(p, "battle_script_step");
    p.label("battle_script_not_13");
    emit(p, M::Cmp, A::Immediate, O::Byte(0x03));
    branch(p, M::Bne, "battle_script_not_03");
    m16(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(code + 4));
    abs(p, M::Sta, SCRIPT_PC);
    m8(p);
    jump(p, "battle_script_step");
    p.label("battle_script_not_03");
    emit(p, M::Cmp, A::Immediate, O::Byte(0x14));
    branch(p, M::Bne, "battle_script_not_14");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(code + 1));
    emit(p, M::Cmp, A::Immediate, O::Byte(0xc6));
    branch(p, M::Bne, "battle_script_result");
    p.emit_label_ref(M::Jsr, A::Absolute, "battle_script_field");
    m16(p);
    emit(p, M::Lda, A::AbsoluteY, O::Word(0));
    emit(p, M::And, A::AbsoluteLongX, O::Long(code + 4));
    m8(p);
    branch(p, M::Bne, "battle_script_taken_14");
    p.emit_label_ref(M::Bra, A::Relative8, "battle_script_not_taken_14");
    p.label("battle_script_result");
    abs(p, M::Lda, RESULT);
    emit(p, M::Cmp, A::AbsoluteLongX, O::Long(code + 4));
    branch(p, M::Beq, "battle_script_taken_14");
    p.label("battle_script_not_taken_14");
    next(p, 12);
    p.label("battle_script_taken_14");
    m16(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(code + 8));
    abs(p, M::Sta, SCRIPT_PC);
    m8(p);
    jump(p, "battle_script_step");
    p.label("battle_script_not_14");
    emit(p, M::Cmp, A::Immediate, O::Byte(0x2b));
    branch(p, M::Bne, "battle_script_not_2b");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(code + 2));
    p.emit_label_ref(M::Jsr, A::Absolute, "encounter_rng");
    abs(p, M::Sta, RESULT);
    next(p, 4);
    p.label("battle_script_not_2b");
    emit(p, M::Cmp, A::Immediate, O::Byte(0x0a));
    branch(p, M::Bne, "battle_script_not_0a");
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(code + 2));
    emit(p, M::Dec, A::Accumulator, O::None);
    abs(p, M::Cmp, RESULT);
    branch(p, M::Bcc, "battle_script_dispatch");
    abs(p, M::Lda, RESULT);
    p.label("battle_script_dispatch");
    m16(p);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, SCRIPT_PC);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(code + 4));
    abs(p, M::Sta, SCRIPT_PC);
    m8(p);
    jump(p, "battle_script_step");
    p.label("battle_script_not_0a");
    m16(p);
    abs(p, M::Ldy, CURRENT);
    m8(p);
    emit(p, M::Lda, A::AbsoluteY, O::Word(ACTORS + 0x44));
    p.emit_label_ref(M::Jsr, A::Absolute, "battle_actor_offset");
    emit(p, M::Ldx, A::Absolute, O::Word(SCRIPT_PC));
    m16(p);
    abs(p, M::Sta, DIVIDEND);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(code + 1));
    emit(p, M::And, A::Immediate, O::Word(15));
    abs(p, M::Sta, DIVISOR);
    implied(p, M::Clc);
    abs(p, M::Adc, DIVIDEND);
    implied(p, M::Tay);
    m8(p);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(code + 2));
    emit(p, M::Sta, A::AbsoluteY, O::Word(ACTORS + 0x30));
    m16(p);
    emit(p, M::Lda, A::Immediate, O::Word(1));
    abs(p, M::Ldy, DIVISOR);
    p.label("battle_script_bit");
    implied(p, M::Dey);
    branch(p, M::Bmi, "battle_script_bit_done");
    emit(p, M::Asl, A::Accumulator, O::None);
    branch(p, M::Bra, "battle_script_bit");
    p.label("battle_script_bit_done");
    abs(p, M::Ldy, DIVIDEND);
    emit(p, M::Ora, A::AbsoluteY, O::Word(ACTORS + 0x2e));
    emit(p, M::Sta, A::AbsoluteY, O::Word(ACTORS + 0x2e));
    m8(p);
    next(p, 4);
    p.label("battle_run_script")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Inc, A::Accumulator, O::None);
    emit(p, M::Cmp, A::Immediate, O::Byte(3));
    branch(p, M::Bcc, "battle_run_script_descriptor");
    abs(p, M::Lda, crate::sfc_encounter::ENEMY);
    p.label("battle_run_script_descriptor");
    m16(p);
    emit(p, M::And, A::Immediate, O::Word(0xff));
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    abs(p, M::Sta, DIVIDEND);
    implied(p, M::Tya);
    implied(p, M::Clc);
    abs(p, M::Adc, DIVIDEND);
    implied(p, M::Tax);
    emit(p, M::Lda, A::AbsoluteLongX, O::Long(roots));
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Cpx, A::Immediate, O::Word(0xffff));
    branch(p, M::Beq, "battle_run_script_none");
    p.emit_label_ref(M::Jsr, A::Absolute, "battle_script");
    p.label("battle_run_script_none");
    implied(p, M::Rts);
    p.label("battle_action_script")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    p.emit_label_ref(M::Jsr, A::Absolute, "battle_pair");
    emit(p, M::Lda, A::AbsoluteY, O::Word(ACTORS));
    branch(p, M::Bne, "battle_action_script_run");
    emit(p, M::Lda, A::Immediate, O::Byte(1));
    abs(p, M::Sta, crate::sfc_battle_vm::MENU);
    abs(p, M::Stz, crate::sfc_battle_vm::COMMAND);
    abs(p, M::Stz, crate::sfc_battle_vm::COMMAND + 1);
    p.label("battle_action_script_run");
    abs(p, M::Lda, CURRENT_INDEX);
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.emit_label_ref(M::Jmp, A::Absolute, "battle_run_script");
    p.label("battle_choose").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Stz, RESULT);
    p.emit_label_ref(M::Jsr, A::Absolute, "battle_pair");
    emit(p, M::Lda, A::AbsoluteY, O::Word(ACTORS));
    branch(p, M::Bne, "battle_choose_not_arle");
    p.emit_label_ref(M::Jmp, A::Absolute, "battle_choose_arle");
    p.label("battle_choose_not_arle");
    emit(p, M::Cmp, A::Immediate, O::Byte(2));
    branch(p, M::Bne, "battle_choose_not_enemy");
    emit(p, M::Lda, A::AbsoluteY, O::Word(ACTORS + 0x40));
    implied(p, M::Clc);
    emit(p, M::Adc, A::Immediate, O::Byte(10));
    emit(p, M::Sta, A::AbsoluteY, O::Word(ACTORS + 0x42));
    emit(p, M::Lda, A::Immediate, O::Byte(0));
    emit(p, M::Sta, A::AbsoluteY, O::Word(ACTORS + 0x44));
    implied(p, M::Clc);
    implied(p, M::Rts);
    p.label("battle_choose_not_enemy");
    emit(p, M::Cmp, A::Immediate, O::Byte(1));
    branch(p, M::Bne, "battle_choose_done");
    emit(p, M::Lda, A::Immediate, O::Byte(30));
    p.emit_label_ref(M::Jsr, A::Absolute, "encounter_rng");
    implied(p, M::Pha);
    m16(p);
    abs(p, M::Ldy, CURRENT);
    m8(p);
    implied(p, M::Pla);
    branch(p, M::Bne, "battle_choose_idle");
    emit(p, M::Lda, A::Immediate, O::Byte(7));
    emit(p, M::Sta, A::AbsoluteY, O::Word(ACTORS + 0x42));
    emit(p, M::Lda, A::Immediate, O::Byte(1));
    p.emit_label_ref(M::Bra, A::Relative8, "battle_choose_attack");
    p.label("battle_choose_idle");
    emit(p, M::Lda, A::Immediate, O::Byte(0));
    p.label("battle_choose_attack");
    emit(p, M::Sta, A::AbsoluteY, O::Word(ACTORS + 0x40));
    emit(p, M::Lda, A::Immediate, O::Byte(2));
    emit(p, M::Sta, A::AbsoluteY, O::Word(ACTORS + 0x44));
    p.label("battle_choose_done");
    implied(p, M::Clc);
    implied(p, M::Rts);
    p.label("battle_divide").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, DIVISOR + 1);
    branch(p, M::Bne, "battle_divide_bits");
    m16(p);
    abs(p, M::Lda, DIVIDEND);
    abs(p, M::Sta, 0x4204);
    m8(p);
    abs(p, M::Lda, DIVISOR);
    abs(p, M::Sta, 0x4206);
    for _ in 0..8 {
        implied(p, M::Nop);
    }
    m16(p);
    abs(p, M::Lda, 0x4214);
    abs(p, M::Sta, QUOTIENT);
    abs(p, M::Lda, 0x4216);
    abs(p, M::Sta, DIVIDEND);
    m8(p);
    implied(p, M::Rts);
    p.label("battle_divide_bits");
    implied(p, M::Phx);
    m16(p);
    abs(p, M::Stz, QUOTIENT);
    emit(p, M::Ldx, A::Immediate, O::Word(16));
    emit(p, M::Lda, A::Immediate, O::Word(0));
    p.label("battle_divide_bit");
    abs(p, M::Asl, DIVIDEND);
    emit(p, M::Rol, A::Accumulator, O::None);
    abs(p, M::Cmp, DIVISOR);
    branch(p, M::Bcc, "battle_divide_low");
    abs(p, M::Sbc, DIVISOR);
    p.label("battle_divide_low");
    abs(p, M::Rol, QUOTIENT);
    implied(p, M::Dex);
    branch(p, M::Bne, "battle_divide_bit");
    abs(p, M::Sta, DIVIDEND);
    m8(p);
    implied(p, M::Plx);
    implied(p, M::Rts);
    Ok(())
}
