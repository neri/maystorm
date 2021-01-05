// myos bitmap Library

use super::*;
use crate::{graphics::*, window::Window};
use core::intrinsics::copy_nonoverlapping;

pub enum OsBitmap {}

pub trait BitmapTrait {
    type PixelType;

    fn bits_per_pixel(&self) -> usize;
    fn width(&self) -> usize;
    fn height(&self) -> usize;
    fn slice(&self) -> &[Self::PixelType];

    fn size(&self) -> Size {
        Size::new(self.width() as isize, self.height() as isize)
    }

    fn get_pixel(&self, point: Point) -> Option<Self::PixelType>
    where
        Self::PixelType: Copy,
    {
        if point.is_within(Rect::from(self.size())) {
            Some(unsafe { self.get_pixel_unchecked(point) })
        } else {
            None
        }
    }

    /// SAFETY: The point must be within the size range.
    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::PixelType
    where
        Self::PixelType: Copy,
    {
        *self
            .slice()
            .get_unchecked(point.x as usize + point.y as usize * self.width())
    }
}

pub trait MutableBitmapTrait: BitmapTrait {
    fn slice_mut(&mut self) -> &mut [Self::PixelType];

    fn set_pixel(&mut self, point: Point, pixel: Self::PixelType) {
        if point.is_within(Rect::from(self.size())) {
            unsafe {
                self.set_pixel_unchecked(point, pixel);
            }
        }
    }

    /// SAFETY: The point must be within the size range.
    unsafe fn set_pixel_unchecked(&mut self, point: Point, pixel: Self::PixelType) {
        let stride = self.width();
        *self
            .slice_mut()
            .get_unchecked_mut(point.x as usize + point.y as usize * stride) = pixel;
    }
}

#[repr(C)]
pub struct OsBitmap8<'a> {
    width: usize,
    height: usize,
    slice: &'a [u8],
}

impl<'a> OsBitmap8<'a> {
    #[inline]
    pub const fn from_slice(slice: &'a [u8], size: Size) -> Self {
        Self {
            width: size.width() as usize,
            height: size.height() as usize,
            slice,
        }
    }
}

impl OsBitmap8<'_> {
    #[inline]
    pub fn blt(&self, window: &Window, origin: Point) {
        os_blt8(
            window.handle().0,
            origin.x as usize,
            origin.y as usize,
            self as *const _ as usize,
        )
    }
}

impl BitmapTrait for OsBitmap8<'_> {
    type PixelType = u8;

    fn bits_per_pixel(&self) -> usize {
        8
    }

    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn slice(&self) -> &[Self::PixelType] {
        self.slice
    }
}

#[repr(C)]
pub struct OsMutBitmap8<'a> {
    width: usize,
    height: usize,
    slice: &'a mut [u8],
}

impl<'a> OsMutBitmap8<'a> {
    #[inline]
    pub fn from_slice(slice: &'a mut [u8], size: Size) -> Self {
        Self {
            width: size.width() as usize,
            height: size.height() as usize,
            slice,
        }
    }
}

impl OsMutBitmap8<'_> {
    #[inline]
    pub fn blt(&self, window: &Window, origin: Point) {
        os_blt8(
            window.handle().0,
            origin.x as usize,
            origin.y as usize,
            self as *const _ as usize,
        )
    }

    /// Copy bitmap
    pub fn copy_from(&mut self, other: &Self) {
        // TODO:
        unsafe {
            let p = self.slice_mut();
            let q = other.slice();
            let count = p.len();
            copy_nonoverlapping(q.as_ptr(), p.as_mut_ptr(), count);
        }
    }
}

impl BitmapTrait for OsMutBitmap8<'_> {
    type PixelType = u8;

    fn bits_per_pixel(&self) -> usize {
        8
    }

    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn slice(&self) -> &[Self::PixelType] {
        self.slice
    }
}

impl MutableBitmapTrait for OsMutBitmap8<'_> {
    fn slice_mut(&mut self) -> &mut [Self::PixelType] {
        &mut self.slice
    }
}

impl<'a> From<&'a OsMutBitmap8<'a>> for OsBitmap8<'a> {
    fn from(src: &'a OsMutBitmap8) -> Self {
        Self::from_slice(src.slice(), src.size())
    }
}

#[repr(C)]
pub struct OsBitmap1<'a> {
    width: usize,
    height: usize,
    stride: usize,
    slice: &'a [u8],
}

impl<'a> OsBitmap1<'a> {
    #[inline]
    pub const fn from_slice(slice: &'a [u8], size: Size) -> Self {
        let stride = (size.width() as usize + 7) / 8;
        Self {
            width: size.width() as usize,
            height: size.height() as usize,
            stride,
            slice,
        }
    }
}

impl OsBitmap1<'_> {
    #[inline]
    pub fn blt(&self, window: &Window, origin: Point, color: Color, mode: usize) {
        os_blt1(
            window.handle().0,
            origin.x as usize,
            origin.y as usize,
            self as *const _ as usize,
            color.argb(),
            mode,
        )
    }
}

impl BitmapTrait for OsBitmap1<'_> {
    type PixelType = u8;

    fn bits_per_pixel(&self) -> usize {
        1
    }

    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn slice(&self) -> &[Self::PixelType] {
        self.slice
    }

    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::PixelType {
        let index = (point.x as usize / 8) + self.stride * point.y as usize;
        let position = 0x80u8 >> ((point.x as usize) & 7);
        *self.slice().get_unchecked(index) & position
    }
}

#[repr(C)]
pub struct OsMutBitmap1<'a> {
    width: usize,
    height: usize,
    stride: usize,
    slice: &'a mut [u8],
}

impl<'a> OsMutBitmap1<'a> {
    #[inline]
    pub fn from_slice(slice: &'a mut [u8], size: Size) -> Self {
        let stride = (size.width() as usize + 7) / 8;
        Self {
            width: size.width() as usize,
            height: size.height() as usize,
            stride,
            slice,
        }
    }
}

impl OsMutBitmap1<'_> {
    #[inline]
    pub fn blt(&self, window: &Window, origin: Point, color: Color, mode: usize) {
        os_blt1(
            window.handle().0,
            origin.x as usize,
            origin.y as usize,
            self as *const _ as usize,
            color.argb(),
            mode,
        )
    }

    /// Copy bitmap
    pub fn copy_from(&mut self, other: &Self) {
        // TODO:
        unsafe {
            let p = self.slice_mut();
            let q = other.slice();
            let count = p.len();
            copy_nonoverlapping(q.as_ptr(), p.as_mut_ptr(), count);
        }
    }
}

impl BitmapTrait for OsMutBitmap1<'_> {
    type PixelType = u8;

    fn bits_per_pixel(&self) -> usize {
        1
    }

    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn slice(&self) -> &[Self::PixelType] {
        self.slice
    }

    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::PixelType {
        let index = (point.x as usize / 8) + self.stride * point.y as usize;
        let position = 0x80u8 >> ((point.x as usize) & 7);
        *self.slice().get_unchecked(index) & position
    }
}

impl MutableBitmapTrait for OsMutBitmap1<'_> {
    fn slice_mut(&mut self) -> &mut [Self::PixelType] {
        &mut self.slice
    }

    unsafe fn set_pixel_unchecked(&mut self, point: Point, pixel: Self::PixelType) {
        let index = (point.x as usize / 8) + self.stride * point.y as usize;
        let position = 0x80u8 >> ((point.x as usize) & 7);
        let bits = self.slice_mut().get_unchecked_mut(index);
        if pixel == 0 {
            *bits &= !position;
        } else {
            *bits |= position;
        }
    }
}

impl<'a> From<&'a OsMutBitmap1<'a>> for OsBitmap1<'a> {
    fn from(src: &'a OsMutBitmap1) -> Self {
        Self::from_slice(src.slice(), src.size())
    }
}

#[repr(C)]
pub struct Bitmap32<'a> {
    width: usize,
    height: usize,
    slice: &'a [u32],
}

impl BitmapTrait for Bitmap32<'_> {
    type PixelType = u32;

    fn bits_per_pixel(&self) -> usize {
        32
    }

    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn slice(&self) -> &[Self::PixelType] {
        self.slice
    }
}
