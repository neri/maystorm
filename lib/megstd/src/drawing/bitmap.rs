use super::*;
use alloc::{borrow::ToOwned, boxed::Box, vec::Vec};
use core::{
    borrow::Borrow,
    cell::UnsafeCell,
    convert::TryFrom,
    intrinsics::copy_nonoverlapping,
    mem::{swap, transmute},
    num::NonZeroUsize,
};
use num_derive::FromPrimitive;

pub trait Blt<T: Drawable>: Drawable {
    fn blt(&mut self, src: &T, origin: Point, rect: Rect);
}

pub trait BltScale<T: Drawable>: Drawable {
    fn blt_scale(&mut self, src: &T, origin: Point, rect: Rect, scale: ScaleMode);
}

pub trait BasicDrawing: SetPixel {
    fn fill_rect(&mut self, rect: Rect, color: Self::ColorType);
    fn draw_hline(&mut self, origin: Point, width: isize, color: Self::ColorType);
    fn draw_vline(&mut self, origin: Point, height: isize, color: Self::ColorType);

    fn clear(&mut self) {
        self.fill_rect(self.bounds(), Default::default());
    }

    fn draw_rect(&mut self, rect: Rect, color: Self::ColorType) {
        let coords = match Coordinates::from_rect(rect) {
            Ok(v) => v,
            Err(_) => return,
        };
        let width = rect.width();
        let height = rect.height();
        self.draw_hline(coords.left_top(), width, color);
        self.draw_hline(coords.left_bottom() - Movement::new(0, 1), width, color);
        if height > 2 {
            self.draw_vline(coords.left_top() + Movement::new(0, 1), height - 2, color);
            self.draw_vline(coords.right_top() + Movement::new(-1, 1), height - 2, color);
        }
    }

    fn draw_circle(&mut self, origin: Point, radius: isize, color: Self::ColorType) {
        let rect = Rect {
            origin: origin - radius,
            size: Size::new(radius * 2, radius * 2),
        };
        self.draw_round_rect(rect, radius, color);
    }

    fn fill_circle(&mut self, origin: Point, radius: isize, color: Self::ColorType) {
        let rect = Rect {
            origin: origin - radius,
            size: Size::new(radius * 2, radius * 2),
        };
        self.fill_round_rect(rect, radius, color);
    }

    fn fill_round_rect(&mut self, rect: Rect, radius: isize, color: Self::ColorType) {
        let width = rect.size.width;
        let height = rect.size.height;
        let dx = rect.origin.x;
        let dy = rect.origin.y;

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
        let width = rect.size.width;
        let height = rect.size.height;
        let dx = rect.origin.x;
        let dy = rect.origin.y;

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

    fn fill_round_rect_outside(&mut self, rect: Rect, radius: isize, color: Self::ColorType) {
        let width = rect.size.width;
        let height = rect.size.height;
        let dx = rect.origin.x;
        let dy = rect.origin.y;
        let left = rect.min_x();
        let right = rect.max_x();

        let mut radius = radius;
        if radius * 2 > width {
            radius = width / 2;
        }
        if radius * 2 > height {
            radius = height / 2;
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
                let lx = dx + bx;
                if lx > left {
                    self.draw_hline(Point::new(left, dy + by), lx - left, color);
                    self.draw_hline(Point::new(left, dy + qh - by), lx - left, color);
                }
                let rx = dx + bx + dw;
                if rx < right {
                    self.draw_hline(Point::new(rx, dy + by), right - rx, color);
                    self.draw_hline(Point::new(rx, dy + qh - by), right - rx, color);
                }
            }

            {
                let bx = radius - cx;
                let by = radius - cy;
                let dw = width - bx * 2 - 1;
                let lx = dx + bx;
                if lx > left {
                    self.draw_hline(Point::new(left, dy + by), lx - left, color);
                    self.draw_hline(Point::new(left, dy + qh - by), lx - left, color);
                }
                let rx = dx + bx + dw;
                if rx < right {
                    self.draw_hline(Point::new(rx, dy + by), right - rx, color);
                    self.draw_hline(Point::new(rx, dy + qh - by), right - rx, color);
                }
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
                let height = c2.y() - c1.y();
                self.draw_vline(c1, height, color);
            } else {
                let height = c1.y() - c2.y();
                self.draw_vline(c2, height, color);
            }
        } else if c1.y() == c2.y() {
            if c1.x() < c2.x() {
                let width = c2.x() - c1.x();
                self.draw_hline(c1, width, color);
            } else {
                let width = c1.x() - c2.x();
                self.draw_hline(c2, width, color);
            }
        } else {
            c1.line_to(c2, |point| {
                self.set_pixel(point, color);
            });
        }
    }
}

pub trait RasterFontWriter: SetPixel {
    fn draw_font(&mut self, src: &[u8], size: Size, origin: Point, color: Self::ColorType) {
        let stride = (size.width as usize + 7) / 8;

        let mut coords = match Coordinates::from_rect(Rect { origin, size }) {
            Ok(v) => v,
            Err(_) => return,
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
                let data = unsafe { src.get_unchecked(cursor + i) };
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
                let data = unsafe { src.get_unchecked(cursor + w8) };
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
}

pub trait BltConvert<T: ColorTrait>: MutableRasterImage {
    fn blt_convert<U, F>(&mut self, src: &U, origin: Point, rect: Rect, mut f: F)
    where
        U: RasterImage<ColorType = T>,
        F: FnMut(T) -> Self::ColorType,
    {
        let (dx, dy, sx, sy, width, height) =
            adjust_blt_coords(self.size(), src.size(), origin, rect);
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
        let mut dx = origin.x;
        let mut dy = origin.y;
        let mut sx = rect.origin.x;
        let mut sy = rect.origin.y;
        let mut width = rect.width();
        let mut height = rect.height();

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
        let sw = src.width() as isize;
        let sh = src.height() as isize;
        if width > sx + sw {
            width = sw - sx;
        }
        if height > sy + sh {
            height = sh - sy;
        }
        let r = dx + width;
        let b = dy + height;
        let dw = self.width() as isize;
        let dh = self.height() as isize;
        if r >= dw {
            width = dw - dx;
        }
        if b >= dh {
            height = dh - dy;
        }
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

#[repr(C)]
pub struct ConstBitmap8<'a> {
    width: usize,
    height: usize,
    stride: usize,
    slice: &'a [IndexedColor],
}

impl Drawable for ConstBitmap8<'_> {
    type ColorType = IndexedColor;

    #[inline]
    fn width(&self) -> usize {
        self.width
    }

    #[inline]
    fn height(&self) -> usize {
        self.height
    }
}

impl RasterImage for ConstBitmap8<'_> {
    #[inline]
    fn stride(&self) -> usize {
        self.stride
    }

    #[inline]
    fn slice(&self) -> &[Self::ColorType] {
        self.slice
    }
}

impl<'a> ConstBitmap8<'a> {
    #[inline]
    pub const fn from_slice(slice: &'a [IndexedColor], size: Size, stride: usize) -> Self {
        Self {
            width: size.width() as usize,
            height: size.height() as usize,
            stride,
            slice,
        }
    }

    #[inline]
    pub const fn from_bytes(bytes: &'a [u8], size: Size) -> Self {
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

impl<'a> const AsRef<ConstBitmap8<'a>> for ConstBitmap8<'a> {
    #[inline]
    fn as_ref(&self) -> &ConstBitmap8<'a> {
        self
    }
}

#[repr(C)]
pub struct Bitmap8<'a> {
    width: usize,
    height: usize,
    stride: usize,
    slice: UnsafeCell<&'a mut [IndexedColor]>,
}

impl Drawable for Bitmap8<'_> {
    type ColorType = IndexedColor;

    #[inline]
    fn width(&self) -> usize {
        self.width
    }

    #[inline]
    fn height(&self) -> usize {
        self.height
    }
}

impl RasterImage for Bitmap8<'_> {
    #[inline]
    fn stride(&self) -> usize {
        self.stride
    }

    #[inline]
    fn slice(&self) -> &[Self::ColorType] {
        unsafe { &*self.slice.get() }
    }
}

impl MutableRasterImage for Bitmap8<'_> {
    #[inline]
    fn slice_mut(&mut self) -> &mut [Self::ColorType] {
        self.slice.get_mut()
    }
}

impl<'a> Bitmap8<'a> {
    #[inline]
    pub const fn from_slice(slice: &'a mut [IndexedColor], size: Size, stride: usize) -> Self {
        Self {
            width: size.width() as usize,
            height: size.height() as usize,
            stride,
            slice: UnsafeCell::new(slice),
        }
    }

    #[inline]
    pub const fn from_bytes(bytes: &'a mut [u8], size: Size) -> Self {
        Self {
            width: size.width() as usize,
            height: size.height() as usize,
            stride: size.width() as usize,
            slice: unsafe { transmute(bytes) },
        }
    }

    /// Clone a bitmap
    #[inline]
    pub fn clone(&self) -> Bitmap8<'a> {
        let slice = unsafe { self.slice.get().as_mut().unwrap() };
        Self {
            width: self.width(),
            height: self.height(),
            stride: self.stride(),
            slice: UnsafeCell::new(slice),
        }
    }
}

impl Bitmap8<'static> {
    /// SAFETY: Must guarantee the existence of the `ptr`.
    #[inline]
    pub unsafe fn from_static(ptr: *mut IndexedColor, size: Size, stride: usize) -> Self {
        let slice = core::slice::from_raw_parts_mut(ptr, size.height() as usize * stride);
        Self {
            width: size.width() as usize,
            height: size.height() as usize,
            stride,
            slice: UnsafeCell::new(slice),
        }
    }
}

impl BltConvert<TrueColor> for Bitmap8<'_> {}
impl BltConvert<IndexedColor> for Bitmap8<'_> {}

impl<'a> Bitmap8<'a> {
    #[inline]
    pub const fn as_const(&self) -> &'a ConstBitmap8<'a> {
        unsafe { transmute(self) }
    }

    pub fn blt<'b, T: AsRef<ConstBitmap8<'b>>>(&mut self, src: &'b T, origin: Point, rect: Rect) {
        let src = src.as_ref();

        let (dx, dy, sx, sy, width, height) =
            adjust_blt_coords(self.size(), src.size(), origin, rect);
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

        if ds == width && ss == width {
            memory_colors::memcpy_colors8(dest_fb, dest_cursor, src_fb, src_cursor, width * height);
        } else {
            for _ in 0..height {
                memory_colors::memcpy_colors8(dest_fb, dest_cursor, src_fb, src_cursor, width);
                dest_cursor += ds;
                src_cursor += ss;
            }
        }
    }

    pub fn blt_with_key<'b, T: AsRef<ConstBitmap8<'b>>>(
        &mut self,
        src: &'b T,
        origin: Point,
        rect: Rect,
        color_key: <Self as Drawable>::ColorType,
    ) {
        let src = src.as_ref();

        let (dx, dy, sx, sy, width, height) =
            adjust_blt_coords(self.size(), src.size(), origin, rect);
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
    pub fn blt32<'b, T: AsRef<ConstBitmap32<'b>>>(
        &mut self,
        src: &'b T,
        origin: Point,
        rect: Rect,
    ) {
        self.blt_convert(src.as_ref(), origin, rect, |c| {
            IndexedColor::from_rgb(c.rgb())
        });
    }
}

impl Bitmap8<'_> {
    pub fn view<F, R>(&mut self, rect: Rect, f: F) -> Option<R>
    where
        F: FnOnce(&mut Bitmap) -> R,
    {
        let coords = match Coordinates::try_from(rect) {
            Ok(v) => v,
            Err(_) => return None,
        };
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

        let offset = rect.x() as usize + rect.y() as usize * stride;
        let new_len = (rect.height() as usize - 1) * stride + rect.width() as usize;
        let r = {
            let slice = self.slice_mut();
            let mut view = Bitmap8 {
                width: rect.width() as usize,
                height: rect.height() as usize,
                stride,
                slice: UnsafeCell::new(&mut slice[offset..offset + new_len]),
            };
            let mut bitmap = Bitmap::from(&mut view);
            f(&mut bitmap)
        };
        Some(r)
    }
}

impl BasicDrawing for Bitmap8<'_> {
    fn fill_rect(&mut self, rect: Rect, color: Self::ColorType) {
        let mut width = rect.width();
        let mut height = rect.height();
        let mut dx = rect.x();
        let mut dy = rect.y();

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
        if r >= self.width as isize {
            width = self.width as isize - dx;
        }
        if b >= self.height as isize {
            height = self.height as isize - dy;
        }
        if width <= 0 || height <= 0 {
            return;
        }

        let width = width as usize;
        let height = height as usize;
        let stride = self.stride;
        let mut cursor = dx as usize + dy as usize * stride;
        if stride == width {
            memory_colors::memset_colors8(self.slice_mut(), cursor, width * height, color);
        } else {
            for _ in 0..height {
                memory_colors::memset_colors8(self.slice_mut(), cursor, width, color);
                cursor += stride;
            }
        }
    }

    fn draw_hline(&mut self, origin: Point, width: isize, color: Self::ColorType) {
        let mut dx = origin.x;
        let dy = origin.y;
        let mut w = width;

        if dy < 0 || dy >= (self.height as isize) {
            return;
        }
        if dx < 0 {
            w += dx;
            dx = 0;
        }
        let r = dx + w;
        if r >= (self.width as isize) {
            w = (self.width as isize) - dx;
        }
        if w <= 0 {
            return;
        }

        let cursor = dx as usize + dy as usize * self.stride;
        memory_colors::memset_colors8(self.slice_mut(), cursor, w as usize, color);
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

impl RasterFontWriter for Bitmap8<'_> {}

impl<'a> const AsRef<ConstBitmap8<'a>> for Bitmap8<'a> {
    #[inline]
    fn as_ref(&self) -> &ConstBitmap8<'a> {
        self.as_const()
    }
}

impl<'a> const AsMut<Bitmap8<'a>> for Bitmap8<'a> {
    #[inline]
    fn as_mut(&mut self) -> &mut Bitmap8<'a> {
        self
    }
}

impl ToOwned for Bitmap8<'_> {
    type Owned = OwnedBitmap8;

    #[inline]
    fn to_owned(&self) -> Self::Owned {
        let vec = self.slice().to_vec();
        OwnedBitmap8::from_vec(vec, self.size())
    }
}

pub struct OwnedBitmap8 {
    width: usize,
    height: usize,
    stride: usize,
    slice: UnsafeCell<Box<[IndexedColor]>>,
}

impl Drawable for OwnedBitmap8 {
    type ColorType = IndexedColor;

    #[inline]
    fn width(&self) -> usize {
        self.width
    }

    #[inline]
    fn height(&self) -> usize {
        self.height
    }
}

impl RasterImage for OwnedBitmap8 {
    #[inline]
    fn stride(&self) -> usize {
        self.stride
    }

    #[inline]
    fn slice(&self) -> &[Self::ColorType] {
        unsafe { &*self.slice.get() }
    }
}

impl MutableRasterImage for OwnedBitmap8 {
    #[inline]
    fn slice_mut(&mut self) -> &mut [Self::ColorType] {
        self.slice.get_mut()
    }
}

impl OwnedBitmap8 {
    #[inline]
    pub fn new(size: Size, bg_color: IndexedColor) -> Self {
        let len = size.width() as usize * size.height() as usize;
        let mut vec = Vec::with_capacity(len);
        vec.resize(len, bg_color);
        Self::from_vec(vec, size)
    }

    #[inline]
    pub fn from_vec(vec: Vec<IndexedColor>, size: Size) -> Self {
        Self {
            width: size.width as usize,
            height: size.height as usize,
            stride: size.width as usize,
            slice: UnsafeCell::new(vec.into_boxed_slice()),
        }
    }

    #[inline]
    pub fn draw<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Bitmap8) -> R,
    {
        let inner = self.as_mut();
        f(inner)
    }
}

impl<'a> const AsRef<ConstBitmap8<'a>> for OwnedBitmap8 {
    #[inline]
    fn as_ref(&self) -> &ConstBitmap8<'a> {
        unsafe { transmute(self) }
    }
}

impl<'a> const AsMut<Bitmap8<'a>> for OwnedBitmap8 {
    #[inline]
    fn as_mut(&mut self) -> &mut Bitmap8<'a> {
        unsafe { transmute(self) }
    }
}

impl<'a> Borrow<Bitmap8<'a>> for OwnedBitmap8 {
    #[inline]
    fn borrow(&self) -> &Bitmap8<'a> {
        unsafe { transmute(self) }
    }
}

#[repr(C)]
pub struct ConstBitmap32<'a> {
    width: usize,
    height: usize,
    stride: usize,
    slice: &'a [TrueColor],
}

impl Drawable for ConstBitmap32<'_> {
    type ColorType = TrueColor;

    #[inline]
    fn width(&self) -> usize {
        self.width
    }

    #[inline]
    fn height(&self) -> usize {
        self.height
    }
}

impl RasterImage for ConstBitmap32<'_> {
    #[inline]
    fn stride(&self) -> usize {
        self.stride
    }

    #[inline]
    fn slice(&self) -> &[Self::ColorType] {
        self.slice
    }
}

impl<'a> ConstBitmap32<'a> {
    #[inline]
    pub const fn from_slice(slice: &'a [TrueColor], size: Size, stride: usize) -> Self {
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

impl<'a> const AsRef<ConstBitmap32<'a>> for ConstBitmap32<'a> {
    #[inline]
    fn as_ref(&self) -> &ConstBitmap32<'a> {
        self
    }
}

#[repr(C)]
pub struct Bitmap32<'a> {
    width: usize,
    height: usize,
    stride: usize,
    slice: UnsafeCell<&'a mut [TrueColor]>,
}

impl Drawable for Bitmap32<'_> {
    type ColorType = TrueColor;

    #[inline]
    fn width(&self) -> usize {
        self.width
    }

    #[inline]
    fn height(&self) -> usize {
        self.height
    }
}

impl RasterImage for Bitmap32<'_> {
    #[inline]
    fn stride(&self) -> usize {
        self.stride
    }

    #[inline]
    fn slice(&self) -> &[Self::ColorType] {
        unsafe { &*self.slice.get() }
    }
}

impl MutableRasterImage for Bitmap32<'_> {
    #[inline]
    fn slice_mut(&mut self) -> &mut [Self::ColorType] {
        self.slice.get_mut()
    }
}

impl<'a> Bitmap32<'a> {
    #[inline]
    pub fn from_slice(slice: &'a mut [TrueColor], size: Size, stride: usize) -> Self {
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
    pub fn clone(&self) -> Bitmap32<'a> {
        let slice = unsafe { self.slice.get().as_mut().unwrap() };
        Self {
            width: self.width(),
            height: self.height(),
            stride: self.stride(),
            slice: UnsafeCell::new(slice),
        }
    }

    #[inline]
    pub const fn as_const(&self) -> &'a ConstBitmap32<'a> {
        unsafe { transmute(self) }
    }
}

impl Bitmap32<'_> {
    pub fn blend_rect(&mut self, rect: Rect, color: TrueColor) {
        let rhs = color.components();
        if rhs.is_opaque() {
            return self.fill_rect(rect, color);
        } else if rhs.is_transparent() {
            return;
        }
        let alpha = rhs.a as usize;
        let alpha_n = 255 - alpha;

        let mut width = rect.size.width;
        let mut height = rect.size.height;
        let mut dx = rect.origin.x;
        let mut dy = rect.origin.y;

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
                    .blend_color(
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
}

impl Bitmap32<'static> {
    /// SAFETY: Must guarantee the existence of the `ptr`.
    #[inline]
    pub unsafe fn from_static(ptr: *mut TrueColor, size: Size, stride: usize) -> Self {
        let slice = core::slice::from_raw_parts_mut(ptr, size.height() as usize * stride);
        Self {
            width: size.width() as usize,
            height: size.height() as usize,
            stride,
            slice: UnsafeCell::new(slice),
        }
    }
}

impl BasicDrawing for Bitmap32<'_> {
    fn fill_rect(&mut self, rect: Rect, color: Self::ColorType) {
        let mut width = rect.width();
        let mut height = rect.height();
        let mut dx = rect.x();
        let mut dy = rect.y();

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
        if r >= self.width as isize {
            width = self.width as isize - dx;
        }
        if b >= self.height as isize {
            height = self.height as isize - dy;
        }
        if width <= 0 || height <= 0 {
            return;
        }

        let width = width as usize;
        let height = height as usize;
        let stride = self.stride;
        let mut cursor = dx as usize + dy as usize * stride;
        if stride == width {
            memory_colors::memset_colors32(self.slice_mut(), cursor, width * height, color);
        } else {
            for _ in 0..height {
                memory_colors::memset_colors32(self.slice_mut(), cursor, width, color);
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
        memory_colors::memset_colors32(self.slice_mut(), cursor, w as usize, color);
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

impl RasterFontWriter for Bitmap32<'_> {}

impl<'a> From<&'a Bitmap32<'a>> for ConstBitmap32<'a> {
    #[inline]
    fn from(src: &'a Bitmap32<'a>) -> Self {
        Self::from_slice(src.slice(), src.size(), src.stride())
    }
}

impl BltConvert<TrueColor> for Bitmap32<'_> {}
impl BltConvert<IndexedColor> for Bitmap32<'_> {}

impl<'a> Bitmap32<'a> {
    pub fn blt<'b, T: AsRef<ConstBitmap32<'b>>>(&mut self, src: &'b T, origin: Point, rect: Rect) {
        let src = src.as_ref();

        let (dx, dy, sx, sy, width, height) =
            adjust_blt_coords(self.size(), src.size(), origin, rect);
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

        if ds == width && ss == width {
            memory_colors::memcpy_colors32(
                dest_fb,
                dest_cursor,
                src_fb,
                src_cursor,
                width * height,
            );
        } else {
            for _ in 0..height {
                memory_colors::memcpy_colors32(dest_fb, dest_cursor, src_fb, src_cursor, width);
                dest_cursor += ds;
                src_cursor += ss;
            }
        }
    }

    pub fn blt_blend<'b, T: AsRef<ConstBitmap32<'b>>>(
        &mut self,
        src: &'b T,
        origin: Point,
        rect: Rect,
    ) {
        let src = src.as_ref();

        let (dx, dy, sx, sy, width, height) =
            adjust_blt_coords(self.size(), src.size(), origin, rect);
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
            memory_colors::blend_line32(dest_fb, dest_cursor, src_fb, src_cursor, width);
            dest_cursor += ds;
            src_cursor += ss;
        }
    }

    #[inline]
    pub fn blt8<'b, T: AsRef<ConstBitmap8<'b>>>(
        &mut self,
        src: &'b T,
        origin: Point,
        rect: Rect,
        palette: &[u32; 256],
    ) {
        self.blt_convert(src.as_ref(), origin, rect, |c| {
            TrueColor::from_argb(palette[c.0 as usize])
        });
    }

    /// expr
    pub fn blt_rotate<'b, T: AsRef<ConstBitmap32<'b>>>(
        &mut self,
        src: &T,
        origin: Point,
        rect: Rect,
    ) {
        let src = src.as_ref();
        let self_size = Size::new(self.height() as isize, self.width() as isize);
        let (mut dx, mut dy, sx, sy, width, height) =
            adjust_blt_coords(self_size, src.size(), origin, rect);
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
}

impl Bitmap32<'_> {
    pub fn view<F, R>(&mut self, rect: Rect, f: F) -> Option<R>
    where
        F: FnOnce(&mut Bitmap) -> R,
    {
        let coords = match Coordinates::try_from(rect) {
            Ok(v) => v,
            Err(_) => return None,
        };
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

        let offset = rect.x() as usize + rect.y() as usize * stride;
        let new_len = (rect.height() as usize - 1) * stride + rect.width() as usize;
        let r = {
            let slice = self.slice_mut();
            let mut view = Bitmap32 {
                width: rect.width() as usize,
                height: rect.height() as usize,
                stride,
                slice: UnsafeCell::new(&mut slice[offset..offset + new_len]),
            };
            let mut bitmap = Bitmap::from(&mut view);
            f(&mut bitmap)
        };
        Some(r)
    }
}

impl<'a> const AsRef<ConstBitmap32<'a>> for Bitmap32<'a> {
    #[inline]
    fn as_ref(&self) -> &ConstBitmap32<'a> {
        self.as_const()
    }
}

impl<'a> const AsMut<Bitmap32<'a>> for Bitmap32<'a> {
    #[inline]
    fn as_mut(&mut self) -> &mut Bitmap32<'a> {
        self
    }
}

impl ToOwned for Bitmap32<'_> {
    type Owned = OwnedBitmap32;

    #[inline]
    fn to_owned(&self) -> Self::Owned {
        let vec = self.slice().to_vec();
        OwnedBitmap32::from_vec(vec, self.size())
    }
}

pub struct OwnedBitmap32 {
    width: usize,
    height: usize,
    stride: usize,
    slice: UnsafeCell<Box<[TrueColor]>>,
}

impl Drawable for OwnedBitmap32 {
    type ColorType = TrueColor;

    #[inline]
    fn width(&self) -> usize {
        self.width
    }

    #[inline]
    fn height(&self) -> usize {
        self.height
    }
}

impl RasterImage for OwnedBitmap32 {
    #[inline]
    fn stride(&self) -> usize {
        self.stride
    }

    #[inline]
    fn slice(&self) -> &[Self::ColorType] {
        unsafe { &*self.slice.get() }
    }
}

impl MutableRasterImage for OwnedBitmap32 {
    #[inline]
    fn slice_mut(&mut self) -> &mut [Self::ColorType] {
        self.slice.get_mut()
    }
}

impl OwnedBitmap32 {
    #[inline]
    pub fn new(size: Size, bg_color: TrueColor) -> Self {
        let len = size.width() as usize * size.height() as usize;
        let mut vec = Vec::with_capacity(len);
        vec.resize(len, bg_color);
        Self::from_vec(vec, size)
    }

    #[inline]
    pub fn from_vec(vec: Vec<TrueColor>, size: Size) -> Self {
        Self {
            width: size.width as usize,
            height: size.height as usize,
            stride: size.width as usize,
            slice: UnsafeCell::new(vec.into_boxed_slice()),
        }
    }

    #[inline]
    pub fn draw<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Bitmap32) -> R,
    {
        let inner = self.as_mut();
        f(inner)
    }
}

impl<'a> const AsRef<ConstBitmap32<'a>> for OwnedBitmap32 {
    #[inline]
    fn as_ref(&self) -> &ConstBitmap32<'a> {
        unsafe { transmute(self) }
    }
}

impl<'a> const AsMut<Bitmap32<'a>> for OwnedBitmap32 {
    #[inline]
    fn as_mut(&mut self) -> &mut Bitmap32<'a> {
        unsafe { transmute(self) }
    }
}

impl<'a> Borrow<Bitmap32<'a>> for OwnedBitmap32 {
    #[inline]
    fn borrow(&self) -> &Bitmap32<'a> {
        unsafe { transmute(self) }
    }
}

pub enum ConstBitmap<'a> {
    Indexed(&'a ConstBitmap8<'a>),
    Argb32(&'a ConstBitmap32<'a>),
}

impl Drawable for ConstBitmap<'_> {
    type ColorType = Color;

    #[inline]
    fn width(&self) -> usize {
        match self {
            Self::Indexed(ref v) => v.width(),
            Self::Argb32(ref v) => v.width(),
        }
    }

    #[inline]
    fn height(&self) -> usize {
        match self {
            Self::Indexed(ref v) => v.height(),
            Self::Argb32(ref v) => v.height(),
        }
    }
}

impl GetPixel for ConstBitmap<'_> {
    #[inline]
    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::ColorType {
        match self {
            Self::Indexed(ref v) => v.get_pixel_unchecked(point).into(),
            Self::Argb32(ref v) => v.get_pixel_unchecked(point).into(),
        }
    }
}

impl<'a> const From<&'a ConstBitmap8<'a>> for ConstBitmap<'a> {
    #[inline]
    fn from(val: &'a ConstBitmap8<'a>) -> ConstBitmap<'a> {
        ConstBitmap::Indexed(val)
    }
}

impl<'a> const From<&'a Bitmap8<'a>> for ConstBitmap<'a> {
    #[inline]
    fn from(val: &'a Bitmap8<'a>) -> Self {
        ConstBitmap::Indexed(val.as_ref())
    }
}

impl<'a> const From<&'a ConstBitmap32<'a>> for ConstBitmap<'a> {
    #[inline]
    fn from(val: &'a ConstBitmap32<'a>) -> ConstBitmap {
        ConstBitmap::Argb32(val)
    }
}

impl<'a> const From<&'a Bitmap32<'a>> for ConstBitmap<'a> {
    #[inline]
    fn from(val: &'a Bitmap32<'a>) -> Self {
        ConstBitmap::Argb32(val.as_ref())
    }
}

impl<'a> const AsRef<ConstBitmap<'a>> for ConstBitmap<'a> {
    fn as_ref(&self) -> &ConstBitmap<'a> {
        self
    }
}

pub enum Bitmap<'a> {
    Indexed(&'a mut Bitmap8<'a>),
    Argb32(&'a mut Bitmap32<'a>),
}

impl Drawable for Bitmap<'_> {
    type ColorType = Color;

    #[inline]
    fn width(&self) -> usize {
        match self {
            Self::Indexed(ref v) => v.width(),
            Self::Argb32(ref v) => v.width(),
        }
    }

    #[inline]
    fn height(&self) -> usize {
        match self {
            Self::Indexed(ref v) => v.height(),
            Self::Argb32(ref v) => v.height(),
        }
    }
}

impl<'a> Bitmap<'a> {
    #[inline]
    pub const fn as_const(&self) -> &'a ConstBitmap<'a> {
        unsafe { transmute(self) }
    }
}

impl Bitmap<'_> {
    /// Make a bitmap view
    #[inline]
    pub fn view<F, R>(&mut self, rect: Rect, f: F) -> Option<R>
    where
        F: FnOnce(&mut Bitmap) -> R,
    {
        match self {
            Self::Indexed(ref mut v) => v.view(rect, f),
            Self::Argb32(ref mut v) => v.view(rect, f),
        }
    }

    #[inline]
    pub fn map_indexed<F, R>(&mut self, f: F) -> Option<R>
    where
        F: FnOnce(&mut Bitmap8) -> R,
    {
        match self {
            Self::Indexed(ref mut v) => Some(f(v)),
            Self::Argb32(_) => None,
        }
    }

    #[inline]
    pub fn map_argb32<F, R>(&mut self, f: F) -> Option<R>
    where
        F: FnOnce(&mut Bitmap32) -> R,
    {
        match self {
            Self::Indexed(_) => None,
            Self::Argb32(ref mut v) => Some(f(v)),
        }
    }
}

impl GetPixel for Bitmap<'_> {
    #[inline]
    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::ColorType {
        match self {
            Self::Indexed(ref v) => v.get_pixel_unchecked(point).into(),
            Self::Argb32(ref v) => v.get_pixel_unchecked(point).into(),
        }
    }
}

impl SetPixel for Bitmap<'_> {
    #[inline]
    unsafe fn set_pixel_unchecked(&mut self, point: Point, pixel: Self::ColorType) {
        match self {
            Self::Indexed(ref mut v) => v.set_pixel_unchecked(point, pixel.into()),
            Self::Argb32(ref mut v) => v.set_pixel_unchecked(point, pixel.into()),
        }
    }
}

impl RasterFontWriter for Bitmap<'_> {
    #[inline]
    fn draw_font(&mut self, src: &[u8], size: Size, origin: Point, color: Self::ColorType) {
        match self {
            Self::Indexed(ref mut v) => v.draw_font(src, size, origin, color.into()),
            Self::Argb32(ref mut v) => v.draw_font(src, size, origin, color.into()),
        }
    }
}

impl BasicDrawing for Bitmap<'_> {
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

impl Bitmap<'_> {
    #[inline]
    pub const fn color_mode(&self) -> usize {
        match self {
            Self::Indexed(_) => 8,
            Self::Argb32(_) => 32,
        }
    }

    #[inline]
    pub fn blt_itself<'a>(&'a mut self, origin: Point, rect: Rect) {
        match self {
            Self::Indexed(v) => v.blt(v.clone().as_ref(), origin, rect),
            Self::Argb32(v) => v.blt(v.clone().as_ref(), origin, rect),
        }
    }
}

impl<'a> Bitmap<'a> {
    #[inline]
    pub fn blt_transparent<'b, T: AsRef<ConstBitmap<'b>>>(
        &mut self,
        src: &'b T,
        origin: Point,
        rect: Rect,
        color_key: IndexedColor,
    ) {
        let src = src.as_ref();
        match self {
            Bitmap::Indexed(ref mut bitmap) => match src {
                ConstBitmap::Indexed(ref src) => bitmap.blt_with_key(src, origin, rect, color_key),
                ConstBitmap::Argb32(ref src) => bitmap.blt_convert_opt(*src, origin, rect, |c| {
                    if c.is_transparent() {
                        None
                    } else {
                        Some(c.into())
                    }
                }),
            },
            Bitmap::Argb32(ref mut bitmap) => match src {
                ConstBitmap::Indexed(ref src) => bitmap.blt_convert_opt(*src, origin, rect, |c| {
                    if c == color_key {
                        None
                    } else {
                        Some(c.into())
                    }
                }),
                ConstBitmap::Argb32(ref src) => bitmap.blt_blend(src, origin, rect),
            },
        }
    }
}

impl<'a, 'b> Blt<ConstBitmap<'b>> for Bitmap<'a> {
    fn blt(&mut self, src: &ConstBitmap<'b>, origin: Point, rect: Rect) {
        match self {
            Bitmap::Indexed(ref mut bitmap) => match src {
                ConstBitmap::Indexed(ref src) => bitmap.blt(src, origin, rect),
                ConstBitmap::Argb32(ref src) => bitmap.blt32(src, origin, rect),
            },
            Bitmap::Argb32(ref mut bitmap) => match src {
                ConstBitmap::Indexed(ref src) => {
                    bitmap.blt8(src, origin, rect, &IndexedColor::COLOR_PALETTE)
                }
                ConstBitmap::Argb32(ref src) => bitmap.blt(src, origin, rect),
            },
        }
    }
}

impl<'a, 'b> Blt<ConstBitmap8<'b>> for Bitmap<'a> {
    fn blt(&mut self, src: &ConstBitmap8<'b>, origin: Point, rect: Rect) {
        match self {
            Self::Indexed(ref mut bitmap) => bitmap.blt(src, origin, rect),
            Self::Argb32(ref mut bitmap) => {
                bitmap.blt8(src, origin, rect, &IndexedColor::COLOR_PALETTE)
            }
        }
    }
}

impl<'a, 'b> Blt<ConstBitmap32<'b>> for Bitmap<'a> {
    fn blt(&mut self, src: &ConstBitmap32<'b>, origin: Point, rect: Rect) {
        match self {
            Self::Indexed(ref mut bitmap) => bitmap.blt32(src, origin, rect),
            Self::Argb32(ref mut bitmap) => bitmap.blt(src, origin, rect),
        }
    }
}

impl<'a> const From<&'a mut Bitmap8<'a>> for Bitmap<'a> {
    #[inline]
    fn from(val: &'a mut Bitmap8<'a>) -> Bitmap<'a> {
        Self::Indexed(val)
    }
}

impl<'a> const From<&'a mut Bitmap32<'a>> for Bitmap<'a> {
    #[inline]
    fn from(val: &'a mut Bitmap32<'a>) -> Bitmap<'a> {
        Self::Argb32(val)
    }
}

impl<'a> const AsRef<ConstBitmap<'a>> for Bitmap<'a> {
    #[inline]
    fn as_ref(&self) -> &ConstBitmap<'a> {
        self.as_const()
    }
}

impl<'a> const AsMut<Bitmap<'a>> for Bitmap<'a> {
    #[inline]
    fn as_mut(&mut self) -> &mut Bitmap<'a> {
        self
    }
}

impl ToOwned for Bitmap<'_> {
    type Owned = OwnedBitmap;

    #[inline]
    fn to_owned(&self) -> Self::Owned {
        match self {
            Bitmap::Indexed(v) => OwnedBitmap::Indexed((*v).to_owned()),
            Bitmap::Argb32(v) => OwnedBitmap::Argb32((*v).to_owned()),
        }
    }
}

pub enum OwnedBitmap {
    Indexed(OwnedBitmap8),
    Argb32(OwnedBitmap32),
}

impl Drawable for OwnedBitmap {
    type ColorType = Color;

    #[inline]
    fn width(&self) -> usize {
        match self {
            Self::Indexed(ref v) => v.width(),
            Self::Argb32(ref v) => v.width(),
        }
    }

    #[inline]
    fn height(&self) -> usize {
        match self {
            Self::Indexed(ref v) => v.height(),
            Self::Argb32(ref v) => v.height(),
        }
    }
}

impl OwnedBitmap {
    #[inline]
    pub fn new<'b, T: AsRef<ConstBitmap<'b>>>(
        template_bitmap: &T,
        size: Size,
        bg_color: Color,
    ) -> OwnedBitmap {
        match template_bitmap.as_ref() {
            ConstBitmap::Indexed(_) => Self::Indexed(OwnedBitmap8::new(size, bg_color.into())),
            ConstBitmap::Argb32(_) => Self::Argb32(OwnedBitmap32::new(size, bg_color.into())),
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
    pub fn into_bitmap<'a>(&'a mut self) -> Bitmap<'a> {
        match self {
            OwnedBitmap::Indexed(ref mut v) => Bitmap::Indexed(v.as_mut()),
            OwnedBitmap::Argb32(ref mut v) => Bitmap::Argb32(v.as_mut()),
        }
    }
}

impl const From<OwnedBitmap8> for OwnedBitmap {
    #[inline]
    fn from(val: OwnedBitmap8) -> Self {
        Self::Indexed(val)
    }
}

impl const From<OwnedBitmap32> for OwnedBitmap {
    #[inline]
    fn from(val: OwnedBitmap32) -> Self {
        Self::Argb32(val)
    }
}

impl<'a> Borrow<Bitmap<'a>> for OwnedBitmap {
    fn borrow(&self) -> &Bitmap<'a> {
        todo!()
    }
}

/// A special bitmap type that can be used for operations such as transparency and shading.
pub struct OperationalBitmap {
    width: usize,
    height: usize,
    vec: Vec<u8>,
}

impl ColorTrait for u8 {}

impl Drawable for OperationalBitmap {
    type ColorType = u8;

    #[inline]
    fn width(&self) -> usize {
        self.width
    }

    #[inline]
    fn height(&self) -> usize {
        self.height
    }
}

impl RasterImage for OperationalBitmap {
    #[inline]
    fn stride(&self) -> usize {
        self.width
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
        Self {
            width: size.width() as usize,
            height: size.height() as usize,
            vec,
        }
    }

    #[inline]
    pub fn from_slice(slice: &[u8], size: Size) -> Self {
        let vec = Vec::from(slice);
        Self {
            width: size.width() as usize,
            height: size.height() as usize,
            vec,
        }
    }

    #[inline]
    pub fn from_vec(vec: Vec<u8>, size: Size) -> Self {
        Self {
            width: size.width() as usize,
            height: size.height() as usize,
            vec,
        }
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
    pub fn draw_line<F>(&mut self, c1: Point, c2: Point, mut f: F)
    where
        F: FnMut(&mut OperationalBitmap, Point),
    {
        c1.line_to(c2, |point| f(self, point));
    }

    /// Draws an anti-aliased line using Xiaolin Wu's algorithm.
    pub fn draw_line_anti_aliasing<F>(&mut self, c1: Point, c2: Point, scale: isize, mut f: F)
    where
        F: FnMut(&mut OperationalBitmap, Point, u8),
    {
        const FRAC_SHIFT: isize = 6;
        const ONE: isize = 1 << FRAC_SHIFT;
        const FRAC_MASK: isize = ONE - 1;
        const FRAC_HALF: isize = ONE / 2;
        const IPART_MASK: isize = !FRAC_MASK;

        let mut plot = |bitmap: &mut OperationalBitmap, x: isize, y: isize, level: isize| {
            f(
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
            FRAC_MASK - fpart(v)
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

    pub fn blt_to<T, F>(&self, dest: &mut T, origin: Point, rect: Rect, mut f: F)
    where
        T: Drawable + GetPixel + SetPixel,
        F: FnMut(u8, <T as Drawable>::ColorType) -> <T as Drawable>::ColorType,
    {
        let (dx, dy, sx, sy, width, height) =
            adjust_blt_coords(dest.size(), self.size(), origin, rect);
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
                        f(self.get_pixel_unchecked(sp), dest.get_pixel_unchecked(dp)),
                    );
                }
            }
        }
    }

    pub fn blt_from<T, F>(&mut self, src: &T, origin: Point, rect: Rect, mut f: F)
    where
        T: GetPixel,
        F: FnMut(<T as Drawable>::ColorType, u8) -> u8,
    {
        let (dx, dy, sx, sy, width, height) =
            adjust_blt_coords(self.size(), src.size(), origin, rect);
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
                        f(src.get_pixel_unchecked(sp), self.get_pixel_unchecked(dp)),
                    );
                }
            }
        }
    }

    pub fn draw_to(&self, dest: &mut Bitmap, origin: Point, rect: Rect, color: Color) {
        match dest {
            Bitmap::Indexed(_) => {
                // TODO:
            }
            Bitmap::Argb32(ref mut bitmap) => {
                let color = color.into_true_color();
                self.blt_to(*bitmap, origin, rect, |a, b| {
                    let mut c = color.components();
                    c.a = a;
                    b.blend_draw(c.into())
                });
            }
        }
    }
}

/// Adjust the coordinates for blt.
///
/// Returns the adjusted destination x, y, source x, y, width and height.
fn adjust_blt_coords(
    dest_size: Size,
    src_size: Size,
    origin: Point,
    rect: Rect,
) -> (isize, isize, isize, isize, isize, isize) {
    let mut dx = origin.x;
    let mut dy = origin.y;
    let mut sx = rect.origin.x;
    let mut sy = rect.origin.y;
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum ScaleMode {
    DotByDot,
    Sparse2X,
    Interlace2X,
    NearestNeighbor2X,
}

impl ScaleMode {
    #[inline]
    pub const fn scale_factor(&self) -> usize {
        use ScaleMode::*;
        match self {
            DotByDot => 1,
            Sparse2X | Interlace2X | NearestNeighbor2X => 2,
        }
    }
}

mod memory_colors {
    use super::*;

    /// Fast fill
    #[inline]
    pub fn memset_colors8(
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

    /// Fast copy
    #[inline]
    pub fn memcpy_colors8(
        dest: &mut [IndexedColor],
        dest_cursor: usize,
        src: &[IndexedColor],
        src_cursor: usize,
        size: usize,
    ) {
        unsafe {
            let dest = dest.get_unchecked_mut(dest_cursor);
            let src = src.get_unchecked(src_cursor);
            let mut ptr_d: *mut u8 = transmute(dest);
            let mut ptr_s: *const u8 = transmute(src);
            let mut remain = size;

            if ((ptr_d as usize) & 0x7) == ((ptr_s as usize) & 0x7) {
                let prologue = usize::min(ptr_d as usize & 0x07, remain);
                remain -= prologue;
                for _ in 0..prologue {
                    ptr_d.write_volatile(ptr_s.read_volatile());
                    ptr_d = ptr_d.add(1);
                    ptr_s = ptr_s.add(1);
                }

                if remain > 8 {
                    let count = remain / 8;
                    let mut ptr2d = ptr_d as *mut u64;
                    let mut ptr2s = ptr_s as *const u64;

                    for _ in 0..count {
                        ptr2d.write_volatile(ptr2s.read_volatile());
                        ptr2d = ptr2d.add(1);
                        ptr2s = ptr2s.add(1);
                    }

                    ptr_d = ptr2d as *mut u8;
                    ptr_s = ptr2s as *const u8;
                    remain -= count * 8;
                }

                for _ in 0..remain {
                    ptr_d.write_volatile(ptr_s.read_volatile());
                    ptr_d = ptr_d.add(1);
                    ptr_s = ptr_s.add(1);
                }
            } else {
                for _ in 0..size {
                    ptr_d.write_volatile(ptr_s.read_volatile());
                    ptr_d = ptr_d.add(1);
                    ptr_s = ptr_s.add(1);
                }
            }
        }
    }

    /// Faster Fill
    #[inline]
    pub fn memset_colors32(slice: &mut [TrueColor], cursor: usize, count: usize, color: TrueColor) {
        for v in unsafe { slice.get_unchecked_mut(cursor..cursor + count) }.iter_mut() {
            *v = color;
        }
    }

    /// Faster copy
    #[inline]
    pub fn memcpy_colors32(
        dest: &mut [TrueColor],
        dest_cursor: usize,
        src: &[TrueColor],
        src_cursor: usize,
        count: usize,
    ) {
        let src = unsafe { src.get_unchecked(src_cursor..src_cursor + count) };
        let dest = unsafe { dest.get_unchecked_mut(dest_cursor..dest_cursor + count) };
        for (dest, src) in dest.iter_mut().zip(src.iter()) {
            *dest = *src;
        }
    }

    // Alpha blending
    #[inline]
    pub fn blend_line32(
        dest: &mut [TrueColor],
        dest_cursor: usize,
        src: &[TrueColor],
        src_cursor: usize,
        count: usize,
    ) {
        let dest = unsafe { &mut dest.get_unchecked_mut(dest_cursor..dest_cursor + count) };
        let src = unsafe { &src.get_unchecked(src_cursor..src_cursor + count) };
        for i in 0..count {
            dest[i] = dest[i].blend_draw(src[i]);
        }
    }
}

#[repr(C)]
pub struct ConstBitmap1<'a> {
    width: usize,
    height: usize,
    stride: usize,
    slice: &'a [u8],
}

impl Drawable for ConstBitmap1<'_> {
    type ColorType = u8;

    #[inline]
    fn width(&self) -> usize {
        self.width
    }

    #[inline]
    fn height(&self) -> usize {
        self.height
    }
}

impl<'a> ConstBitmap1<'a> {
    #[inline]
    pub const fn from_slice(slice: &'a [u8], size: Size, stride: Option<NonZeroUsize>) -> Self {
        Self {
            width: size.width() as usize,
            height: size.height() as usize,
            stride: match stride {
                Some(v) => v.get(),
                None => (size.width() + 7) as usize / 8,
            },
            slice,
        }
    }
}

impl ConstBitmap1<'_> {
    #[inline]
    fn slice(&self) -> &[u8] {
        self.slice
    }
}

impl GetPixel for ConstBitmap1<'_> {
    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::ColorType {
        let index = (point.x as usize / 8) + self.stride * point.y as usize;
        let position = 0x80u8 >> ((point.x as usize) & 7);
        *self.slice().get_unchecked(index) & position
    }
}

impl<'a> const AsRef<ConstBitmap1<'a>> for ConstBitmap1<'a> {
    #[inline]
    fn as_ref(&self) -> &ConstBitmap1<'a> {
        self
    }
}

#[repr(C)]
pub struct Bitmap1<'a> {
    width: usize,
    height: usize,
    stride: usize,
    slice: UnsafeCell<&'a mut [u8]>,
}

impl Drawable for Bitmap1<'_> {
    type ColorType = u8;

    #[inline]
    fn width(&self) -> usize {
        self.width
    }

    #[inline]
    fn height(&self) -> usize {
        self.height
    }
}

impl<'a> Bitmap1<'a> {
    #[inline]
    pub const fn from_slice(slice: &'a mut [u8], size: Size, stride: Option<NonZeroUsize>) -> Self {
        Self {
            width: size.width() as usize,
            height: size.height() as usize,
            stride: match stride {
                Some(v) => v.get(),
                None => (size.width() + 7) as usize / 8,
            },
            slice: UnsafeCell::new(slice),
        }
    }

    #[inline]
    pub const fn as_const(&self) -> &'a ConstBitmap1<'a> {
        unsafe { transmute(self) }
    }
}

impl Bitmap1<'_> {
    #[inline]
    fn slice_mut(&self) -> &mut [u8] {
        unsafe { &mut *self.slice.get() }
    }

    pub fn copy_from<'a, T: AsRef<ConstBitmap1<'a>>>(&mut self, other: &T) {
        unsafe {
            let p = self.slice_mut();
            let q = other.as_ref().slice();
            let count = p.len();
            copy_nonoverlapping(q.as_ptr(), p.as_mut_ptr(), count);
        }
    }
}

impl GetPixel for Bitmap1<'_> {
    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::ColorType {
        self.as_ref().get_pixel_unchecked(point)
    }
}

impl SetPixel for Bitmap1<'_> {
    unsafe fn set_pixel_unchecked(&mut self, point: Point, pixel: Self::ColorType) {
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

impl<'a> const AsRef<ConstBitmap1<'a>> for Bitmap1<'a> {
    #[inline]
    fn as_ref(&self) -> &ConstBitmap1<'a> {
        self.as_const()
    }
}

impl<'a> const AsMut<Bitmap1<'a>> for Bitmap1<'a> {
    #[inline]
    fn as_mut(&mut self) -> &mut Bitmap1<'a> {
        self
    }
}
