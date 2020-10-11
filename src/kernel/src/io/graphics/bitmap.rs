// Bitmap

use super::color::*;
use super::coords::*;
use crate::io::fonts::*;
use crate::num::*;
use crate::*;
use alloc::sync::Arc;
use alloc::vec::Vec;
use bitflags::*;
use bootprot::BootInfo;
use byteorder::*;
use core::cell::RefCell;
use core::mem::swap;
use core::slice;

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

    pub fn from_vec(vec: Vec<Color>, width: usize, height: usize, is_translucent: bool) -> Self {
        let vec = Arc::new(RefCell::new(vec));
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
            _ => unreachable!(),
        }
        Some(Self::from_vec(bits, width, height, false))
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
            self.fb_unsafe()
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

    pub fn with_same_size(src: &Bitmap) -> Self {
        Self::new(
            src.size.width as usize,
            src.size.height as usize,
            src.is_translucent(),
        )
    }

    #[inline]
    pub const fn size(&self) -> Size<isize> {
        self.size
    }

    #[inline]
    pub const fn width(&self) -> isize {
        self.size.width
    }

    #[inline]
    pub const fn height(&self) -> isize {
        self.size.height
    }

    #[inline]
    pub fn bounds(&self) -> Rect<isize> {
        Rect::from(self.size)
    }

    #[inline]
    pub const fn is_portrait(&self) -> bool {
        self.flags.contains(BitmapFlags::PORTRAIT)
    }

    #[inline]
    pub const fn is_translucent(&self) -> bool {
        self.flags.contains(BitmapFlags::TRANSLUCENT)
    }

    #[inline]
    pub const fn is_opaque(&self) -> bool {
        !self.is_translucent()
    }

    #[inline]
    const unsafe fn fb_unsafe(&self) -> *mut u32 {
        self.base
    }

    #[inline]
    fn get_fb<'a>(&self) -> &'a mut [Color] {
        unsafe {
            slice::from_raw_parts_mut(
                self.base as *mut Color,
                self.stride * self.height() as usize,
            )
        }
    }

    #[inline]
    pub fn update_bitmap<F>(&self, f: F) -> Result<(), ()>
    where
        F: FnOnce(&mut [Color]),
    {
        if self.stride != self.size.width as usize {
            return Err(());
        }
        f(self.get_fb());
        Ok(())
    }

    pub fn reset(&self) {
        self.fill_rect(Rect::from(self.size), Color::zero());
    }

    fn memset_colors(fb: &mut [Color], cursor: usize, size: usize, color: Color) {
        let slice = &mut fb[cursor..cursor + size];
        unsafe {
            let color32 = color.argb();
            let mut ptr: *mut u32 = core::mem::transmute(&slice[0]);
            let mut remain = size;

            while (ptr as usize & 0xF) != 0 && remain > 0 {
                ptr.write_volatile(color32);
                ptr = ptr.add(1);
                remain -= 1;
            }

            if remain > 4 {
                let color128 = color32 as u128
                    | (color32 as u128) << 32
                    | (color32 as u128) << 64
                    | (color32 as u128) << 96;
                let count = remain / 4;
                let mut ptr2 = ptr as *mut u128;

                for _ in 0..count {
                    ptr2.write_volatile(color128);
                    ptr2 = ptr2.add(1);
                }

                ptr = ptr2 as *mut u32;
                remain -= count * 4;
            }

            for _ in 0..remain {
                ptr.write_volatile(color32);
                ptr = ptr.add(1);
            }
        }
    }

    #[inline]
    fn memcpy_colors(
        dest: &mut [Color],
        dest_cursor: usize,
        src: &[Color],
        src_cursor: usize,
        size: usize,
    ) {
        let dest = &mut dest[dest_cursor..dest_cursor + size];
        let src = &src[src_cursor..src_cursor + size];
        for i in 0..size {
            dest[i] = src[i];
        }
    }

    #[inline]
    fn blend_line(
        dest: &mut [Color],
        dest_cursor: usize,
        src: &[Color],
        src_cursor: usize,
        size: usize,
    ) {
        let dest = &mut dest[dest_cursor..dest_cursor + size];
        let src = &src[src_cursor..src_cursor + size];
        for i in 0..size {
            dest[i] = dest[i].blend(src[i]);
        }
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

        let fb = self.get_fb();
        let mut cursor = dx as usize + dy as usize * self.stride;
        if self.stride - width as usize > 0 {
            let stride = self.stride;
            for _ in 0..height {
                Self::memset_colors(fb, cursor, width as usize, color);
                cursor += stride;
            }
        } else {
            Self::memset_colors(fb, cursor, width as usize * height as usize, color);
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

        let fb = self.get_fb();
        let mut cursor = dx as usize + dy as usize * self.stride;
        let stride = self.stride - width as usize;
        for _ in 0..height {
            for _ in 0..width {
                let lhs = fb[cursor].components();
                let c = lhs
                    .blend_color(
                        rhs,
                        |lhs, rhs| {
                            (((lhs as usize) * alpha_n + (rhs as usize) * alpha) / 255) as u8
                        },
                        |a, b| a.saturating_add(b),
                    )
                    .into();
                fb[cursor] = c;
                cursor += 1;
            }
            cursor += stride;
        }
    }

    pub fn draw_multiple_pixels(&self, points: &[Point<isize>], color: Color) {
        let fb = self.get_fb();
        for point in points {
            let mut dx = point.x;
            let mut dy = point.y;
            if dx >= 0 && dx < self.size.width && dy >= 0 && dy < self.size.height {
                if self.is_portrait() {
                    let temp = dx;
                    dx = self.size.height - dy - 1;
                    dy = temp;
                }
                fb[dx as usize + dy as usize * self.stride] = color;
            }
        }
    }

    #[inline]
    pub fn draw_pixel(&self, point: Point<isize>, color: Color) {
        self.draw_multiple_pixels(&[point], color);
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
            todo!();
        } else {
            let fb = self.get_fb();
            let cursor = dx as usize + dy as usize * self.stride;
            Self::memset_colors(fb, cursor, w as usize, color);
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
            todo!();
        } else {
            let fb = self.get_fb();
            let stride = self.stride;
            let mut cursor = dx as usize + dy as usize * stride;
            for _ in 0..h {
                fb[cursor] = color;
                cursor += stride;
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

    pub fn draw_circle(&self, origin: Point<isize>, radius: isize, color: Color) {
        let rect = Rect {
            origin: origin - radius,
            size: Size::new(radius * 2, radius * 2),
        };
        self.draw_round_rect(rect, radius, color);
    }

    pub fn fill_circle(&self, origin: Point<isize>, radius: isize, color: Color) {
        let rect = Rect {
            origin: origin - radius,
            size: Size::new(radius * 2, radius * 2),
        };
        self.fill_round_rect(rect, radius, color);
    }

    pub fn fill_round_rect(&self, rect: Rect<isize>, radius: isize, color: Color) {
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

    pub fn draw_round_rect(&self, rect: Rect<isize>, radius: isize, color: Color) {
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
                let points = [
                    Point::new(dx + bx, dy + by),
                    Point::new(dx + bx, dy + qh - by),
                    Point::new(dx + bx + dw, dy + by),
                    Point::new(dx + bx + dw, dy + qh - by),
                ];
                self.draw_multiple_pixels(&points, color);
            }

            {
                let bx = radius - cx;
                let by = radius - cy;
                let dw = width - bx * 2 - 1;
                let points = [
                    Point::new(dx + bx, dy + by),
                    Point::new(dx + bx, dy + qh - by),
                    Point::new(dx + bx + dw, dy + by),
                    Point::new(dx + bx + dw, dy + qh - by),
                ];
                self.draw_multiple_pixels(&points, color);
            }

            if f >= 0 {
                cx -= 1;
                f -= 4 * cx;
            }
            cy += 1;
            f += 4 * cy + 2;
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

        // TODO:
        if dx < 0 || dx >= self.size.width || dy < 0 || dy >= self.size.height || height == 0 {
            return;
        }

        let fb = self.get_fb();
        if self.is_portrait() {
            dy = self.size.height - dy - height;
            let mut cursor = dy as usize + dx as usize * self.stride;
            let stride = self.stride - height as usize;
            for x in 0..w8 {
                for mask in BIT_MASKS.iter() {
                    for y in (0..height).rev() {
                        let data = pattern[(x + y * w8) as usize];
                        if (data & mask) != 0 {
                            fb[cursor] = color;
                        }
                        cursor += 1;
                    }
                    cursor += stride;
                }
            }
        } else {
            let mut src_cursor = 0;
            let mut cursor = dx as usize + dy as usize * self.stride;
            let stride = self.stride - 8 * w8 as usize;
            for _ in 0..height {
                for _ in 0..w8 {
                    let data = pattern[src_cursor];
                    for mask in BIT_MASKS.iter() {
                        if (data & mask) != 0 {
                            fb[cursor] = color;
                        }
                        cursor += 1;
                    }
                    src_cursor += 1;
                }
                cursor += stride;
            }
        }
    }

    pub fn blt(&self, src: &Self, origin: Point<isize>, rect: Rect<isize>, option: BltOption) {
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
            if width > sx + src.size.width {
                width = src.size.width - sx;
            }
            if height > sy + src.size.height {
                height = src.size.height - sy;
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

        let width = width as usize;
        let height = height as usize;

        if self.is_portrait() {
            let temp = dx;
            dx = self.size.height - dy;
            dy = temp;
            let dest_fb = self.get_fb();
            let src_fb = src.get_fb();
            let mut p = dx as usize + dy as usize * self.stride - height as usize;
            let q0 = sx as usize + (sy as usize + height - 1) * src.stride;
            let stride_p = self.stride - height;
            let stride_q = src.stride;
            if option.contains(BltOption::COPY) || src.is_opaque() {
                for x in 0..width {
                    let mut q = q0 + x;
                    for _ in 0..height {
                        dest_fb[p] = src_fb[q];
                        p += 1;
                        q -= stride_q;
                    }
                    p += stride_p;
                }
            } else {
                for x in 0..width {
                    let mut q = q0 + x;
                    for _ in 0..height {
                        let c = src_fb[q].components();
                        let alpha_l = c.a;
                        let alpha_r = 255 - alpha_l;
                        let c = c.blend_each(dest_fb[p].components(), |a, b| {
                            ((a as usize * alpha_l as usize + b as usize * alpha_r as usize) / 255)
                                as u8
                        });
                        dest_fb[p] = c.into();

                        p += 1;
                        q -= stride_q;
                    }
                    p += stride_p;
                }
            }
        } else {
            let dest_fb = self.get_fb();
            let src_fb = src.get_fb();
            let mut dest_cursor = dx as usize + dy as usize * self.stride;
            let mut src_cursor = sx as usize + sy as usize * src.stride;

            if option.contains(BltOption::COPY) || src.is_opaque() {
                for _ in 0..height {
                    Self::memcpy_colors(dest_fb, dest_cursor, src_fb, src_cursor, width);
                    dest_cursor += self.stride;
                    src_cursor += src.stride;
                }
            } else {
                for _ in 0..height {
                    Self::blend_line(dest_fb, dest_cursor, src_fb, src_cursor, width);
                    dest_cursor += self.stride;
                    src_cursor += src.stride;
                }
            }
        }
    }

    pub fn draw_string(&self, font: &FontDriver, rect: Rect<isize>, color: Color, text: &str) {
        let mut cursor = Point::<isize>::zero();
        let coords = Coordinates::from_rect(rect).unwrap();

        for c in text.chars() {
            let font_size = Size::new(font.width_of(c), font.height());
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
                    font.draw_char(c, self, rect.origin + cursor, color);
                    cursor.x += font_size.width;
                }
            }
        }
    }

    /// query known modes for benchmark
    pub fn known_bench_modes() -> &'static [usize] {
        &[]
    }

    /// Do benchmark
    pub fn bench(_dest: &Self, _src: &Self, mode: usize, _count: usize) {
        match mode {
            _ => unreachable!(),
        }
    }
}

bitflags! {
    pub struct BltOption: usize {
        const COPY = 0b0000_0001;
    }
}
