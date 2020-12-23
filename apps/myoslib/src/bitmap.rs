// myos bitmap Library

use core::cell::UnsafeCell;

use super::*;
use crate::{graphics::*, window::Window};
use alloc::vec::Vec;

pub enum OsBitmap {}

pub trait BitmapTrait {
    type PixelType;

    fn width(&self) -> usize;
    fn height(&self) -> usize;
    fn slice(&self) -> &[Self::PixelType];
}

#[repr(C)]
pub struct OsBitmap8<'a> {
    width: usize,
    height: usize,
    bits: UnsafeCell<&'a [u8]>,
}

impl<'a> OsBitmap8<'a> {
    pub const fn from_slice(slice: &'a [u8], size: Size) -> Self {
        Self {
            width: size.width() as usize,
            height: size.height() as usize,
            bits: UnsafeCell::new(slice),
        }
    }
}

impl OsBitmap8<'_> {
    #[inline]
    pub fn blt(&self, window: &Window, origin: Point) {
        os_blt1(
            window.handle().0,
            origin.x as usize,
            origin.y as usize,
            self as *const _ as usize,
        )
    }
}

impl BitmapTrait for OsBitmap8<'_> {
    type PixelType = u8;

    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn slice(&self) -> &[Self::PixelType] {
        unsafe { &(*self.bits.get()) }
    }
}

#[repr(C)]
pub struct Bitmap32 {
    width: usize,
    height: usize,
    vec: Vec<u32>,
}

// impl Bitmap32 {
//     pub fn new(width: usize, height: usize) -> Self {
//         let size = width * height;
//         let mut vec = Vec::with_capacity(size);
//         vec.resize(size, 0);
//         Self { width, height, vec }
//     }
// }

impl BitmapTrait for Bitmap32 {
    type PixelType = u32;

    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn slice(&self) -> &[Self::PixelType] {
        self.vec.as_slice()
    }
}
