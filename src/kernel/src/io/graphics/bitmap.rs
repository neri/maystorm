// Bitmap

use super::color::*;
use super::coords::*;
use crate::io::fonts::*;
use crate::num::*;
use alloc::sync::Arc;
use alloc::vec::Vec;
use bitflags::*;
use bootprot::BootInfo;
use byteorder::*;
use core::cell::RefCell;
use core::mem::swap;

#[repr(C)]
pub struct Bitmap {
    base: *mut u32,
    size: Size<isize>,
    stride: usize,
    flags: BitmapFlags,
    managed: Option<Arc<RefCell<Vec<Color>>>>,
}

bitflags! {
    pub struct BitmapFlags: usize {
        const PORTRAIT = 0b0000_0001;
        const TRANSLUCENT = 0b0000_0010;
        const VIEW = 0b1000_0000;
    }
}

static BIT_MASKS: [u8; 8] = [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01];

impl From<&BootInfo> for Bitmap {
    fn from(info: &BootInfo) -> Self {
        let stride = info.vram_stride;
        let mut width = info.screen_width;
        let mut height = info.screen_height;
        let mut is_portrait = height > width;
        let mut flags = BitmapFlags::VIEW;
        if is_portrait {
            // portrait
            swap(&mut width, &mut height);
        }
        if stride > width {
            // GPD micro PC fake landscape mode
            is_portrait = true;
        }
        if is_portrait {
            flags.insert(BitmapFlags::PORTRAIT);
        }
        Bitmap {
            base: info.vram_base as *mut u32,
            size: Size::new(width as isize, height as isize),
            stride: stride.into(),
            flags,
            managed: None,
        }
    }
}

impl Bitmap {
    pub fn new(width: usize, height: usize, is_translucent: bool) -> Self {
        let mut vec = Vec::with_capacity(width * height);
        unsafe {
            vec.set_len(vec.capacity());
        }
        let base = &vec[0] as *const _ as *mut _;
        let mut flags = BitmapFlags::empty();
        if is_translucent {
            flags.insert(BitmapFlags::TRANSLUCENT);
        }
        Self {
            base,
            size: Size::new(width as isize, height as isize),
            stride: width.into(),
            flags,
            managed: Some(Arc::new(RefCell::new(vec))),
        }
    }

    pub fn from_vec(
        vec: Arc<RefCell<Vec<Color>>>,
        width: usize,
        height: usize,
        is_translucent: bool,
    ) -> Self {
        let base = vec.borrow().as_ptr() as *mut _;
        let mut flags = BitmapFlags::empty();
        if is_translucent {
            flags.insert(BitmapFlags::TRANSLUCENT);
        }
        Self {
            base,
            size: Size::new(width as isize, height as isize),
            stride: width.into(),
            flags,
            managed: Some(vec),
        }
    }

    pub fn from_msdib(dib: &[u8]) -> Option<Self> {
        if LE::read_u16(dib) != 0x4D42 {
            return None;
        }
        let bpp = LE::read_u16(&dib[0x1C..0x1E]) as usize;
        match bpp {
            24 | 32 => (),
            _ => return None,
        }
        let offset = LE::read_u32(&dib[0x0A..0x0E]) as usize;
        let width = LE::read_u32(&dib[0x12..0x16]) as usize;
        let height = LE::read_u32(&dib[0x16..0x1A]) as usize;
        let bpp8 = bpp / 8;
        let stride = (width * bpp8 + 3) & !3;
        let mut bits = Vec::with_capacity(width * height);
        match bpp {
            24 => {
                for y in 0..height {
                    let mut src = offset + (height - y - 1) * stride;
                    for _ in 0..width {
                        let b = dib[src] as u32;
                        let g = dib[src + 1] as u32;
                        let r = dib[src + 2] as u32;
                        bits.push(Color::from_rgb(b + g * 0x100 + r * 0x10000));
                        src += bpp8;
                    }
                }
            }
            32 => {
                for y in 0..height {
                    let mut src = offset + (height - y - 1) * stride;
                    for _ in 0..width {
                        bits.push(Color::from_rgb(LE::read_u32(&dib[src..src + bpp8])));
                        src += bpp8;
                    }
                }
            }
            _ => unimplemented!(),
        }
        Some(Self::from_vec(
            Arc::new(RefCell::new(bits)),
            width,
            height,
            false,
        ))
    }

    pub fn view(&self, rect: Rect<isize>) -> Option<Self> {
        let mut coords = match Coordinates::from_rect(rect) {
            None => return None,
            Some(coords) => coords,
        };
        if coords.left < 0 || coords.top < 0 {
            return None;
        }
        if coords.right > self.width() {
            coords.right = self.width();
        }
        if coords.bottom > self.height() {
            coords.bottom = self.height();
        }

        let base = unsafe {
            self.get_fb()
                .add(coords.left as usize + coords.top as usize * self.stride)
        };

        Some(Self {
            base,
            size: Rect::from(coords).size,
            stride: self.stride,
            flags: self.flags | BitmapFlags::VIEW,
            managed: self.managed.clone(),
        })
    }

    pub fn with_same_size(fb: &Bitmap) -> Self {
        Self::new(
            fb.size.width as usize,
            fb.size.height as usize,
            fb.is_translucent(),
        )
    }

    #[inline]
    pub fn size(&self) -> Size<isize> {
        self.size
    }

    #[inline]
    pub fn width(&self) -> isize {
        self.size.width
    }

    #[inline]
    pub fn height(&self) -> isize {
        self.size.height
    }

    #[inline]
    pub fn bounds(&self) -> Rect<isize> {
        Rect::from(self.size)
    }

    #[inline]
    pub fn is_portrait(&self) -> bool {
        self.flags.contains(BitmapFlags::PORTRAIT)
    }

    #[inline]
    pub fn is_translucent(&self) -> bool {
        self.flags.contains(BitmapFlags::TRANSLUCENT)
    }

    #[inline]
    pub fn is_opaque(&self) -> bool {
        !self.is_translucent()
    }

    #[inline]
    unsafe fn get_fb(&self) -> *mut u32 {
        self.base
    }

    #[inline]
    pub fn update_bitmap<F>(&self, f: F) -> Result<(), ()>
    where
        F: FnOnce(&mut [Color]),
    {
        if self.stride != self.size.width as usize {
            return Err(());
        }
        let slice = unsafe {
            core::slice::from_raw_parts_mut(
                self.base as *mut Color,
                self.stride * self.size.height as usize,
            )
        };
        f(slice);
        Ok(())
    }

    pub fn reset(&self) {
        self.fill_rect(Rect::from(self.size), Color::zero());
    }

    pub fn fill_rect(&self, rect: Rect<isize>, color: Color) {
        let mut width = rect.size.width;
        let mut height = rect.size.height;
        let mut dx = rect.origin.x;
        let mut dy = rect.origin.y;

        {
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
        }

        if self.is_portrait() {
            let temp = dx;
            dx = self.size.height - dy - height;
            dy = temp;
            swap(&mut width, &mut height);
        }

        unsafe {
            let mut ptr = self.get_fb().add(dx as usize + dy as usize * self.stride);
            let stride_ptr = self.stride - width as usize;
            if stride_ptr == 0 {
                let count = width * height;
                for _ in 0..count {
                    ptr.write_volatile(color.argb());
                    ptr = ptr.add(1);
                }
            } else {
                for _y in 0..height {
                    for _x in 0..width {
                        ptr.write_volatile(color.argb());
                        ptr = ptr.add(1);
                    }
                    ptr = ptr.add(stride_ptr);
                }
            }
        }
    }

    pub fn blend_rect(&self, rect: Rect<isize>, color: Color) {
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

        {
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
        }

        if self.is_portrait() {
            let temp = dx;
            dx = self.size.height - dy - height;
            dy = temp;
            swap(&mut width, &mut height);
        }

        unsafe {
            let mut ptr = self.get_fb().add(dx as usize + dy as usize * self.stride);
            let stride_ptr = self.stride - width as usize;
            for _y in 0..height {
                for _x in 0..width {
                    let lhs = Color::from_argb(ptr.read_volatile()).components();
                    let c = lhs.blend_color(
                        rhs,
                        |lhs, rhs| {
                            (((lhs as usize) * alpha_n + (rhs as usize) * alpha) / 255) as u8
                        },
                        |a, b| a.saturating_add(b),
                    );
                    ptr.write_volatile(c.into());
                    ptr = ptr.add(1);
                }
                ptr = ptr.add(stride_ptr);
            }
        }
    }

    pub fn draw_pixels(&self, points: &[Point<isize>], color: Color) {
        let fb = unsafe { self.get_fb() };
        for point in points {
            let mut dx = point.x;
            let mut dy = point.y;
            if dx >= 0 && dx < self.size.width && dy >= 0 && dy < self.size.height {
                if self.is_portrait() {
                    let temp = dx;
                    dx = self.size.height - dy - 1;
                    dy = temp;
                }
                unsafe {
                    fb.add(dx as usize + dy as usize * self.stride)
                        .write_volatile(color.argb());
                }
            }
        }
    }

    #[inline]
    pub fn draw_pixel(&self, point: Point<isize>, color: Color) {
        self.draw_pixels(&[point], color);
    }

    pub fn draw_hline(&self, point: Point<isize>, width: isize, color: Color) {
        let mut dx = point.x;
        let dy = point.y;
        let mut w = width;

        {
            if dy < 0 || dy >= self.size.height {
                return;
            }
            if dx < 0 {
                w += dx;
                dx = 0;
            }
            let r = dx + w;
            if r >= self.size.width {
                w = self.size.width - dx;
            }
            if w <= 0 {
                return;
            }
        }

        if self.is_portrait() {
            // TODO:
        } else {
            unsafe {
                let mut ptr = self.get_fb().add(dx as usize + dy as usize * self.stride);
                for _ in 0..w {
                    ptr.write_volatile(color.argb());
                    ptr = ptr.add(1);
                }
            }
        }
    }

    pub fn draw_vline(&self, point: Point<isize>, height: isize, color: Color) {
        let dx = point.x;
        let mut dy = point.y;
        let mut h = height;

        {
            if dx < 0 || dx >= self.size.width {
                return;
            }
            if dy < 0 {
                h += dy;
                dy = 0;
            }
            let b = dy + h;
            if b >= self.size.height {
                h = self.size.height - dy;
            }
            if h <= 0 {
                return;
            }
        }

        if self.is_portrait() {
            // TODO:
        } else {
            unsafe {
                let dd = self.stride;
                let mut ptr = self.get_fb().add(dx as usize + dy as usize * dd);
                for _ in 0..h {
                    ptr.write_volatile(color.argb());
                    ptr = ptr.add(dd);
                }
            }
        }
    }

    pub fn draw_rect(&self, rect: Rect<isize>, color: Color) {
        let coords = Coordinates::from_rect(rect).unwrap();
        let width = rect.width();
        let height = rect.height();
        self.draw_hline(coords.left_top(), width, color);
        self.draw_hline(coords.left_bottom() - Point::new(0, 1), width, color);
        if height > 2 {
            self.draw_vline(coords.left_top() + Point::new(0, 1), height - 2, color);
            self.draw_vline(coords.right_top() + Point::new(-1, 1), height - 2, color);
        }
    }

    pub fn draw_pattern(&self, rect: Rect<isize>, pattern: &[u8], color: Color) {
        let width = rect.size.width;
        let mut height = rect.size.height;
        let dx = rect.origin.x;
        let mut dy = rect.origin.y;
        let w8 = (width + 7) / 8;

        let h_limit = self.size.height - dy;
        if h_limit < height {
            height = h_limit;
        }

        // TODO: more better clipping
        if dx < 0 || dx >= self.size.width || dy < 0 || dy >= self.size.height || height == 0 {
            return;
        }

        unsafe {
            if self.is_portrait() {
                dy = self.size.height - dy - height;
                let mut ptr = self.get_fb().add(dy as usize + dx as usize * self.stride);
                let stride_ptr = self.stride - height as usize;
                for x in 0..w8 {
                    for mask in BIT_MASKS.iter() {
                        for y in (0..height).rev() {
                            let data = pattern[(x + y * w8) as usize];
                            if (data & mask) != 0 {
                                ptr.write_volatile(color.argb());
                            }
                            ptr = ptr.add(1);
                        }
                        ptr = ptr.add(stride_ptr);
                    }
                }
            } else {
                let mut src_ptr = 0;
                let mut ptr0 = self.get_fb().add(dx as usize + dy as usize * self.stride);
                for _y in 0..height {
                    let mut ptr = ptr0;
                    for _x in 0..w8 {
                        let data = pattern[src_ptr];
                        for mask in BIT_MASKS.iter() {
                            if (data & mask) != 0 {
                                ptr.write_volatile(color.argb());
                            }
                            ptr = ptr.add(1);
                        }
                        src_ptr += 1;
                    }
                    ptr0 = ptr0.add(self.stride);
                }
            }
        }
    }

    pub fn blt(&self, src: &Self, origin: Point<isize>, rect: Rect<isize>) {
        let mut dx = origin.x;
        let mut dy = origin.y;
        let mut sx = rect.origin.x;
        let mut sy = rect.origin.y;
        let mut width = rect.size.width;
        let mut height = rect.size.height;

        {
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
            if width > src.size.width {
                width = src.size.width;
            }
            if height > src.size.height {
                height = src.size.height;
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
        }

        if self.is_portrait() {
            let temp = dx;
            dx = self.size.height - dy;
            dy = temp;
            unsafe {
                let mut p = self
                    .get_fb()
                    .add(dx as usize + dy as usize * self.stride - height as usize);
                let stride_p = self.stride - height as usize;
                let q0 = src
                    .get_fb()
                    .add(sx as usize + (sy + height - 1) as usize * src.stride);
                let stride_q = src.stride;
                if src.is_opaque() {
                    for x in 0..width {
                        let mut q = q0.add(x as usize);
                        for _y in 0..height {
                            let c = q.read_volatile();
                            p.write_volatile(c);
                            p = p.add(1);
                            q = q.sub(stride_q);
                        }
                        p = p.add(stride_p);
                    }
                } else {
                    for x in 0..width {
                        let mut q = q0.add(x as usize);
                        for _y in 0..height {
                            let c = Color::from_argb(q.read_volatile()).components();
                            if c.is_opaque() {
                                p.write_volatile(c.into());
                            } else {
                                let alpha = c.a as usize;
                                let alpha_n = 255 - alpha;
                                let d = c.blend_each(
                                    Color::from_argb(p.read_volatile()).components(),
                                    |a, b| {
                                        ((a as usize * alpha + b as usize * alpha_n) / 255) as u8
                                    },
                                );
                                p.write_volatile(d.into());
                            }

                            p = p.add(1);
                            q = q.sub(stride_q);
                        }
                        p = p.add(stride_p);
                    }
                }
            }
        } else {
            unsafe {
                let mut p = self.get_fb().add(dx as usize + dy as usize * self.stride);
                let stride_p = self.stride - width as usize;
                let mut q = src.get_fb().add(sx as usize + sy as usize * src.stride);
                let stride_q = src.stride - width as usize;
                if src.is_opaque() {
                    if stride_p == 0 && stride_q == 0 {
                        let count = width * height;
                        for _ in 0..count {
                            let c = q.read_volatile();
                            p.write_volatile(c);
                            p = p.add(1);
                            q = q.add(1);
                        }
                    } else {
                        for _y in 0..height {
                            for _x in 0..width {
                                let c = q.read_volatile();
                                p.write_volatile(c);
                                p = p.add(1);
                                q = q.add(1);
                            }
                            p = p.add(stride_p);
                            q = q.add(stride_q);
                        }
                    }
                } else {
                    for _y in 0..height {
                        for _x in 0..width {
                            let c = Color::from_argb(q.read_volatile()).components();
                            if c.is_opaque() {
                                p.write_volatile(c.into());
                            } else {
                                let alpha = c.a as usize;
                                let alpha_n = 255 - alpha;
                                let d = c.blend_each(
                                    Color::from_argb(p.read_volatile()).components(),
                                    |a, b| {
                                        ((a as usize * alpha + b as usize * alpha_n) / 255) as u8
                                    },
                                );
                                p.write_volatile(d.into());
                            }
                            p = p.add(1);
                            q = q.add(1);
                        }
                        p = p.add(stride_p);
                        q = q.add(stride_q);
                    }
                }
            }
        }
    }

    pub fn draw_string(&self, font: &FontDriver, rect: Rect<isize>, color: Color, text: &str) {
        let mut cursor = Point::<isize>::zero();
        let coords = Coordinates::from_rect(rect).unwrap();

        for c in text.chars() {
            let font_size = Size::new(font.width(), font.height());
            if cursor.x + font_size.width >= coords.right {
                cursor.x = 0;
                cursor.y += font.line_height();
            }
            if cursor.y + font_size.height >= coords.bottom {
                break;
            }
            match c {
                '\n' => {
                    cursor.x = 0;
                    cursor.y += font.line_height();
                }
                _ => {
                    let origin = rect.origin + cursor;
                    font.draw_char(c, self, origin, color);
                    cursor.x += font_size.width;
                }
            }
        }
    }
}
