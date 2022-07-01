use super::*;
use alloc::{borrow::ToOwned, boxed::Box, vec::Vec};
use core::{borrow::Borrow, cell::UnsafeCell, mem::transmute};

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct HtmlCanvasColor(u32);

impl ColorTrait for HtmlCanvasColor {}

impl HtmlCanvasColor {
    pub const TRANSPARENT: Self = Self(0);
    pub const WHITE: Self = Self(0xFFFFFFFF);

    #[inline]
    pub const fn from_gray(white: u8, alpha: u8) -> Self {
        Self(white as u32 * 0x00_01_01_01 + alpha as u32 * 0x01_00_00_00)
    }

    #[inline]
    pub const fn components(&self) -> HtmlCanvasColorComponents {
        HtmlCanvasColorComponents::from(*self)
    }
}

impl const From<TrueColor> for HtmlCanvasColor {
    #[inline]
    fn from(v: TrueColor) -> Self {
        Self::from(HtmlCanvasColorComponents::from(v.components()))
    }
}

impl const From<HtmlCanvasColor> for TrueColor {
    #[inline]
    fn from(v: HtmlCanvasColor) -> Self {
        Self::from(ColorComponents::from(v.components()))
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
#[cfg(target_endian = "little")]
pub struct HtmlCanvasColorComponents {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl HtmlCanvasColorComponents {
    #[inline]
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            r,
            g,
            b,
            a: u8::MAX,
        }
    }

    #[inline]
    pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

impl const From<HtmlCanvasColor> for HtmlCanvasColorComponents {
    #[inline]
    fn from(color: HtmlCanvasColor) -> Self {
        unsafe { transmute(color) }
    }
}

impl const From<HtmlCanvasColorComponents> for HtmlCanvasColor {
    #[inline]
    fn from(components: HtmlCanvasColorComponents) -> Self {
        unsafe { transmute(components) }
    }
}

impl const From<HtmlCanvasColorComponents> for ColorComponents {
    #[inline]
    fn from(v: HtmlCanvasColorComponents) -> Self {
        Self {
            b: v.b,
            g: v.g,
            r: v.r,
            a: v.a,
        }
    }
}

impl const From<ColorComponents> for HtmlCanvasColorComponents {
    #[inline]
    fn from(v: ColorComponents) -> Self {
        Self {
            r: v.r,
            g: v.g,
            b: v.b,
            a: v.a,
        }
    }
}

/// Images compatible with HTML Canvas Image Data
#[repr(C)]
pub struct ConstHtmlCanvas<'a> {
    width: usize,
    height: usize,
    stride: usize,
    slice: &'a [HtmlCanvasColor],
}

impl Drawable for ConstHtmlCanvas<'_> {
    type ColorType = HtmlCanvasColor;

    #[inline]
    fn width(&self) -> usize {
        self.width
    }

    #[inline]
    fn height(&self) -> usize {
        self.height
    }
}

impl RasterImage for ConstHtmlCanvas<'_> {
    #[inline]
    fn stride(&self) -> usize {
        self.stride
    }

    #[inline]
    fn slice(&self) -> &[Self::ColorType] {
        self.slice
    }
}

impl<'a> ConstHtmlCanvas<'a> {
    #[inline]
    pub const fn from_slice(slice: &'a [HtmlCanvasColor], size: Size, stride: usize) -> Self {
        Self {
            width: size.width() as usize,
            height: size.height() as usize,
            stride,
            slice,
        }
    }

    #[inline]
    pub const fn from_bytes(bytes: &'a [u32], size: Size) -> Self {
        Self {
            width: size.width() as usize,
            height: size.height() as usize,
            stride: size.width() as usize,
            slice: unsafe { transmute(bytes) },
        }
    }

    #[inline]
    pub fn clone(&'a self) -> Self {
        Self {
            width: self.width(),
            height: self.height(),
            stride: self.stride(),
            slice: self.slice(),
        }
    }
}

impl<'a> const AsRef<ConstHtmlCanvas<'a>> for ConstHtmlCanvas<'a> {
    #[inline]
    fn as_ref(&self) -> &ConstHtmlCanvas<'a> {
        self
    }
}

/// Images compatible with HTML Canvas Image Data
#[repr(C)]
pub struct HtmlCanvas<'a> {
    width: usize,
    height: usize,
    stride: usize,
    slice: UnsafeCell<&'a mut [HtmlCanvasColor]>,
}

impl Drawable for HtmlCanvas<'_> {
    type ColorType = HtmlCanvasColor;

    #[inline]
    fn width(&self) -> usize {
        self.width
    }

    #[inline]
    fn height(&self) -> usize {
        self.height
    }
}

impl RasterImage for HtmlCanvas<'_> {
    #[inline]
    fn stride(&self) -> usize {
        self.stride
    }

    #[inline]
    fn slice(&self) -> &[Self::ColorType] {
        unsafe { &*self.slice.get() }
    }
}

impl MutableRasterImage for HtmlCanvas<'_> {
    #[inline]
    fn slice_mut(&mut self) -> &mut [Self::ColorType] {
        self.slice.get_mut()
    }
}

impl<'a> HtmlCanvas<'a> {
    #[inline]
    pub fn from_slice(slice: &'a mut [HtmlCanvasColor], size: Size, stride: usize) -> Self {
        Self {
            width: size.width() as usize,
            height: size.height() as usize,
            stride,
            slice: UnsafeCell::new(slice),
        }
    }

    #[inline]
    pub fn from_bytes(bytes: &'a mut [u32], size: Size) -> Self {
        Self {
            width: size.width() as usize,
            height: size.height() as usize,
            stride: size.width() as usize,
            slice: unsafe { transmute(bytes) },
        }
    }

    #[inline]
    pub fn clone(&self) -> HtmlCanvas<'a> {
        let slice = unsafe { self.slice.get().as_mut().unwrap() };
        Self {
            width: self.width(),
            height: self.height(),
            stride: self.stride(),
            slice: UnsafeCell::new(slice),
        }
    }

    #[inline]
    pub const fn as_const(&self) -> &'a ConstHtmlCanvas<'a> {
        unsafe { transmute(self) }
    }
}

impl HtmlCanvas<'static> {
    /// SAFETY: Must guarantee the existence of the `ptr`.
    #[inline]
    pub unsafe fn from_static(ptr: *mut HtmlCanvasColor, size: Size, stride: usize) -> Self {
        let slice = core::slice::from_raw_parts_mut(ptr, size.height() as usize * stride);
        Self {
            width: size.width() as usize,
            height: size.height() as usize,
            stride,
            slice: UnsafeCell::new(slice),
        }
    }
}

impl<'a> const AsRef<ConstHtmlCanvas<'a>> for HtmlCanvas<'a> {
    #[inline]
    fn as_ref(&self) -> &ConstHtmlCanvas<'a> {
        self.as_const()
    }
}

impl<'a> const AsMut<HtmlCanvas<'a>> for HtmlCanvas<'a> {
    #[inline]
    fn as_mut(&mut self) -> &mut HtmlCanvas<'a> {
        self
    }
}

impl ToOwned for HtmlCanvas<'_> {
    type Owned = OwnedHtmlCanvas;

    #[inline]
    fn to_owned(&self) -> Self::Owned {
        let vec = self.slice().to_vec();
        OwnedHtmlCanvas::from_vec(vec, self.size())
    }
}

#[repr(C)]
pub struct OwnedHtmlCanvas {
    width: usize,
    height: usize,
    stride: usize,
    slice: UnsafeCell<Box<[HtmlCanvasColor]>>,
}

impl Drawable for OwnedHtmlCanvas {
    type ColorType = HtmlCanvasColor;

    #[inline]
    fn width(&self) -> usize {
        self.width
    }

    #[inline]
    fn height(&self) -> usize {
        self.height
    }
}

impl RasterImage for OwnedHtmlCanvas {
    #[inline]
    fn stride(&self) -> usize {
        self.stride
    }

    #[inline]
    fn slice(&self) -> &[Self::ColorType] {
        unsafe { &*self.slice.get() }
    }
}

impl MutableRasterImage for OwnedHtmlCanvas {
    #[inline]
    fn slice_mut(&mut self) -> &mut [Self::ColorType] {
        self.slice.get_mut()
    }
}

impl OwnedHtmlCanvas {
    #[inline]
    pub fn new(size: Size, bg_color: HtmlCanvasColor) -> Self {
        let len = size.width() as usize * size.height() as usize;
        let mut vec = Vec::with_capacity(len);
        vec.resize(len, bg_color);
        Self::from_vec(vec, size)
    }

    #[inline]
    pub fn from_vec(vec: Vec<HtmlCanvasColor>, size: Size) -> Self {
        Self {
            width: size.width as usize,
            height: size.height as usize,
            stride: size.width as usize,
            slice: UnsafeCell::new(vec.into_boxed_slice()),
        }
    }
}

impl<'a> AsRef<HtmlCanvas<'a>> for OwnedHtmlCanvas {
    #[inline]
    fn as_ref(&self) -> &HtmlCanvas<'a> {
        unsafe { transmute(self) }
    }
}

impl<'a> AsMut<HtmlCanvas<'a>> for OwnedHtmlCanvas {
    #[inline]
    fn as_mut(&mut self) -> &mut HtmlCanvas<'a> {
        unsafe { transmute(self) }
    }
}

impl<'a> Borrow<HtmlCanvas<'a>> for OwnedHtmlCanvas {
    #[inline]
    fn borrow(&self) -> &HtmlCanvas<'a> {
        unsafe { transmute(self) }
    }
}

impl BltConvert<HtmlCanvasColor> for HtmlCanvas<'_> {}
impl BltConvert<TrueColor> for HtmlCanvas<'_> {}
