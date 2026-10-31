use crate::{
    evaluator::{evaluate, ExpressionError, ExpressionResult},
    output::{BitArgument, Output, OutputError, U16Value, U8Value},
    string::comma_iter,
    symbols::{DirectPageFlag, Symbols},
};
#[derive(Debug, PartialEq)]
pub enum AddressingModeError<'s> {
    UnknownAddressingMode(&'s str),
    ExpressionError(&'s str, ExpressionError),
    NotANumber(&'s str),
    DpAddressOutOfBounds(&'s str, i64),
    AbsoluteAddressOutOfBounds(&'s str, i64),
    DpOutOfBounds(&'s str, DirectPageFlag, i64),
    UnknownDpValue(&'s str),
}
impl std::fmt::Display for AddressingModeError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddressingModeError::UnknownAddressingMode(arg) => {
                write!(f, "unknown addressing mode: {arg}")
            }
            AddressingModeError::ExpressionError(expr, e) => {
                write!(f, "expression error: {expr}: {e}")
            }
            AddressingModeError::NotANumber(expr) => write!(f, "not a number: {expr}"),
            AddressingModeError::DpAddressOutOfBounds(expr, value) => {
                write!(f, "dp address out of bounds : {expr} = ${value:4x}")
            }
            AddressingModeError::AbsoluteAddressOutOfBounds(expr, value) => {
                write!(f, "abs address out of bounds: {expr} = ${value:4x}")
            }
            AddressingModeError::DpOutOfBounds(expr, dp, value) => {
                let p = match dp {
                    DirectPageFlag::Zero => "(P = 0)",
                    DirectPageFlag::One => "(P = 1)",
                };
                write!(f, "dp address out of bounds: {expr} = ${value:4x} {p}")
            }
            AddressingModeError::UnknownDpValue(expr) => {
                write!(f, "unknown dp value: {expr}")
            }
        }
    }
}
#[derive(Debug, PartialEq)]
pub enum InstructionError<'s> {
    AddressingModeError(AddressingModeError<'s>),
    ExpressionError(&'s str, ExpressionError),
    OutputError(OutputError<'s>),
    UnknownInstruction(&'s str),
    UnknownInstructionArguments(&'s str, String),
    InvalidNumberOfArguments { expected: u8, got: usize },
    InvalidBitInstructionBit,
    InvalidTcallArgument,
}
impl std::fmt::Display for InstructionError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstructionError::AddressingModeError(e) => write!(f, "{e}"),
            InstructionError::ExpressionError(expr, e) => {
                write!(f, "expression error: {expr}: {e}")
            }
            InstructionError::OutputError(e) => write!(f, "{e}"),
            InstructionError::UnknownInstruction(inst) => {
                write!(f, "unknown instruction: {inst}")
            }
            InstructionError::UnknownInstructionArguments(inst, args) => {
                write!(f, "unknown instruction: {inst} {args}")
            }
            InstructionError::InvalidNumberOfArguments { expected, got } => {
                write!(
                    f,
                    "invalid number of arguments (expected {expected}, got {got})"
                )
            }
            InstructionError::InvalidBitInstructionBit => {
                write!(f, "invalid bit instruction bit")
            }
            InstructionError::InvalidTcallArgument => write!(f, "invalid tcall argument"),
        }
    }
}
impl<'s> From<AddressingModeError<'s>> for InstructionError<'s> {
    fn from(v: AddressingModeError<'s>) -> Self {
        Self::AddressingModeError(v)
    }
}
impl<'s> From<OutputError<'s>> for InstructionError<'s> {
    fn from(v: OutputError<'s>) -> Self {
        Self::OutputError(v)
    }
}
fn unknown_instruction_args_err<'s>(
    instruction: &'s str,
    modes: impl AddressingModeString,
) -> Result<(), InstructionError<'s>> {
    Err(InstructionError::UnknownInstructionArguments(
        instruction,
        AddressingModeString::stringify(modes),
    ))
}
trait AddressingModeString {
    fn stringify(modes: Self) -> String;
}
impl AddressingModeString for AddressingMode<'_> {
    fn stringify(modes: AddressingMode) -> String {
        modes.shorthand().to_owned()
    }
}
impl AddressingModeString for (AddressingMode<'_>, &str) {
    fn stringify(modes: (AddressingMode, &str)) -> String {
        format!("{}, rel", modes.0.shorthand())
    }
}
impl AddressingModeString for (AddressingMode<'_>, BitArgument, &str) {
    fn stringify(modes: (AddressingMode, BitArgument, &str)) -> String {
        format!("{}, bit, rel", modes.0.shorthand())
    }
}
impl AddressingModeString for (AddressingMode<'_>, BitArgument) {
    fn stringify(modes: (AddressingMode, BitArgument)) -> String {
        format!("{}, bit", modes.0.shorthand())
    }
}
impl AddressingModeString for [AddressingMode<'_>; 2] {
    fn stringify(modes: [AddressingMode; 2]) -> String {
        format!("{}, {}", modes[0].shorthand(), modes[1].shorthand())
    }
}
impl AddressingModeString for [AddressingMode<'_>; 3] {
    fn stringify(modes: [AddressingMode; 3]) -> String {
        format!(
            "{}, {}, {}",
            modes[0].shorthand(),
            modes[1].shorthand(),
            modes[2].shorthand()
        )
    }
}
fn invalid_n_argments_err(expected: u8, s: &str) -> InstructionError<'static> {
    InstructionError::InvalidNumberOfArguments {
        expected,
        got: comma_iter(s).count(),
    }
}
#[derive(Debug, PartialEq)]
pub enum AddressingMode<'s> {
    A,
    X,
    Y,
    Ya,
    Sp,
    Psw,
    C,
    XIndirect,
    XIndirectIncrement,
    YIndirect,
    Dp(u8),
    DpX(u8),
    DpY(u8),
    DpIndirectX(u8),
    DpIndirectY(u8),
    Abs(U16Value<'s>),
    AbsX(U16Value<'s>),
    AbsY(U16Value<'s>),
    NotAbs(U16Value<'s>),
    AbsIndirectX(U16Value<'s>),
    AbsIndirectY(U16Value<'s>),
    Immediate(U8Value<'s>),
}
impl AddressingMode<'_> {
    pub fn shorthand(&self) -> &'static str {
        match self {
            Self::A => "A",
            Self::X => "X",
            Self::Y => "Y",
            Self::Ya => "YA",
            Self::Sp => "SP",
            Self::Psw => "PSW",
            Self::C => "C",
            Self::XIndirect => "(X)",
            Self::XIndirectIncrement => "(X)+",
            Self::YIndirect => "(Y)",
            Self::Dp(_) => "dp",
            Self::DpX(_) => "dp+X",
            Self::DpY(_) => "dp+Y",
            Self::DpIndirectX(_) => "[dp+X]",
            Self::DpIndirectY(_) => "[dp]+Y",
            Self::Abs(_) => "abs",
            Self::AbsX(_) => "abs+X",
            Self::AbsY(_) => "abs+Y",
            Self::NotAbs(_) => "/abs",
            Self::AbsIndirectX(_) => "[abs+X]",
            Self::AbsIndirectY(_) => "[abs]+Y",
            Self::Immediate(_) => "#imm",
        }
    }
    fn try_into_bit_argument(&self) -> Result<BitArgument, InstructionError<'static>> {
        match self {
            &Self::Abs(U16Value::Known(v)) => v
                .try_into()
                .map_err(|_| InstructionError::InvalidBitInstructionBit),
            _ => Err(InstructionError::InvalidBitInstructionBit),
        }
    }
}
fn parse_immediate_value<'s>(
    s: &'s str,
    symbols: &Symbols<'s>,
) -> Result<U8Value<'s>, AddressingModeError<'s>> {
    match evaluate(s, symbols) {
        ExpressionResult::Value(value) => match value.try_into() {
            Ok(v) => Ok(U8Value::Known(v)),
            Err(_) => Err(AddressingModeError::DpAddressOutOfBounds(s, value)),
        },
        ExpressionResult::Unknown => Ok(U8Value::Unknown(s, symbols.scope())),
        ExpressionResult::Boolean(_) => Err(AddressingModeError::NotANumber(s)),
        ExpressionResult::Error(e) => Err(AddressingModeError::ExpressionError(s, e)),
    }
}
fn parse_abs_address<'s>(
    s: &'s str,
    symbols: &Symbols<'s>,
) -> Result<U16Value<'s>, AddressingModeError<'s>> {
    match evaluate(s, symbols) {
        ExpressionResult::Value(value) => match value.try_into() {
            Ok(v) => Ok(U16Value::Known(v)),
            Err(_) => Err(AddressingModeError::AbsoluteAddressOutOfBounds(s, value)),
        },
        ExpressionResult::Unknown => Ok(U16Value::Unknown(s, symbols.scope())),
        ExpressionResult::Boolean(_) => Err(AddressingModeError::NotANumber(s)),
        ExpressionResult::Error(e) => Err(AddressingModeError::ExpressionError(s, e)),
    }
}
#[derive(Debug, PartialEq)]
enum DpOrAbs<'s> {
    Dp(u8),
    Abs(U16Value<'s>),
}
fn parse_dp_or_abs_address<'s>(
    s: &'s str,
    symbols: &Symbols<'s>,
) -> Result<DpOrAbs<'s>, AddressingModeError<'s>> {
    match evaluate(s, symbols) {
        ExpressionResult::Value(value) => match u16::try_from(value) {
            Ok(addr) => match (symbols.direct_page(), addr) {
                (DirectPageFlag::Zero, addr @ 0..0x100) => {
                    Ok(DpOrAbs::Dp(u8::try_from(addr).unwrap()))
                }
                (DirectPageFlag::One, addr @ 0x100..0x200) => {
                    Ok(DpOrAbs::Dp(u8::try_from(addr - 0x100).unwrap()))
                }
                (DirectPageFlag::One | DirectPageFlag::Zero, addr) => {
                    Ok(DpOrAbs::Abs(U16Value::Known(addr)))
                }
            },
            Err(_) => Err(AddressingModeError::AbsoluteAddressOutOfBounds(s, value)),
        },
        ExpressionResult::Unknown => Ok(DpOrAbs::Abs(U16Value::Unknown(s, symbols.scope()))),
        ExpressionResult::Boolean(_) => Err(AddressingModeError::NotANumber(s)),
        ExpressionResult::Error(e) => Err(AddressingModeError::ExpressionError(s, e)),
    }
}
fn parse_dp_address<'s>(s: &'s str, symbols: &Symbols) -> Result<u8, AddressingModeError<'s>> {
    match evaluate(s, symbols) {
        ExpressionResult::Value(value) => match (symbols.direct_page(), value) {
            (DirectPageFlag::Zero, addr @ 0..0x100) => Ok(u8::try_from(addr).unwrap()),
            (DirectPageFlag::One, addr @ 0x100..0x200) => Ok(u8::try_from(addr - 0x100).unwrap()),
            (dp @ DirectPageFlag::One, value) | (dp @ DirectPageFlag::Zero, value) => {
                Err(AddressingModeError::DpOutOfBounds(s, dp, value))
            }
        },
        ExpressionResult::Unknown => Err(AddressingModeError::UnknownDpValue(s)),
        ExpressionResult::Boolean(_) => Err(AddressingModeError::NotANumber(s)),
        ExpressionResult::Error(e) => Err(AddressingModeError::ExpressionError(s, e)),
    }
}
fn strip_final_three_chars(s: &str) -> ([u8; 3], &str) {
    let mut it = s
        .bytes()
        .enumerate()
        .rev()
        .filter(|(_, c)| !c.is_ascii_whitespace());
    let (a, b, c) = (it.next(), it.next(), it.next());
    match (a, b, c) {
        (Some((_, a)), Some((_, b)), Some((i, c))) => ([c, b, a], s[..i].trim()),
        _ => ([0, 0, 0], ""),
    }
}
fn strip_plus_index_suffix<'s>(s: &'s str, register: &'static str) -> Option<&'s str> {
    if s.ends_with(register) {
        let mut it = s
            .bytes()
            .enumerate()
            .rev()
            .filter(|(_, c)| !c.is_ascii_whitespace());
        let (_, b) = (it.next(), it.next());
        match b {
            Some((i, b'+')) => Some(s[..i].trim_end()),
            _ => None,
        }
    } else {
        None
    }
}
fn parse_addressing_mode<'s>(
    input: &'s str,
    symbols: &Symbols<'s>,
) -> Result<AddressingMode<'s>, AddressingModeError<'s>> {
    match input {
        "A" => Ok(AddressingMode::A),
        "X" => Ok(AddressingMode::X),
        "Y" => Ok(AddressingMode::Y),
        "YA" => Ok(AddressingMode::Ya),
        "SP" => Ok(AddressingMode::Sp),
        "PSW" => Ok(AddressingMode::Psw),
        "C" => Ok(AddressingMode::C),
        "(X)" => Ok(AddressingMode::XIndirect),
        "(X)+" => Ok(AddressingMode::XIndirectIncrement),
        "(Y)" => Ok(AddressingMode::YIndirect),
        s => {
            if let Some(s) = s.strip_prefix("#") {
                Ok(AddressingMode::Immediate(parse_immediate_value(
                    s, symbols,
                )?))
            } else if let Some(s) = s.strip_prefix("[") {
                let (suffix, expr) = strip_final_three_chars(s);
                match suffix {
                    [b'+', b'X', b']'] => Ok(AddressingMode::DpIndirectX(parse_dp_address(
                        expr, symbols,
                    )?)),
                    [b']', b'+', b'Y'] => Ok(AddressingMode::DpIndirectY(parse_dp_address(
                        expr, symbols,
                    )?)),
                    _ => Err(AddressingModeError::UnknownAddressingMode(input)),
                }
            } else if let Some(s) = s.strip_prefix("/") {
                Ok(AddressingMode::NotAbs(parse_abs_address(s, symbols)?))
            } else if let Some(s) = strip_plus_index_suffix(s, "X") {
                match parse_dp_or_abs_address(s, symbols)? {
                    DpOrAbs::Dp(a) => Ok(AddressingMode::DpX(a)),
                    DpOrAbs::Abs(a) => Ok(AddressingMode::AbsX(a)),
                }
            } else if let Some(s) = strip_plus_index_suffix(s, "Y") {
                match parse_dp_or_abs_address(s, symbols)? {
                    DpOrAbs::Dp(a) => Ok(AddressingMode::DpY(a)),
                    DpOrAbs::Abs(a) => Ok(AddressingMode::AbsY(a)),
                }
            } else {
                match parse_dp_or_abs_address(s, symbols)? {
                    DpOrAbs::Dp(a) => Ok(AddressingMode::Dp(a)),
                    DpOrAbs::Abs(a) => Ok(AddressingMode::Abs(a)),
                }
            }
        }
    }
}
fn parse_no_dp_addressing_mode<'s>(
    input: &'s str,
    symbols: &Symbols<'s>,
) -> Result<AddressingMode<'s>, AddressingModeError<'s>> {
    match input {
        "A" => Ok(AddressingMode::A),
        "X" => Ok(AddressingMode::X),
        "Y" => Ok(AddressingMode::Y),
        "YA" => Ok(AddressingMode::Ya),
        "SP" => Ok(AddressingMode::Sp),
        "PSW" => Ok(AddressingMode::Psw),
        "C" => Ok(AddressingMode::C),
        "(X)" => Ok(AddressingMode::XIndirect),
        "(X)+" => Ok(AddressingMode::XIndirectIncrement),
        "(Y)" => Ok(AddressingMode::YIndirect),
        s => {
            if let Some(s) = s.strip_prefix("#") {
                Ok(AddressingMode::Immediate(parse_immediate_value(
                    s, symbols,
                )?))
            } else if let Some(s) = s.strip_prefix("[") {
                let (suffix, expr) = strip_final_three_chars(s);
                match suffix {
                    [b'+', b'X', b']'] => Ok(AddressingMode::AbsIndirectX(parse_abs_address(
                        expr, symbols,
                    )?)),
                    [b']', b'+', b'Y'] => Ok(AddressingMode::AbsIndirectY(parse_abs_address(
                        expr, symbols,
                    )?)),
                    _ => Err(AddressingModeError::UnknownAddressingMode(input)),
                }
            } else if let Some(s) = s.strip_prefix("/") {
                Ok(AddressingMode::NotAbs(parse_abs_address(s, symbols)?))
            } else if let Some(s) = strip_plus_index_suffix(s, "X") {
                Ok(AddressingMode::AbsX(parse_abs_address(s, symbols)?))
            } else if let Some(s) = strip_plus_index_suffix(s, "Y") {
                Ok(AddressingMode::AbsY(parse_abs_address(s, symbols)?))
            } else {
                Ok(AddressingMode::Abs(parse_abs_address(s, symbols)?))
            }
        }
    }
}
fn parse_bit_argument<'s>(
    s: &'s str,
    symbols: &Symbols,
) -> Result<BitArgument, InstructionError<'s>> {
    match evaluate(s, symbols) {
        ExpressionResult::Value(bit) => {
            BitArgument::try_from(bit).map_err(|_| InstructionError::InvalidBitInstructionBit)
        }
        ExpressionResult::Error(e) => Err(InstructionError::ExpressionError(s, e)),
        _ => Err(InstructionError::InvalidBitInstructionBit),
    }
}
fn split_one_argument<'s>(s: &'s str) -> Result<&'s str, InstructionError<'s>> {
    let mut it = comma_iter(s);
    match (it.next(), it.next()) {
        (Some(a), None) => Ok(a),
        _ => Err(invalid_n_argments_err(1, s)),
    }
}
fn split_two_arguments<'s>(s: &'s str) -> Result<[&'s str; 2], InstructionError<'s>> {
    let mut it = comma_iter(s);
    match (it.next(), it.next(), it.next()) {
        (Some(a), Some(b), None) => Ok([a, b]),
        _ => Err(invalid_n_argments_err(2, s)),
    }
}
fn split_three_arguments<'s>(s: &'s str) -> Result<[&'s str; 3], InstructionError<'s>> {
    let mut it = comma_iter(s);
    match (it.next(), it.next(), it.next(), it.next()) {
        (Some(a), Some(b), Some(c), None) => Ok([a, b, c]),
        _ => Err(invalid_n_argments_err(3, s)),
    }
}
fn parse_one_argument<'s>(
    s: &'s str,
    symbols: &Symbols<'s>,
) -> Result<AddressingMode<'s>, InstructionError<'s>> {
    let arg = split_one_argument(s)?;
    Ok(parse_addressing_mode(arg, symbols)?)
}
fn parse_one_no_dp_argument<'s>(
    s: &'s str,
    symbols: &Symbols<'s>,
) -> Result<AddressingMode<'s>, InstructionError<'s>> {
    let arg = split_one_argument(s)?;
    Ok(parse_no_dp_addressing_mode(arg, symbols)?)
}
fn parse_two_arguments<'s>(
    s: &'s str,
    symbols: &Symbols<'s>,
) -> Result<[AddressingMode<'s>; 2], InstructionError<'s>> {
    let [a1, a2] = split_two_arguments(s)?;
    Ok([
        parse_addressing_mode(a1, symbols)?,
        parse_addressing_mode(a2, symbols)?,
    ])
}
fn parse_two_no_dp_arguments<'s>(
    s: &'s str,
    symbols: &Symbols<'s>,
) -> Result<[AddressingMode<'s>; 2], InstructionError<'s>> {
    let [a1, a2] = split_two_arguments(s)?;
    Ok([
        parse_no_dp_addressing_mode(a1, symbols)?,
        parse_no_dp_addressing_mode(a2, symbols)?,
    ])
}
fn parse_three_no_dp_arguments<'s>(
    s: &'s str,
    symbols: &Symbols<'s>,
) -> Result<[AddressingMode<'s>; 3], InstructionError<'s>> {
    let [a1, a2, a3] = split_three_arguments(s)?;
    Ok([
        parse_no_dp_addressing_mode(a1, symbols)?,
        parse_no_dp_addressing_mode(a2, symbols)?,
        parse_no_dp_addressing_mode(a3, symbols)?,
    ])
}
fn parse_argument_and_rel<'s>(
    s: &'s str,
    symbols: &Symbols<'s>,
) -> Result<(AddressingMode<'s>, &'s str), InstructionError<'s>> {
    let [a1, a2] = split_two_arguments(s)?;
    Ok((parse_addressing_mode(a1, symbols)?, a2))
}
fn arithmatic_instruction_impl<'s>(
    instruction: &'s str,
    arguments: [AddressingMode<'s>; 2],
    opcode_base: u8,
    symbols: &Symbols<'s>,
    output: &mut Output<'s>,
) -> Result<(), InstructionError<'s>> {
    match arguments {
        [AddressingMode::A, AddressingMode::Immediate(i)] => {
            output.write_op_u8v(opcode_base | 0x08, i)
        }
        [AddressingMode::A, AddressingMode::XIndirect] => output.write_u8(opcode_base | 0x06),
        [AddressingMode::A, AddressingMode::Dp(a)] => output.write_op_u8(opcode_base | 0x04, a),
        [AddressingMode::A, AddressingMode::DpX(a)] => output.write_op_u8(opcode_base | 0x14, a),
        [AddressingMode::A, AddressingMode::Abs(a)] => output.write_op_u16v(opcode_base | 0x05, a),
        [AddressingMode::A, AddressingMode::AbsX(a)] => output.write_op_u16v(opcode_base | 0x15, a),
        [AddressingMode::A, AddressingMode::AbsY(a)] => output.write_op_u16v(opcode_base | 0x16, a),
        [AddressingMode::A, AddressingMode::DpIndirectX(a)] => {
            output.write_op_u8(opcode_base | 0x07, a)
        }
        [AddressingMode::A, AddressingMode::DpIndirectY(a)] => {
            output.write_op_u8(opcode_base | 0x17, a)
        }
        [AddressingMode::A, AddressingMode::DpY(a)] => {
            output.write_u8(opcode_base | 0x16);
            output.write_u16(symbols.dp_addr_to_addr(a));
        }
        [AddressingMode::Dp(d), AddressingMode::Dp(s)] => {
            output.write_u8(opcode_base | 0x09);
            output.write_u8(s);
            output.write_u8(d);
        }
        [AddressingMode::Dp(d), AddressingMode::Immediate(i)] => {
            output.write_u8(opcode_base | 0x18);
            output.write_u8v(i);
            output.write_u8(d);
        }
        [AddressingMode::XIndirect, AddressingMode::YIndirect] => {
            output.write_u8(opcode_base | 0x19)
        }
        am => return unknown_instruction_args_err(instruction, am),
    }
    Ok(())
}
fn no_argument_instruction<'s>(
    arguments: &'s str,
    opcode: u8,
    output: &mut Output<'s>,
) -> Result<(), InstructionError<'s>> {
    if arguments.is_empty() {
        output.write_u8(opcode);
        Ok(())
    } else {
        Err(invalid_n_argments_err(0, arguments))
    }
}
fn only_a_instruction<'s>(
    instruction: &'s str,
    arguments: &'s str,
    opcode: u8,
    symbols: &Symbols<'s>,
    output: &mut Output<'s>,
) -> Result<(), InstructionError<'s>> {
    match parse_one_argument(arguments, symbols)? {
        AddressingMode::A => {
            output.write_u8(opcode);
            Ok(())
        }
        am => unknown_instruction_args_err(instruction, am),
    }
}
fn only_ya_instruction<'s>(
    instruction: &'s str,
    arguments: &'s str,
    opcode: u8,
    symbols: &Symbols<'s>,
    output: &mut Output<'s>,
) -> Result<(), InstructionError<'s>> {
    match parse_one_argument(arguments, symbols)? {
        AddressingMode::Ya => {
            output.write_u8(opcode);
            Ok(())
        }
        am => unknown_instruction_args_err(instruction, am),
    }
}
fn only_dp_instruction<'s>(
    instruction: &'s str,
    arguments: &'s str,
    opcode: u8,
    symbols: &Symbols<'s>,
    output: &mut Output<'s>,
) -> Result<(), InstructionError<'s>> {
    match parse_one_argument(arguments, symbols)? {
        AddressingMode::Dp(dp) => {
            output.write_op_u8(opcode, dp);
            Ok(())
        }
        am => unknown_instruction_args_err(instruction, am),
    }
}
fn only_abs_instruction<'s>(
    instruction: &'s str,
    arguments: &'s str,
    opcode: u8,
    symbols: &Symbols<'s>,
    output: &mut Output<'s>,
) -> Result<(), InstructionError<'s>> {
    match parse_one_no_dp_argument(arguments, symbols)? {
        AddressingMode::Abs(abs) => {
            output.write_op_u16v(opcode, abs);
            Ok(())
        }
        am => unknown_instruction_args_err(instruction, am),
    }
}
fn ya_dp_instruction<'s>(
    instruction: &'s str,
    arguments: &'s str,
    opcode: u8,
    symbols: &Symbols<'s>,
    output: &mut Output<'s>,
) -> Result<(), InstructionError<'s>> {
    match parse_two_arguments(arguments, symbols)? {
        [AddressingMode::Ya, AddressingMode::Dp(a)] => {
            output.write_op_u8(opcode, a);
            Ok(())
        }
        am => unknown_instruction_args_err(instruction, am),
    }
}
fn arithmatic_instruction<'s>(
    instruction: &'s str,
    arguments: &'s str,
    opcode_base: u8,
    symbols: &Symbols<'s>,
    output: &mut Output<'s>,
) -> Result<(), InstructionError<'s>> {
    arithmatic_instruction_impl(
        instruction,
        parse_two_arguments(arguments, symbols)?,
        opcode_base,
        symbols,
        output,
    )
}
fn inc_dec_instruction<'s>(
    instruction: &'s str,
    arguments: &'s str,
    opcode_base: u8,
    symbols: &Symbols<'s>,
    output: &mut Output<'s>,
) -> Result<(), InstructionError<'s>> {
    match parse_one_argument(arguments, symbols)? {
        AddressingMode::A => output.write_u8(opcode_base + 0x9c),
        AddressingMode::Dp(a) => output.write_op_u8(opcode_base + 0x8b, a),
        AddressingMode::DpX(a) => output.write_op_u8(opcode_base + 0x9b, a),
        AddressingMode::Abs(a) => output.write_op_u16v(opcode_base + 0x8c, a),
        AddressingMode::X => output.write_u8(opcode_base + 0x1d),
        AddressingMode::Y => output.write_u8(opcode_base + 0xdc),
        am => return unknown_instruction_args_err(instruction, am),
    }
    Ok(())
}
fn rmw_instruction<'s>(
    instruction: &'s str,
    arguments: &'s str,
    opcode_base: u8,
    symbols: &Symbols<'s>,
    output: &mut Output<'s>,
) -> Result<(), InstructionError<'s>> {
    match parse_one_argument(arguments, symbols)? {
        AddressingMode::A => output.write_u8(opcode_base | 0x1c),
        AddressingMode::Dp(a) => output.write_op_u8(opcode_base | 0x0b, a),
        AddressingMode::DpX(a) => output.write_op_u8(opcode_base | 0x1b, a),
        AddressingMode::Abs(a) => output.write_op_u16v(opcode_base | 0x0c, a),
        am => return unknown_instruction_args_err(instruction, am),
    }
    Ok(())
}
fn branch_instruction<'s>(
    arguments: &'s str,
    opcode: u8,
    symbols: &Symbols<'s>,
    output: &mut Output<'s>,
) -> Result<(), InstructionError<'s>> {
    let rel = split_one_argument(arguments)?;
    output.write_u8(opcode);
    output.write_relative_goto(rel, symbols);
    Ok(())
}
fn branch_bit_instruction<'s>(
    instruction: &'s str,
    arguments: &'s str,
    opcode_base: u8,
    symbols: &Symbols<'s>,
    output: &mut Output<'s>,
) -> Result<(), InstructionError<'s>> {
    let [addr, bit, rel] = split_three_arguments(arguments)?;
    let am = parse_addressing_mode(addr, symbols)?;
    let bit = parse_bit_argument(bit, symbols)?;
    let opcode = bit.to_opcode(opcode_base);
    match am {
        AddressingMode::Dp(dp) => {
            output.write_op_u8(opcode, dp);
            output.write_relative_goto(rel, symbols);
            Ok(())
        }
        am => unknown_instruction_args_err(instruction, (am, bit, rel)),
    }
}
fn pcall_instruction<'s>(
    arguments: &'s str,
    symbols: &Symbols<'s>,
    output: &mut Output<'s>,
) -> Result<(), InstructionError<'s>> {
    let addr = split_one_argument(arguments)?;
    output.write_u8(0x4f);
    output.write_pcall_address(addr, symbols);
    Ok(())
}
fn tcall_instruction<'s>(
    arguments: &'s str,
    symbols: &Symbols<'s>,
    output: &mut Output<'s>,
) -> Result<(), InstructionError<'s>> {
    let addr = split_one_argument(arguments)?;
    match evaluate(addr, symbols) {
        ExpressionResult::Value(i) if (0..=16).contains(&i) => {
            output.write_u8(0x01 | u8::try_from(i << 4).unwrap());
            Ok(())
        }
        ExpressionResult::Error(e) => Err(InstructionError::ExpressionError(addr, e)),
        _ => Err(InstructionError::InvalidTcallArgument),
    }
}
fn stack_instruction<'s>(
    instruction: &'s str,
    arguments: &'s str,
    opcode_base: u8,
    symbols: &Symbols<'s>,
    output: &mut Output<'s>,
) -> Result<(), InstructionError<'s>> {
    match parse_one_argument(arguments, symbols)? {
        AddressingMode::A => {
            output.write_u8(opcode_base + 0x20);
            Ok(())
        }
        AddressingMode::X => {
            output.write_u8(opcode_base + 0x40);
            Ok(())
        }
        AddressingMode::Y => {
            output.write_u8(opcode_base + 0x60);
            Ok(())
        }
        AddressingMode::Psw => {
            output.write_u8(opcode_base);
            Ok(())
        }
        am => unknown_instruction_args_err(instruction, am),
    }
}
fn dp_bit_instruction<'s>(
    instruction: &'s str,
    arguments: &'s str,
    opcode_base: u8,
    symbols: &Symbols<'s>,
    output: &mut Output<'s>,
) -> Result<(), InstructionError<'s>> {
    let [addr, bit] = split_two_arguments(arguments)?;
    let am = parse_addressing_mode(addr, symbols)?;
    let bit = parse_bit_argument(bit, symbols)?;
    let opcode = bit.to_opcode(opcode_base);
    match am {
        AddressingMode::Dp(dp) => {
            output.write_op_u8(opcode, dp);
            Ok(())
        }
        am => unknown_instruction_args_err(instruction, (am, bit)),
    }
}
fn abs_bit_instruction<'s>(
    instruction: &'s str,
    arguments: &'s str,
    opcode: u8,
    symbols: &Symbols<'s>,
    output: &mut Output<'s>,
) -> Result<(), InstructionError<'s>> {
    match parse_two_no_dp_arguments(arguments, symbols)? {
        [AddressingMode::Abs(abs), bit] => {
            let bit = bit.try_into_bit_argument()?;
            output.write_op_abs_bit(opcode, abs, bit)?;
            Ok(())
        }
        am => unknown_instruction_args_err(instruction, am),
    }
}
fn abs_or_not_abs_bit_instruction<'s>(
    instruction: &'s str,
    arguments: &'s str,
    opcode_base: u8,
    symbols: &Symbols<'s>,
    output: &mut Output<'s>,
) -> Result<(), InstructionError<'s>> {
    match parse_three_no_dp_arguments(arguments, symbols)? {
        [AddressingMode::C, AddressingMode::Abs(abs), bit] => {
            let bit = bit.try_into_bit_argument()?;
            output.write_op_abs_bit(opcode_base, abs, bit)?;
            Ok(())
        }
        [AddressingMode::C, AddressingMode::NotAbs(abs), bit] => {
            let bit = bit.try_into_bit_argument()?;
            output.write_op_abs_bit(opcode_base | 0x20, abs, bit)?;
            Ok(())
        }
        am => unknown_instruction_args_err(instruction, am),
    }
}
fn carry_abs_bit_instruction<'s>(
    instruction: &'s str,
    arguments: &'s str,
    opcode: u8,
    symbols: &Symbols<'s>,
    output: &mut Output<'s>,
) -> Result<(), InstructionError<'s>> {
    match parse_three_no_dp_arguments(arguments, symbols)? {
        [AddressingMode::C, AddressingMode::Abs(abs), bit] => {
            let bit = bit.try_into_bit_argument()?;
            output.write_op_abs_bit(opcode, abs, bit)?;
            Ok(())
        }
        am => unknown_instruction_args_err(instruction, am),
    }
}
pub fn process_instruction<'s>(
    instruction: &'s str,
    arguments: &'s str,
    symbols: &Symbols<'s>,
    output: &mut Output<'s>,
) -> Result<(), InstructionError<'s>> {
    match instruction {
        "or" => arithmatic_instruction(instruction, arguments, 0x00, symbols, output),
        "and" => arithmatic_instruction(instruction, arguments, 0x20, symbols, output),
        "eor" => arithmatic_instruction(instruction, arguments, 0x40, symbols, output),
        "adc" => arithmatic_instruction(instruction, arguments, 0x80, symbols, output),
        "sbc" => arithmatic_instruction(instruction, arguments, 0xa0, symbols, output),
        "dec" => inc_dec_instruction(instruction, arguments, 0x00, symbols, output),
        "inc" => inc_dec_instruction(instruction, arguments, 0x20, symbols, output),
        "asl" => rmw_instruction(instruction, arguments, 0x00, symbols, output),
        "rol" => rmw_instruction(instruction, arguments, 0x20, symbols, output),
        "lsr" => rmw_instruction(instruction, arguments, 0x40, symbols, output),
        "ror" => rmw_instruction(instruction, arguments, 0x60, symbols, output),
        "xcn" => only_a_instruction(instruction, arguments, 0x9f, symbols, output),
        "incw" => only_dp_instruction(instruction, arguments, 0x3a, symbols, output),
        "decw" => only_dp_instruction(instruction, arguments, 0x1a, symbols, output),
        "addw" => ya_dp_instruction(instruction, arguments, 0x7a, symbols, output),
        "subw" => ya_dp_instruction(instruction, arguments, 0x9a, symbols, output),
        "cmpw" => ya_dp_instruction(instruction, arguments, 0x5a, symbols, output),
        "mul" => only_ya_instruction(instruction, arguments, 0xcf, symbols, output),
        "daa" => only_a_instruction(instruction, arguments, 0xdf, symbols, output),
        "das" => only_a_instruction(instruction, arguments, 0xbe, symbols, output),
        "bra" => branch_instruction(arguments, 0x2f, symbols, output),
        "beq" => branch_instruction(arguments, 0xf0, symbols, output),
        "bne" => branch_instruction(arguments, 0xd0, symbols, output),
        "bcs" => branch_instruction(arguments, 0xb0, symbols, output),
        "bcc" => branch_instruction(arguments, 0x90, symbols, output),
        "bvs" => branch_instruction(arguments, 0x70, symbols, output),
        "bvc" => branch_instruction(arguments, 0x50, symbols, output),
        "bmi" => branch_instruction(arguments, 0x30, symbols, output),
        "bpl" => branch_instruction(arguments, 0x10, symbols, output),
        "bbs" => branch_bit_instruction(instruction, arguments, 0x03, symbols, output),
        "bbc" => branch_bit_instruction(instruction, arguments, 0x13, symbols, output),
        "call" => only_abs_instruction(instruction, arguments, 0x3f, symbols, output),
        "pcall" => pcall_instruction(arguments, symbols, output),
        "tcall" => tcall_instruction(arguments, symbols, output),
        "brk" => no_argument_instruction(arguments, 0x0f, output),
        "ret" => no_argument_instruction(arguments, 0x6f, output),
        "reti" => no_argument_instruction(arguments, 0x7f, output),
        "push" => stack_instruction(instruction, arguments, 0x0d, symbols, output),
        "pop" => stack_instruction(instruction, arguments, 0x8e, symbols, output),
        "set1" => dp_bit_instruction(instruction, arguments, 0x02, symbols, output),
        "clr1" => dp_bit_instruction(instruction, arguments, 0x12, symbols, output),
        "tset1" => only_abs_instruction(instruction, arguments, 0x0e, symbols, output),
        "tclr1" => only_abs_instruction(instruction, arguments, 0x4e, symbols, output),
        "and1" => abs_or_not_abs_bit_instruction(instruction, arguments, 0x4a, symbols, output),
        "or1" => abs_or_not_abs_bit_instruction(instruction, arguments, 0x0a, symbols, output),
        "eor1" => carry_abs_bit_instruction(instruction, arguments, 0x8a, symbols, output),
        "not1" => abs_bit_instruction(instruction, arguments, 0xea, symbols, output),
        "clrc" => no_argument_instruction(arguments, 0x60, output),
        "setc" => no_argument_instruction(arguments, 0x80, output),
        "notc" => no_argument_instruction(arguments, 0xED, output),
        "clrv" => no_argument_instruction(arguments, 0xE0, output),
        "clrp" => no_argument_instruction(arguments, 0x20, output),
        "setp" => no_argument_instruction(arguments, 0x40, output),
        "ei" => no_argument_instruction(arguments, 0xA0, output),
        "di" => no_argument_instruction(arguments, 0xC0, output),
        "nop" => no_argument_instruction(arguments, 0x00, output),
        "sleep" => no_argument_instruction(arguments, 0xEF, output),
        "stop" => no_argument_instruction(arguments, 0xFF, output),
        "mov" => {
            match parse_two_arguments(arguments, symbols)? {
                [AddressingMode::A, AddressingMode::Immediate(i)] => output.write_op_u8v(0xe8, i),
                [AddressingMode::A, AddressingMode::XIndirect] => output.write_u8(0xe6),
                [AddressingMode::A, AddressingMode::XIndirectIncrement] => output.write_u8(0xbf),
                [AddressingMode::A, AddressingMode::Dp(a)] => output.write_op_u8(0xe4, a),
                [AddressingMode::A, AddressingMode::DpX(a)] => output.write_op_u8(0xf4, a),
                [AddressingMode::A, AddressingMode::Abs(a)] => output.write_op_u16v(0xe5, a),
                [AddressingMode::A, AddressingMode::AbsX(a)] => output.write_op_u16v(0xf5, a),
                [AddressingMode::A, AddressingMode::AbsY(a)] => output.write_op_u16v(0xf6, a),
                [AddressingMode::A, AddressingMode::DpIndirectX(a)] => output.write_op_u8(0xe7, a),
                [AddressingMode::A, AddressingMode::DpIndirectY(a)] => output.write_op_u8(0xf7, a),
                [AddressingMode::A, AddressingMode::DpY(a)] => {
                    output.write_u8(0xf6);
                    output.write_u16(symbols.dp_addr_to_addr(a));
                }
                [AddressingMode::X, AddressingMode::Immediate(v)] => output.write_op_u8v(0xcd, v),
                [AddressingMode::X, AddressingMode::Dp(a)] => output.write_op_u8(0xf8, a),
                [AddressingMode::X, AddressingMode::DpY(a)] => output.write_op_u8(0xf9, a),
                [AddressingMode::X, AddressingMode::Abs(a)] => output.write_op_u16v(0xe9, a),
                [AddressingMode::Y, AddressingMode::Immediate(v)] => output.write_op_u8v(0x8d, v),
                [AddressingMode::Y, AddressingMode::Dp(a)] => output.write_op_u8(0xeb, a),
                [AddressingMode::Y, AddressingMode::DpX(a)] => output.write_op_u8(0xfb, a),
                [AddressingMode::Y, AddressingMode::Abs(a)] => output.write_op_u16v(0xec, a),
                [AddressingMode::XIndirect, AddressingMode::A] => output.write_u8(0xc6),
                [AddressingMode::XIndirectIncrement, AddressingMode::A] => output.write_u8(0xaf),
                [AddressingMode::Dp(a), AddressingMode::A] => output.write_op_u8(0xc4, a),
                [AddressingMode::DpX(a), AddressingMode::A] => output.write_op_u8(0xd4, a),
                [AddressingMode::Abs(a), AddressingMode::A] => output.write_op_u16v(0xc5, a),
                [AddressingMode::AbsX(a), AddressingMode::A] => output.write_op_u16v(0xd5, a),
                [AddressingMode::AbsY(a), AddressingMode::A] => output.write_op_u16v(0xd6, a),
                [AddressingMode::DpIndirectX(a), AddressingMode::A] => output.write_op_u8(0xc7, a),
                [AddressingMode::DpIndirectY(a), AddressingMode::A] => output.write_op_u8(0xd7, a),
                [AddressingMode::DpY(a), AddressingMode::A] => {
                    output.write_u8(0xd6);
                    output.write_u16(symbols.dp_addr_to_addr(a));
                }
                [AddressingMode::Dp(a), AddressingMode::X] => output.write_op_u8(0xd8, a),
                [AddressingMode::DpY(a), AddressingMode::X] => output.write_op_u8(0xd9, a),
                [AddressingMode::Abs(a), AddressingMode::X] => output.write_op_u16v(0xc9, a),
                [AddressingMode::Dp(a), AddressingMode::Y] => output.write_op_u8(0xcb, a),
                [AddressingMode::DpX(a), AddressingMode::Y] => output.write_op_u8(0xdb, a),
                [AddressingMode::Abs(a), AddressingMode::Y] => output.write_op_u16v(0xcc, a),
                [AddressingMode::A, AddressingMode::X] => output.write_u8(0x7d),
                [AddressingMode::A, AddressingMode::Y] => output.write_u8(0xdd),
                [AddressingMode::X, AddressingMode::A] => output.write_u8(0x5d),
                [AddressingMode::Y, AddressingMode::A] => output.write_u8(0xfd),
                [AddressingMode::X, AddressingMode::Sp] => output.write_u8(0x9d),
                [AddressingMode::Sp, AddressingMode::X] => output.write_u8(0xbd),
                [AddressingMode::Dp(ad), AddressingMode::Dp(ds)] => {
                    output.write_u8(0xfa);
                    output.write_u8(ds);
                    output.write_u8(ad);
                }
                [AddressingMode::Dp(a), AddressingMode::Immediate(i)] => {
                    output.write_u8(0x8f);
                    output.write_u8v(i);
                    output.write_u8(a);
                }
                am => return unknown_instruction_args_err(instruction, am),
            }
            Ok(())
        }
        "movw" => {
            match parse_two_arguments(arguments, symbols)? {
                [AddressingMode::Ya, AddressingMode::Dp(a)] => output.write_op_u8(0xba, a),
                [AddressingMode::Dp(a), AddressingMode::Ya] => output.write_op_u8(0xda, a),
                am => arithmatic_instruction_impl(instruction, am, 0x60, symbols, output)?,
            }
            Ok(())
        }
        "mov1" => match parse_three_no_dp_arguments(arguments, symbols)? {
            [AddressingMode::C, AddressingMode::Abs(abs), bit] => {
                let bit = bit.try_into_bit_argument()?;
                output.write_op_abs_bit(0xaa, abs, bit)?;
                Ok(())
            }
            [AddressingMode::Abs(abs), bit, AddressingMode::C] => {
                let bit = bit.try_into_bit_argument()?;
                output.write_op_abs_bit(0xca, abs, bit)?;
                Ok(())
            }
            am => unknown_instruction_args_err(instruction, am),
        },
        "cmp" => {
            match parse_two_arguments(arguments, symbols)? {
                [AddressingMode::X, AddressingMode::Immediate(v)] => output.write_op_u8v(0xc8, v),
                [AddressingMode::X, AddressingMode::Dp(a)] => output.write_op_u8(0x3e, a),
                [AddressingMode::X, AddressingMode::Abs(a)] => output.write_op_u16v(0x1e, a),
                [AddressingMode::Y, AddressingMode::Immediate(v)] => output.write_op_u8v(0xad, v),
                [AddressingMode::Y, AddressingMode::Dp(a)] => output.write_op_u8(0x7e, a),
                [AddressingMode::Y, AddressingMode::Abs(a)] => output.write_op_u16v(0x5e, a),
                am => arithmatic_instruction_impl(instruction, am, 0x60, symbols, output)?,
            }
            Ok(())
        }
        "div" => match parse_two_arguments(arguments, symbols)? {
            [AddressingMode::Ya, AddressingMode::X] => {
                output.write_u8(0x9e);
                Ok(())
            }
            am => unknown_instruction_args_err(instruction, am),
        },
        "cbne" => match parse_argument_and_rel(arguments, symbols)? {
            (AddressingMode::Dp(dp), rel) => {
                output.write_op_u8(0x2e, dp);
                output.write_relative_goto(rel, symbols);
                Ok(())
            }
            (AddressingMode::DpX(dp), rel) => {
                output.write_op_u8(0xde, dp);
                output.write_relative_goto(rel, symbols);
                Ok(())
            }
            am => unknown_instruction_args_err(instruction, am),
        },
        "dbnz" => match parse_argument_and_rel(arguments, symbols)? {
            (AddressingMode::Dp(dp), rel) => {
                output.write_op_u8(0x6e, dp);
                output.write_relative_goto(rel, symbols);
                Ok(())
            }
            (AddressingMode::Y, rel) => {
                output.write_u8(0xfe);
                output.write_relative_goto(rel, symbols);
                Ok(())
            }
            am => unknown_instruction_args_err(instruction, am),
        },
        "jmp" => match parse_one_no_dp_argument(arguments, symbols)? {
            AddressingMode::Abs(abs) => {
                output.write_op_u16v(0x5f, abs);
                Ok(())
            }
            AddressingMode::AbsIndirectX(abs) => {
                output.write_op_u16v(0x1f, abs);
                Ok(())
            }
            am => unknown_instruction_args_err(instruction, am),
        },
        _ => Err(InstructionError::UnknownInstruction(instruction)),
    }
}
