use crate::{fs::FileManager, *};
use ab_glyph::{self, Font as AbFont};
use alloc::collections::BTreeMap;
use core::{cell::UnsafeCell, mem::MaybeUninit};
use megstd::{drawing::*, io::Read, Arc, Vec};

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
    fonts: BTreeMap<FontFamily, Arc<dyn FontDriver>>,
    monospace_font: MaybeUninit<FontDescriptor>,
    title_font: MaybeUninit<FontDescriptor>,
    ui_font: MaybeUninit<FontDescriptor>,
}

impl FontManager {
    const fn new() -> Self {
        Self {
            fonts: BTreeMap::new(),
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
        assert_call_once!();

        let shared = Self::shared_mut();

        let fonts = &mut shared.fonts;

        fonts.insert(FontFamily::FixedSystem, Arc::new(SYSTEM_FONT));
        fonts.insert(FontFamily::SmallFixed, Arc::new(SMALL_FONT));
        fonts.insert(FontFamily::Terminal, Arc::new(TERMINAL_FONT));

        if let Ok(mut file) = FileManager::open("/megos/fonts/mono.ttf") {
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            let font = Arc::new(TrueTypeFont::new(data).unwrap());
            fonts.insert(FontFamily::Monospace, font);
        }

        if let Ok(mut file) = FileManager::open("/megos/fonts/sans.ttf") {
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            let font = Arc::new(TrueTypeFont::new(data).unwrap());
            fonts.insert(FontFamily::SansSerif, font);
        }

        if let Ok(mut file) = FileManager::open("/megos/fonts/serif.ttf") {
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            let font = Arc::new(TrueTypeFont::new(data).unwrap());
            fonts.insert(FontFamily::Serif, font);
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

    fn driver_for(family: FontFamily) -> Option<Arc<dyn FontDriver>> {
        let shared = Self::shared();
        shared.fonts.get(&family).map(|v| v.clone())
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
        unsafe { Self::shared().monospace_font.assume_init_ref().clone() }
    }

    #[inline]
    #[track_caller]
    pub fn ui_font() -> FontDescriptor {
        unsafe { Self::shared().ui_font.assume_init_ref().clone() }
    }

    #[inline]
    #[track_caller]
    pub fn title_font() -> FontDescriptor {
        unsafe { Self::shared().title_font.assume_init_ref().clone() }
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

#[derive(Clone)]
pub struct FontDescriptor {
    driver: Arc<dyn FontDriver>,
    point: i32,
    line_height: i32,
    em_width: i32,
}

impl FontDescriptor {
    pub fn new(family: FontFamily, point: isize) -> Option<Self> {
        FontManager::driver_for(family).map(|driver| {
            if driver.is_scalable() {
                Self {
                    driver: driver.clone(),
                    point: point as i32,
                    line_height: ((driver.preferred_line_height() * point
                        + driver.base_height() / 2)
                        / driver.base_height()) as i32,
                    em_width: ((driver.width_of('M') * point + driver.base_height() / 2)
                        / driver.base_height()) as i32,
                }
            } else {
                Self {
                    driver: driver.clone(),
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
    pub fn kern(&self, first: char, second: char) -> isize {
        if self.point() == self.driver.base_height() {
            self.driver.kern(first, second)
        } else {
            (self.driver.kern(first, second) * self.point() + self.driver.base_height() / 2)
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

    fn kern(&self, first: char, second: char) -> isize;

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
    offset: Movement,
    line_height: isize,
    stride: usize,
}

impl FixedFontDriver<'_> {
    pub const fn new(width: usize, height: usize, data: &'static [u8]) -> FixedFontDriver<'static> {
        let width = width as isize;
        let height = height as isize;
        let line_height = height * 5 / 4;
        let offset = Movement::new(0, (line_height - height) / 2);
        let stride = ((width as usize + 7) >> 3) * height as usize;
        FixedFontDriver {
            size: Size::new(width, height),
            offset,
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
    #[inline]
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
    pub fn draw_glyph<F>(&self, character: char, origin: Point, f: F)
    where
        F: FnOnce(&[u8], Size, Point),
    {
        if let Some(glyph) = self.glyph_for(character) {
            f(
                glyph,
                Size::new(self.width_of(character), self.size.height()),
                origin + self.offset,
            );
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

    fn kern(&self, _first: char, _second: char) -> isize {
        0
    }

    fn draw_char(
        &self,
        character: char,
        bitmap: &mut Bitmap,
        origin: Point,
        _height: isize,
        color: Color,
    ) {
        self.draw_glyph(character, origin, |glyph, size, origin| {
            bitmap.draw_glyph(glyph, size, origin, color)
        })
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
    pub fn new(font_data: Vec<u8>) -> Option<Self> {
        let Ok(font) = ab_glyph::FontVec::try_from_vec(font_data) else {
            return None
        };
        let units_per_em = font.units_per_em().unwrap();
        let line_height = (Self::BASE_HEIGHT as f32
            * (font.ascent_unscaled() - font.descent_unscaled() + font.line_gap_unscaled())
            / units_per_em) as isize;

        Some(Self {
            font,
            units_per_em,
            line_height,
        })
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

    fn kern(&self, first: char, second: char) -> isize {
        (self
            .font
            .kern_unscaled(self.font.glyph_id(first), self.font.glyph_id(second))
            * Self::BASE_HEIGHT as f32
            / self.units_per_em) as isize
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

            let origin =
                origin + Movement::new(bounds.min.x as isize, ascent + bounds.min.y as isize);
            let color = color.into_true_color();
            glyph.draw(|x, y, a| {
                let point = origin + Movement::new(x as isize, y as isize);
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
