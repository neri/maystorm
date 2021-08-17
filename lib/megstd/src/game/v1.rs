//! Retro Game Framework v1
//!
//! This framework provides functionality similar to the screen display of retro games.
//!
//! # Restrictions
//!
//! * Frame size is within 256 x 240 pixels.
//! * The number of patterns is up to 256.
//! * The number of colors is up to 24? of the true colors.

use crate::drawing::*;
use num_derive::FromPrimitive;

pub type PatternIndex = u8;
pub type PaletteEntry = PackedColor;
pub const CHAR_SIZE: isize = 8;
pub const MAX_WIDTH: isize = 256;
pub const MAX_HEIGHT: isize = 240;
pub const MAX_VWIDTH: usize = 256;
pub const MAX_VHEIGHT: usize = 256;
pub const STRIDE: usize = MAX_VWIDTH / CHAR_SIZE as usize;
pub const MAX_NAMES: usize = MAX_VWIDTH / (CHAR_SIZE as usize) * MAX_VHEIGHT / (CHAR_SIZE as usize);
pub const MAX_X: u8 = 255;
pub const MAX_Y: u8 = 239;
pub const SPRITE_DISABLED: u8 = 1 + MAX_Y;
pub const MAX_CHAR_DATA: usize = 256;
pub const MAX_SPRITES: usize = 256;
pub const MAX_PALETTES: usize = 32;

pub const CHAR_DATA_LEN: usize = 16;
pub type CharData = [u8; CHAR_DATA_LEN];
pub const CHAR_DATA_EMPTY: CharData = [0u8; CHAR_DATA_LEN];

/// If set, the width of the sprite is 16, otherwise it is 8
pub const OAM_ATTR_W16: u8 = 0b0001_0000;
/// If set, the sprite height is 16, otherwise it is 8
pub const OAM_ATTR_H16: u8 = 0b0010_0000;
/// If set, the left and right sides of the sprite will be flipped.
pub const OAM_ATTR_FLIP_X: u8 = 0b0100_0000;
/// If set, the top and bottom of the sprite will be flipped.
pub const OAM_ATTR_FLIP_Y: u8 = 0b1000_0000;
pub const OAM_ATTR_FLIP_XY: u8 = OAM_ATTR_FLIP_X | OAM_ATTR_FLIP_Y;
pub const OAM_PALETTE_MASK: u8 = 0b0000_0111;

/// Retro Game Presenter
pub trait GamePresenter {
    /// Gets the reference of the screen object
    fn screen<'a>(&'a self) -> &'a mut Screen;
    /// Transfers the drawing buffer to the window and synchronizes the frames.
    fn sync(&self) -> bool;
    /// Transfers the drawing buffer to the window if needed.
    fn display_if_needed(&self);
    /// Redraws the entire screen buffer.
    fn set_needs_display(&self);
    /// Redraws the drawing buffer of the specified range.
    fn invalidate_rect(&self, rect: Rect);
    /// Moves the sprite and redraw it.
    fn move_sprite(&self, index: PatternIndex, origin: Point);
    /// Loads the font data from `start_char` to `end_char` into the character data from `start_index`.
    fn load_font(&self, start_index: u8, start_char: u8, end_char: u8);
    /// Gets the status of some buttons for the game.
    fn buttons(&self) -> u8;

    fn dispatch_buttons<F>(&self, mut f: F)
    where
        F: FnMut(JoyPad),
    {
        use JoyPad::*;
        let buttons = self.buttons();
        for button in &[
            DpadRight, DpadLeft, DpadDown, DpadUp, Start, Select, Fire1, Fire2,
        ] {
            if (buttons & (1u8 << *button as usize)) != 0 {
                f(*button);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum JoyPad {
    DpadRight = 0,
    DpadLeft,
    DpadDown,
    DpadUp,
    Start,
    Select,
    Fire1,
    Fire2,
}

pub const DPAD_RIGHT: u8 = 1u8 << JoyPad::DpadRight as u8;
pub const DPAD_LEFT: u8 = 1u8 << JoyPad::DpadLeft as u8;
pub const DPAD_DOWN: u8 = 1u8 << JoyPad::DpadDown as u8;
pub const DPAD_UP: u8 = 1u8 << JoyPad::DpadUp as u8;
pub const JOYPAD_START: u8 = 1u8 << JoyPad::Start as u8;
pub const JOYPAD_SELECT: u8 = 1u8 << JoyPad::Select as u8;
pub const JOYPAD_FIRE_1: u8 = 1u8 << JoyPad::Fire1 as u8;
pub const JOYPAD_FIRE_2: u8 = 1u8 << JoyPad::Fire2 as u8;

/// An object that mimics the screen of a retro game.
///
/// When you change the content of this object directly, you need to notify the GamePresenter of the change.
#[repr(C)]
pub struct Screen {
    // 256 x 16 = 4096bytes
    patterns: [CharData; MAX_CHAR_DATA],
    // 1 x 32 x 32 = 1024 bytes?
    name_table: [NameTableEntry; MAX_NAMES],
    // 4 x 256 = 1024 bytes?
    oam: [Sprite; MAX_SPRITES],
    // 4 x 64 = 256 bytes?
    palettes: [PaletteEntry; MAX_PALETTES],
    // control registers
    control: Control,
}

impl Screen {
    #[inline]
    pub const fn new() -> Self {
        Self {
            patterns: [CHAR_DATA_EMPTY; MAX_CHAR_DATA],
            name_table: [NameTableEntry::empty(); MAX_NAMES],
            oam: [Sprite::empty(); MAX_SPRITES],
            palettes: [PackedColor(0); MAX_PALETTES],
            control: Control::new(),
        }
    }

    #[inline]
    pub const fn char_data(&self) -> &[CharData; MAX_CHAR_DATA] {
        &self.patterns
    }

    #[inline]
    pub const fn char_data_mut(&mut self) -> &mut [CharData; MAX_CHAR_DATA] {
        &mut self.patterns
    }

    #[inline]
    pub fn get_char_data(&self, index: PatternIndex) -> &CharData {
        unsafe { self.patterns.get_unchecked(index as usize) }
    }

    #[inline]
    pub fn set_char_data(&mut self, index: PatternIndex, data: &CharData) {
        let p = unsafe { self.patterns.get_unchecked_mut(index as usize) };
        p.copy_from_slice(data);
    }

    #[inline]
    pub const fn name_table(&self) -> &[NameTableEntry; MAX_NAMES] {
        &self.name_table
    }

    #[inline]
    pub const fn name_table_mut(&mut self) -> &mut [NameTableEntry; MAX_NAMES] {
        &mut self.name_table
    }

    #[inline]
    pub fn get_name(&self, x: isize, y: isize) -> NameTableEntry {
        let index = (y as usize * STRIDE + x as usize) & (MAX_NAMES - 1);
        unsafe { *self.name_table.get_unchecked(index) }
    }

    #[inline]
    pub fn set_name(&mut self, x: isize, y: isize, value: NameTableEntry) {
        let index = (y as usize * STRIDE + x as usize) & (MAX_NAMES - 1);
        unsafe {
            *self.name_table.get_unchecked_mut(index) = value;
        }
    }

    #[inline]
    pub fn fill_all(&mut self, value: NameTableEntry) {
        for y in 0..MAX_VHEIGHT {
            for x in 0..MAX_VWIDTH {
                self.set_name(x as isize, y as isize, value);
            }
        }
    }

    #[inline]
    pub fn fill_names(&mut self, rect: Rect, value: NameTableEntry) {
        for y in rect.min_y()..rect.max_y() {
            for x in rect.min_x()..rect.max_x() {
                self.set_name(x, y, value);
            }
        }
    }

    #[inline]
    pub fn draw_string(&mut self, origin: Point, str: &[u8]) {
        for (index, byte) in str.iter().enumerate() {
            self.set_name(
                origin.x + index as isize,
                origin.y,
                NameTableEntry::from_index(*byte),
            );
        }
    }

    #[inline]
    pub const fn palettes(&self) -> &[PaletteEntry; MAX_PALETTES] {
        &self.palettes
    }

    #[inline]
    pub const fn palettes_mut(&mut self) -> &mut [PaletteEntry; MAX_PALETTES] {
        &mut self.palettes
    }

    #[inline]
    pub fn get_palette(&self, index: usize) -> &PaletteEntry {
        unsafe { self.palettes.get_unchecked(index & (MAX_PALETTES - 1)) }
    }

    #[inline]
    fn get_palette_mut(&mut self, index: usize) -> &mut PaletteEntry {
        unsafe { self.palettes.get_unchecked_mut(index & (MAX_PALETTES - 1)) }
    }

    #[inline]
    pub fn set_palette(&mut self, index: usize, color: PackedColor) {
        *self.get_palette_mut(index) = color;
    }

    #[inline]
    pub const fn sprites(&self) -> &[Sprite; MAX_SPRITES] {
        &self.oam
    }

    #[inline]
    pub const fn sprites_mut(&mut self) -> &mut [Sprite; MAX_SPRITES] {
        &mut self.oam
    }

    #[inline]
    pub fn get_sprite(&self, index: usize) -> Sprite {
        unsafe { *self.oam.get_unchecked(index & (MAX_SPRITES - 1)) }
    }

    #[inline]
    pub fn get_sprite_mut(&mut self, index: usize) -> &mut Sprite {
        unsafe { &mut *self.oam.get_unchecked_mut(index & (MAX_SPRITES - 1)) }
    }

    #[inline]
    pub const fn control(&self) -> &Control {
        &self.control
    }

    #[inline]
    pub const fn control_mut(&mut self) -> &mut Control {
        &mut self.control
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct NameTableEntry(u8);

impl NameTableEntry {
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn from_index(index: PatternIndex) -> Self {
        Self(index)
    }

    #[inline]
    pub const fn index(&self) -> PatternIndex {
        self.0 as PatternIndex
    }
}

/// Object Attribute Memory (Sprite)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Sprite {
    pub y: u8,
    pub x: u8,
    pub index: PatternIndex,
    pub attr: u8,
}

impl Sprite {
    #[inline]
    pub const fn new(origin: Point, index: PatternIndex, attr: u8) -> Self {
        Self {
            x: origin.x as u8,
            y: origin.y as u8,
            index,
            attr,
        }
    }

    #[inline]
    pub const fn empty() -> Self {
        Self {
            x: 0,
            y: 0,
            index: 0,
            attr: 0,
        }
    }

    #[inline]
    pub const fn x(&self) -> isize {
        self.x as isize
    }

    #[inline]
    pub const fn y(&self) -> isize {
        self.y as isize
    }

    #[inline]
    pub const fn width(&self) -> isize {
        if (self.attr & OAM_ATTR_W16) != 0 {
            CHAR_SIZE * 2
        } else {
            CHAR_SIZE
        }
    }

    #[inline]
    pub const fn height(&self) -> isize {
        if (self.attr & OAM_ATTR_H16) != 0 {
            CHAR_SIZE * 2
        } else {
            CHAR_SIZE
        }
    }

    #[inline]
    pub const fn frame(&self) -> Rect {
        Rect::new(self.x(), self.y(), self.width(), self.height())
    }

    #[inline]
    pub const fn size(&self) -> Size {
        Size::new(self.width(), self.height())
    }

    #[inline]
    pub const fn palette(&self) -> usize {
        (self.attr & OAM_PALETTE_MASK) as usize
    }

    #[inline]
    pub const fn index(&self) -> PatternIndex {
        self.index
    }

    #[inline]
    pub const fn is_visible(&self) -> bool {
        !self.is_gone()
    }

    #[inline]
    pub const fn is_gone(&self) -> bool {
        self.y >= SPRITE_DISABLED
    }

    #[inline]
    pub fn gone(&mut self) {
        self.y = SPRITE_DISABLED;
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum ScaleMode {
    DotByDot,
    Sparse2X,
    NearestNeighbor2X,
}

impl ScaleMode {
    #[inline]
    pub const fn scale_factor(&self) -> usize {
        match self {
            ScaleMode::DotByDot => 1,
            ScaleMode::Sparse2X | ScaleMode::NearestNeighbor2X => 2,
        }
    }
}

/// Screen control registers
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Control {
    pub control: u32,
    pub scroll_x: u8,
    pub scroll_y: u8,
}

impl Control {
    #[inline]
    pub const fn new() -> Self {
        Self {
            control: 0,
            scroll_x: 0,
            scroll_y: 0,
        }
    }

    #[inline]
    pub const fn get_scroll(&self) -> Point {
        Point::new(self.scroll_x as isize, self.scroll_y as isize)
    }

    #[inline]
    pub fn set_scroll(&mut self, point: Point) {
        self.scroll_x = point.x as u8;
        self.scroll_y = point.y as u8;
    }
}
