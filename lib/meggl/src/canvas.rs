use super::*;
use alloc::{borrow::ToOwned, boxed::Box, vec::Vec};
use core::{borrow::Borrow, cell::UnsafeCell, mem::transmute, num::NonZeroUsize};

/// Images compatible with HTML Canvas Image Data
#[repr(C)]
pub struct ConstHtmlCanvas<'a> {
    size: Size,
    stride: usize,
    slice: &'a [RGBA8888],
}

impl const Drawable for ConstHtmlCanvas<'_> {
    type ColorType = RGBA8888;

    #[inline]
    fn size(&self) -> Size {
        self.size
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
    pub const fn from_slice(
        slice: &'a [RGBA8888],
        size: Size,
        stride: Option<NonZeroUsize>,
    ) -> Self {
        Self {
            size,
            stride: match stride {
                Some(v) => v.get(),
                None => size.width() as usize,
            },
            slice,
        }
    }

    #[inline]
    pub const fn from_bytes(bytes: &'a [u32], size: Size, stride: Option<NonZeroUsize>) -> Self {
        Self::from_slice(unsafe { transmute(bytes) }, size, stride)
    }

    #[inline]
    pub fn clone(&'a self) -> Self {
        Self {
            size: self.size(),
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
    size: Size,
    stride: usize,
    slice: UnsafeCell<&'a mut [RGBA8888]>,
}

impl const Drawable for HtmlCanvas<'_> {
    type ColorType = RGBA8888;

    #[inline]
    fn size(&self) -> Size {
        self.size
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
    pub fn from_slice(slice: &'a mut [RGBA8888], size: Size, stride: Option<NonZeroUsize>) -> Self {
        Self {
            size,
            stride: match stride {
                Some(v) => v.get(),
                None => size.width() as usize,
            },
            slice: UnsafeCell::new(slice),
        }
    }

    #[inline]
    pub fn from_bytes(bytes: &'a mut [u32], size: Size, stride: Option<NonZeroUsize>) -> Self {
        Self::from_slice(unsafe { transmute(bytes) }, size, stride)
    }

    #[inline]
    pub fn clone(&self) -> HtmlCanvas<'a> {
        let slice = unsafe { self.slice.get().as_mut().unwrap() };
        Self {
            size: self.size(),
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
    /// # SAFETY
    /// Must guarantee the existence of the `ptr`.
    #[inline]
    pub unsafe fn from_static(ptr: *mut RGBA8888, size: Size, stride: usize) -> Self {
        let slice = core::slice::from_raw_parts_mut(ptr, size.height() as usize * stride);
        Self {
            size,
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
    size: Size,
    stride: usize,
    slice: UnsafeCell<Box<[RGBA8888]>>,
}

impl const Drawable for OwnedHtmlCanvas {
    type ColorType = RGBA8888;

    #[inline]
    fn size(&self) -> Size {
        self.size
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
    pub fn new(size: Size, bg_color: RGBA8888) -> Self {
        let len = size.width() as usize * size.height() as usize;
        let mut vec = Vec::with_capacity(len);
        vec.resize(len, bg_color);
        Self::from_vec(vec, size)
    }

    #[inline]
    pub fn from_vec(vec: Vec<RGBA8888>, size: Size) -> Self {
        Self {
            size,
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

impl BltConvert<RGBA8888> for HtmlCanvas<'_> {}
impl BltConvert<TrueColor> for HtmlCanvas<'_> {}
