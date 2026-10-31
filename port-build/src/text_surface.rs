use crate::{
    Result,
    font::PortFont,
    world::{RenderEvent, Renderer},
};
pub struct TextSurface {
    pub pixels: Vec<u8>,
    pub cells: std::collections::BTreeMap<(usize, usize), (char, u8)>,
}
impl TextSurface {
    pub fn new() -> Self {
        Self {
            pixels: vec![16; 640 * 144],
            cells: std::collections::BTreeMap::new(),
        }
    }
    fn clear(&mut self, window: u32, renderer: &Renderer) -> Result<()> {
        if window & 65535 != 4 {
            return Err("Unimplemented text background restoration".into());
        }
        let [l, t, r, b] = *renderer.windows.get(4).ok_or("Missing text window")?;
        if l < 0 || t < 0 || l > r || t > b || r > 640 || b > 144 {
            return Err("Invalid text clear rectangle".into());
        }
        for y in t..b {
            self.pixels[y as usize * 640 + l as usize..y as usize * 640 + r as usize].fill(16);
        }
        self.cells.clear();
        Ok(())
    }
    pub fn apply(
        &mut self,
        event: &RenderEvent,
        renderer: &Renderer,
        font: &PortFont,
        content: Option<&str>,
    ) -> Result<()> {
        match event {
            RenderEvent::Clear { window } => self.clear(*window, renderer)?,
            RenderEvent::Text {
                x,
                y,
                font: kind,
                color,
                bytes,
                ..
            } => {
                if *kind != 2 || renderer.fonts.get(2) != Some(&[24, 20]) {
                    return Err("Unsupported port-font metrics".into());
                }
                let rgb = [*color as u8, (*color >> 8) as u8, (*color >> 16) as u8];
                let index = renderer
                    .palette
                    .iter()
                    .position(|c| *c == rgb)
                    .ok_or("Unmapped text color")?
                    + 1;
                if index > 6 {
                    return Err("Text palette extent exceeded".into());
                }
                let characters = match content {
                    Some(text) => text.chars().collect(),
                    None => font.characters(bytes)?,
                };
                if *x < 0
                    || *y < 0
                    || i64::from(*x) + characters.len() as i64 * 20 > 460
                    || i64::from(*y) + 24 > 96
                {
                    return Err("Port text would be clipped by the prologue window".into());
                }
                for (column, ch) in characters.into_iter().enumerate() {
                    if x % 20 != 0 || y % 24 != 0 {
                        return Err("Non-cell source text position".into());
                    }
                    self.cells.insert(
                        (*y as usize / 24, *x as usize / 20 + column),
                        (ch, index as u8),
                    );
                    let mask = font.mask(ch)?;
                    for (offset, pixel) in mask.iter().enumerate() {
                        if *pixel == 0 {
                            continue;
                        }
                        let dx = i64::from(*x) + column as i64 * 20 + (offset % 20) as i64;
                        let dy = i64::from(*y) + 2 + (offset / 20) as i64;
                        if (0..640).contains(&dx) && (0..144).contains(&dy) {
                            self.pixels[dy as usize * 640 + dx as usize] = index as u8;
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
