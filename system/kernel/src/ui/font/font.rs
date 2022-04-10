use crate::{fs::FileManager, sync::Mutex, *};
use alloc::{boxed::Box, collections::BTreeMap, vec::*};
use core::{cell::UnsafeCell, mem::MaybeUninit};
use megstd::{drawing::*, io::Read};

use ab_glyph::{self, Font as AbFont};

#[allow(dead_code)]
mod embedded {
    include!("megh0816.rs");
    // include!("megh0810.rs");
    // include!("megg0808.rs");
    include!("megh0710.rs");
    include!("megh0608.rs");
}
const SYSTEM_FONT: FixedFontDriver = FixedFontDriver::new(8, 16, &embedded::FONT_MEGH0816_DATA);
// const TERMINAL_FONT: FixedFontDriver = FixedFontDriver::new(8, 8, &embedded::FONT_MEGG0808_DATA);
const TERMINAL_FONT: FixedFontDriver = FixedFontDriver::new(7, 10, &embedded::FONT_MEGH0710_DATA);
// const TERMINAL_FONT: FixedFontDriver = FixedFontDriver::new(8, 10, &embedded::FONT_MEGH0810_DATA);
const SMALL_FONT: FixedFontDriver = FixedFontDriver::new(6, 8, &embedded::FONT_MEGH0608_DATA);

static mut FONT_MANAGER: UnsafeCell<FontManager> = UnsafeCell::new(FontManager::new());

pub struct FontManager {
    fonts: BTreeMap<FontFamily, Box<dyn FontDriver>>,
    buffer: Mutex<OperationalBitmap>,

    monospace_font: MaybeUninit<FontDescriptor>,
    title_font: MaybeUninit<FontDescriptor>,
    ui_font: MaybeUninit<FontDescriptor>,
}

impl FontManager {
    const fn new() -> Self {
        Self {
            fonts: BTreeMap::new(),
            buffer: Mutex::new(OperationalBitmap::new(Size::new(96, 96))),
            monospace_font: MaybeUninit::uninit(),
            title_font: MaybeUninit::uninit(),
            ui_font: MaybeUninit::uninit(),
        }
    }

    #[inline]
    fn shared<'a>() -> &'a Self {
        unsafe { &*FONT_MANAGER.get() }
    }

    #[inline]
    unsafe fn shared_mut<'a>() -> &'a mut Self {
        FONT_MANAGER.get_mut()
    }

    pub unsafe fn init() {
        let shared = Self::shared_mut();

        shared
            .fonts
            .insert(FontFamily::FixedSystem, Box::new(SYSTEM_FONT));
        shared
            .fonts
            .insert(FontFamily::SmallFixed, Box::new(SMALL_FONT));
        shared
            .fonts
            .insert(FontFamily::Terminal, Box::new(TERMINAL_FONT));

        let font = Box::new(HersheyFont::new(
            4,
            include_bytes!("../../../../../ext/hershey/cursive.jhf"),
        ));
        shared.fonts.insert(FontFamily::Cursive, font);

        if let Ok(mut file) = FileManager::open("/megos/fonts/mono.ttf") {
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            let font = Box::new(TrueTypeFont::new(data));
            shared.fonts.insert(FontFamily::Monospace, font);
        }

        if let Ok(mut file) = FileManager::open("/megos/fonts/sans.ttf") {
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            let font = Box::new(TrueTypeFont::new(data));
            shared.fonts.insert(FontFamily::SansSerif, font);
        } else {
            let font = Box::new(HersheyFont::new(
                0,
                include_bytes!("../../../../../ext/hershey/futuram.jhf"),
            ));
            shared.fonts.insert(FontFamily::SansSerif, font);
        }

        if let Ok(mut file) = FileManager::open("/megos/fonts/serif.ttf") {
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            let font = Box::new(TrueTypeFont::new(data));
            shared.fonts.insert(FontFamily::Serif, font);
        } else {
            let font = Box::new(HersheyFont::new(
                0,
                include_bytes!("../../../../../ext/hershey/timesrb.jhf"),
            ));
            shared.fonts.insert(FontFamily::Serif, font);
        }

        shared.monospace_font.write(
            FontDescriptor::new(FontFamily::Monospace, 14)
                .unwrap_or(FontDescriptor::new(FontFamily::FixedSystem, 0).unwrap()),
        );

        shared.ui_font.write(
            FontDescriptor::new(FontFamily::SansSerif, 16).unwrap_or(Self::monospace_font()),
        );

        shared
            .title_font
            .write(FontDescriptor::new(FontFamily::SansSerif, 16).unwrap_or(Self::ui_font()));
    }

    fn driver_for(family: FontFamily) -> Option<&'static dyn FontDriver> {
        let shared = Self::shared();
        shared.fonts.get(&family).map(|v| v.as_ref())
    }

    #[inline]
    pub const fn fixed_system_font() -> &'static FixedFontDriver<'static> {
        &SYSTEM_FONT
    }

    #[inline]
    pub const fn preferred_console_font() -> &'static FixedFontDriver<'static> {
        &SYSTEM_FONT
    }

    #[inline]
    pub fn monospace_font() -> FontDescriptor {
        unsafe { Self::shared().monospace_font.assume_init() }
    }

    #[inline]
    #[track_caller]
    pub fn ui_font() -> FontDescriptor {
        unsafe { Self::shared().ui_font.assume_init() }
    }

    #[inline]
    #[track_caller]
    pub fn title_font() -> FontDescriptor {
        unsafe { Self::shared().title_font.assume_init() }
    }
}

#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FontFamily {
    SansSerif,
    Serif,
    Cursive,
    Monospace,
    FixedSystem,
    Terminal,
    SmallFixed,
}

#[derive(Copy, Clone)]
pub struct FontDescriptor {
    driver: &'static dyn FontDriver,
    point: i32,
    line_height: i32,
    em_width: i32,
}

impl FontDescriptor {
    pub fn new(family: FontFamily, point: isize) -> Option<Self> {
        FontManager::driver_for(family).map(|driver| {
            if driver.is_scalable() {
                Self {
                    driver,
                    point: point as i32,
                    line_height: ((driver.preferred_line_height() * point
                        + driver.base_height() / 2)
                        / driver.base_height()) as i32,
                    em_width: ((driver.width_of('M') * point + driver.base_height() / 2)
                        / driver.base_height()) as i32,
                }
            } else {
                Self {
                    driver,
                    point: driver.base_height() as i32,
                    line_height: driver.preferred_line_height() as i32,
                    em_width: driver.width_of('M') as i32,
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
    pub const fn em_width(&self) -> isize {
        self.em_width as isize
    }

    #[inline]
    pub fn width_of(&self, character: char) -> isize {
        if self.point() == self.driver.base_height() {
            self.driver.width_of(character)
        } else {
            (self.driver.width_of(character) * self.point() + self.driver.base_height() / 2)
                / self.driver.base_height()
        }
    }

    #[inline]
    pub fn is_scalable(&self) -> bool {
        self.driver.is_scalable()
    }

    #[inline]
    pub fn draw_char(&self, character: char, bitmap: &mut Bitmap, origin: Point, color: Color) {
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
        color: Color,
    );
}

pub struct FixedFontDriver<'a> {
    size: Size,
    data: &'a [u8],
    fix_y: isize,
    line_height: isize,
    stride: usize,
}

impl FixedFontDriver<'_> {
    pub const fn new(width: usize, height: usize, data: &'static [u8]) -> FixedFontDriver<'static> {
        let width = width as isize;
        let height = height as isize;
        let line_height = height * 5 / 4;
        let fix_y = (line_height - height) / 2;
        let stride = ((width as usize + 7) >> 3) * height as usize;
        FixedFontDriver {
            size: Size::new(width, height),
            fix_y,
            line_height,
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
    fn width_of(&self, _character: char) -> isize {
        self.size.width
    }

    fn draw_char(
        &self,
        character: char,
        bitmap: &mut Bitmap,
        origin: Point,
        _height: isize,
        color: Color,
    ) {
        if let Some(font) = self.glyph_for(character) {
            let origin = Point::new(origin.x, origin.y + self.fix_y);
            let size = Size::new(self.width_of(character), self.size.height());
            bitmap.draw_font(font, size, origin, color);
        }
    }
}

#[allow(dead_code)]
struct HersheyFont<'a> {
    data: &'a [u8],
    fix_y: isize,
    line_height: isize,
    glyph_info: Vec<(usize, usize, isize)>,
}

impl<'a> HersheyFont<'a> {
    const MAGIC_20: isize = 0x20;
    const MAGIC_52: isize = 0x52;
    const DESCENT: isize = 4;
    const POINT: isize = 32;

    fn new(extra_height: isize, font_data: &'a [u8]) -> Self {
        let descent = Self::DESCENT + extra_height;
        let fix_y = descent / 2;
        let mut font = Self {
            data: font_data,
            fix_y,
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
        color: Color,
    ) {
        if data.len() >= 12 {
            let origin = origin + Point::new(0, self.fix_y * height / Self::POINT);
            let mut buffer = FontManager::shared().buffer.lock().unwrap();
            buffer.reset();

            let master_scale = if height < Self::POINT / 2 { 2 } else { 1 };
            let extra_weight = if height / master_scale <= Self::POINT / 2 {
                0
            } else {
                usize::min(
                    255,
                    ((master_scale * height - Self::POINT / 2) * 256 / Self::POINT) as usize,
                )
            };
            let n_pairs = (data[6] & 0x0F) * 10 + (data[7] & 0x0F);
            let left = data[8] as isize - Self::MAGIC_52;

            let border_bounds = buffer.bounds().insets_by(EdgeInsets::padding_each(1));
            let quality = 64;
            let center1 = Point::new(buffer.size().width() / 2, buffer.size().height() / 2);
            let mut cursor = 10;
            let mut c1: Option<Point> = None;
            let center2 = Point::new(center1.x * quality, center1.y * quality);
            let mut min_x = center2.x();
            let mut max_x = center2.x();
            let mut min_y = center2.y();
            let mut max_y = center2.y();

            for _ in 1..n_pairs {
                let p1 = unsafe { *data.get_unchecked(cursor) as isize };
                let p2 = unsafe { *data.get_unchecked(cursor + 1) as isize };
                cursor += 2;

                if p1 == Self::MAGIC_20 && p2 == Self::MAGIC_52 {
                    c1 = None;
                } else {
                    let d1 = p1 - Self::MAGIC_52;
                    let d2 = p2 - Self::MAGIC_52;
                    let c2 = center2
                        + Point::new(
                            d1 * quality * height * master_scale / Self::POINT,
                            d2 * quality * height * master_scale / Self::POINT,
                        );
                    if let Some(c1) = c1 {
                        min_x = isize::min(min_x, isize::min(c1.x(), c2.x()));
                        max_x = isize::max(max_x, isize::max(c1.x(), c2.x()));
                        min_y = isize::min(min_y, isize::min(c1.y(), c2.y()));
                        max_y = isize::max(max_y, isize::max(c1.y(), c2.y()));

                        if extra_weight > 0 {
                            buffer.draw_line_anti_aliasing(
                                c1,
                                c2,
                                quality,
                                |bitmap, point, value| {
                                    if border_bounds.contains(point) {
                                        unsafe {
                                            bitmap.process_pixel_unchecked(point, |v| {
                                                v.saturating_add(value)
                                            });
                                            bitmap.process_pixel_unchecked(
                                                point + Point::new(0, -1),
                                                |v| {
                                                    v.saturating_add(
                                                        (value as usize * extra_weight / 256) as u8,
                                                    )
                                                },
                                            );
                                            bitmap.process_pixel_unchecked(
                                                point + Point::new(-1, 0),
                                                |v| {
                                                    v.saturating_add(
                                                        (value as usize * extra_weight / 256) as u8,
                                                    )
                                                },
                                            );
                                            bitmap.process_pixel_unchecked(
                                                point + Point::new(1, 0),
                                                |v| {
                                                    v.saturating_add(
                                                        (value as usize * extra_weight / 256) as u8,
                                                    )
                                                },
                                            );
                                            bitmap.process_pixel_unchecked(
                                                point + Point::new(0, 1),
                                                |v| {
                                                    v.saturating_add(
                                                        (value as usize * extra_weight / 256) as u8,
                                                    )
                                                },
                                            );
                                        }
                                    }
                                },
                            );
                        } else {
                            buffer.draw_line_anti_aliasing(
                                c1,
                                c2,
                                quality,
                                |bitmap, point, value| {
                                    if border_bounds.contains(point) {
                                        unsafe {
                                            bitmap.process_pixel_unchecked(point, |v| {
                                                v.saturating_add(value)
                                            });
                                        }
                                    }
                                },
                            );
                        }
                    }
                    c1 = Some(c2);
                }
            }

            let extra_offset = if extra_weight > 0 {
                min_x -= quality;
                max_x += quality;
                1
            } else {
                0
            };
            let box_w = width * height / Self::POINT;
            let act_w = ((max_x - min_x + quality) / quality + master_scale) / master_scale;
            let act_h = self.line_height * height / Self::POINT;
            let offset_x = min_x / quality;
            let offset_y = center1.y - (height / 2) * master_scale;
            let offset_box_x =
                center1.x - (-left * height) * master_scale / Self::POINT + extra_offset;
            let offset_act_x = offset_x - offset_box_x;

            if master_scale > 1 {
                let buf_w = (max_x - min_x + quality) / quality;
                // let buf_w = width * height * master_scale / Self::POINT;
                let buf_h = self.line_height * height * master_scale / Self::POINT;

                for y in (0..buf_h + master_scale - 1).step_by(master_scale as usize) {
                    for x in (0..buf_w + master_scale - 1).step_by(master_scale as usize) {
                        let mut acc = 0;
                        for y0 in 0..master_scale {
                            for x0 in 0..master_scale {
                                acc += unsafe {
                                    buffer.get_pixel_unchecked(Point::new(
                                        offset_x + x + x0,
                                        offset_y + y + y0,
                                    )) as isize
                                };
                            }
                        }
                        unsafe {
                            buffer.set_pixel_unchecked(
                                Point::new(
                                    offset_x + x / master_scale,
                                    offset_y + y / master_scale,
                                ),
                                (acc / (master_scale * master_scale)) as u8,
                            );
                        }
                    }
                }
            }

            // DEBUG
            if false {
                let rect = Rect::new(origin.x, origin.y, box_w, act_h);
                bitmap.draw_rect(rect, Color::from_argb(0x80FF8888));
                let rect = Rect::new(origin.x + offset_act_x, origin.y, act_w, act_h);
                bitmap.draw_rect(rect, Color::from_argb(0xC08888FF));
                bitmap.draw_hline(
                    Point::new(origin.x, origin.y + height - 1),
                    box_w,
                    Color::from_rgb(0xFFFF33),
                );
                bitmap.draw_hline(
                    Point::new(origin.x, origin.y + height * 3 / 4),
                    box_w,
                    Color::from_rgb(0xFF3333),
                );
            }

            {
                let origin = origin + Point::new(offset_act_x, 0);
                let rect = Rect::new(offset_x, offset_y, act_w, act_h);
                buffer.draw_to(bitmap, origin, rect, color);
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
        color: Color,
    ) {
        let (base, last, width) = match self.glyph_for(character) {
            Some(info) => info,
            None => return,
        };
        let data = &self.data[base..last];
        self.draw_data(data, bitmap, origin, width, height, color);
    }
}

pub struct TrueTypeFont {
    font: ab_glyph::FontVec,
    line_height: isize,
    units_per_em: f32,
}

impl TrueTypeFont {
    const BASE_HEIGHT: isize = 256;

    #[inline]
    pub fn new(font_data: Vec<u8>) -> Self {
        let font = ab_glyph::FontVec::try_from_vec(font_data).unwrap();
        let units_per_em = font.units_per_em().unwrap();
        let line_height = (Self::BASE_HEIGHT as f32
            * (font.ascent_unscaled() - font.descent_unscaled() + font.line_gap_unscaled())
            / units_per_em) as isize;

        Self {
            font,
            units_per_em,
            line_height,
        }
    }
}

impl FontDriver for TrueTypeFont {
    fn is_scalable(&self) -> bool {
        true
    }

    fn base_height(&self) -> isize {
        Self::BASE_HEIGHT
    }

    fn preferred_line_height(&self) -> isize {
        self.line_height
    }

    fn width_of(&self, character: char) -> isize {
        let glyph_id = self.font.glyph_id(character);
        (self.font.h_advance_unscaled(glyph_id) * Self::BASE_HEIGHT as f32 / self.units_per_em)
            as isize
    }

    fn draw_char(
        &self,
        character: char,
        bitmap: &mut Bitmap,
        origin: Point,
        height: isize,
        color: Color,
    ) {
        let scale = height as f32 * self.font.height_unscaled() / self.units_per_em;
        let ascent = (height as f32 * self.font.ascent_unscaled() / self.units_per_em) as isize;
        // let descent = (height as f32 * self.font.descent_unscaled() / self.units_per_em) as isize;
        let glyph = self.font.glyph_id(character).with_scale(scale);
        self.font.outline_glyph(glyph).map(|glyph| {
            let bounds = glyph.px_bounds();

            // debug
            // if false {
            //     bitmap.draw_rect(
            //         Rect::new(
            //             origin.x + bounds.min.x as isize,
            //             origin.y,
            //             bounds.width() as isize,
            //             ascent - descent,
            //         ),
            //         Color::LIGHT_RED,
            //     );
            //     bitmap.draw_hline(
            //         origin + Point::new(bounds.min.x as isize, isize::min(ascent, ascent)),
            //         bounds.width() as isize,
            //         Color::BLUE,
            //     );
            // }

            let origin = origin + Point::new(bounds.min.x as isize, ascent + bounds.min.y as isize);
            let color = color.into_true_color();
            glyph.draw(|x, y, a| {
                let point = origin + Point::new(x as isize, y as isize);
                if let Some(b) = bitmap.get_pixel(point) {
                    let b = b.into_true_color();
                    let mut c = color.components();
                    c.a = (a * 255.0) as u8;
                    unsafe {
                        bitmap.set_pixel_unchecked(point, b.blend_draw(c.into()).into());
                    }
                }
            })
        });
    }
}
