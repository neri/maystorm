// Font Driver

use crate::sync::Mutex;
use crate::*;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::*;
use megstd::drawing::*;

#[allow(dead_code)]
mod embedded {
    // include!("megbtan.rs");
    include!("megh0816.rs");
    include!("megh0710.rs");
    include!("megh0608.rs");
}
const SYSTEM_FONT: FixedFontDriver = FixedFontDriver::new(8, 16, &embedded::FONT_MEGH0816_DATA);
const TERMINAL_FONT: FixedFontDriver = FixedFontDriver::new(7, 10, &embedded::FONT_MEGH0710_DATA);
const SMALL_FONT: FixedFontDriver = FixedFontDriver::new(6, 8, &embedded::FONT_MEGH0608_DATA);

static mut FONT_MANAGER: FontManager = FontManager::new();

pub struct FontManager {
    fonts: Option<BTreeMap<FontFamily, Box<dyn FontDriver>>>,
    buffer: Mutex<OperationalBitmap>,
}

impl FontManager {
    const fn new() -> Self {
        Self {
            fonts: None,
            buffer: Mutex::new(OperationalBitmap::new(Size::new(96, 96))),
        }
    }

    #[inline]
    fn shared<'a>() -> &'a mut Self {
        unsafe { &mut FONT_MANAGER }
    }

    pub fn init() {
        let shared = Self::shared();

        let mut fonts: BTreeMap<FontFamily, Box<dyn FontDriver>> = BTreeMap::new();

        fonts.insert(FontFamily::FixedSystem, Box::new(SYSTEM_FONT));
        fonts.insert(FontFamily::SmallFixed, Box::new(SMALL_FONT));
        fonts.insert(FontFamily::Terminal, Box::new(TERMINAL_FONT));

        let font = Box::new(HersheyFont::new(
            0,
            include_bytes!("../../../../../ext/hershey/futural.jhf"),
        ));
        fonts.insert(FontFamily::SystemUI, font);

        let font = Box::new(HersheyFont::new(
            4,
            include_bytes!("../../../../../ext/hershey/cursive.jhf"),
        ));
        fonts.insert(FontFamily::Cursive, font);

        let font = Box::new(HersheyFont::new(
            0,
            include_bytes!("../../../../../ext/hershey/futuram.jhf"),
        ));
        fonts.insert(FontFamily::SansSerif, font);

        let font = Box::new(HersheyFont::new(
            0,
            include_bytes!("../../../../../ext/hershey/timesrb.jhf"),
        ));
        fonts.insert(FontFamily::Serif, font);

        shared.fonts = Some(fonts);
    }

    fn driver_for(family: FontFamily) -> Option<&'static dyn FontDriver> {
        let shared = Self::shared();
        shared
            .fonts
            .as_ref()
            .and_then(|v| v.get(&family))
            .map(|v| v.as_ref())
    }

    #[inline]
    pub const fn fixed_system_font() -> &'static FixedFontDriver<'static> {
        &SYSTEM_FONT
    }

    #[inline]
    #[track_caller]
    pub fn system_font() -> FontDescriptor {
        FontDescriptor::new(FontFamily::FixedSystem, 0).unwrap()
    }

    #[inline]
    #[track_caller]
    pub fn title_font() -> FontDescriptor {
        FontDescriptor::new(FontFamily::SansSerif, 16).unwrap_or(Self::system_font())
    }

    #[inline]
    #[track_caller]
    pub fn ui_font() -> FontDescriptor {
        FontDescriptor::new(FontFamily::SystemUI, 16).unwrap_or(Self::system_font())
    }
}

#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FontFamily {
    SystemUI,
    SansSerif,
    Serif,
    Cursive,
    FixedSystem,
    Terminal,
    SmallFixed,
    Japanese,
}

#[derive(Copy, Clone)]
pub struct FontDescriptor {
    driver: &'static dyn FontDriver,
    point: i32,
    line_height: i32,
}

impl FontDescriptor {
    pub fn new(family: FontFamily, point: isize) -> Option<Self> {
        FontManager::driver_for(family).map(|driver| {
            if driver.is_scalable() {
                Self {
                    driver,
                    point: point as i32,
                    line_height: (driver.preferred_line_height() * point / driver.base_height())
                        as i32,
                }
            } else {
                Self {
                    driver,
                    point: driver.base_height() as i32,
                    line_height: driver.preferred_line_height() as i32,
                }
            }
        })
    }

    #[inline]
    pub const fn point(&self) -> isize {
        self.point as isize
    }

    #[inline]
    pub const fn line_height(&self) -> isize {
        self.line_height as isize
    }

    #[inline]
    pub fn width_of(&self, character: char) -> isize {
        if self.point() == self.driver.base_height() {
            self.driver.width_of(character)
        } else {
            self.driver.width_of(character) * self.point() / self.driver.base_height()
        }
    }

    #[inline]
    pub fn is_scalable(&self) -> bool {
        self.driver.is_scalable()
    }

    #[inline]
    pub fn draw_char(&self, character: char, bitmap: &mut Bitmap, origin: Point, color: SomeColor) {
        self.driver
            .draw_char(character, bitmap, origin, self.point(), color)
    }
}

pub trait FontDriver {
    fn is_scalable(&self) -> bool;

    fn base_height(&self) -> isize;

    fn preferred_line_height(&self) -> isize;

    fn width_of(&self, character: char) -> isize;

    fn draw_char(
        &self,
        character: char,
        bitmap: &mut Bitmap,
        origin: Point,
        height: isize,
        color: SomeColor,
    );
}

pub struct FixedFontDriver<'a> {
    size: Size,
    data: &'a [u8],
    leading: isize,
    line_height: isize,
    stride: usize,
}

impl FixedFontDriver<'_> {
    pub const fn new(width: usize, height: usize, data: &'static [u8]) -> FixedFontDriver<'static> {
        let width = width as isize;
        let height = height as isize;
        let line_height = height * 5 / 4;
        let leading = (line_height - height) / 2;
        let stride = ((width as usize + 7) >> 3) * height as usize;
        FixedFontDriver {
            size: Size::new(width, height),
            line_height,
            leading,
            stride,
            data,
        }
    }

    #[inline]
    pub const fn width(&self) -> isize {
        self.size.width
    }

    #[inline]
    pub const fn line_height(&self) -> isize {
        self.line_height
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
}

impl FontDriver for FixedFontDriver<'_> {
    #[inline]
    fn is_scalable(&self) -> bool {
        false
    }

    #[inline]
    fn base_height(&self) -> isize {
        self.size.height
    }

    #[inline]
    fn preferred_line_height(&self) -> isize {
        self.line_height
    }

    #[inline]
    fn width_of(&self, character: char) -> isize {
        let _ = character;
        self.size.width
    }

    fn draw_char(
        &self,
        character: char,
        bitmap: &mut Bitmap,
        origin: Point,
        _height: isize,
        color: SomeColor,
    ) {
        if let Some(font) = self.glyph_for(character) {
            let origin = Point::new(origin.x, origin.y + self.leading);
            let size = Size::new(self.width_of(character), self.size.height());
            bitmap.draw_font(font, size, origin, color);
        }
    }
}

#[allow(dead_code)]
struct HersheyFont<'a> {
    data: &'a [u8],
    line_height: isize,
    glyph_info: Vec<(usize, usize, isize)>,
}

impl<'a> HersheyFont<'a> {
    const MAGIC_20: isize = 0x20;
    const MAGIC_52: isize = 0x52;
    const POINT: isize = 32;
    const DESCENT: isize = 4;

    fn new(extra_height: isize, font_data: &'a [u8]) -> Self {
        let descent = Self::DESCENT + extra_height;
        let mut font = Self {
            data: font_data,
            line_height: Self::POINT + descent,
            glyph_info: Vec::with_capacity(96),
        };

        for c in 0x20..0x80 {
            let character = c as u8 as char;

            let (base, last) = match font.search_for_glyph(character) {
                Some(tuple) => tuple,
                None => break,
            };

            let data = &font_data[base..last];

            let w1 = data[8] as isize;
            let w2 = data[9] as isize;

            font.glyph_info.push((base, last, w2 - w1));
        }

        font
    }

    fn draw_data(
        &self,
        data: &[u8],
        bitmap: &mut Bitmap,
        origin: Point,
        width: isize,
        height: isize,
        color: SomeColor,
    ) {
        if data.len() >= 12 {
            let mut buffer = FontManager::shared().buffer.lock().unwrap();
            buffer.reset();

            let master_scale = 1;
            let n_pairs = (data[6] & 0x0F) * 10 + (data[7] & 0x0F);
            let left = data[8] as isize - Self::MAGIC_52;

            let quality = 64;
            let center1 = Point::new(buffer.size().width() / 2, buffer.size().height() / 2);
            let mut cursor = 10;
            let mut c1: Option<Point> = None;
            let center2 = Point::new(center1.x * quality, center1.y * quality);
            for _ in 1..n_pairs {
                let p1 = data[cursor] as isize;
                let p2 = data[cursor + 1] as isize;
                if p1 == Self::MAGIC_20 && p2 == Self::MAGIC_52 {
                    c1 = None;
                } else {
                    let d1 = p1 - Self::MAGIC_52;
                    let d2 = p2 - Self::MAGIC_52;
                    let c2 = center2
                        + Point::new(
                            d1 * quality * height / Self::POINT,
                            d2 * quality * height / Self::POINT,
                        );
                    if let Some(c1) = c1 {
                        buffer.draw_line_anti_aliasing(c1, c2, quality, |bitmap, point, value| {
                            if point.is_within(bitmap.bounds()) {
                                unsafe {
                                    bitmap
                                        .process_pixel_unchecked(point, |v| v.saturating_add(value))
                                }
                            }
                        });
                    }
                    c1 = Some(c2);
                }
                cursor += 2;
            }

            let act_w = width * height / Self::POINT;
            let offset_x = center1.x - (-left * master_scale) * height / Self::POINT;
            let offset_y = center1.y - height * master_scale / 2;

            // DEBUG
            if false {
                let rect = Rect::new(
                    origin.x,
                    origin.y,
                    width * height / Self::POINT,
                    self.line_height * height / Self::POINT,
                );
                bitmap.draw_rect(rect, SomeColor::from_rgb(0xFFCCFF));
                bitmap.draw_hline(
                    Point::new(origin.x, origin.y + height - 1),
                    width * height / Self::POINT,
                    SomeColor::from_rgb(0xFFFF33),
                );
                bitmap.draw_hline(
                    Point::new(origin.x, origin.y + height * 3 / 4),
                    width * height / Self::POINT,
                    SomeColor::from_rgb(0xFF3333),
                );
            }

            match bitmap {
                Bitmap::Indexed(_) => {
                    // TODO:
                }
                Bitmap::Argb32(bitmap) => {
                    let color = color.into_argb();
                    for y in 0..=height {
                        for x in 0..act_w {
                            let point = origin + Point::new(x, y);
                            unsafe {
                                bitmap.process_pixel_unchecked(point, |v| {
                                    let mut c = color.components();
                                    let alpha = buffer.get_pixel_unchecked(Point::new(
                                        offset_x + x * master_scale,
                                        offset_y + y * master_scale,
                                    ));
                                    c.a = alpha; //u8::MAX - alpha;
                                    v.blend_draw(c.into())
                                })
                            }
                        }
                    }
                }
            }
        }
    }

    fn search_for_glyph(&self, character: char) -> Option<(usize, usize)> {
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

    fn glyph_for(&self, character: char) -> Option<(usize, usize, isize)> {
        let i = (character as usize) - 0x20;
        if i < (0x80 - 0x20) && i < self.glyph_info.len() {
            return Some(self.glyph_info[i]);
        }
        None
    }
}

impl FontDriver for HersheyFont<'_> {
    #[inline]
    fn is_scalable(&self) -> bool {
        true
    }

    #[inline]
    fn base_height(&self) -> isize {
        Self::POINT
    }

    #[inline]
    fn preferred_line_height(&self) -> isize {
        self.line_height
    }

    fn width_of(&self, character: char) -> isize {
        match self.glyph_for(character) {
            Some(info) => info.2,
            None => 0,
        }
    }

    fn draw_char(
        &self,
        character: char,
        bitmap: &mut Bitmap,
        origin: Point,
        height: isize,
        color: SomeColor,
    ) {
        let (base, last, width) = match self.glyph_for(character) {
            Some(info) => info,
            None => return,
        };
        let data = &self.data[base..last];
        self.draw_data(data, bitmap, origin, width, height, color);
    }
}
