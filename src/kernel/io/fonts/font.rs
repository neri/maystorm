// Font Driver
use crate::kernel::io::graphics::*;

include!("megbtan.rs");
const SYSTEM_FONT: FontDriver =
    FontDriver::new(FONT_MEGBTAN_WIDTH, FONT_MEGBTAN_HEIGHT, &FONT_MEGBTAN_DATA);

include!("megh0608.rs");
const SMALL_FONT: FontDriver = FontDriver::new(
    FONT_MEGH0608_WIDTH,
    FONT_MEGH0608_HEIGHT,
    &FONT_MEGH0608_DATA,
);

pub struct FontDriver<'a> {
    size: Size<isize>,
    data: &'a [u8],
    leading: isize,
    line_height: isize,
    delta: usize,
}

impl<'a> FontDriver<'_> {
    const fn new(width: usize, height: usize, data: &'static [u8]) -> FontDriver<'static> {
        let width = width as isize;
        let height = height as isize;
        let line_height = height * 5 / 4;
        let leading = (line_height - height) / 2;
        let delta = ((width as usize + 7) >> 3) * height as usize;
        FontDriver {
            size: Size::new(width, height),
            line_height,
            leading,
            delta,
            data,
        }
    }

    pub const fn system_font() -> &'static FontDriver<'static> {
        &SYSTEM_FONT
    }

    pub const fn small_font() -> &'static FontDriver<'static> {
        &SMALL_FONT
    }

    pub fn glyph_for(&self, c: char) -> Option<&[u8]> {
        let c = c as usize;
        if c > 0x20 && c < 0x80 {
            let base = self.delta * (c - 0x20);
            Some(&self.data[base..base + self.delta])
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

    #[inline]
    pub fn leading(&self) -> isize {
        self.leading
    }
}
