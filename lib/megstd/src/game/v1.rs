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
pub const CHAR_SIZE: isize = 8;
pub const MAX_WIDTH: isize = 256;
pub const STRIDE: usize = (MAX_WIDTH / CHAR_SIZE) as usize;
pub const MAX_HEIGHT: isize = 240;
pub const MAX_X: u8 = 255;
pub const MAX_Y: u8 = 239;
pub const SPRITE_DISABLED: u8 = 1 + MAX_Y;
pub const MAX_CHAR_DATA: usize = 256;
pub const MAX_NAMES: usize = 1024;
pub const MAX_SPRITES: usize = 64;
pub const MAX_PALETTES: usize = 32;

/// If set, the width of the sprite is 16, otherwise it is 8
pub const OAM_ATTR_W16: u8 = 0b0001_0000;
/// If set, the sprite height is 16, otherwise it is 8
pub const OAM_ATTR_H16: u8 = 0b0010_0000;
/// If set, the left and right sides of the sprite will be flipped.
pub const OAM_ATTR_FLIP_X: u8 = 0b0100_0000;
/// If set, the top and bottom of the sprite will be flipped.
pub const OAM_ATTR_FLIP_Y: u8 = 0b1000_0000;
pub const OAM_PALETTE_MASK: u8 = 0b0000_0111;

/// Retro Game Presenter
pub trait GamePresenter {
    /// Gets the reference of the screen object
    fn screen<'a>(&'a mut self) -> &'a mut Screen;
    /// Transfers the drawing buffer to the window and synchronizes the frames.
    fn sync(&mut self) -> bool;
    /// Redraws the entire screen buffer.
    fn set_needs_display(&mut self);
    /// Redraws the drawing buffer of the specified range.
    fn invalidate_rect(&mut self, rect: Rect);
    /// Transfer the drawing buffer to the window if needed.
    fn display_if_needed(&mut self);
    /// Move the sprite and redraw it.
    fn move_sprite(&mut self, index: PatternIndex, origin: Point);
}

/// An object that mimics the screen of a retro game.
///
/// When you change the content of this object directly, you need to notify the GamePresenter of the change.
#[repr(C)]
pub struct Screen {
    patterns: [PatternData; MAX_CHAR_DATA],
    name_table: [PatternIndex; MAX_NAMES],
    oam: [Sprite; MAX_SPRITES],
    palettes: [PaletteEntry; MAX_PALETTES],
}

impl Screen {
    #[inline]
    pub const fn new() -> Self {
        Self {
            patterns: [PatternData::new(); MAX_CHAR_DATA],
            name_table: [0; MAX_NAMES],
            oam: [Sprite::empty(); MAX_SPRITES],
            palettes: [PaletteEntry::empty(); MAX_PALETTES],
        }
    }

    #[inline]
    pub unsafe fn get_name(&self, x: isize, y: isize) -> PatternIndex {
        let index = y as usize * STRIDE + x as usize;
        *self.name_table.get_unchecked(index)
    }

    #[inline]
    pub unsafe fn get_name_mut(&mut self, x: isize, y: isize) -> &mut PatternIndex {
        let index = y as usize * STRIDE + x as usize;
        self.name_table.get_unchecked_mut(index)
    }

    #[inline]
    pub unsafe fn set_name(&mut self, x: isize, y: isize, value: PatternIndex) {
        *self.get_name_mut(x, y) = value;
    }

    #[inline]
    pub fn fill_names(&mut self, rect: Rect, value: PatternIndex) {
        for y in rect.min_y()..rect.max_y() {
            for x in rect.min_x()..rect.max_x() {
                unsafe {
                    *self.get_name_mut(x, y) = value;
                }
            }
        }
    }

    #[inline]
    pub const fn palettes(&self) -> &[PaletteEntry; MAX_PALETTES] {
        &self.palettes
    }

    #[inline]
    pub fn get_char_data(&self, index: PatternIndex) -> &PatternData {
        unsafe { self.patterns.get_unchecked(index as usize) }
    }

    #[inline]
    pub fn get_char_data_mut(&mut self, index: PatternIndex) -> &mut PatternData {
        unsafe { self.patterns.get_unchecked_mut(index as usize) }
    }

    #[inline]
    pub fn set_char_data(&mut self, index: PatternIndex, data: &[u8; PatternData::DATA_LEN]) {
        self.get_char_data_mut(index).copy_from(data);
    }

    #[inline]
    pub fn get_palette(&self, index: usize) -> &PaletteEntry {
        unsafe { self.palettes.get_unchecked(index & (MAX_PALETTES - 1)) }
    }

    #[inline]
    pub fn get_palette_mut(&mut self, index: usize) -> &mut PaletteEntry {
        unsafe { self.palettes.get_unchecked_mut(index & (MAX_PALETTES - 1)) }
    }

    #[inline]
    pub fn set_palette(&mut self, index: usize, color: PackedColor) {
        self.get_palette_mut(index).0 = color;
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
    pub const fn sprites(&self) -> &[Sprite; MAX_SPRITES] {
        &self.oam
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
            16
        } else {
            8
        }
    }

    #[inline]
    pub const fn height(&self) -> isize {
        if (self.attr & OAM_ATTR_H16) != 0 {
            16
        } else {
            8
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
    pub fn gone(&mut self) {
        self.y = SPRITE_DISABLED;
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PaletteEntry(pub PackedColor);

impl PaletteEntry {
    #[inline]
    pub const fn empty() -> Self {
        Self(PackedColor(0))
    }

    #[inline]
    pub const fn get_color(&self) -> Color {
        self.0.as_color()
    }

    #[inline]
    pub fn set_color(&mut self, color: Color) {
        self.0 = PackedColor::from_color(color);
    }
}

/// Object Pattern Data (TBD)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PatternData {
    data: [u8; Self::DATA_LEN],
}

impl PatternData {
    const DATA_LEN: usize = 16;

    #[inline]
    pub const fn new() -> Self {
        Self {
            data: [0; Self::DATA_LEN],
        }
    }

    #[inline]
    pub const fn data(&self) -> &[u8; Self::DATA_LEN] {
        &self.data
    }

    #[inline]
    pub fn clear(&mut self) {
        self.data.fill(0);
    }

    #[inline]
    pub fn copy_from(&mut self, data: &[u8; Self::DATA_LEN]) {
        self.data.copy_from_slice(data);
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
