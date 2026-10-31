pub use crate::sfc_tad::Assets;
pub(crate) use crate::sound_commands::*;
pub fn emit(
    p: &mut w65c816::Assembler,
    layout: &sfc_builder::cartridge::CartridgeLayout,
) -> crate::Result<()> {
    crate::sfc_tad::emit(p, layout)
}
