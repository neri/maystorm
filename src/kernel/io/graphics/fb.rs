// Framebuffer

use super::color::*;
use super::coords::*;
use core::mem::swap;

#[repr(C)]
pub struct FrameBuffer {
    base: *mut u8,
    size: Size<isize>,
    delta: usize,
    is_portrait: bool,
}

static BIT_MASKS: [u8; 8] = [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01];

impl From<&crate::boot::BootInfo> for FrameBuffer {
    fn from(info: &crate::boot::BootInfo) -> Self {
        let delta = info.fb_delta;
        let mut width = info.screen_width;
        let mut height = info.screen_height;
        let mut is_portrait = height > width;
        if is_portrait {
            // portrait
            swap(&mut width, &mut height);
        }
        if delta > width {
            // GPD micro PC fake landscape mode
            is_portrait = true;
        }
        FrameBuffer {
            base: info.fb_base as *mut u8,
            size: Size {
                width: width as isize,
                height: height as isize,
            },
            delta: delta.into(),
            is_portrait: is_portrait,
        }
    }
}

impl FrameBuffer {
    #[inline]
    pub fn size(&self) -> Size<isize> {
        self.size
    }

    #[inline]
    unsafe fn get_fb(&self) -> *mut u32 {
        self.base as *mut u32
    }

    pub fn reset(&self) {
        self.fill_rect(Rect::from(self.size), Color::from(0));
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

        if self.is_portrait {
            let temp = dx;
            dx = self.size.height - dy - height;
            dy = temp;
            swap(&mut width, &mut height);
        }

        unsafe {
            let mut ptr = self.get_fb().add(dx as usize + dy as usize * self.delta);
            let delta_ptr = self.delta - width as usize;
            for _y in 0..height {
                for _x in 0..width {
                    ptr.write_volatile(color.rgb());
                    ptr = ptr.add(1);
                }
                ptr = ptr.add(delta_ptr);
            }
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
            if self.is_portrait {
                dy = self.size.height - dy - height;
                let mut ptr = self.get_fb().add(dy as usize + dx as usize * self.delta);
                let delta_ptr = self.delta - height as usize;

                for x in 0..w8 {
                    for mask in BIT_MASKS.iter() {
                        for y in (0..height).rev() {
                            let data = pattern[(x + y * w8) as usize];
                            if (data & mask) != 0 {
                                ptr.write_volatile(color.rgb());
                            }
                            ptr = ptr.add(1);
                        }
                        ptr = ptr.add(delta_ptr);
                    }
                }
            } else {
                let mut src_ptr = 0;
                let mut ptr = self.get_fb().add(dx as usize + dy as usize * self.delta);
                let delta_ptr = self.delta - width as usize;
                for _y in 0..height {
                    for _x in 0..w8 {
                        let data = pattern[src_ptr];
                        for mask in BIT_MASKS.iter() {
                            if (data & mask) != 0 {
                                ptr.write_volatile(color.rgb());
                            }
                            ptr = ptr.add(1);
                        }
                        src_ptr += 1;
                    }
                    ptr = ptr.add(delta_ptr);
                }
            }
        }
    }

    //pub fn blt(&self) {}
}
