// Font Driver
use crate::graphics::*;

static SYSTEM_FONT_DATA: &[u8; 4113] = include_bytes!("./moesys16.fnt");

pub struct FontDriver<'a> {
    size: Size<isize>,
    data: &'a [u8],
    line_height: isize,
}

impl<'a> FontDriver<'_> {
    pub fn glyph_for(&self, c: char) -> Option<&[u8]> {
        let c = c as usize;
        if c > 0x20 && c < 0x80 {
            let delta = (self.size.width as usize + 7) / 8 * self.size.height as usize;
            let base = 0x11 + delta * c;
//            panic!("Font size: {:#x} x {:#x} code {:#x} delta {:#x} base {:#x}", self.size.width, self.size.height, c, delta, base);
            Some(&self.data[base..base + delta])
        } else {
            None
        }
    }

    #[inline]
    pub fn size(&self) -> Size<isize> {
        self.size
    }

    #[inline]
    pub fn width(&self) -> isize {
        self.size.width
    }

    #[inline]
    pub fn height(&self) -> isize {
        self.size.height
    }

    #[inline]
    pub fn line_height(&self) -> isize {
        self.line_height
    }

    pub fn system_font() -> FontDriver<'static> {
        let width = SYSTEM_FONT_DATA[14] as isize;
        let height = SYSTEM_FONT_DATA[15] as isize;
        let lh = height * 5 / 4;
        FontDriver {
            size: Size::new((width, height)),
            line_height: lh,
            data: SYSTEM_FONT_DATA,
        }
    }
}
