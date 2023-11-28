use super::*;
use alloc::{borrow::ToOwned, boxed::Box, vec::Vec};
use core::{
    borrow::{Borrow, BorrowMut},
    cell::UnsafeCell,
    convert::TryFrom,
    intrinsics::copy_nonoverlapping,
    mem::{swap, transmute, ManuallyDrop},
    num::NonZeroUsize,
    ptr::slice_from_raw_parts_mut,
    slice,
};
use paste::paste;

pub trait Blt<T: Drawable>: Drawable {
    fn blt(&mut self, src: &T, origin: Point, rect: Rect);
}

pub trait DrawRect: SetPixel {
    fn fill_rect(&mut self, rect: Rect, color: Self::ColorType);

    fn draw_hline(&mut self, origin: Point, width: isize, color: Self::ColorType);

    fn draw_vline(&mut self, origin: Point, height: isize, color: Self::ColorType);

    fn clear(&mut self) {
        self.fill_rect(self.bounds(), Default::default());
    }

    fn draw_rect(&mut self, rect: Rect, color: Self::ColorType) {
        let Ok(coords) = Coordinates::from_rect(rect) else {
            return;
        };
        let width = rect.width();
        let height = rect.height();
        self.draw_hline(coords.left_top(), width, color);
        if height > 1 {
            self.draw_hline(coords.left_bottom() - Movement::new(0, 1), width, color);
            if height > 2 {
                self.draw_vline(coords.left_top() + Movement::new(0, 1), height - 2, color);
                self.draw_vline(coords.right_top() + Movement::new(-1, 1), height - 2, color);
            }
        }
    }

    fn draw_circle(&mut self, origin: Point, radius: isize, color: Self::ColorType) {
        let rect = Rect::from((origin - radius, Size::new(radius * 2, radius * 2)));
        self.draw_round_rect(rect, radius, color);
    }

    fn fill_circle(&mut self, origin: Point, radius: isize, color: Self::ColorType) {
        let rect = Rect::from((origin - radius, Size::new(radius * 2, radius * 2)));
        self.fill_round_rect(rect, radius, color);
    }

    fn fill_round_rect(&mut self, rect: Rect, radius: isize, color: Self::ColorType) {
        let width = rect.width();
        let height = rect.height();
        let dx = rect.min_x();
        let dy = rect.min_y();

        let mut radius = radius;
        if radius * 2 > width {
            radius = width / 2;
        }
        if radius * 2 > height {
            radius = height / 2;
        }

        let lh = height - radius * 2;
        if lh > 0 {
            let rect_line = Rect::new(dx, dy + radius, width, lh);
            self.fill_rect(rect_line, color);
        }

        let mut cx = radius;
        let mut cy = 0;
        let mut f = -2 * radius + 3;
        let qh = height - 1;

        while cx >= cy {
            {
                let bx = radius - cy;
                let by = radius - cx;
                let dw = width - bx * 2;
                self.draw_hline(Point::new(dx + bx, dy + by), dw, color);
                self.draw_hline(Point::new(dx + bx, dy + qh - by), dw, color);
            }

            {
                let bx = radius - cx;
                let by = radius - cy;
                let dw = width - bx * 2;
                self.draw_hline(Point::new(dx + bx, dy + by), dw, color);
                self.draw_hline(Point::new(dx + bx, dy + qh - by), dw, color);
            }

            if f >= 0 {
                cx -= 1;
                f -= 4 * cx;
            }
            cy += 1;
            f += 4 * cy + 2;
        }
    }

    fn draw_round_rect(&mut self, rect: Rect, radius: isize, color: Self::ColorType) {
        let width = rect.width();
        let height = rect.height();
        let dx = rect.min_x();
        let dy = rect.min_y();

        let mut radius = radius;
        if radius * 2 > width {
            radius = width / 2;
        }
        if radius * 2 > height {
            radius = height / 2;
        }

        let lh = height - radius * 2;
        if lh > 0 {
            self.draw_vline(Point::new(dx, dy + radius), lh, color);
            self.draw_vline(Point::new(dx + width - 1, dy + radius), lh, color);
        }
        let lw = width - radius * 2;
        if lw > 0 {
            self.draw_hline(Point::new(dx + radius, dy), lw, color);
            self.draw_hline(Point::new(dx + radius, dy + height - 1), lw, color);
        }

        let mut cx = radius;
        let mut cy = 0;
        let mut f = -2 * radius + 3;
        let qh = height - 1;

        while cx >= cy {
            {
                let bx = radius - cy;
                let by = radius - cx;
                let dw = width - bx * 2 - 1;
                self.set_pixel(Point::new(dx + bx, dy + by), color);
                self.set_pixel(Point::new(dx + bx, dy + qh - by), color);
                self.set_pixel(Point::new(dx + bx + dw, dy + by), color);
                self.set_pixel(Point::new(dx + bx + dw, dy + qh - by), color);
            }

            {
                let bx = radius - cx;
                let by = radius - cy;
                let dw = width - bx * 2 - 1;
                self.set_pixel(Point::new(dx + bx, dy + by), color);
                self.set_pixel(Point::new(dx + bx, dy + qh - by), color);
                self.set_pixel(Point::new(dx + bx + dw, dy + by), color);
                self.set_pixel(Point::new(dx + bx + dw, dy + qh - by), color);
            }

            if f >= 0 {
                cx -= 1;
                f -= 4 * cx;
            }
            cy += 1;
            f += 4 * cy + 2;
        }
    }

    fn draw_line(&mut self, c1: Point, c2: Point, color: Self::ColorType) {
        if c1.x() == c2.x() {
            if c1.y() < c2.y() {
                let height = 1 + c2.y() - c1.y();
                self.draw_vline(c1, height, color);
            } else {
                let height = 1 + c1.y() - c2.y();
                self.draw_vline(c2, height, color);
            }
        } else if c1.y() == c2.y() {
            if c1.x() < c2.x() {
                let width = 1 + c2.x() - c1.x();
                self.draw_hline(c1, width, color);
            } else {
                let width = 1 + c1.x() - c2.x();
                self.draw_hline(c2, width, color);
            }
        } else {
            c1.line_to(c2, |point| {
                self.set_pixel(point, color);
            });
        }
    }
}

pub trait DrawGlyph: SetPixel {
    fn draw_glyph(&mut self, glyph: &[u8], size: Size, origin: Point, color: Self::ColorType) {
        let stride = (size.width as usize + 7) / 8;

        let Ok(mut coords) = Coordinates::from_rect(Rect::from((origin, size))) else {
            return;
        };

        let width = self.width() as isize;
        let height = self.height() as isize;
        if coords.right > width {
            coords.right = width;
        }
        if coords.bottom > height {
            coords.bottom = height;
        }
        if coords.left < 0 || coords.left >= width || coords.top < 0 || coords.top >= height {
            return;
        }

        let new_rect = Rect::from(coords);
        let width = new_rect.width() as usize;
        let height = new_rect.height();
        let w8 = width / 8;
        let w7 = width & 7;
        let mut cursor = 0;
        for y in 0..height {
            for i in 0..w8 {
                let data = unsafe { glyph.get_unchecked(cursor + i) };
                for j in 0..8 {
                    let position = 0x80u8 >> j;
                    if (data & position) != 0 {
                        let x = (i * 8 + j) as isize;
                        let y = y;
                        let point = Point::new(origin.x + x, origin.y + y);
                        self.set_pixel(point, color);
                    }
                }
            }
            if w7 > 0 {
                let data = unsafe { glyph.get_unchecked(cursor + w8) };
                let base_x = w8 * 8;
                for i in 0..w7 {
                    let position = 0x80u8 >> i;
                    if (data & position) != 0 {
                        let x = (i + base_x) as isize;
                        let y = y;
                        let point = Point::new(origin.x + x, origin.y + y);
                        self.set_pixel(point, color);
                    }
                }
            }
            cursor += stride;
        }
    }

    fn draw_glyph_cw(&mut self, glyph: &[u8], size: Size, origin: Point, color: Self::ColorType) {
        let stride = (size.width as usize + 7) / 8;
        let width = self.width() as isize;
        let height = self.height() as isize;

        let Ok(mut coords) = Coordinates::from_rect(Rect::new(
            width - origin.y - size.height,
            origin.x,
            size.height,
            size.width,
        )) else {
            return;
        };

        if coords.right > width {
            coords.right = width;
        }
        if coords.bottom > height {
            coords.bottom = height;
        }
        if coords.left < 0 || coords.left >= width || coords.top < 0 || coords.top >= height {
            return;
        }

        let new_rect = Rect::from(coords);
        let width = new_rect.height() as usize;
        let height = new_rect.width();

        let w8 = width / 8;
        let w7 = width & 7;
        let mut cursor = 0;
        for i in 0..height {
            for j in 0..w8 {
                let data = unsafe { glyph.get_unchecked(cursor + j) };
                for k in 0..8 {
                    let position = 0x80u8 >> k;
                    if (data & position) != 0 {
                        let point = Point::new(coords.right - i, coords.top + (j * 8 + k) as isize);
                        self.set_pixel(point, color);
                    }
                }
            }
            if w7 > 0 {
                let data = unsafe { glyph.get_unchecked(cursor + w8) };
                let base_x = w8 * 8;
                for k in 0..w7 {
                    let position = 0x80u8 >> k;
                    if (data & position) != 0 {
                        let point =
                            Point::new(coords.right - i, coords.top + (base_x + k) as isize);
                        self.set_pixel(point, color);
                    }
                }
            }
            cursor += stride;
        }
    }
}

pub trait BltConvert<T: PixelColor>: MutableRasterImage {
    #[inline]
    fn blt_convert<U, F>(&mut self, src: &U, origin: Point, rect: Rect, mut f: F)
    where
        U: RasterImage<ColorType = T>,
        F: FnMut(T) -> Self::ColorType,
    {
        let (dx, dy, sx, sy, width, height) =
            _adjust_blt_coords(self.size(), src.size(), origin, rect);
        if width <= 0 || height <= 0 {
            return;
        }
        let width = width as usize;
        let height = height as usize;

        let ds = self.stride();
        let ss = src.stride();
        let mut dest_cursor = dx as usize + dy as usize * ds;
        let mut src_cursor = sx as usize + sy as usize * ss;
        let dest_fb = self.slice_mut();
        let src_fb = src.slice();

        let dd = ds - width;
        let sd = ss - width;
        if dd == 0 && sd == 0 {
            for _ in 0..height * width {
                unsafe {
                    let c = src_fb.get_unchecked(src_cursor);
                    *dest_fb.get_unchecked_mut(dest_cursor) = f(*c);
                }
                src_cursor += 1;
                dest_cursor += 1;
            }
        } else {
            for _ in 0..height {
                for _ in 0..width {
                    unsafe {
                        let c = src_fb.get_unchecked(src_cursor);
                        *dest_fb.get_unchecked_mut(dest_cursor) = f(*c);
                    }
                    src_cursor += 1;
                    dest_cursor += 1;
                }
                dest_cursor += dd;
                src_cursor += sd;
            }
        }
    }

    fn blt_convert_opt<U, F>(&mut self, src: &U, origin: Point, rect: Rect, mut f: F)
    where
        U: RasterImage<ColorType = T>,
        F: FnMut(T) -> Option<Self::ColorType>,
    {
        let (dx, dy, sx, sy, width, height) =
            _adjust_blt_coords(self.size(), src.size(), origin, rect);
        if width <= 0 || height <= 0 {
            return;
        }
        let width = width as usize;
        let height = height as usize;

        let ds = self.stride();
        let ss = src.stride();
        let mut dest_cursor = dx as usize + dy as usize * ds;
        let mut src_cursor = sx as usize + sy as usize * ss;
        let dest_fb = self.slice_mut();
        let src_fb = src.slice();

        let dd = ds - width;
        let sd = ss - width;
        if dd == 0 && sd == 0 {
            for _ in 0..height * width {
                unsafe {
                    let c = src_fb.get_unchecked(src_cursor);
                    match f(*c) {
                        Some(c) => *dest_fb.get_unchecked_mut(dest_cursor) = c,
                        None => {}
                    }
                }
                src_cursor += 1;
                dest_cursor += 1;
            }
        } else {
            for _ in 0..height {
                for _ in 0..width {
                    unsafe {
                        let c = src_fb.get_unchecked(src_cursor);
                        match f(*c) {
                            Some(c) => *dest_fb.get_unchecked_mut(dest_cursor) = c,
                            None => {}
                        }
                    }
                    src_cursor += 1;
                    dest_cursor += 1;
                }
                dest_cursor += dd;
                src_cursor += sd;
            }
        }
    }
}

macro_rules! define_bitmap {
    ( $suffix:tt, $inner_type:ty, $color_type:ty, $slice_type:ty, ) => {
        paste! {
            // The memory layouts of BitmapRefXX, BitmapRefMutXX and OwnedBitmapXX are guaranteed to be identical.
            #[repr(C)]
            pub struct [<BitmapRef $suffix>]<'a> {
                slice: &'a [$slice_type],
                size: Size,
                stride: usize,
            }

            #[repr(C)]
            pub struct [<BitmapRefMut $suffix>]<'a> {
                slice: UnsafeCell<&'a mut [$slice_type]>,
                size: Size,
                stride: usize,
            }

            #[repr(C)]
            pub struct [<OwnedBitmap $suffix>] {
                slice: UnsafeCell<Box<[$slice_type]>>,
                size: Size,
                stride: usize,
            }

            impl Drawable for [<BitmapRef $suffix>]<'_> {
                type ColorType = $color_type;

                #[inline]
                fn size(&self) -> Size {
                    self.size
                }
            }

            impl Drawable for [<BitmapRefMut $suffix>]<'_> {
                type ColorType = $color_type;

                #[inline]
                fn size(&self) -> Size {
                    self.size
                }
            }

            impl Drawable for [<OwnedBitmap $suffix>] {
                type ColorType = $color_type;

                #[inline]
                fn size(&self) -> Size {
                    self.size
                }
            }

            impl<'a> [<BitmapRef $suffix>]<'a> {
                #[inline]
                pub fn from_slice(
                    slice: &'a [$slice_type],
                    size: Size,
                    stride: Option<NonZeroUsize>,
                ) -> Self
                {
                    Self {
                        size,
                        stride: match stride {
                            Some(v) => v.get(),
                            None => <Self as Drawable>::ColorType::stride_for(size.width()),
                        },
                        slice,
                    }
                }

                #[inline]
                pub fn from_bytes(bytes: &'a [$inner_type], size: Size) -> Self
                {
                    Self {
                        size,
                        stride: <Self as Drawable>::ColorType::stride_for(size.width()),
                        slice: unsafe { transmute(bytes) },
                    }
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

            impl<'a> [<BitmapRefMut $suffix>]<'a> {
                #[inline]
                pub fn from_slice(
                    slice: &'a mut [$slice_type],
                    size: Size,
                    stride: Option<NonZeroUsize>,
                ) -> Self
                {
                    Self {
                        size,
                        stride: match stride {
                            Some(v) => v.get(),
                            None => <Self as Drawable>::ColorType::stride_for(size.width()),
                        },
                        slice: UnsafeCell::new(slice),
                    }
                }

                #[inline]
                pub fn from_bytes(bytes: &'a mut [$inner_type], size: Size) -> Self
                {
                    Self {
                        size,
                        stride: <Self as Drawable>::ColorType::stride_for(size.width()),
                        slice: unsafe { transmute(bytes) },
                    }
                }

                #[inline]
                pub const fn as_const(&'a self) -> &'a [<BitmapRef $suffix>]<'a> {
                    unsafe { transmute(self) }
                }

                #[inline]
                pub const fn into_const(self) -> [<BitmapRef $suffix>]<'a> {
                    unsafe { transmute(self) }
                }

                #[inline]
                pub fn clone_mut(&'a mut self) -> [<BitmapRefMut $suffix>]<'a> {
                    let slice = unsafe { &mut *self.slice.get() };
                    Self {
                        size: self.size(),
                        stride: self.stride(),
                        slice: UnsafeCell::new(slice),
                    }
                }
            }

            impl [<BitmapRefMut $suffix>]<'static> {
                /// # Safety
                ///
                /// Must guarantee the existence of the `ptr`.
                #[inline]
                pub unsafe fn from_static(ptr: *mut $slice_type, size: Size, stride: usize) -> Self {
                    let slice = core::slice::from_raw_parts_mut(ptr, size.height() as usize * stride);
                    Self {
                        size,
                        stride,
                        slice: UnsafeCell::new(slice),
                    }
                }
            }

            impl [<OwnedBitmap $suffix>] {
                #[inline]
                pub fn from_boxed_slice(slice: Box<[$slice_type]>, size: Size) -> Self {
                    Self {
                        size: size,
                        stride: <Self as Drawable>::ColorType::stride_for(size.width()),
                        slice: UnsafeCell::new(slice),
                    }
                }

                #[inline]
                pub fn from_vec(vec: Vec<$slice_type>, size: Size) -> Self {
                    Self::from_boxed_slice(vec.into_boxed_slice(), size)
                }
            }

            impl<'a> AsRef<[<BitmapRef $suffix>]<'a>> for [<BitmapRef $suffix>]<'a> {
                #[inline]
                fn as_ref(&self) -> &Self {
                    self
                }
            }

            impl<'a> AsRef<[<BitmapRef $suffix>]<'a>> for [<BitmapRefMut $suffix>]<'a> {
                #[inline]
                fn as_ref(&self) -> &[<BitmapRef $suffix>]<'a> {
                    unsafe { transmute(self) }
                }
            }

            impl<'a> AsMut<[<BitmapRefMut $suffix>]<'a>> for [<BitmapRefMut $suffix>]<'a> {
                #[inline]
                fn as_mut(&mut self) -> &mut [<BitmapRefMut $suffix>]<'a> {
                    self
                }
            }

            impl<'a> Borrow<[<BitmapRef $suffix>]<'a>> for [<BitmapRefMut $suffix>]<'a> {
                #[inline]
                fn borrow(&self) -> &[<BitmapRef $suffix>]<'a> {
                    unsafe { transmute(self) }
                }
            }

            impl ToOwned for [<BitmapRefMut $suffix>]<'_> {
                type Owned = [<OwnedBitmap $suffix>];

                #[inline]
                fn to_owned(&self) -> Self::Owned {
                    let vec = self.slice().to_vec();
                    <[<OwnedBitmap $suffix>]>::from_vec(vec, self.size())
                }
            }

            impl<'a> AsRef<[<BitmapRef $suffix>]<'a>> for [<OwnedBitmap $suffix>] {
                #[inline]
                fn as_ref(&self) -> &[<BitmapRef $suffix>]<'a> {
                    unsafe { transmute(self) }
                }
            }

            impl<'a> AsMut<[<BitmapRefMut $suffix>]<'a>> for [<OwnedBitmap $suffix>] {
                #[inline]
                fn as_mut(&mut self) -> &mut [<BitmapRefMut $suffix>]<'a> {
                    unsafe { transmute(self) }
                }
            }

            impl<'a> Borrow<[<BitmapRef $suffix>]<'a>> for [<OwnedBitmap $suffix>] {
                #[inline]
                fn borrow(&self) -> &[<BitmapRef $suffix>]<'a> {
                    unsafe { transmute(self) }
                }
            }

            impl<'a> Borrow<[<BitmapRefMut $suffix>]<'a>> for [<OwnedBitmap $suffix>] {
                #[inline]
                fn borrow(&self) -> &[<BitmapRefMut $suffix>]<'a> {
                    unsafe { transmute(self) }
                }
            }

            impl<'a> BorrowMut<[<BitmapRefMut $suffix>]<'a>> for [<OwnedBitmap $suffix>] {
                #[inline]
                fn borrow_mut(&mut self) -> &mut [<BitmapRefMut $suffix>]<'a> {
                    unsafe { transmute(self) }
                }
            }

        }
    };
    ( $suffix:tt, $inner_type:ty, $color_type:ty, ) => {
        define_bitmap!($suffix, $inner_type, $color_type, $color_type,);

        paste! {
            impl RasterImage for [<BitmapRef $suffix>]<'_> {
                #[inline]
                fn stride(&self) -> usize {
                    self.stride
                }

                #[inline]
                fn slice(&self) -> &[Self::ColorType] {
                    self.slice
                }
            }

            impl RasterImage for [<BitmapRefMut $suffix>]<'_> {
                #[inline]
                fn stride(&self) -> usize {
                    self.stride
                }

                #[inline]
                fn slice(&self) -> &[Self::ColorType] {
                    unsafe { &*self.slice.get() }
                }
            }

            impl MutableRasterImage for [<BitmapRefMut $suffix>]<'_> {
                #[inline]
                fn slice_mut(&mut self) -> &mut [Self::ColorType] {
                    self.slice.get_mut()
                }
            }

            impl RasterImage for [<OwnedBitmap $suffix>] {
                #[inline]
                fn stride(&self) -> usize {
                    self.stride
                }

                #[inline]
                fn slice(&self) -> &[Self::ColorType] {
                    unsafe { &*self.slice.get() }
                }
            }

            impl MutableRasterImage for [<OwnedBitmap $suffix>] {
                #[inline]
                fn slice_mut(&mut self) -> &mut [Self::ColorType] {
                    self.slice.get_mut()
                }
            }

            impl<'a> [<BitmapRefMut $suffix>]<'a> {
                pub fn view(&mut self, rect: Rect) -> Option<[<BitmapRefMut $suffix>]<'a>>
                {
                    let Ok(coords) = Coordinates::try_from(rect) else { return None };
                    let width = self.width() as isize;
                    let height = self.height() as isize;
                    let stride = self.stride();

                    if coords.left < 0
                        || coords.left >= width
                        || coords.right > width
                        || coords.top < 0
                        || coords.top >= height
                        || coords.bottom > height
                    {
                        return None;
                    }

                    let offset = rect.min_x() as usize + rect.min_y() as usize * stride;
                    let new_len = (rect.height() as usize - 1) * stride + rect.width() as usize;
                    Some(unsafe {
                        let p = self.slice_mut().as_mut_ptr();
                        Self {
                            size: rect.size(),
                            stride,
                            slice: UnsafeCell::new(slice::from_raw_parts_mut(p.add(offset), new_len)),
                        }
                    })
                }
            }

            impl [<BitmapRefMut $suffix>]<'_> {
                pub fn copy(&mut self, origin: Point, rect: Rect) {
                    let (dx, dy, sx, sy, width, height) =
                        _adjust_blt_coords(self.size(), self.size(), origin, rect);
                    if width <= 0 || height <= 0 {
                        return;
                    }
                    let width = width as usize;
                    let height = height as usize;
                    let stride = self.stride();

                    // TODO: overlapping
                    unsafe {
                        let dest_fb = self.slice_mut().as_mut_ptr();
                        let mut dest_ptr = dest_fb.add(dx as usize + dy as usize * stride);
                        let mut src_ptr = dest_fb.add(sx as usize + sy as usize * stride) as *const _;

                        if stride == width {
                            dest_ptr.copy_from_nonoverlapping(src_ptr, width * height);
                        } else {
                            for _ in 0..height {
                                dest_ptr.copy_from_nonoverlapping(src_ptr, width);
                                dest_ptr = dest_ptr.add(stride);
                                src_ptr = src_ptr.add(stride);
                            }
                        }
                    }
                }
            }

            impl<'a, 'b> Blt<[<BitmapRef $suffix>]<'b>> for [<BitmapRefMut $suffix>]<'a> {
                fn blt(&mut self, src: &[<BitmapRef $suffix>]<'b>, origin: Point, rect: Rect) {
                    let (dx, dy, sx, sy, width, height) =
                        _adjust_blt_coords(self.size(), src.size(), origin, rect);
                    if width <= 0 || height <= 0 {
                        return;
                    }
                    let width = width as usize;
                    let height = height as usize;

                    let ds = self.stride();
                    let ss = src.stride();
                    unsafe {
                        let mut dest_fb = self
                            .slice_mut()
                            .as_mut_ptr()
                            .add(dx as usize + dy as usize * ds);
                        let mut src_fb = src
                            .slice()
                            .as_ptr()
                            .add(sx as usize + sy as usize * ss) as *const _;

                        if ds == width && ss == width {
                            dest_fb.copy_from_nonoverlapping(src_fb, width * height);
                        } else {
                            for _ in 0..height {
                                dest_fb.copy_from_nonoverlapping(src_fb, width);
                                dest_fb = dest_fb.add(ds);
                                src_fb = src_fb.add(ss);
                            }
                        }
                    }
                }
            }

            impl DrawGlyph for [<BitmapRefMut $suffix>]<'_> {}

            impl DrawRect for [<BitmapRefMut $suffix>]<'_> {
                fn fill_rect(&mut self, rect: Rect, color: Self::ColorType) {
                    let mut width = rect.width();
                    let mut height = rect.height();
                    let mut dx = rect.min_x();
                    let mut dy = rect.min_y();

                    if dx < 0 {
                        width += dx;
                        dx = 0;
                    }
                    if dy < 0 {
                        height += dy;
                        dy = 0;
                    }
                    let r = dx + width;
                    let b = dy + height;
                    if r >= self.size.width {
                        width = self.size.width - dx;
                    }
                    if b >= self.size.height {
                        height = self.size.height - dy;
                    }
                    if width <= 0 || height <= 0 {
                        return;
                    }

                    let width = width as usize;
                    let height = height as usize;
                    let stride = self.stride;
                    let mut cursor = dx as usize + dy as usize * stride;
                    if stride == width {
                        memory_colors::[<_memset_colors $suffix>](self.slice_mut(), cursor, width * height, color);
                    } else {
                        for _ in 0..height {
                            memory_colors::[<_memset_colors $suffix>](self.slice_mut(), cursor, width, color);
                            cursor += stride;
                        }
                    }
                }

                fn draw_hline(&mut self, origin: Point, width: isize, color: Self::ColorType) {
                    let size = self.size();
                    let mut dx = origin.x;
                    let dy = origin.y;
                    let mut w = width;

                    if dy < 0 || dy >= size.height {
                        return;
                    }
                    if dx < 0 {
                        w += dx;
                        dx = 0;
                    }
                    let r = dx + w;
                    if r >= size.width {
                        w = size.width - dx;
                    }
                    if w <= 0 {
                        return;
                    }

                    let cursor = dx as usize + dy as usize * self.stride;
                    memory_colors::[<_memset_colors $suffix>](self.slice_mut(), cursor, w as usize, color);
                }

                fn draw_vline(&mut self, origin: Point, height: isize, color: Self::ColorType) {
                    let size = self.size();
                    let dx = origin.x;
                    let mut dy = origin.y;
                    let mut h = height;

                    if dx < 0 || dx >= size.width {
                        return;
                    }
                    if dy < 0 {
                        h += dy;
                        dy = 0;
                    }
                    let b = dy + h;
                    if h >= size.height || b >= size.height {
                        h = size.height - dy - 1;
                    }
                    if h <= 0 {
                        return;
                    }

                    let stride = self.stride;
                    let mut cursor = dx as usize + dy as usize * stride;
                    for _ in 0..h {
                        self.slice_mut()[cursor] = color;
                        cursor += stride;
                    }
                }
            }

            impl [<OwnedBitmap $suffix>] {
                #[inline]
                pub fn new(size: Size, bg_color: <Self as Drawable>::ColorType) -> Self {
                    let len = size.width() as usize * size.height() as usize;
                    let mut vec = Vec::with_capacity(len);
                    vec.resize(len, bg_color);
                    Self::from_vec(vec, size)
                }
            }

        }
    };
}

define_bitmap!(8, u8, IndexedColor,);
define_bitmap!(16, u16, RGB565,);
define_bitmap!(32, u32, ARGB8888,);

impl BltConvert<ARGB8888> for BitmapRefMut8<'_> {}
impl BltConvert<IndexedColor> for BitmapRefMut8<'_> {}

impl BitmapRefMut8<'_> {
    pub fn blt_with_key(
        &mut self,
        src: &BitmapRef8,
        origin: Point,
        rect: Rect,
        color_key: <Self as Drawable>::ColorType,
    ) {
        let (dx, dy, sx, sy, width, height) =
            _adjust_blt_coords(self.size(), src.size(), origin, rect);
        if width <= 0 || height <= 0 {
            return;
        }
        let width = width as usize;
        let height = height as usize;

        let ds = self.stride();
        let ss = src.stride();
        let mut dest_cursor = dx as usize + dy as usize * ds;
        let mut src_cursor = sx as usize + sy as usize * ss;
        let dest_fb = self.slice_mut();
        let src_fb = src.slice();

        for _ in 0..height {
            for i in 0..width {
                let c = src_fb[src_cursor + i];
                if c != color_key {
                    dest_fb[dest_cursor + i] = c;
                }
            }
            dest_cursor += ds;
            src_cursor += ss;
        }
    }

    #[inline]
    pub fn blt32(&mut self, src: &BitmapRef32, origin: Point, rect: Rect) {
        self.blt_convert(src, origin, rect, |c| IndexedColor::from_rgb(c.rgb()));
    }
}

impl BltConvert<ARGB8888> for BitmapRefMut32<'_> {}
impl BltConvert<IndexedColor> for BitmapRefMut32<'_> {}

impl BitmapRefMut32<'_> {
    pub fn blend_rect(&mut self, rect: Rect, color: ARGB8888) {
        let rhs = color.components();
        if rhs.is_opaque() {
            return self.fill_rect(rect, color);
        } else if rhs.is_transparent() {
            return;
        }
        let alpha = rhs.a.0 as usize;
        let alpha_n = 255 - alpha;

        let mut width = rect.width();
        let mut height = rect.height();
        let mut dx = rect.min_x();
        let mut dy = rect.min_y();

        if dx < 0 {
            width += dx;
            dx = 0;
        }
        if dy < 0 {
            height += dy;
            dy = 0;
        }
        let r = dx + width;
        let b = dy + height;
        if r >= self.size().width {
            width = self.size().width - dx;
        }
        if b >= self.size().height {
            height = self.size().height - dy;
        }
        if width <= 0 || height <= 0 {
            return;
        }

        let mut cursor = dx as usize + dy as usize * self.stride();
        let stride = self.stride() - width as usize;
        let slice = self.slice_mut();
        for _ in 0..height {
            for _ in 0..width {
                let lhs = unsafe { slice.get_unchecked(cursor) }.components();
                let c = lhs
                    .blending(
                        rhs,
                        |lhs, rhs| {
                            (((lhs as usize) * alpha_n + (rhs as usize) * alpha) / 255) as u8
                        },
                        |a, b| a.saturating_add(b),
                    )
                    .into();
                unsafe {
                    *slice.get_unchecked_mut(cursor) = c;
                }
                cursor += 1;
            }
            cursor += stride;
        }
    }

    pub fn blt_blend(&mut self, src: &BitmapRef32, origin: Point, rect: Rect, opacity: Alpha8) {
        let (dx, dy, sx, sy, width, height) =
            _adjust_blt_coords(self.size(), src.size(), origin, rect);
        if opacity.is_transparent() || width <= 0 || height <= 0 {
            return;
        }
        let width = width as usize;
        let height = height as usize;

        let ds = self.stride();
        let ss = src.stride();
        let mut dest_cursor = dx as usize + dy as usize * ds;
        let mut src_cursor = sx as usize + sy as usize * ss;
        let dest_fb = self.slice_mut();
        let src_fb = src.slice();

        if opacity == Alpha8::OPAQUE {
            for _ in 0..height {
                memory_colors::_memcpy_blend32(dest_fb, dest_cursor, src_fb, src_cursor, width);
                dest_cursor += ds;
                src_cursor += ss;
            }
        } else {
            // TODO:
            for _ in 0..height {
                memory_colors::_memcpy_blend32(dest_fb, dest_cursor, src_fb, src_cursor, width);
                dest_cursor += ds;
                src_cursor += ss;
            }
        }
    }

    pub fn blt8(&mut self, src: &BitmapRef8, origin: Point, rect: Rect, palette: &[u32; 256]) {
        self.blt_convert(src, origin, rect, |c| {
            ARGB8888::from_argb(palette[c.0 as usize])
        });
    }

    pub fn blt_cw(&mut self, src: &BitmapRef32, origin: Point, rect: Rect) {
        let self_size = Size::new(self.height() as isize, self.width() as isize);
        let (mut dx, mut dy, sx, sy, width, height) =
            _adjust_blt_coords(self_size, src.size(), origin, rect);
        if width <= 0 || height <= 0 {
            return;
        }
        let width = width as usize;
        let height = height as usize;

        let ds = self.stride();
        let ss = src.stride();
        let temp = dx;
        dx = self_size.height() - dy;
        dy = temp;
        let mut p = dx as usize + dy as usize * ds - height as usize;
        let q0 = sx as usize + (sy as usize + height - 1) * ss;
        let stride_p = ds - height;
        let stride_q = ss;
        let dest_fb = self.slice_mut();
        let src_fb = src.slice();

        for x in 0..width {
            let mut q = q0 + x;
            for _ in 0..height {
                dest_fb[p] = src_fb[q];
                p += 1;
                q -= stride_q;
            }
            p += stride_p;
        }
    }

    #[inline]
    pub fn blt_transparent(
        &mut self,
        src: &BitmapRef,
        origin: Point,
        rect: Rect,
        color_key: IndexedColor,
    ) {
        match src {
            BitmapRef::Indexed(src) => self.blt_convert_opt(*src, origin, rect, |c| {
                if c == color_key {
                    None
                } else {
                    Some(c.into())
                }
            }),
            BitmapRef::Argb32(src) => self.blt_blend(src, origin, rect, Alpha8::OPAQUE),
        }
    }
}

impl OwnedBitmap32 {
    pub fn from_vec_rgba(mut vec: Vec<u8>, size: Size) -> Self {
        const MAGIC_NUMBER: usize = 4;
        let stride = size.width() as usize;
        let count = stride * size.height() as usize;
        let slice = unsafe {
            vec.resize(count * MAGIC_NUMBER, 0);
            let slice = vec.into_boxed_slice();
            let mut slice = ManuallyDrop::new(slice);
            let mut slice =
                Box::from_raw(slice_from_raw_parts_mut(
                    slice.as_mut_ptr() as *mut u32,
                    count,
                ));
            for pixel in slice.iter_mut() {
                let rgba: [u8; 4] = transmute(*pixel);
                let bgra = [rgba[2], rgba[1], rgba[0], rgba[3]];
                *pixel = transmute(bgra);
            }
            transmute::<_, Box<[ARGB8888]>>(slice)
        };
        Self::from_boxed_slice(slice, size)
    }

    pub fn from_bytes_rgba(bytes: &[u8], size: Size) -> Option<Self> {
        const MAGIC_NUMBER: usize = 4;
        let stride = size.width() as usize;
        let count = stride * size.height() as usize;
        if bytes.len() < count * MAGIC_NUMBER {
            return None;
        }
        let mut vec = Vec::with_capacity(count);
        for rgba in bytes.chunks_exact(MAGIC_NUMBER).take(count) {
            let rgba: [u8; MAGIC_NUMBER] = rgba.try_into().unwrap();
            let argb = ColorComponents::from_rgba(rgba[0], rgba[1], rgba[2], Alpha8(rgba[3]))
                .into_true_color();
            vec.push(argb);
        }
        Some(Self::from_vec(vec, size))
    }

    pub fn from_bytes_rgb(bytes: &[u8], size: Size) -> Option<Self> {
        const MAGIC_NUMBER: usize = 3;
        let stride = size.width() as usize;
        let count = stride * size.height() as usize;
        if bytes.len() < count * MAGIC_NUMBER {
            return None;
        }
        let mut vec = Vec::with_capacity(count);
        for rgb in bytes.chunks_exact(MAGIC_NUMBER).take(count) {
            let rgb: [u8; MAGIC_NUMBER] = rgb.try_into().unwrap();
            let argb = ColorComponents::from_rgba(rgb[0], rgb[1], rgb[2], Alpha8::OPAQUE)
                .into_true_color();
            vec.push(argb);
        }
        Some(Self::from_vec(vec, size))
    }
}

#[derive(Clone, Copy)]
pub enum BitmapRef<'a> {
    Indexed(&'a BitmapRef8<'a>),
    Argb32(&'a BitmapRef32<'a>),
}

impl Drawable for BitmapRef<'_> {
    type ColorType = Color;

    #[inline]
    fn size(&self) -> Size {
        match self {
            Self::Indexed(v) => v.size(),
            Self::Argb32(v) => v.size(),
        }
    }
}

impl GetPixel for BitmapRef<'_> {
    #[inline]
    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::ColorType {
        match self {
            Self::Indexed(v) => v.get_pixel_unchecked(point).into(),
            Self::Argb32(v) => v.get_pixel_unchecked(point).into(),
        }
    }
}

impl<'a> From<&'a BitmapRef8<'a>> for BitmapRef<'a> {
    #[inline]
    fn from(val: &'a BitmapRef8<'a>) -> BitmapRef<'a> {
        BitmapRef::Indexed(val)
    }
}

impl<'a> From<&'a BitmapRefMut8<'a>> for BitmapRef<'a> {
    #[inline]
    fn from(val: &'a BitmapRefMut8<'a>) -> Self {
        BitmapRef::Indexed(unsafe { transmute(val) })
    }
}

impl<'a> From<&'a BitmapRef32<'a>> for BitmapRef<'a> {
    #[inline]
    fn from(val: &'a BitmapRef32<'a>) -> BitmapRef {
        BitmapRef::Argb32(val)
    }
}

impl<'a> From<&'a BitmapRefMut32<'a>> for BitmapRef<'a> {
    #[inline]
    fn from(val: &'a BitmapRefMut32<'a>) -> Self {
        BitmapRef::Argb32(unsafe { transmute(val) })
    }
}

impl<'a> AsRef<BitmapRef<'a>> for BitmapRef<'a> {
    fn as_ref(&self) -> &BitmapRef<'a> {
        self
    }
}

pub enum BitmapRefMut<'a> {
    Indexed(BitmapRefMut8<'a>),
    Argb32(BitmapRefMut32<'a>),
}

impl Drawable for BitmapRefMut<'_> {
    type ColorType = Color;

    #[inline]
    fn size(&self) -> Size {
        match self {
            Self::Indexed(ref v) => v.size(),
            Self::Argb32(ref v) => v.size(),
        }
    }
}

impl<'a> BitmapRefMut<'a> {
    #[inline]
    pub fn as_const(&'a self) -> BitmapRef<'a> {
        match self {
            BitmapRefMut::Indexed(v) => BitmapRef::Indexed(v.as_ref()),
            BitmapRefMut::Argb32(v) => BitmapRef::Argb32(v.as_ref()),
        }
    }

    /// Returns a subset of the specified range of bitmaps.
    /// The function returns None if the rectangle points outside the range of the bitmap.
    pub fn view(&mut self, rect: Rect) -> Option<Self> {
        match self {
            BitmapRefMut::Indexed(v) => v.view(rect).map(|v| BitmapRefMut::Indexed(v)),
            BitmapRefMut::Argb32(v) => v.view(rect).map(|v| BitmapRefMut::Argb32(v)),
        }
    }
}

impl BitmapRefMut<'_> {
    #[inline]
    pub fn copy(&mut self, origin: Point, rect: Rect) {
        match self {
            Self::Indexed(ref mut v) => v.copy(origin, rect),
            Self::Argb32(ref mut v) => v.copy(origin, rect),
        }
    }

    #[inline]
    pub fn blt_transparent(
        &mut self,
        src: &BitmapRef,
        origin: Point,
        rect: Rect,
        color_key: IndexedColor,
    ) {
        match self {
            BitmapRefMut::Indexed(bitmap) => match src {
                BitmapRef::Indexed(src) => bitmap.blt_with_key(src, origin, rect, color_key),
                BitmapRef::Argb32(src) => bitmap.blt_convert_opt(*src, origin, rect, |c| {
                    if c.is_transparent() {
                        None
                    } else {
                        Some(c.into())
                    }
                }),
            },
            BitmapRefMut::Argb32(bitmap) => bitmap.blt_transparent(src, origin, rect, color_key),
        }
    }

    #[inline]
    pub fn map_indexed<F, R>(&mut self, f: F) -> Option<R>
    where
        F: FnOnce(&mut BitmapRefMut8) -> R,
    {
        match self {
            Self::Indexed(ref mut v) => Some(f(v)),
            Self::Argb32(_) => None,
        }
    }

    #[inline]
    pub fn map_argb32<F, R>(&mut self, f: F) -> Option<R>
    where
        F: FnOnce(&mut BitmapRefMut32) -> R,
    {
        match self {
            Self::Indexed(_) => None,
            Self::Argb32(ref mut v) => Some(f(v)),
        }
    }

    #[inline]
    pub const fn color_mode(&self) -> usize {
        match self {
            Self::Indexed(_) => 8,
            Self::Argb32(_) => 32,
        }
    }
}

impl GetPixel for BitmapRefMut<'_> {
    #[inline]
    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::ColorType {
        match self {
            Self::Indexed(ref v) => v.get_pixel_unchecked(point).into(),
            Self::Argb32(ref v) => v.get_pixel_unchecked(point).into(),
        }
    }
}

impl SetPixel for BitmapRefMut<'_> {
    #[inline]
    unsafe fn set_pixel_unchecked(&mut self, point: Point, pixel: Self::ColorType) {
        match self {
            Self::Indexed(ref mut v) => v.set_pixel_unchecked(point, pixel.into()),
            Self::Argb32(ref mut v) => v.set_pixel_unchecked(point, pixel.into()),
        }
    }
}

impl DrawGlyph for BitmapRefMut<'_> {
    #[inline]
    fn draw_glyph(&mut self, glyph: &[u8], size: Size, origin: Point, color: Self::ColorType) {
        match self {
            Self::Indexed(ref mut v) => v.draw_glyph(glyph, size, origin, color.into()),
            Self::Argb32(ref mut v) => v.draw_glyph(glyph, size, origin, color.into()),
        }
    }

    fn draw_glyph_cw(&mut self, glyph: &[u8], size: Size, origin: Point, color: Self::ColorType) {
        match self {
            Self::Indexed(ref mut v) => v.draw_glyph_cw(glyph, size, origin, color.into()),
            Self::Argb32(ref mut v) => v.draw_glyph_cw(glyph, size, origin, color.into()),
        }
    }
}

impl DrawRect for BitmapRefMut<'_> {
    #[inline]
    fn fill_rect(&mut self, rect: Rect, color: Self::ColorType) {
        match self {
            Self::Indexed(ref mut v) => v.fill_rect(rect, color.into()),
            Self::Argb32(ref mut v) => v.fill_rect(rect, color.into()),
        }
    }

    #[inline]
    fn draw_hline(&mut self, origin: Point, width: isize, color: Self::ColorType) {
        match self {
            Self::Indexed(ref mut v) => v.draw_hline(origin, width, color.into()),
            Self::Argb32(ref mut v) => v.draw_hline(origin, width, color.into()),
        }
    }

    #[inline]
    fn draw_vline(&mut self, origin: Point, height: isize, color: Self::ColorType) {
        match self {
            Self::Indexed(ref mut v) => v.draw_vline(origin, height, color.into()),
            Self::Argb32(ref mut v) => v.draw_vline(origin, height, color.into()),
        }
    }
}

impl<'a, 'b> Blt<BitmapRef<'b>> for BitmapRefMut<'a> {
    fn blt(&mut self, src: &BitmapRef<'b>, origin: Point, rect: Rect) {
        match self {
            BitmapRefMut::Indexed(ref mut bitmap) => match src {
                BitmapRef::Indexed(ref src) => bitmap.blt(src, origin, rect),
                BitmapRef::Argb32(ref src) => bitmap.blt32(src, origin, rect),
            },
            BitmapRefMut::Argb32(ref mut bitmap) => match src {
                BitmapRef::Indexed(ref src) => {
                    bitmap.blt8(src, origin, rect, &IndexedColor::COLOR_PALETTE)
                }
                BitmapRef::Argb32(ref src) => bitmap.blt(src, origin, rect),
            },
        }
    }
}

impl<'a, 'b> Blt<BitmapRef8<'b>> for BitmapRefMut<'a> {
    fn blt(&mut self, src: &BitmapRef8<'b>, origin: Point, rect: Rect) {
        match self {
            Self::Indexed(ref mut bitmap) => bitmap.blt(src, origin, rect),
            Self::Argb32(ref mut bitmap) => {
                bitmap.blt8(src, origin, rect, &IndexedColor::COLOR_PALETTE)
            }
        }
    }
}

impl<'a, 'b> Blt<BitmapRef32<'b>> for BitmapRefMut<'a> {
    fn blt(&mut self, src: &BitmapRef32<'b>, origin: Point, rect: Rect) {
        match self {
            Self::Indexed(ref mut bitmap) => bitmap.blt32(src, origin, rect),
            Self::Argb32(ref mut bitmap) => bitmap.blt(src, origin, rect),
        }
    }
}

impl<'a> From<BitmapRefMut8<'a>> for BitmapRefMut<'a> {
    #[inline]
    fn from(val: BitmapRefMut8<'a>) -> BitmapRefMut<'a> {
        Self::Indexed(val)
    }
}

impl<'a> From<BitmapRefMut32<'a>> for BitmapRefMut<'a> {
    #[inline]
    fn from(val: BitmapRefMut32<'a>) -> BitmapRefMut<'a> {
        Self::Argb32(val)
    }
}

impl<'a> AsMut<BitmapRefMut<'a>> for BitmapRefMut<'a> {
    #[inline]
    fn as_mut(&mut self) -> &mut BitmapRefMut<'a> {
        self
    }
}

pub enum OwnedBitmap {
    Indexed(OwnedBitmap8),
    Argb32(OwnedBitmap32),
}

impl Drawable for OwnedBitmap {
    type ColorType = Color;

    #[inline]
    fn size(&self) -> Size {
        match self {
            Self::Indexed(ref v) => v.size(),
            Self::Argb32(ref v) => v.size(),
        }
    }
}

impl OwnedBitmap {
    #[inline]
    pub fn new<'b, T: AsRef<BitmapRef<'b>>>(
        template_bitmap: &T,
        size: Size,
        bg_color: Color,
    ) -> OwnedBitmap {
        match template_bitmap.as_ref() {
            BitmapRef::Indexed(_) => Self::Indexed(OwnedBitmap8::new(size, bg_color.into())),
            BitmapRef::Argb32(_) => Self::Argb32(OwnedBitmap32::new(size, bg_color.into())),
        }
    }

    #[inline]
    pub fn same_format(&self, size: Size, bg_color: Color) -> OwnedBitmap {
        match self {
            Self::Indexed(_) => Self::Indexed(OwnedBitmap8::new(size, bg_color.into())),
            Self::Argb32(_) => Self::Argb32(OwnedBitmap32::new(size, bg_color.into())),
        }
    }

    #[inline]
    pub fn into_bitmap<'a>(self) -> BitmapRefMut<'a> {
        unsafe { transmute(self) }
    }

    #[inline]
    pub fn as_const<'a>(&'a self) -> BitmapRef<'a> {
        match self {
            OwnedBitmap::Indexed(v) => BitmapRef::Indexed(v.as_ref()),
            OwnedBitmap::Argb32(v) => BitmapRef::Argb32(v.as_ref()),
        }
    }
}

impl From<OwnedBitmap8> for OwnedBitmap {
    #[inline]
    fn from(val: OwnedBitmap8) -> Self {
        Self::Indexed(val)
    }
}

impl From<OwnedBitmap32> for OwnedBitmap {
    #[inline]
    fn from(val: OwnedBitmap32) -> Self {
        Self::Argb32(val)
    }
}

impl<'a> AsMut<BitmapRefMut<'a>> for OwnedBitmap {
    #[inline]
    fn as_mut(&mut self) -> &mut BitmapRefMut<'a> {
        unsafe { transmute(self) }
    }
}

/// A special bitmap type that can be used for operations such as transparency and shading.
pub struct OperationalBitmap {
    size: Size,
    vec: Vec<u8>,
}

impl PixelColor for u8 {}

impl Drawable for OperationalBitmap {
    type ColorType = u8;

    #[inline]
    fn size(&self) -> Size {
        self.size
    }
}

impl RasterImage for OperationalBitmap {
    #[inline]
    fn stride(&self) -> usize {
        self.width()
    }

    #[inline]
    fn slice(&self) -> &[Self::ColorType] {
        self.vec.as_slice()
    }
}

impl MutableRasterImage for OperationalBitmap {
    #[inline]
    fn slice_mut(&mut self) -> &mut [Self::ColorType] {
        self.vec.as_mut_slice()
    }
}

impl OperationalBitmap {
    #[inline]
    pub const fn new(size: Size) -> Self {
        let vec = Vec::new();
        Self { size, vec }
    }

    #[inline]
    pub fn from_slice(slice: &[u8], size: Size) -> Self {
        let vec = Vec::from(slice);
        Self { size, vec }
    }

    #[inline]
    pub fn from_vec(vec: Vec<u8>, size: Size) -> Self {
        Self { size, vec }
    }

    #[inline]
    pub fn from_pixels<F, T>(data: &T, mut kernel: F) -> OperationalBitmap
    where
        T: GetPixel + ?Sized,
        F: FnMut(<T as Drawable>::ColorType) -> u8,
    {
        let size = data.size();
        let vec = data.all_pixels().map(|v| kernel(v)).collect();
        OperationalBitmap { size, vec }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.fill(0);
    }

    pub fn fill(&mut self, color: u8) {
        let count = self.stride() * self.height() as usize;
        if self.vec.capacity() >= count {
            self.vec.fill(color);
        } else {
            self.vec.resize(count, color);
        }
    }

    /// Draws a straight line at high speed using Bresenham's line algorithm.
    #[inline]
    pub fn draw_line<F>(&mut self, c1: Point, c2: Point, mut kernel: F)
    where
        F: FnMut(&mut OperationalBitmap, Point),
    {
        c1.line_to(c2, |point| kernel(self, point));
    }

    /// Draws an anti-aliased line using Xiaolin Wu's algorithm.
    pub fn draw_line_anti_aliasing_f<F>(&mut self, c1: Point, c2: Point, mut kernel: F)
    where
        F: FnMut(&mut OperationalBitmap, Point, f64),
    {
        let mut plot = |bitmap: &mut OperationalBitmap, x: f64, y: f64, level: f64| {
            kernel(bitmap, Point::new(x as isize, y as isize), level);
        };
        #[inline]
        fn ipart(v: f64) -> f64 {
            libm::floor(v)
        }
        #[inline]
        fn fpart(v: f64) -> f64 {
            v - ipart(v)
        }
        #[inline]
        fn rfpart(v: f64) -> f64 {
            1.0 - fpart(v)
        }

        let mut x1 = c1.x as f64;
        let mut x2 = c2.x as f64;
        let mut y1 = c1.y as f64;
        let mut y2 = c2.y as f64;

        let width = f64::max(x1, x2) - f64::min(x1, x2);
        let height = f64::max(y1, y2) - f64::min(y1, y2);
        let steep = height > width;

        if steep {
            swap(&mut x1, &mut y1);
            swap(&mut x2, &mut y2);
        }
        if x1 > x2 {
            swap(&mut x1, &mut x2);
            swap(&mut y1, &mut y2);
        }
        let dx = x2 - x1;
        let dy = y2 - y1;
        let gradient = if dx == 0.0 { 1.0 } else { dy / dx };

        let xend = libm::round(x1);
        let yend = y1 + gradient * (xend - x1);
        let xgap = rfpart(x1 + 0.5);
        let xpxl1 = xend;
        let ypxl1 = ipart(yend);
        if steep {
            plot(self, ypxl1, xpxl1, rfpart(yend) * xgap);
            plot(self, ypxl1 + 1.0, xpxl1, fpart(yend) * xgap);
        } else {
            plot(self, xpxl1, ypxl1, rfpart(yend) * xgap);
            plot(self, xpxl1, ypxl1 + 1.0, fpart(yend) * xgap);
        }
        let mut intery = yend + gradient;

        let xend = libm::round(x2);
        let yend = y2 + gradient * (xend - x2);
        let xgap = fpart(x2 + 0.5);
        let xpxl2 = xend;
        let ypxl2 = ipart(yend);
        if steep {
            plot(self, ypxl2, xpxl2, rfpart(yend) * xgap);
            plot(self, ypxl2 + 1.0, xpxl2, fpart(yend) * xgap);
        } else {
            plot(self, xpxl2, ypxl2, rfpart(yend) * xgap);
            plot(self, xpxl2, ypxl2 + 1.0, fpart(yend) * xgap);
        }

        if steep {
            for i in (xpxl1 as isize + 1)..(xpxl2 as isize) {
                let y = i as f64;
                plot(self, intery, y, rfpart(intery));
                plot(self, intery + 1.0, y, fpart(intery));
                intery += gradient;
            }
        } else {
            for i in (xpxl1 as isize + 1)..(xpxl2 as isize) {
                let x = i as f64;
                plot(self, x, intery, rfpart(intery));
                plot(self, x, intery + 1.0, fpart(intery));
                intery += gradient;
            }
        }
    }

    #[deprecated]
    pub fn draw_line_anti_aliasing_i<F>(
        &mut self,
        c1: Point,
        c2: Point,
        scale: isize,
        mut kernel: F,
    ) where
        F: FnMut(&mut OperationalBitmap, Point, u8),
    {
        const FRAC_SHIFT: isize = 6;
        const ONE: isize = 1 << FRAC_SHIFT;
        const FRAC_MASK: isize = ONE - 1;
        const FRAC_HALF: isize = ONE / 2;
        const IPART_MASK: isize = !FRAC_MASK;

        let mut plot = |bitmap: &mut OperationalBitmap, x: isize, y: isize, level: isize| {
            kernel(
                bitmap,
                Point::new(x >> FRAC_SHIFT, y >> FRAC_SHIFT),
                (0xFF * level >> FRAC_SHIFT) as u8,
            );
        };
        #[inline]
        fn ipart(v: isize) -> isize {
            v & IPART_MASK
        }
        #[inline]
        fn round(v: isize) -> isize {
            ipart(v + FRAC_HALF)
        }
        #[inline]
        fn fpart(v: isize) -> isize {
            v & FRAC_MASK
        }
        #[inline]
        fn rfpart(v: isize) -> isize {
            ONE - fpart(v)
        }
        #[inline]
        fn mul(a: isize, b: isize) -> isize {
            (a * b) >> FRAC_SHIFT
        }
        #[inline]
        fn div(a: isize, b: isize) -> Option<isize> {
            (a << FRAC_SHIFT).checked_div(b)
        }

        let mut x1 = (c1.x() << FRAC_SHIFT) / scale;
        let mut x2 = (c2.x() << FRAC_SHIFT) / scale;
        let mut y1 = (c1.y() << FRAC_SHIFT) / scale;
        let mut y2 = (c2.y() << FRAC_SHIFT) / scale;

        let width = isize::max(x1, x2) - isize::min(x1, x2);
        let height = isize::max(y1, y2) - isize::min(y1, y2);
        let steep = height > width;

        if steep {
            swap(&mut x1, &mut y1);
            swap(&mut x2, &mut y2);
        }
        if x1 > x2 {
            swap(&mut x1, &mut x2);
            swap(&mut y1, &mut y2);
        }
        let dx = x2 - x1;
        let dy = y2 - y1;
        let gradient = div(dy, dx).unwrap_or(ONE);
        //if dx == 0 { ONE } else { div(dy, dx) };

        let xend = round(x1);
        let yend = y1 + mul(gradient, xend - x1);
        let xgap = rfpart(x1 + FRAC_HALF);
        let xpxl1 = xend;
        let ypxl1 = ipart(yend);
        if steep {
            plot(self, ypxl1, xpxl1, mul(rfpart(yend), xgap));
            plot(self, ypxl1 + ONE, xpxl1, mul(fpart(yend), xgap));
        } else {
            plot(self, xpxl1, ypxl1, mul(rfpart(yend), xgap));
            plot(self, xpxl1, ypxl1 + ONE, mul(fpart(yend), xgap));
        }
        let mut intery = yend + gradient;

        let xend = round(x2);
        let yend = y2 + mul(gradient, xend - x2);
        let xgap = fpart(x2 + FRAC_HALF);
        let xpxl2 = xend;
        let ypxl2 = ipart(yend);
        if steep {
            plot(self, ypxl2, xpxl2, mul(rfpart(yend), xgap));
            plot(self, ypxl2 + ONE, xpxl2, mul(fpart(yend), xgap));
        } else {
            plot(self, xpxl2, ypxl2, mul(rfpart(yend), xgap));
            plot(self, xpxl2, ypxl2 + ONE, mul(fpart(yend), xgap));
        }

        if steep {
            for i in (xpxl1 >> FRAC_SHIFT) + 1..(xpxl2 >> FRAC_SHIFT) {
                let y = i << FRAC_SHIFT;
                plot(self, intery, y, rfpart(intery));
                plot(self, intery + ONE, y, fpart(intery));
                intery += gradient;
            }
        } else {
            for i in (xpxl1 >> FRAC_SHIFT) + 1..(xpxl2 >> FRAC_SHIFT) {
                let x = i << FRAC_SHIFT;
                plot(self, x, intery, rfpart(intery));
                plot(self, x, intery + ONE, fpart(intery));
                intery += gradient;
            }
        }
    }

    /// Like box linear filter
    pub fn blur(&mut self, radius: isize, level: usize) {
        let bounds = self.bounds();
        let length = radius * 2 + 1;

        for y in (length..bounds.height()).rev() {
            for x in 0..bounds.width() {
                let mut acc = 0;
                for r in 0..length {
                    acc += unsafe { self.get_pixel_unchecked(Point::new(x, y - r)) as usize };
                }
                unsafe {
                    self.set_pixel_unchecked(Point::new(x, y), (acc / length as usize) as u8);
                }
            }
        }
        for y in (0..length).rev() {
            for x in 0..bounds.width() {
                let mut acc = 0;
                for r in 0..y {
                    acc += unsafe { self.get_pixel_unchecked(Point::new(x, y - r)) as usize };
                }
                unsafe {
                    self.set_pixel_unchecked(Point::new(x, y), (acc / length as usize) as u8);
                }
            }
        }

        for y in 0..bounds.height() {
            for x in (length..bounds.width()).rev() {
                let mut acc = 0;
                for r in 0..length {
                    acc += unsafe { self.get_pixel_unchecked(Point::new(x - r, y)) as usize };
                }
                unsafe {
                    self.set_pixel_unchecked(
                        Point::new(x, y),
                        usize::min(255, (acc / length as usize) * level / 256) as u8,
                    );
                }
            }
            for x in (0..length).rev() {
                let mut acc = 0;
                for r in 0..x {
                    acc += unsafe { self.get_pixel_unchecked(Point::new(x - r, y)) as usize };
                }
                unsafe {
                    self.set_pixel_unchecked(
                        Point::new(x, y),
                        usize::min(255, (acc / length as usize) * level / 256) as u8,
                    );
                }
            }
        }
    }

    pub fn blt_to<T, F>(&self, dest: &mut T, origin: Point, rect: Rect, mut kernel: F)
    where
        T: Drawable + GetPixel + SetPixel,
        F: FnMut(u8, <T as Drawable>::ColorType) -> <T as Drawable>::ColorType,
    {
        let (dx, dy, sx, sy, width, height) =
            _adjust_blt_coords(dest.size(), self.size(), origin, rect);
        if width <= 0 || height <= 0 {
            return;
        }

        for y in 0..height {
            for x in 0..width {
                let dp = Point::new(dx + x, dy + y);
                let sp = Point::new(sx + x, sy + y);
                unsafe {
                    dest.set_pixel_unchecked(
                        dp,
                        kernel(self.get_pixel_unchecked(sp), dest.get_pixel_unchecked(dp)),
                    );
                }
            }
        }
    }

    #[inline]
    pub fn blt_shadow(&self, dest: &mut BitmapRefMut32, origin: Point, rect: Rect) {
        self.blt_to(dest, origin, rect, |a, b| b.shadowed(a))
    }

    pub fn blt_from<T, F>(&mut self, src: &T, origin: Point, rect: Rect, mut kernel: F)
    where
        T: GetPixel,
        F: FnMut(<T as Drawable>::ColorType, u8) -> u8,
    {
        let (dx, dy, sx, sy, width, height) =
            _adjust_blt_coords(self.size(), src.size(), origin, rect);
        if width <= 0 || height <= 0 {
            return;
        }

        for y in 0..height {
            for x in 0..width {
                let dp = Point::new(dx + x, dy + y);
                let sp = Point::new(sx + x, sy + y);
                unsafe {
                    self.set_pixel_unchecked(
                        dp,
                        kernel(src.get_pixel_unchecked(sp), self.get_pixel_unchecked(dp)),
                    );
                }
            }
        }
    }

    pub fn draw_to(&self, dest: &mut BitmapRefMut, origin: Point, rect: Rect, color: Color) {
        match dest {
            BitmapRefMut::Indexed(_) => {
                // TODO:
            }
            BitmapRefMut::Argb32(bitmap) => {
                let color = color.into_true_color();
                self.blt_to(bitmap, origin, rect, |a, b| {
                    let mut c = color.components();
                    c.a = Alpha8(a);
                    b.blend_draw(c.into())
                });
            }
        }
    }
}

/// Adjust the coordinates for blt.
///
/// Returns the adjusted destination x, y, source x, y, width and height.
fn _adjust_blt_coords(
    dest_size: Size,
    src_size: Size,
    origin: Point,
    rect: Rect,
) -> (isize, isize, isize, isize, isize, isize) {
    let mut dx = origin.x;
    let mut dy = origin.y;
    let mut sx = rect.min_x();
    let mut sy = rect.min_y();
    let mut width = rect.width();
    let mut height = rect.height();
    let dw = dest_size.width();
    let dh = dest_size.height();
    let sw = src_size.width();
    let sh = src_size.height();

    if sx < 0 {
        dx -= sx;
        width += sx;
        sx = 0;
    }
    if sy < 0 {
        dy -= sy;
        height += sy;
        sy = 0;
    }
    if dx < 0 {
        sx -= dx;
        width += dx;
        dx = 0;
    }
    if dy < 0 {
        sy -= dy;
        height += dy;
        dy = 0;
    }
    if sx + width > sw {
        width = sw - sx;
    }
    if sy + height > sh {
        height = sh - sy;
    }
    if dx + width >= dw {
        width = dw - dx;
    }
    if dy + height >= dh {
        height = dh - dy;
    }

    (dx, dy, sx, sy, width, height)
}

mod memory_colors {
    use super::*;

    #[inline]
    pub fn _memset_colors8(
        slice: &mut [IndexedColor],
        cursor: usize,
        size: usize,
        color: IndexedColor,
    ) {
        unsafe {
            let slice = slice.get_unchecked_mut(cursor);
            let color = color.0;
            let mut ptr: *mut u8 = transmute(slice);
            let mut remain = size;

            let prologue = usize::min(ptr as usize & 0x0F, remain);
            remain -= prologue;
            for _ in 0..prologue {
                ptr.write_volatile(color);
                ptr = ptr.add(1);
            }

            if remain > 16 {
                let color32 = color as u32
                    | (color as u32) << 8
                    | (color as u32) << 16
                    | (color as u32) << 24;
                let color64 = color32 as u64 | (color32 as u64) << 32;
                let color128 = color64 as u128 | (color64 as u128) << 64;
                let count = remain / 16;
                let mut ptr2 = ptr as *mut u128;

                for _ in 0..count {
                    ptr2.write_volatile(color128);
                    ptr2 = ptr2.add(1);
                }

                ptr = ptr2 as *mut u8;
                remain -= count * 16;
            }

            for _ in 0..remain {
                ptr.write_volatile(color);
                ptr = ptr.add(1);
            }
        }
    }

    #[inline]
    pub fn _memset_colors16(slice: &mut [RGB565], cursor: usize, count: usize, color: RGB565) {
        for v in unsafe { slice.get_unchecked_mut(cursor..cursor + count) }.iter_mut() {
            *v = color;
        }
    }

    #[inline]
    pub fn _memset_colors32(slice: &mut [ARGB8888], cursor: usize, count: usize, color: ARGB8888) {
        for v in unsafe { slice.get_unchecked_mut(cursor..cursor + count) }.iter_mut() {
            *v = color;
        }
    }

    // Alpha blending
    #[inline]
    pub fn _memcpy_blend32(
        dest: &mut [ARGB8888],
        dest_cursor: usize,
        src: &[ARGB8888],
        src_cursor: usize,
        count: usize,
    ) {
        let dest = unsafe { &mut dest.get_unchecked_mut(dest_cursor..dest_cursor + count) };
        let src = unsafe { &src.get_unchecked(src_cursor..src_cursor + count) };
        for (dest, src) in dest.iter_mut().zip(src.iter()) {
            *dest = dest.blend_draw(*src);
        }
    }
}

define_bitmap!(1, u8, Monochrome, Octet,);

impl BitmapRef1<'_> {
    #[inline]
    fn slice(&self) -> &[Octet] {
        self.slice
    }

    #[inline]
    pub const fn stride(&self) -> usize {
        self.stride
    }
}

impl GetPixel for BitmapRef1<'_> {
    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::ColorType {
        let bit_position = point.x as usize & 7;
        let index = (point.x as usize / 8) + self.stride * point.y as usize;
        self.slice.get_unchecked(index).get(bit_position)
    }
}

impl BitmapRefMut1<'_> {
    #[inline]
    fn slice(&self) -> &[Octet] {
        unsafe { &mut *self.slice.get() }
    }

    #[inline]
    fn slice_mut(&mut self) -> &mut [Octet] {
        self.slice.get_mut()
    }

    #[inline]
    pub const fn stride(&self) -> usize {
        self.stride
    }

    pub fn copy_from<'a, T: AsRef<BitmapRef1<'a>>>(&mut self, other: &T) {
        unsafe {
            let p = self.slice_mut();
            let q = other.as_ref().slice();
            let count = p.len();
            copy_nonoverlapping(q.as_ptr(), p.as_mut_ptr(), count);
        }
    }
}

impl GetPixel for BitmapRefMut1<'_> {
    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::ColorType {
        self.as_ref().get_pixel_unchecked(point)
    }
}

impl SetPixel for BitmapRefMut1<'_> {
    unsafe fn set_pixel_unchecked(&mut self, point: Point, pixel: Self::ColorType) {
        let bit_position = point.x as usize & 7;
        let index = (point.x as usize / 8) + self.stride * point.y as usize;
        self.slice
            .get_mut()
            .get_unchecked_mut(index)
            .set(bit_position, pixel);
    }
}

define_bitmap!(4, u8, IndexedColor4, IndexedColorPair44,);

impl BitmapRef4<'_> {
    #[inline]
    pub fn slice(&self) -> &[IndexedColorPair44] {
        self.slice
    }

    #[inline]
    pub const fn stride(&self) -> usize {
        self.stride
    }
}

impl BitmapRefMut4<'_> {
    #[inline]
    pub fn slice(&self) -> &[IndexedColorPair44] {
        unsafe { &*self.slice.get() }
    }

    #[inline]
    pub fn slice_mut(&mut self) -> &mut [IndexedColorPair44] {
        self.slice.get_mut()
    }

    #[inline]
    pub const fn stride(&self) -> usize {
        self.stride
    }
}

impl GetPixel for BitmapRef4<'_> {
    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::ColorType {
        let x = point.x as usize;
        let y = point.y as usize;
        let pair = self.slice.get_unchecked(y * self.stride + x / 2);
        if (x & 1) == 0 {
            pair.lhs()
        } else {
            pair.rhs()
        }
    }
}
