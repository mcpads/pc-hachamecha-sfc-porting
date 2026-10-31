use crate::Result;
use crate::sfc::{emit, implied, write};
use serde::Deserialize;
use sfc_builder::cartridge::{CartridgeLayout, Section};
use w65c816::{AddressingMode as A, Assembler, CpuMode, Mnemonic as M, Operand as O};
pub const STOCK: u16 = 0xaa0;
pub const RECORDS: usize = 50;
pub const SHOP: u16 = 0xad2;
const STEP: u16 = 0xad3;
pub const CODE: u16 = 0xad4;
pub const LIST_KIND: u16 = 0xad6;
const LIST_NEXT: u16 = 0xad7;
const SCRATCH: u16 = 0xad8;
pub const AVAILABLE: u16 = 0xe10;
pub const END: u8 = 0xff;
const SHARED: u16 = 0x8c0;
const MODE: u16 = SHARED + 0x14;
const RESULT: u16 = SHARED + 0x15;
const PRICE: u16 = SHARED + 0x16;
const INDEX: u16 = SHARED + 0x18;
const GOLD: u16 = 0x974;
const INVENTORY: u16 = crate::sfc_b1_menu::INVENTORY;
const STATUS_MESSAGE: u32 = 0x4b1376;
const GROUPS: usize = 5;
const GREET: u8 = 1;
const PROMPT: u8 = 2;
const INPUT: u8 = 3;
const TRADE: u8 = 4;
const STATUS: u8 = 5;
const FAREWELL: u8 = 6;
const AFTER: u8 = 8;
#[derive(Deserialize)]
struct Items {
    shop_groups: Vec<Group>,
    shop_bindings: Vec<Binding>,
    price_table: Prices,
}
#[derive(Deserialize)]
struct Group {
    group: usize,
    entries: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    item_id: u8,
    stock_index: usize,
    initial_stock: u8,
}
#[derive(Deserialize)]
struct Binding {
    descriptor_va: u32,
    group: usize,
    message_vas: Vec<u32>,
}
#[derive(Deserialize)]
struct Prices {
    item_ids: Vec<u8>,
    prices: Vec<u16>,
}
pub struct Assets {
    records: Vec<(u8, u8)>,
    groups: Vec<(u8, u8)>,
    messages: Vec<[u32; 4]>,
    pub descriptors: Vec<(u32, u8)>,
    prices: Vec<u16>,
}
pub fn load(content: &crate::content::Content) -> Result<Assets> {
    let items: Items = content.json("items.json")?;
    let mut records = vec![(0u8, 0u8); RECORDS];
    let mut groups = vec![(0u8, 0u8); GROUPS];
    for g in &items.shop_groups {
        let first = g
            .entries
            .iter()
            .map(|e| e.stock_index)
            .min()
            .ok_or("Empty shop group")?;
        for (k, e) in g.entries.iter().enumerate() {
            if e.stock_index != first + k {
                return Err("Shop group records are not contiguous".into());
            }
            records[e.stock_index] = (e.item_id, e.initial_stock);
        }
        if 2 * g.entries.len() + 1 > crate::sfc_b1_menu::ITEM_KINDS {
            return Err("Shop group exceeds the buy list".into());
        }
        groups[g.group] = (first as u8, g.entries.len() as u8);
    }
    let mut messages = vec![[0u32; 4]; GROUPS];
    let mut descriptors = Vec::new();
    for b in &items.shop_bindings {
        messages[b.group] = b
            .message_vas
            .clone()
            .try_into()
            .map_err(|_| "Shop messages")?;
        descriptors.push((b.descriptor_va, b.group as u8));
    }
    let expected: Vec<u8> = (40..54).chain(88..98).collect();
    if items.price_table.item_ids != expected {
        return Err("Unexpected shop price slots".into());
    }
    Ok(Assets {
        records,
        groups,
        messages,
        descriptors,
        prices: items.price_table.prices,
    })
}
impl Assets {
    pub fn message_roots(&self) -> impl Iterator<Item = u32> + '_ {
        self.messages.iter().flatten().copied()
    }
    pub fn sections(
        &self,
        sections: &mut Vec<Section>,
        space: &mut crate::sfc_rom::Space,
    ) -> Result<()> {
        use crate::sfc_b1::add;
        add(
            sections,
            space,
            "shop_stock_start".into(),
            self.records.iter().map(|r| r.1).collect(),
        )?;
        add(
            sections,
            space,
            "shop_record_items".into(),
            self.records.iter().map(|r| r.0).collect(),
        )?;
        add(
            sections,
            space,
            "shop_groups".into(),
            self.groups.iter().flat_map(|g| [g.0, g.1]).collect(),
        )?;
        add(
            sections,
            space,
            "shop_messages".into(),
            self.messages
                .iter()
                .flatten()
                .flat_map(|v| v.to_le_bytes())
                .collect(),
        )?;
        let mut descriptors = Vec::new();
        for &(va, group) in &self.descriptors {
            descriptors.extend(va.to_le_bytes());
            descriptors.push(group);
        }
        descriptors.extend([0; 5]);
        add(sections, space, "shop_descriptors".into(), descriptors)?;
        add(
            sections,
            space,
            "shop_prices".into(),
            self.prices.iter().flat_map(|v| v.to_le_bytes()).collect(),
        )?;
        Ok(())
    }
}
fn abs(p: &mut Assembler, m: M, at: u16) {
    emit(p, m, A::Absolute, O::Word(at));
}
fn long_x(p: &mut Assembler, m: M, at: u32) {
    emit(p, m, A::AbsoluteLongX, O::Long(at));
}
fn branch(p: &mut Assembler, m: M, label: &str) {
    p.emit_label_ref(m, A::Relative8, label);
}
fn call(p: &mut Assembler, label: &str) {
    p.emit_label_ref(M::Jsr, A::Absolute, label);
}
fn jump(p: &mut Assembler, label: &str) {
    p.emit_label_ref(M::Jmp, A::Absolute, label);
}
fn m16(p: &mut Assembler) {
    emit(p, M::Rep, A::ImmediateByte, O::Byte(0x20));
}
fn m8(p: &mut Assembler) {
    emit(p, M::Sep, A::ImmediateByte, O::Byte(0x20));
}
fn byte(p: &mut Assembler, m: M, v: u8) {
    emit(p, m, A::Immediate, O::Byte(v));
}
fn word(p: &mut Assembler, m: M, v: u16) {
    emit(p, m, A::Immediate, O::Word(v));
}
pub fn emit_trade(p: &mut Assembler, layout: &CartridgeLayout) -> Result<()> {
    let section = |name: &str| layout.cpu_address(layout.section(name)?.offset);
    let (items, groups) = (section("shop_record_items")?, section("shop_groups")?);
    p.label("shop_trade").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Pha);
    abs(p, M::Lda, INDEX);
    byte(p, M::Cmp, 14);
    branch(p, M::Bcc, "shop_trade_index");
    implied(p, M::Clc);
    byte(p, M::Adc, 0x22);
    abs(p, M::Sta, INDEX);
    p.label("shop_trade_index");
    m16(p);
    abs(p, M::Lda, INDEX);
    word(p, M::And, 0xff);
    implied(p, M::Tay);
    m8(p);
    implied(p, M::Pla);
    branch(p, M::Beq, "shop_trade_buy");
    jump(p, "shop_trade_sell");
    p.label("shop_trade_buy");
    m16(p);
    abs(p, M::Lda, GOLD + 2);
    branch(p, M::Bne, "shop_trade_afford");
    abs(p, M::Lda, PRICE);
    abs(p, M::Cmp, GOLD);
    branch(p, M::Bcc, "shop_trade_afford");
    branch(p, M::Beq, "shop_trade_afford");
    m8(p);
    byte(p, M::Lda, 1);
    abs(p, M::Sta, RESULT);
    implied(p, M::Rts);
    p.label("shop_trade_afford")
        .assume_mode(CpuMode::NATIVE_M16_X16);
    abs(p, M::Lda, GOLD);
    implied(p, M::Sec);
    abs(p, M::Sbc, PRICE);
    abs(p, M::Sta, GOLD);
    abs(p, M::Lda, GOLD + 2);
    word(p, M::Sbc, 0);
    abs(p, M::Sta, GOLD + 2);
    m8(p);
    emit(p, M::Lda, A::AbsoluteY, O::Word(INVENTORY));
    byte(p, M::Cmp, 99);
    branch(p, M::Bcc, "shop_trade_add");
    byte(p, M::Lda, 2);
    abs(p, M::Sta, RESULT);
    implied(p, M::Rts);
    p.label("shop_trade_add");
    emit(p, M::Inc, A::Accumulator, O::None);
    emit(p, M::Sta, A::AbsoluteY, O::Word(INVENTORY));
    m16(p);
    abs(p, M::Lda, SHOP);
    word(p, M::And, 0xff);
    emit(p, M::Dec, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    m8(p);
    long_x(p, M::Lda, groups + 1);
    abs(p, M::Sta, SCRATCH);
    long_x(p, M::Lda, groups);
    m16(p);
    word(p, M::And, 0xff);
    implied(p, M::Tax);
    m8(p);
    abs(p, M::Lda, INDEX);
    implied(p, M::Clc);
    byte(p, M::Adc, crate::sfc_b1_menu::FIRST_ITEM);
    abs(p, M::Sta, SCRATCH + 1);
    p.label("shop_trade_record");
    abs(p, M::Lda, SCRATCH);
    branch(p, M::Beq, "shop_trade_done");
    long_x(p, M::Lda, items);
    abs(p, M::Cmp, SCRATCH + 1);
    branch(p, M::Bne, "shop_trade_next");
    emit(p, M::Lda, A::AbsoluteX, O::Word(STOCK));
    byte(p, M::Cmp, 0xff);
    branch(p, M::Beq, "shop_trade_next");
    emit(p, M::Dec, A::AbsoluteX, O::Word(STOCK));
    p.label("shop_trade_next");
    implied(p, M::Inx);
    abs(p, M::Dec, SCRATCH);
    branch(p, M::Bra, "shop_trade_record");
    p.label("shop_trade_sell");
    emit(p, M::Lda, A::AbsoluteY, O::Word(INVENTORY));
    branch(p, M::Beq, "shop_trade_paid");
    emit(p, M::Dec, A::Accumulator, O::None);
    emit(p, M::Sta, A::AbsoluteY, O::Word(INVENTORY));
    p.label("shop_trade_paid");
    m16(p);
    abs(p, M::Lda, GOLD);
    implied(p, M::Clc);
    abs(p, M::Adc, PRICE);
    abs(p, M::Sta, GOLD);
    abs(p, M::Lda, GOLD + 2);
    word(p, M::Adc, 0);
    abs(p, M::Sta, GOLD + 2);
    m8(p);
    p.label("shop_trade_done");
    implied(p, M::Rts);
    Ok(())
}
pub fn emit_field(p: &mut Assembler, layout: &CartridgeLayout, pressed: u16) -> Result<()> {
    let section = |name: &str| layout.cpu_address(layout.section(name)?.offset);
    let (start, items, groups, messages, prices) = (
        section("shop_stock_start")?,
        section("shop_record_items")?,
        section("shop_groups")?,
        section("shop_messages")?,
        section("shop_prices")?,
    );
    use crate::sfc_battle_vm::{KIND_SOURCE, MESSAGE, MESSAGE_KIND, MESSAGE_SOURCE};
    p.label("shop_reset").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Ldx, A::Immediate, O::Word(0));
    p.label("shop_reset_stock");
    long_x(p, M::Lda, start);
    emit(p, M::Sta, A::AbsoluteX, O::Word(STOCK));
    implied(p, M::Inx);
    emit(p, M::Cpx, A::Immediate, O::Word(RECORDS as u16));
    branch(p, M::Bne, "shop_reset_stock");
    abs(p, M::Stz, SHOP);
    implied(p, M::Rts);
    p.label("shop_open").assume_mode(CpuMode::NATIVE_M8_X16);
    emit(p, M::Inc, A::Accumulator, O::None);
    abs(p, M::Sta, SHOP);
    for at in [
        MESSAGE,
        crate::sfc_battle_message::STARTED,
        crate::sfc_battle_message::DRAW,
        crate::sfc_battle_message::SOUND_REQUEST,
    ] {
        abs(p, M::Stz, at);
    }
    write(p, STEP, GREET);
    abs(p, M::Stz, LIST_NEXT);
    implied(p, M::Rts);
    p.label("shop_say").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Pha);
    implied(p, M::Txa);
    implied(p, M::Clc);
    byte(p, M::Adc, AFTER);
    abs(p, M::Sta, STEP);
    m16(p);
    abs(p, M::Lda, SHOP);
    word(p, M::And, 0xff);
    emit(p, M::Dec, A::Accumulator, O::None);
    for _ in 0..4 {
        emit(p, M::Asl, A::Accumulator, O::None);
    }
    abs(p, M::Sta, SCRATCH);
    m8(p);
    implied(p, M::Pla);
    m16(p);
    word(p, M::And, 3);
    emit(p, M::Asl, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Clc);
    abs(p, M::Adc, SCRATCH);
    implied(p, M::Tax);
    long_x(p, M::Lda, messages);
    abs(p, M::Sta, MESSAGE_SOURCE);
    long_x(p, M::Lda, messages + 2);
    abs(p, M::Sta, MESSAGE_SOURCE + 2);
    m8(p);
    p.label("shop_say_source");
    abs(p, M::Stz, crate::sfc_battle_message::STARTED);
    write(p, MESSAGE_KIND, KIND_SOURCE);
    write(p, MESSAGE, 1);
    implied(p, M::Rts);
    p.label("field_message_frame")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Stz, crate::sfc_battle_message::INPUT);
    abs(p, M::Lda, pressed);
    byte(p, M::And, 0x80);
    branch(p, M::Beq, "field_message_no_a");
    abs(p, M::Inc, crate::sfc_battle_message::INPUT);
    p.label("field_message_no_a");
    abs(p, M::Lda, pressed + 1);
    byte(p, M::And, 0x80);
    branch(p, M::Beq, "field_message_no_b");
    byte(p, M::Lda, 2);
    abs(p, M::Ora, crate::sfc_battle_message::INPUT);
    abs(p, M::Sta, crate::sfc_battle_message::INPUT);
    p.label("field_message_no_b");
    call(p, "battle_message_tick");
    abs(p, M::Lda, crate::sfc_battle_message::SOUND_REQUEST);
    branch(p, M::Beq, "field_message_no_sound");
    abs(p, M::Stz, crate::sfc_battle_message::SOUND_REQUEST);
    call(p, "sound_effect");
    p.label("field_message_no_sound");
    abs(p, M::Lda, crate::sfc_battle_message::DRAW);
    branch(p, M::Beq, "field_message_drawn");
    abs(p, M::Stz, crate::sfc_battle_message::DRAW);
    crate::sfc::write_word(p, 0x501, crate::sfc_b1_text::RUNTIME_PAGE);
    call(p, "b1_text_request");
    p.label("field_message_drawn");
    implied(p, M::Rts);
    p.label("b1_shop_tick").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, crate::sfc_b1_menu::MODE);
    branch(p, M::Beq, "shop_no_bar");
    call(p, "b1_menu_tick");
    abs(p, M::Lda, SHOP);
    branch(p, M::Bne, "shop_no_bar");
    implied(p, M::Rts);
    p.label("shop_no_bar");
    call(p, "field_message_frame");
    abs(p, M::Lda, STEP);
    byte(p, M::Cmp, AFTER);
    branch(p, M::Bcc, "shop_step");
    abs(p, M::Lda, MESSAGE);
    branch(p, M::Beq, "shop_said");
    implied(p, M::Rts);
    p.label("shop_said");
    abs(p, M::Lda, STEP);
    implied(p, M::Sec);
    byte(p, M::Sbc, AFTER);
    abs(p, M::Sta, STEP);
    implied(p, M::Rts);
    p.label("shop_step");
    for (step, label) in [
        (GREET, "shop_greet"),
        (PROMPT, "shop_prompt"),
        (INPUT, "shop_input"),
        (TRADE, "shop_prompt"),
        (STATUS, "shop_prompt"),
        (FAREWELL, "shop_leave"),
    ] {
        let next = format!("shop_step_not_{step}");
        byte(p, M::Cmp, step);
        branch(p, M::Bne, &next);
        jump(p, label);
        p.label(next);
    }
    implied(p, M::Rts);
    p.label("shop_greet").assume_mode(CpuMode::NATIVE_M8_X16);
    byte(p, M::Lda, 0);
    emit(p, M::Ldx, A::Immediate, O::Word(PROMPT.into()));
    jump(p, "shop_say");
    p.label("shop_prompt").assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Stz, CODE);
    m8(p);
    byte(p, M::Lda, 1);
    emit(p, M::Ldx, A::Immediate, O::Word(INPUT.into()));
    call(p, "shop_say");
    implied(p, M::Rts);
    p.label("shop_input").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Lda, crate::sfc_b1_menu::MODE);
    branch(p, M::Beq, "shop_input_closed");
    implied(p, M::Rts);
    p.label("shop_input_closed");
    m16(p);
    abs(p, M::Lda, CODE);
    m8(p);
    branch(p, M::Bne, "shop_chosen");
    abs(p, M::Lda, LIST_NEXT);
    branch(p, M::Beq, "shop_open_bar");
    abs(p, M::Stz, LIST_NEXT);
    jump(p, "b1_menu_open_shop_list");
    p.label("shop_open_bar");
    jump(p, "b1_menu_open_shop");
    p.label("shop_chosen");
    byte(p, M::Cmp, 40);
    branch(p, M::Bcc, "shop_command");
    jump(p, "shop_item");
    p.label("shop_command");
    for (code, label) in [
        (6u8, "shop_buy"),
        (7, "shop_sell"),
        (5, "shop_status"),
        (8, "shop_farewell"),
    ] {
        let next = format!("shop_code_not_{code}");
        byte(p, M::Cmp, code);
        branch(p, M::Bne, &next);
        jump(p, label);
        p.label(next);
    }
    m16(p);
    abs(p, M::Stz, CODE);
    m8(p);
    implied(p, M::Rts);
    for (label, mode) in [("shop_buy", 0u8), ("shop_sell", 1)] {
        p.label(label).assume_mode(CpuMode::NATIVE_M8_X16);
        write(p, MODE, mode);
        write(p, LIST_KIND, 1 - mode);
        write(p, LIST_NEXT, 1);
        byte(p, M::Lda, 2);
        emit(p, M::Ldx, A::Immediate, O::Word(TRADE.into()));
        jump(p, "shop_say");
    }
    p.label("shop_status").assume_mode(CpuMode::NATIVE_M8_X16);
    p.emit_label_ref(M::Jsr, A::Absolute, "status_prepare");
    m16(p);
    word(p, M::Lda, STATUS_MESSAGE as u16);
    abs(p, M::Sta, MESSAGE_SOURCE);
    word(p, M::Lda, (STATUS_MESSAGE >> 16) as u16);
    abs(p, M::Sta, MESSAGE_SOURCE + 2);
    m8(p);
    write(p, STEP, AFTER + STATUS);
    jump(p, "shop_say_source");
    p.label("shop_farewell").assume_mode(CpuMode::NATIVE_M8_X16);
    byte(p, M::Lda, 3);
    emit(p, M::Ldx, A::Immediate, O::Word(FAREWELL.into()));
    jump(p, "shop_say");
    p.label("shop_item").assume_mode(CpuMode::NATIVE_M8_X16);
    implied(p, M::Sec);
    byte(p, M::Sbc, crate::sfc_b1_menu::FIRST_ITEM);
    abs(p, M::Sta, INDEX);
    abs(p, M::Inc, MODE);
    abs(p, M::Inc, MODE);
    abs(p, M::Stz, RESULT);
    abs(p, M::Lda, INDEX);
    byte(p, M::Cmp, 14);
    branch(p, M::Bcc, "shop_item_price");
    abs(p, M::Lda, MODE);
    byte(p, M::Cmp, 3);
    branch(p, M::Bne, "shop_item_slot");
    write(p, RESULT, 1);
    branch(p, M::Bra, "shop_item_price");
    p.label("shop_item_slot");
    abs(p, M::Lda, INDEX);
    implied(p, M::Sec);
    byte(p, M::Sbc, 0x22);
    abs(p, M::Sta, INDEX);
    p.label("shop_item_price");
    m16(p);
    abs(p, M::Lda, INDEX);
    word(p, M::And, 0xff);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    long_x(p, M::Lda, prices);
    abs(p, M::Sta, PRICE);
    m8(p);
    abs(p, M::Lda, MODE);
    byte(p, M::Cmp, 3);
    branch(p, M::Bne, "shop_item_said");
    m16(p);
    abs(p, M::Lsr, PRICE);
    m8(p);
    p.label("shop_item_said");
    byte(p, M::Lda, 2);
    emit(p, M::Ldx, A::Immediate, O::Word(TRADE.into()));
    jump(p, "shop_say");
    p.label("shop_leave").assume_mode(CpuMode::NATIVE_M8_X16);
    abs(p, M::Stz, SHOP);
    abs(p, M::Stz, STEP);
    abs(p, M::Lda, 0x500);
    byte(p, M::Eor, 2);
    abs(p, M::Sta, 0x500);
    byte(p, M::And, 3);
    for (direction, delta) in [(0u8, 0xe0u8), (1, 0x04), (2, 0x20), (3, 0xfc)] {
        let next = format!("shop_leave_not_{direction}");
        byte(p, M::Cmp, direction);
        branch(p, M::Bne, &next);
        abs(p, M::Lda, 0x500);
        implied(p, M::Clc);
        byte(p, M::Adc, delta);
        abs(p, M::Sta, 0x500);
        p.label(next);
    }
    jump(p, "field_shop_left");
    p.label("shop_available")
        .assume_mode(CpuMode::NATIVE_M8_X16);
    m16(p);
    abs(p, M::Lda, SHOP);
    word(p, M::And, 0xff);
    emit(p, M::Dec, A::Accumulator, O::None);
    emit(p, M::Asl, A::Accumulator, O::None);
    implied(p, M::Tax);
    m8(p);
    long_x(p, M::Lda, groups + 1);
    abs(p, M::Sta, SCRATCH);
    long_x(p, M::Lda, groups);
    m16(p);
    word(p, M::And, 0xff);
    implied(p, M::Tax);
    m8(p);
    emit(p, M::Ldy, A::Immediate, O::Word(0));
    p.label("shop_available_record");
    abs(p, M::Lda, SCRATCH);
    branch(p, M::Beq, "shop_available_done");
    emit(p, M::Lda, A::AbsoluteX, O::Word(STOCK));
    branch(p, M::Beq, "shop_available_next");
    emit(p, M::Sta, A::AbsoluteY, O::Word(AVAILABLE + 1));
    long_x(p, M::Lda, items);
    implied(p, M::Sec);
    byte(p, M::Sbc, crate::sfc_b1_menu::FIRST_ITEM);
    emit(p, M::Sta, A::AbsoluteY, O::Word(AVAILABLE));
    implied(p, M::Iny);
    implied(p, M::Iny);
    p.label("shop_available_next");
    implied(p, M::Inx);
    abs(p, M::Dec, SCRATCH);
    branch(p, M::Bra, "shop_available_record");
    p.label("shop_available_done");
    byte(p, M::Lda, END);
    emit(p, M::Sta, A::AbsoluteY, O::Word(AVAILABLE));
    implied(p, M::Rts);
    Ok(())
}
