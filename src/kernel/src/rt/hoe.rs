// H-OS Emulator

use super::*;
use crate::io::fonts::*;
use crate::mem::memory::*;
use crate::window::*;
use crate::*;
use alloc::boxed::Box;
use core::{slice, str};

#[allow(dead_code)]
pub struct Hoe {
    context: LegacyAppContext,
}

impl Hoe {
    const WINDOW_ADJUST_X: isize = 8;
    const WINDOW_TITLE_PADDING: isize = 28;
    const WINDOW_ADJUST_Y: isize = 8;
    const FONT_ADJUST_Y: isize = 2;

    fn new(context: LegacyAppContext) -> Box<Self> {
        Box::new(Self { context })
    }

    /// Hoe System Call
    pub fn syscall(ctx: &mut HoeSyscallContext) {
        // let personality: &Box<Self> = MyScheduler::current_personality().unwrap().into();

        match ctx.edx {
            1 => {
                // putchar(eax)
                stdout().write_char(ctx.eax as u8 as char).unwrap();
            }
            2 => {
                // putstring(ebx)
                Self::load_cstring(ctx.ebx, ctx).map(|text| println!("{}", text));
            }
            3 => {
                // putstring(ebx, ecx)
                Self::load_string(ctx.ebx, ctx.ecx, ctx).map(|text| println!("{}", text));
            }
            4 => {
                // Exit
                RuntimeEnvironment::exit(0);
            }
            5 => {
                // Window Open
                let title = Self::load_cstring(ctx.ecx, ctx).unwrap_or_default();
                let window = WindowBuilder::new(title)
                    .style_add(WindowStyle::NAKED)
                    .size(Size::new(
                        ctx.esi as isize - Self::WINDOW_ADJUST_X * 2,
                        ctx.edi as isize - (Self::WINDOW_TITLE_PADDING + Self::WINDOW_ADJUST_Y),
                    ))
                    .build();
                window.make_active();
                ctx.eax = (window.0.get() as u32) * 2;
            }
            6 => {
                // Draw String on Window
                let window = Self::get_window(ctx.ebx).unwrap();
                let text = Self::load_string(ctx.ebp, ctx.ecx, ctx).unwrap_or_default();
                let mut rect = window.frame();
                rect.origin = Self::adjusted_coord(ctx.esi, ctx.edi);
                rect.origin.y -= Self::FONT_ADJUST_Y;
                let color = Self::get_color(ctx.eax as u8);
                window
                    .draw(|bitmap| {
                        AttributedString::with(text, FontManager::system_font(), color)
                            .draw(bitmap, rect);
                    })
                    .unwrap();
            }
            7 => {
                // Fill Rect
                let window = Self::get_window(ctx.ebx).unwrap();
                let rect = Self::adjusted_coords(ctx.eax, ctx.ecx, ctx.esi, ctx.edi);
                let color = Self::get_color(ctx.eax as u8);
                window.draw(|bitmap| bitmap.fill_rect(rect, color)).unwrap();
            }
            // 8 | 9 | 10 => {
            //     // TODO: memory
            // }
            11 => {
                // Draw pixel
                let window = Self::get_window(ctx.ebx).unwrap();
                let point = Self::adjusted_coord(ctx.esi, ctx.edi);
                let color = Self::get_color(ctx.eax as u8);
                window
                    .draw(|bitmap| bitmap.draw_pixel(point, color))
                    .unwrap();
            }
            12 => {
                // Refresh Window
                let window = Self::get_window(ctx.ebx).unwrap();
                let rect = Self::adjusted_coords(ctx.eax, ctx.ecx, ctx.esi, ctx.edi);
                window.invalidate_rect(rect);
            }
            13 => {
                // Draw Line
                let window = Self::get_window(ctx.ebx).unwrap();
                let c0 = Self::adjusted_coord(ctx.eax, ctx.ecx);
                let c1 = Self::adjusted_coord(ctx.esi, ctx.edi);
                let color = Self::get_color(ctx.ebp as u8);
                window
                    .draw(|bitmap| bitmap.draw_line(c0, c1, color))
                    .unwrap();
            }
            14 => {
                // Close Window
                let window = Self::get_window(ctx.ebx).unwrap();
                window.close();
            }
            15 => {
                // TODO: Get Key
                // eax = 0; Returns -1 if there is no keystroke. No sleep.
                // eax = 1; Sleep until a keystroke is entered.
                let _flag = ctx.eax != 0;
                Timer::msleep(100);
                ctx.eax = 0xFFFFFFFF;
            }
            // 16 | 17 | 18 | 19 => {
            //     // TODO: timer
            // }
            20 => {
                // TODO: Sound
            }
            // 21 | 22 | 23 | 24 | 25 => {
            //     // TODO: file
            // }
            26 => {
                // TODO: command line
                ctx.eax = 0;
            }
            27 => {
                // langmode
                ctx.eax = 0;
            }
            _ => {
                println!("Unknown syscall {}", ctx.edx);
                RuntimeEnvironment::exit(1);
            }
        }
    }

    fn load_cstring<'a>(offset: u32, ctx: &HoeSyscallContext) -> Option<&'a str> {
        unsafe {
            let base = ctx.ds_base as usize as *const u8;
            let limit = base.add(ctx.ds_limit as usize);
            let ptr = base.add(offset as usize);

            let mut len = 0;
            loop {
                if ptr > limit {
                    return None;
                }
                if ptr.add(len).read_volatile() == 0 {
                    break;
                }
                len += 1;
            }

            str::from_utf8(slice::from_raw_parts(ptr, len)).ok()
        }
    }

    fn load_string<'a>(offset: u32, len: u32, ctx: &HoeSyscallContext) -> Option<&'a str> {
        if offset + len <= ctx.ds_limit {
            unsafe {
                let base = ctx.ds_base as usize as *const u8;
                let ptr = base.add(offset as usize);
                str::from_utf8(slice::from_raw_parts(ptr, len as usize)).ok()
            }
        } else {
            None
        }
    }

    fn get_color(index: u8) -> Color {
        IndexedColor::from(index).into()
    }

    fn get_window(local_handle: u32) -> Option<WindowHandle> {
        WindowHandle::new(local_handle as usize / 2)
    }

    fn adjusted_coord(x: u32, y: u32) -> Point<isize> {
        Point::new(
            x as isize - Self::WINDOW_ADJUST_X,
            y as isize - Self::WINDOW_TITLE_PADDING,
        )
    }

    fn adjusted_coords(l: u32, t: u32, r: u32, b: u32) -> Rect<isize> {
        let c0 = Self::adjusted_coord(l, t);
        let c1 = Self::adjusted_coord(r, b);
        Rect::new(c0.x, c0.y, c1.x - c0.x + 1, c1.y - c0.y + 1)
    }
}

impl Personality for Hoe {
    fn context(&self) -> PersonalityContext {
        PersonalityContext::LegacyApp(self.context)
    }

    fn on_exit(&mut self) {
        // TODO:
    }
}

pub(super) struct HrbRecognizer {
    _phantom: (),
}

impl HrbRecognizer {
    pub fn new() -> Box<Self> {
        Box::new(Self { _phantom: () })
    }
}

impl BinaryRecognizer for HrbRecognizer {
    fn recognize(&self, blob: &[u8]) -> Option<Box<dyn BinaryLoader>> {
        if blob.len() > 0x24 && &blob[4..8] == HrbExecutable::SIGNATURE {
            let hrb = HrbBinaryLoader::new();
            Some(Box::new(hrb))
        } else {
            None
        }
    }
}

struct HrbBinaryLoader {
    lio: LoadedImageOption,
    ctx: LegacyAppContext,
}

impl HrbBinaryLoader {
    fn new() -> Self {
        Self {
            lio: LoadedImageOption::default(),
            ctx: LegacyAppContext::default(),
        }
    }

    fn start(_: usize) {
        let ctx = match MyScheduler::current_personality().unwrap().context() {
            PersonalityContext::LegacyApp(ctx) => ctx,
            _ => unreachable!(),
        };
        unsafe {
            RuntimeEnvironment::invoke_legacy(&ctx);
        }
    }
}

impl BinaryLoader for HrbBinaryLoader {
    fn option(&mut self) -> &mut LoadedImageOption {
        &mut self.lio
    }

    fn load(&mut self, blob: &[u8]) {
        unsafe {
            let blob_ptr = &blob[0] as *const u8;
            let header = (blob_ptr as *const HrbExecutable).as_ref().unwrap();
            let size_of_code = header.start_data as usize;
            let rva_data = (size_of_code + 0xFFF) & !0xFFF;
            let size_of_ds = header.size_of_ds as usize;
            let size_of_data = header.size_of_data as usize;
            let size_of_image = rva_data + size_of_ds;
            let stack_pointer = header.esp as usize;

            let base = MemoryManager::zalloc(size_of_image).unwrap().get() as *mut u8;

            let base_code = base;
            base_code.copy_from_nonoverlapping(blob_ptr, size_of_code);
            let base_data = base.add(rva_data);
            base_data
                .add(stack_pointer)
                .copy_from_nonoverlapping(blob_ptr.add(size_of_code), size_of_data);

            self.ctx.base_of_image = base as u32;
            self.ctx.size_of_image = size_of_image as u32;
            self.ctx.base_of_code = base_code as u32;
            self.ctx.size_of_code = size_of_code as u32;
            self.ctx.base_of_data = base_data as u32;
            self.ctx.size_of_data = size_of_ds as u32;
            self.ctx.start = HrbExecutable::START;
            self.ctx.stack_pointer = stack_pointer as u32;
        }
    }

    fn invoke_start(&mut self, name: &str) -> Option<ThreadHandle> {
        SpawnOption::new()
            .personality(Hoe::new(self.ctx))
            .spawn(Self::start, 0, name)
    }
}

#[repr(C)]
#[derive(Debug)]
struct HrbExecutable {
    size_of_ds: u32,
    /// Must be b"Hari"
    signature: [u8; 4],
    size_of_bss: u32,
    esp: u32,
    size_of_data: u32,
    start_data: u32,
    _start: [u8; 8],
    start_malloc: u32,
}

impl HrbExecutable {
    const SIGNATURE: &'static [u8; 4] = b"Hari";
    const START: u32 = 0x1B;
}

#[repr(C)]
pub struct HoeSyscallContext {
    eax: u32,
    ecx: u32,
    edx: u32,
    ebx: u32,
    esi: u32,
    edi: u32,
    ebp: u32,
    _padding7: u32,
    ds_base: u32,
    _padding8: u32,
    ds_limit: u32,
    _padding9: u32,
}
