//! MEG-OS Game Framework v0
//!
//! This framework provides functionality similar to the screen display of retro games.
//!

use crate::drawing::*;

pub type TileIndex = u8;
pub type SpriteIndex = u8;
pub type PaletteEntry = PackedColor;
pub const TILE_SIZE: isize = 8;
pub const MAX_WIDTH: isize = 256;
pub const MAX_HEIGHT: isize = 240;
pub const MAX_VWIDTH: usize = 256;
pub const MAX_VHEIGHT: usize = 256;
pub const STRIDE: usize = MAX_VWIDTH / TILE_SIZE as usize;
pub const MAX_NAMES: usize = MAX_VWIDTH / (TILE_SIZE as usize) * MAX_VHEIGHT / (TILE_SIZE as usize);
pub const MAX_X: u8 = 255;
pub const MAX_Y: u8 = 239;
pub const SPRITE_DISABLED: u8 = 1 + MAX_Y;
pub const MAX_TILE_DATA: usize = 256;
pub const MAX_SPRITES: usize = 256;
pub const MAX_PALETTES: usize = 64;

pub const TILE_DATA_LEN: usize = 16;
pub type TileData = [u8; TILE_DATA_LEN];
pub const TILE_DATA_EMPTY: TileData = [0u8; TILE_DATA_LEN];

pub const TILE_ATTR_FLIP_XY: u8 = TILE_ATTR_VFLIP | TILE_ATTR_HFLIP;
pub const TILE_ATTR_VFLIP: u8 = 0b1000_0000;
pub const TILE_ATTR_HFLIP: u8 = 0b0100_0000;
// pub const TILE_ATTR_xxxx: u8 = 0b0010_0000;
// pub const TILE_ATTR_xxxx: u8 = 0b0001_0000;
pub const TILE_ATTR_MASK: u8 = 0b1100_0111;
pub const TILE_ATTR_PAL_MASK: u8 = 0b0000_1111;

/// If set, the top and bottom of the sprite will be flipped.
pub const OAM_ATTR_VFLIP: u8 = TILE_ATTR_VFLIP;
/// If set, the left and right sides of the sprite will be flipped.
pub const OAM_ATTR_HFLIP: u8 = TILE_ATTR_HFLIP;
/// If set, the sprite height is 16, otherwise it is 8
pub const OAM_ATTR_H16: u8 = 0b0010_0000;
/// If set, the width of the sprite is 16, otherwise it is 8
pub const OAM_ATTR_W16: u8 = 0b0001_0000;
// pub const OAM_ATTR_xxxx: u8 = 0b0000_1000;
pub const OAM_PALETTE_MASK: u8 = 0b0000_0111;
pub const OAM_PALETTE_BASE: u8 = 0b0000_1000;
pub const OAM_DRAW_ATTR_MASK: u8 = 0b1100_0111;

pub const PALETTE_0: u8 = 0;
pub const PALETTE_1: u8 = 1;
pub const PALETTE_2: u8 = 2;
pub const PALETTE_3: u8 = 3;
pub const PALETTE_4: u8 = 4;
pub const PALETTE_5: u8 = 5;
pub const PALETTE_6: u8 = 6;
pub const PALETTE_7: u8 = 7;

#[cfg(feature = "game")]
pub mod prelude {
    pub use crate::sys::game_v0_imp::*;
}

/// An object that mimics the screen of a retro game.
///
/// You can change the contents of this object directly, but you need to notify GamePresenter in order for the changes to be displayed correctly.
#[repr(C)]
pub struct Screen {
    // 256 x 16 = 4096bytes
    tile_data: [TileData; MAX_TILE_DATA],
    // 2 x 32 x 32 = 2048 bytes
    name_table: [NameTableEntry; MAX_NAMES],
    // 4 x 256 = 1024 bytes
    sprites: [Sprite; MAX_SPRITES],
    // 4 x 64 = 256 bytes
    palettes: [PaletteEntry; MAX_PALETTES],
    // control registers
    control: Control,
}

/// Retro Game Presenter
pub trait GamePresenter {
    /// Gets the reference of the screen object
    fn screen<'a>(&'a self) -> &'a mut Screen;
    /// Redraws the buffer contents to the window and synchronizes the frames.
    /// Returns the number of skipped frames, if any.
    fn sync(&self) -> usize;
    /// Redraws the entire screen buffer.
    fn set_needs_display(&self);
    /// Redraws the drawing buffer of the specified range.
    fn invalidate_rect(&self, rect: Rect);
    /// Moves the sprite and redraw it.
    fn move_sprite(&self, index: SpriteIndex, origin: Point);
    /// Loads the system stock fonts from `start_char` to `end_char` into the tile data from `start_index`.
    fn load_font(&self, start_index: TileIndex, start_char: u8, end_char: u8);
    /// Gets the status of some buttons for the game.
    fn buttons(&self) -> u32;

    fn dispatch_buttons<F>(&self, mut f: F)
    where
        F: FnMut(JoyPad),
    {
        use JoyPad::*;
        let buttons = self.buttons();
        for button in &[
            DpadRight, DpadLeft, DpadDown, DpadUp, Start, Select, Fire1, Fire2, Menu,
        ] {
            if (buttons & (1u32 << *button as usize)) != 0 {
                f(*button);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum JoyPad {
    DpadUp = 0,
    DpadDown,
    DpadLeft,
    DpadRight,
    Start,
    Select,
    ThumbL,
    ThumbR,
    LButton,
    RButton,
    Menu,
    _Reserved,
    Fire1,
    Fire2,
    Fire3,
    Fire4,
}

pub const DPAD_RIGHT: u32 = 1u32 << JoyPad::DpadRight as u32;
pub const DPAD_LEFT: u32 = 1u32 << JoyPad::DpadLeft as u32;
pub const DPAD_DOWN: u32 = 1u32 << JoyPad::DpadDown as u32;
pub const DPAD_UP: u32 = 1u32 << JoyPad::DpadUp as u32;
pub const JOYPAD_START: u32 = 1u32 << JoyPad::Start as u32;
pub const JOYPAD_SELECT: u32 = 1u32 << JoyPad::Select as u32;
pub const JOYPAD_FIRE_1: u32 = 1u32 << JoyPad::Fire1 as u32;
pub const JOYPAD_FIRE_2: u32 = 1u32 << JoyPad::Fire2 as u32;

impl Screen {
    #[inline]
    pub const fn new() -> Self {
        Self {
            tile_data: [TILE_DATA_EMPTY; MAX_TILE_DATA],
            name_table: [NameTableEntry::empty(); MAX_NAMES],
            sprites: [Sprite::empty(); MAX_SPRITES],
            palettes: [PackedColor(0); MAX_PALETTES],
            control: Control::new(),
        }
    }

    #[inline]
    pub const fn tile_data(&self) -> &[TileData; MAX_TILE_DATA] {
        &self.tile_data
    }

    #[inline]
    pub const fn tile_data_mut(&mut self) -> &mut [TileData; MAX_TILE_DATA] {
        &mut self.tile_data
    }

    #[inline]
    pub fn get_tile_data(&self, index: TileIndex) -> &TileData {
        unsafe { self.tile_data.get_unchecked(index as usize) }
    }

    #[inline]
    pub fn set_tile_data(&mut self, index: TileIndex, data: &TileData) {
        let p = unsafe { self.tile_data.get_unchecked_mut(index as usize) };
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
    pub fn draw_string(&mut self, origin: Point, attr: u8, str: &[u8]) {
        for (index, byte) in str.iter().enumerate() {
            self.set_name(
                origin.x + index as isize,
                origin.y,
                NameTableEntry::new(*byte, attr),
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
        &self.sprites
    }

    #[inline]
    pub const fn sprites_mut(&mut self) -> &mut [Sprite; MAX_SPRITES] {
        &mut self.sprites
    }

    #[inline]
    pub fn get_sprite(&self, index: usize) -> Sprite {
        unsafe { *self.sprites.get_unchecked(index & (MAX_SPRITES - 1)) }
    }

    #[inline]
    pub fn get_sprite_mut(&mut self, index: usize) -> &mut Sprite {
        unsafe { &mut *self.sprites.get_unchecked_mut(index & (MAX_SPRITES - 1)) }
    }

    #[inline]
    pub fn set_sprite(&mut self, index: usize, tile_index: TileIndex, attr: u8) {
        *self.get_sprite_mut(index) = Sprite::new(Point::new(0, MAX_HEIGHT), tile_index, attr);
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
pub struct NameTableEntry(u16);

impl NameTableEntry {
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn from_index(index: TileIndex) -> Self {
        Self(index as u16)
    }

    #[inline]
    pub const fn new(index: TileIndex, attr: u8) -> Self {
        Self(index as u16 | ((attr as u16) << 8))
    }

    #[inline]
    pub const fn index(&self) -> TileIndex {
        self.0 as TileIndex
    }

    #[inline]
    pub const fn attr(&self) -> u8 {
        (self.0 >> 8) as u8
    }
}

/// Sprite
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Sprite {
    pub y: u8,
    pub x: u8,
    pub index: u8,
    pub attr: u8,
}

impl Sprite {
    #[inline]
    pub const fn new(origin: Point, index: TileIndex, attr: u8) -> Self {
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
            TILE_SIZE * 2
        } else {
            TILE_SIZE
        }
    }

    #[inline]
    pub const fn height(&self) -> isize {
        if (self.attr & OAM_ATTR_H16) != 0 {
            TILE_SIZE * 2
        } else {
            TILE_SIZE
        }
    }

    #[inline]
    pub const fn frame(&self) -> Rect {
        Rect::new(self.x(), self.y(), self.width(), self.height())
    }

    #[inline]
    pub const fn origin(&self) -> Point {
        Point::new(self.x(), self.y())
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
    pub const fn index(&self) -> TileIndex {
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

/// Screen control registers
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Control {
    pub control: ControlWord,
    pub scroll_x: u8,
    pub scroll_y: u8,
    pub sprite_min: u8,
    pub sprite_max: u8,
}

impl Control {
    #[inline]
    pub const fn new() -> Self {
        Self {
            control: ControlWord::empty(),
            scroll_x: 0,
            scroll_y: 0,
            sprite_min: 0,
            sprite_max: 0,
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.control = ControlWord::empty();
        self.scroll_x = 0;
        self.scroll_y = 0;
        self.sprite_min = 0x00;
        self.sprite_max = 0xFF;
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

/// Screen control flags (TBD)
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct ControlWord(u32);

impl ControlWord {
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn reset(&mut self) {
        *self = Self::empty();
    }
}
