// Font Driver
use crate::io::graphics::*;

include!("megbtan.rs");
const SYSTEM_FONT: FontDriver = FontDriver::new(8, 16, &FONT_MEGBTAN_DATA);

include!("megh0608.rs");
const SMALL_FONT: FontDriver = FontDriver::new(6, 8, &FONT_MEGH0608_DATA);

pub struct FontDriver<'a> {
    size: Size<isize>,
    data: &'a [u8],
    leading: isize,
    line_height: isize,
    stride: usize,
}

impl FontDriver<'_> {
    const fn new(width: usize, height: usize, data: &'static [u8]) -> FontDriver<'static> {
        let width = width as isize;
        let height = height as isize;
        let line_height = height * 5 / 4;
        let leading = (line_height - height) / 2;
        let stride = ((width as usize + 7) >> 3) * height as usize;
        FontDriver {
            size: Size::new(width, height),
            line_height,
            leading,
            stride,
            data,
        }
    }

    pub const fn system_font() -> &'static FontDriver<'static> {
        &SYSTEM_FONT
    }

    pub const fn small_font() -> &'static FontDriver<'static> {
        &SMALL_FONT
    }

    fn glyph_for(&self, character: char) -> Option<&[u8]> {
        let c = character as usize;
        if c > 0x20 && c < 0x80 {
            let base = self.stride * (c - 0x20);
            Some(&self.data[base..base + self.stride])
        } else {
            None
        }
    }

    #[inline]
    pub const fn size(&self) -> Size<isize> {
        self.size
    }

    #[inline]
    pub const fn width(&self) -> isize {
        self.size.width
    }

    #[inline]
    pub const fn height(&self) -> isize {
        self.size.height
    }

    #[inline]
    pub const fn line_height(&self) -> isize {
        self.line_height
    }

    #[inline]
    pub const fn leading(&self) -> isize {
        self.leading
    }

    #[inline]
    pub fn width_of(&self, character: char) -> isize {
        let _ = character;
        self.width()
    }

    pub fn draw_char(&self, character: char, bitmap: &Bitmap, origin: Point<isize>, color: Color) {
        if let Some(glyph) = self.glyph_for(character) {
            let rect = Rect::new(
                origin.x,
                origin.y + self.leading(),
                self.width(),
                self.height(),
            );
            bitmap.draw_pattern(rect, glyph, color);
        }
    }
}

pub struct TextAttributes {
    _phantom: (),
}
