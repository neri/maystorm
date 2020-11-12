// H-OS Emulator

use super::*;
use crate::io::fonts::*;
use crate::mem::memory::*;
use crate::window::*;
use crate::*;
use alloc::boxed::Box;
use core::time::Duration;
use core::{slice, str};

#[allow(dead_code)]
pub struct Hoe {
    context: LegacyAppContext,
    windows: Vec<WindowHandle>,
    timers: Vec<HoeTimer>,
    malloc_start: u32,
    malloc_free: u32,
}

impl Hoe {
    const WINDOW_ADJUST_X: isize = 2;
    const WINDOW_TITLE_PADDING: isize = 22;
    const WINDOW_ADJUST_Y: isize = 2;
    const FONT_ADJUST_Y: isize = 2;

    const PALETTE: [u32; 16] = [
        0x000000, 0xFF0000, 0x00FF00, 0xFFFF00, 0x0000FF, 0xFF00FF, 0x00FFFF, 0xFFFFFF, 0xC6C6C6,
        0x840000, 0x008400, 0x848400, 0x000084, 0x840084, 0x008484, 0x848484,
    ];

    fn new(context: LegacyAppContext) -> Box<Self> {
        Box::new(Self {
            context,
            windows: Vec::new(),
            timers: Vec::new(),
            malloc_start: 0,
            malloc_free: 0,
        })
    }

    fn abort(&self) {
        RuntimeEnvironment::exit(1);
    }

    fn exit(&self) {
        RuntimeEnvironment::exit(0);
    }

    /// Hoe System Call
    pub fn syscall(&mut self, regs: &mut HoeSyscallRegs) {
        match regs.edx {
            1 => {
                // putchar(eax)
                stdout().write_char(regs.eax as u8 as char).unwrap();
            }
            2 => {
                // putstring(ebx)
                self.load_cstring(regs.ebx).map(|text| print!("{}", text));
            }
            3 => {
                // putstring(ebx, ecx)
                self.load_string(regs.ebx, regs.ecx)
                    .map(|text| print!("{}", text));
            }
            4 => {
                // Exit
                self.exit();
            }
            5 => {
                // Window Open
                let title = self.load_cstring(regs.ecx).unwrap_or_default();
                let window = WindowBuilder::new(title)
                    .style_add(WindowStyle::NAKED)
                    .size(Size::new(
                        regs.esi as isize - Self::WINDOW_ADJUST_X * 2,
                        regs.edi as isize - (Self::WINDOW_TITLE_PADDING + Self::WINDOW_ADJUST_Y),
                    ))
                    .default_message_queue()
                    .build();
                window.make_active();
                regs.eax = self.alloc_window(window);
            }
            6 => {
                // Draw String on Window
                let (window, _refreshing) = self.get_window(regs.ebx);
                window.map(|window| {
                    let text = self.load_string(regs.ebp, regs.ecx).unwrap_or_default();
                    let mut rect = window.frame();
                    rect.origin = Self::adjusted_coord(regs.esi, regs.edi);
                    rect.origin.y -= Self::FONT_ADJUST_Y;
                    let color = Self::get_color(regs.eax as u8);
                    window
                        .draw(|bitmap| {
                            AttributedString::with(text, FontManager::system_font(), color)
                                .draw(bitmap, rect);
                        })
                        .unwrap()
                });
            }
            7 => {
                // Fill Rect
                let (window, _refreshing) = self.get_window(regs.ebx);
                let rect = Self::adjusted_rect(regs.eax, regs.ecx, regs.esi, regs.edi);
                let color = Self::get_color(regs.ebp as u8);
                window.map(|window| window.draw(|bitmap| bitmap.fill_rect(rect, color)).unwrap());
            }
            8 => {
                // init malloc
                self.malloc_start = regs.eax;
                self.malloc_free = regs.ecx;
            }
            9 => {
                // malloc
                regs.eax = self.malloc(regs.ecx);
            }
            10 => {
                // free
                self.free(regs.eax, regs.ecx);
            }
            11 => {
                // Draw pixel
                let (window, _refreshing) = self.get_window(regs.ebx);
                let point = Self::adjusted_coord(regs.esi, regs.edi);
                let color = Self::get_color(regs.eax as u8);
                window.map(|window| {
                    window
                        .draw(|bitmap| bitmap.draw_pixel(point, color))
                        .unwrap()
                });
            }
            12 => {
                // Refresh Window
                let (window, _refreshing) = self.get_window(regs.ebx);
                let rect = Self::adjusted_rect(regs.eax, regs.ecx, regs.esi, regs.edi);
                window.map(|window| window.invalidate_rect(rect));
            }
            13 => {
                // Draw Line
                let (window, _refreshing) = self.get_window(regs.ebx);
                let c0 = Self::adjusted_coord(regs.eax, regs.ecx);
                let c1 = Self::adjusted_coord(regs.esi, regs.edi);
                let color = Self::get_color(regs.ebp as u8);
                window.map(|window| {
                    window
                        .draw(|bitmap| bitmap.draw_line(c0, c1, color))
                        .unwrap()
                });
            }
            14 => {
                // TODO: Close Window
                // let (window, _refreshing) = self.get_window(regs.ebx);
                // window.map(|window| window.close());
            }
            15 => {
                // Get Key
                let sleep = regs.eax != 0;
                regs.eax = self
                    .windows
                    .first()
                    .and_then(|window| loop {
                        while let Some(message) = window.read_message() {
                            match message {
                                WindowMessage::Char(c) => return Some(c as u8 as u32),
                                WindowMessage::Timer(timer_id) => return Some(timer_id as u32),
                                _ => window.handle_default_message(message),
                            }
                        }
                        if sleep {
                            Timer::msleep(100);
                        } else {
                            return None;
                        }
                    })
                    .map(|k| match k {
                        0x0D => 0x0A,
                        _ => k,
                    })
                    .unwrap_or(0xFFFFFFFF);
            }
            16 => {
                // alloc timer
                regs.eax = self.alloc_timer();
            }
            17 => {
                // init timer
                self.get_timer(regs.ebx).map(|timer| timer.data = regs.eax);
            }
            18 => {
                // set timer
                let window = match self.windows.first() {
                    Some(v) => *v,
                    None => return,
                };
                let timer_id = match self.get_timer(regs.ebx) {
                    Some(t) => t.data as usize,
                    None => return,
                };
                window.create_timer(timer_id, Duration::from_millis(regs.eax as u64 * 10));
            }
            19 => {
                // TODO: free timer
            }
            20 => {
                // TODO: Sound
            }
            // 21 | 22 | 23 | 24 | 25 => {
            //     // TODO: file
            // }
            26 => {
                // TODO: command line
                regs.eax = 0;
            }
            27 => {
                // langmode
                regs.eax = 0;
            }
            _ => {
                println!("Unimplemented syscall {}", regs.edx);
                self.abort();
            }
        }
    }

    fn load_cstring<'a>(&self, offset: u32) -> Option<&'a str> {
        unsafe {
            let base = self.context.base_of_data as usize as *const u8;
            let limit = base.add(self.context.size_of_data as usize);
            let ptr = base.add(offset as usize);

            let mut len = 0;
            loop {
                if ptr >= limit {
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

    fn load_string<'a>(&self, offset: u32, len: u32) -> Option<&'a str> {
        if offset + len < self.context.size_of_data {
            unsafe {
                let base = self.context.base_of_data as usize as *const u8;
                let ptr = base.add(offset as usize);
                str::from_utf8(slice::from_raw_parts(ptr, len as usize)).ok()
            }
        } else {
            None
        }
    }

    fn get_color(index: u8) -> Color {
        Color::from_rgb(Self::PALETTE[index as usize & 0x0F])
    }

    fn alloc_window(&mut self, window: WindowHandle) -> u32 {
        self.windows.push(window);
        self.windows.len() as u32 * 2
    }

    fn get_window(&self, handle: u32) -> (Option<WindowHandle>, bool) {
        let refreshing = (handle & 1) != 0;
        let index = handle as usize / 2 - 1;
        let window = self.windows.get(index).map(|v| v.clone());
        (window, refreshing)
    }

    fn alloc_timer(&mut self) -> u32 {
        self.timers.push(HoeTimer::new());
        self.timers.len() as u32
    }

    fn get_timer(&mut self, handle: u32) -> Option<&mut HoeTimer> {
        self.timers.get_mut(handle as usize - 1)
    }

    fn adjusted_coord(x: u32, y: u32) -> Point<isize> {
        Point::new(
            x as isize - Self::WINDOW_ADJUST_X,
            y as isize - Self::WINDOW_TITLE_PADDING,
        )
    }

    fn adjusted_rect(l: u32, t: u32, r: u32, b: u32) -> Rect<isize> {
        let c0 = Self::adjusted_coord(l, t);
        let c1 = Self::adjusted_coord(r, b);
        Rect::new(c0.x, c0.y, c1.x - c0.x + 1, c1.y - c0.y + 1)
    }

    fn malloc(&mut self, size: u32) -> u32 {
        let size = (size + 0xF) & !0xF;
        // TODO:
        self.malloc_free -= size;
        let result = self.malloc_start;
        self.malloc_start += size;
        result as u32
    }

    fn free(&mut self, ptr: u32, size: u32) {
        let _ = ptr;
        let _ = size;
        // TODO:
    }

    pub fn handle_syscall(regs: &mut HoeSyscallRegs) {
        MyScheduler::current_personality(|personality| {
            let hoe = match personality.context() {
                PersonalityContext::Hoe(hoe) => hoe,
                _ => unreachable!(),
            };
            hoe.syscall(regs);
        });
    }
}

impl Personality for Hoe {
    fn context(&mut self) -> PersonalityContext {
        PersonalityContext::Hoe(self)
    }

    fn on_exit(&mut self) {
        for window in &self.windows {
            window.close();
        }
    }
}

#[repr(C)]
#[derive(Debug)]
struct HrbExecutable {
    /// Size of data segment
    size_of_ds: u32,
    /// Must be "Hari"
    signature: [u8; 4],
    /// Size of bss?
    size_of_bss: u32,
    /// Initial Stack Pointer
    esp: u32,
    /// Size of data in file
    size_of_data: u32,
    /// Size of code and start data in file
    start_data: u32,
    /// startup machine code
    _start: [u8; 8],
    /// Malloc area?
    start_malloc: u32,
}

impl HrbExecutable {
    const SIGNATURE: &'static [u8; 4] = b"Hari";
    const START: u32 = 0x1B;
    const MINIMAL_BIN_SIZE: usize = 0x24;
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
        if blob.len() > HrbExecutable::MINIMAL_BIN_SIZE && &blob[4..8] == HrbExecutable::SIGNATURE {
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
        let context = MyScheduler::current_personality(|personality| {
            let hoe = match personality.context() {
                PersonalityContext::Hoe(hoe) => hoe,
                _ => unreachable!(),
            };
            hoe.context
        });
        unsafe {
            RuntimeEnvironment::invoke_legacy(&context.unwrap());
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
#[derive(Debug, Default)]
pub struct HoeSyscallRegs {
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
    pub ebx: u32,
    pub esi: u32,
    pub edi: u32,
    pub ebp: u32,
    _padding7: u32,
}

struct HoeTimer {
    data: u32,
}

impl HoeTimer {
    fn new() -> Self {
        Self { data: u32::MAX }
    }
}
