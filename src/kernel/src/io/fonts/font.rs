// Font Driver

use crate::io::graphics::*;
use crate::*;
use alloc::boxed::Box;
use alloc::vec::*;
// use core::num::*;

include!("megbtan.rs");
const SYSTEM_FONT: FontDriver = FontDriver::with_rasterized(8, 16, &FONT_MEGBTAN_DATA);

include!("megh0608.rs");
const SMALL_FONT: FontDriver = FontDriver::with_rasterized(6, 8, &FONT_MEGH0608_DATA);

const DUMMY_ARRAY: [u8; 0] = [];

static mut HERSHEY_FONT: Option<Box<HersheyFontCache>> = None;

pub struct FontDriver<'a> {
    font_type: FontType,
    size: Size<isize>,
    data: &'a [u8],
    leading: isize,
    line_height: isize,
    stride: usize,
}

#[derive(Debug, Copy, Clone)]
pub enum FontType {
    Rasterized,
    Hershey,
}

impl<'a> FontDriver<'a> {
    pub fn with_hershey(point: isize) -> Box<FontDriver<'a>> {
        let width = point / 2;
        let height = point;
        let line_height = height;
        let leading = (line_height - height) / 2;
        Box::new(FontDriver {
            font_type: FontType::Hershey,
            size: Size::new(width, height),
            line_height,
            leading,
            stride: 0,
            data: &DUMMY_ARRAY,
        })
    }
}

impl FontDriver<'_> {
    pub const fn with_rasterized(
        width: usize,
        height: usize,
        data: &'static [u8],
    ) -> FontDriver<'static> {
        let width = width as isize;
        let height = height as isize;
        let line_height = height * 5 / 4;
        let leading = (line_height - height) / 2;
        let stride = ((width as usize + 7) >> 3) * height as usize;
        FontDriver {
            font_type: FontType::Rasterized,
            size: Size::new(width, height),
            line_height,
            leading,
            stride,
            data,
        }
    }

    pub(crate) fn init() {
        let hershey = HersheyFontCache::new();
        unsafe {
            HERSHEY_FONT = Some(Box::new(hershey));
        }
    }

    pub const fn system_font_static() -> &'static FontDriver<'static> {
        &SYSTEM_FONT
    }

    pub fn system_font() -> Box<FontDriver<'static>> {
        Box::new(SYSTEM_FONT)
    }

    pub fn small_font() -> Box<FontDriver<'static>> {
        Box::new(SMALL_FONT)
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
        match self.font_type {
            FontType::Rasterized => self.width(),
            FontType::Hershey => unsafe {
                let shared = HERSHEY_FONT.as_ref().unwrap();
                shared.width_for(character) * self.height() / shared.point
            },
        }
    }

    /// Glyph Data for Rasterized Font
    fn glyph_for(&self, character: char) -> Option<&[u8]> {
        let c = character as usize;
        if c > 0x20 && c < 0x80 {
            let base = self.stride * (c - 0x20);
            Some(&self.data[base..base + self.stride])
        } else {
            None
        }
    }

    pub fn draw_char(&self, character: char, bitmap: &Bitmap, origin: Point<isize>, color: Color) {
        match self.font_type {
            FontType::Rasterized => {
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
            FontType::Hershey => {
                let shared = unsafe { HERSHEY_FONT.as_ref().unwrap() };
                // let width = shared.width_for(character);
                // bitmap.draw_rect(
                //     Rect::new(origin.x, origin.y, width, self.height()),
                //     Color::from_rgb(0xFFCCFF),
                // );
                shared.draw_char(character, bitmap, origin, self.height(), color);
            }
        }
    }
}

#[allow(dead_code)]
pub struct HersheyFontCache<'a> {
    point: isize,
    data: &'a [u8],
    bitmap: Bitmap,
    width_array: Vec<isize>,
}

impl<'a> HersheyFontCache<'a> {
    fn new() -> Self {
        let point = 32;

        let bitmap = Bitmap::new(point as usize, point as usize * 96, true);
        bitmap.reset();
        // let data = include_bytes!("futuram.jhf");
        let data = &[];

        let mut font = Self {
            point,
            data,
            bitmap,
            width_array: Vec::with_capacity(256),
        };

        for c in 0x20..0x80 {
            let character = c as u8 as char;

            let data = match font.glyph_for(character) {
                Some((base, last)) => &font.data[base..last],
                None => break,
            };

            let w1 = data[8] as isize - 0x52;
            let w2 = data[9] as isize - 0x52;
            font.width_array.push(w2 - w1);

            let position = (character as isize - 0x20) * 32;
            font.draw_data(
                data,
                &font.bitmap,
                Point::new(0, position),
                font.point,
                Color::from_argb(0xFF000000),
            );
        }

        font
    }

    fn draw_char(
        &self,
        character: char,
        bitmap: &Bitmap,
        origin: Point<isize>,
        point: isize,
        color: Color,
    ) {
        let data = match self.glyph_for(character) {
            Some((base, last)) => &self.data[base..last],
            None => return,
        };
        self.draw_data(data, &bitmap, origin, point, color);
    }

    fn draw_data(
        &self,
        data: &[u8],
        bitmap: &Bitmap,
        origin: Point<isize>,
        point: isize,
        color: Color,
    ) {
        if data.len() >= 12 {
            let n_pairs = (data[6] & 0x0F) * 10 + (data[7] & 0x0F);
            let left = data[8] as isize - 0x40;

            let center = Point::new(origin.x + point / 2 - 1, origin.y + point / 2 - 1);
            let mut cursor = 10;
            let mut c0: Option<Point<isize>> = None;
            for _ in 1..n_pairs {
                let c1 = data[cursor];
                let c2 = data[cursor + 1];
                if c1 != 0x20 && c2 != 0x52 {
                    let d1 = c1 as isize - 0x52 - left;
                    let d2 = c2 as isize - 0x52;
                    let c1 = center + Point::new(d1 * point / 32, d2 * point / 32);
                    if let Some(c0) = c0 {
                        bitmap.draw_line(c0, c1, color)
                    }
                    c0 = Some(c1);
                } else {
                    c0 = None;
                }
                cursor += 2;
            }
        }
    }

    #[allow(dead_code)]
    fn draw_cache(
        &self,
        character: char,
        bitmap: &Bitmap,
        origin: Point<isize>,
        font: &FontDriver,
        color: Color,
    ) {
        let w = self.width_for(character);
        if w > 0 {
            let _ = color;
            let _ = font;

            bitmap.draw_rect(
                Rect::new(origin.x, origin.y, w, self.point),
                Color::from_rgb(0xFFFFCCFF),
            );

            let c = character as isize - 0x20;
            let origin = Point::new(origin.x, origin.y);
            let rect = Rect::new(0, c * self.point, w, self.point);
            bitmap.blt(&self.bitmap, origin, rect, BltOption::empty());
        }
    }

    fn glyph_for(&self, character: char) -> Option<(usize, usize)> {
        let c = character as usize;
        if c >= 0x20 && c < 0x80 {
            let c = c - 0x20;
            let mut cursor = 0;
            for current in 0..96 {
                if self.data.len() <= cursor {
                    return None;
                }
                if current == c {
                    let base = cursor;
                    while self.data[cursor] >= 0x20 {
                        cursor += 1;
                    }
                    return Some((base, cursor));
                }
                while self.data[cursor] >= 0x20 {
                    cursor += 1;
                }
                cursor += 1;
            }
        }
        None
    }

    fn width_for(&self, character: char) -> isize {
        let i = character as usize;
        if i >= 0x20 && i < 0x80 {
            let i = i - 0x20;
            if i < self.width_array.len() {
                return self.width_array[i];
            }
        }
        0
    }
}
