// Haribote-OS Emulator

use super::*;
use crate::fs::*;
use crate::mem::MemoryManager;
use crate::ui::window::*;
use crate::*;
use alloc::boxed::Box;
use core::time::Duration;
use core::{alloc::Layout, ptr::*};
use core::{slice, str};
use megstd::drawing::*;

#[allow(dead_code)]
mod fonts {
    include!("hankaku.rs");
}

/// Contextual structure of the Haribote-OS Emulator subsystem
pub struct Hoe {
    context: LegacyAppContext,
    cmdline: String,
    windows: Vec<HoeWindow>,
    timers: Vec<HoeTimer>,
    files: Vec<HoeFile>,
    malloc_start: u32,
    malloc_free: u32,
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

impl Hoe {
    const OS_ID: u32 = 0x534F594D;
    const OS_VER: u32 = 0;
    const PALETTE: [u32; 256] = [
        0xFF000000, 0xFFFF0000, 0xFF00FF00, 0xFFFFFF00, 0xFF0000FF, 0xFFFF00FF, 0xFF00FFFF,
        0xFFFFFFFF, 0xFFC6C6C6, 0xFF840000, 0xFF008400, 0xFF848400, 0xFF000084, 0xFF840084,
        0xFF008484, 0xFF848484, 0xFF000000, 0xFF330000, 0xFF660000, 0xFF990000, 0xFFCC0000,
        0xFFFF0000, 0xFF003300, 0xFF333300, 0xFF663300, 0xFF993300, 0xFFCC3300, 0xFFFF3300,
        0xFF006600, 0xFF336600, 0xFF666600, 0xFF996600, 0xFFCC6600, 0xFFFF6600, 0xFF009900,
        0xFF339900, 0xFF669900, 0xFF999900, 0xFFCC9900, 0xFFFF9900, 0xFF00CC00, 0xFF33CC00,
        0xFF66CC00, 0xFF99CC00, 0xFFCCCC00, 0xFFFFCC00, 0xFF00FF00, 0xFF33FF00, 0xFF66FF00,
        0xFF99FF00, 0xFFCCFF00, 0xFFFFFF00, 0xFF000033, 0xFF330033, 0xFF660033, 0xFF990033,
        0xFFCC0033, 0xFFFF0033, 0xFF003333, 0xFF333333, 0xFF663333, 0xFF993333, 0xFFCC3333,
        0xFFFF3333, 0xFF006633, 0xFF336633, 0xFF666633, 0xFF996633, 0xFFCC6633, 0xFFFF6633,
        0xFF009933, 0xFF339933, 0xFF669933, 0xFF999933, 0xFFCC9933, 0xFFFF9933, 0xFF00CC33,
        0xFF33CC33, 0xFF66CC33, 0xFF99CC33, 0xFFCCCC33, 0xFFFFCC33, 0xFF00FF33, 0xFF33FF33,
        0xFF66FF33, 0xFF99FF33, 0xFFCCFF33, 0xFFFFFF33, 0xFF000066, 0xFF330066, 0xFF660066,
        0xFF990066, 0xFFCC0066, 0xFFFF0066, 0xFF003366, 0xFF333366, 0xFF663366, 0xFF993366,
        0xFFCC3366, 0xFFFF3366, 0xFF006666, 0xFF336666, 0xFF666666, 0xFF996666, 0xFFCC6666,
        0xFFFF6666, 0xFF009966, 0xFF339966, 0xFF669966, 0xFF999966, 0xFFCC9966, 0xFFFF9966,
        0xFF00CC66, 0xFF33CC66, 0xFF66CC66, 0xFF99CC66, 0xFFCCCC66, 0xFFFFCC66, 0xFF00FF66,
        0xFF33FF66, 0xFF66FF66, 0xFF99FF66, 0xFFCCFF66, 0xFFFFFF66, 0xFF000099, 0xFF330099,
        0xFF660099, 0xFF990099, 0xFFCC0099, 0xFFFF0099, 0xFF003399, 0xFF333399, 0xFF663399,
        0xFF993399, 0xFFCC3399, 0xFFFF3399, 0xFF006699, 0xFF336699, 0xFF666699, 0xFF996699,
        0xFFCC6699, 0xFFFF6699, 0xFF009999, 0xFF339999, 0xFF669999, 0xFF999999, 0xFFCC9999,
        0xFFFF9999, 0xFF00CC99, 0xFF33CC99, 0xFF66CC99, 0xFF99CC99, 0xFFCCCC99, 0xFFFFCC99,
        0xFF00FF99, 0xFF33FF99, 0xFF66FF99, 0xFF99FF99, 0xFFCCFF99, 0xFFFFFF99, 0xFF0000CC,
        0xFF3300CC, 0xFF6600CC, 0xFF9900CC, 0xFFCC00CC, 0xFFFF00CC, 0xFF0033CC, 0xFF3333CC,
        0xFF6633CC, 0xFF9933CC, 0xFFCC33CC, 0xFFFF33CC, 0xFF0066CC, 0xFF3366CC, 0xFF6666CC,
        0xFF9966CC, 0xFFCC66CC, 0xFFFF66CC, 0xFF0099CC, 0xFF3399CC, 0xFF6699CC, 0xFF9999CC,
        0xFFCC99CC, 0xFFFF99CC, 0xFF00CCCC, 0xFF33CCCC, 0xFF66CCCC, 0xFF99CCCC, 0xFFCCCCCC,
        0xFFFFCCCC, 0xFF00FFCC, 0xFF33FFCC, 0xFF66FFCC, 0xFF99FFCC, 0xFFCCFFCC, 0xFFFFFFCC,
        0xFF0000FF, 0xFF3300FF, 0xFF6600FF, 0xFF9900FF, 0xFFCC00FF, 0xFFFF00FF, 0xFF0033FF,
        0xFF3333FF, 0xFF6633FF, 0xFF9933FF, 0xFFCC33FF, 0xFFFF33FF, 0xFF0066FF, 0xFF3366FF,
        0xFF6666FF, 0xFF9966FF, 0xFFCC66FF, 0xFFFF66FF, 0xFF0099FF, 0xFF3399FF, 0xFF6699FF,
        0xFF9999FF, 0xFFCC99FF, 0xFFFF99FF, 0xFF00CCFF, 0xFF33CCFF, 0xFF66CCFF, 0xFF99CCFF,
        0xFFCCCCFF, 0xFFFFCCFF, 0xFF00FFFF, 0xFF33FFFF, 0xFF66FFFF, 0xFF99FFFF, 0xFFCCFFFF,
        0xFFFFFFFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

    fn new(context: LegacyAppContext, cmdline: String) -> Box<Self> {
        Box::new(Self {
            context,
            cmdline,
            windows: Vec::new(),
            timers: Vec::new(),
            files: Vec::new(),
            malloc_start: 0,
            malloc_free: 0,
        })
    }

    fn abort(&self) -> ! {
        RuntimeEnvironment::exit(1);
    }

    fn exit(&self) -> ! {
        RuntimeEnvironment::exit(0);
    }

    fn raise_segv(&self) -> ! {
        println!("Segmentation Violation");
        self.abort();
    }

    /// Perform Haribote-OS System Call
    pub fn syscall(&mut self, regs: &mut HoeSyscallRegs) {
        match regs.edx {
            1 => {
                // putchar(eax)
                System::stdout().write_char(regs.eax as u8 as char).unwrap();
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
                // open window
                regs.eax = self.alloc_window(
                    self.load_cstring(regs.ecx).unwrap_or_default(),
                    regs.esi,
                    regs.edi,
                    regs.ebx,
                );
            }
            6 => {
                // draw text
                // BUG: The official documentation says to use ECX for the length of the string,
                // but the actual implementation uses the ASCIZ string
                self.get_window(regs.ebx).map(|(window, refreshing)| {
                    let text = self.load_cstring(regs.ebp).unwrap_or_default();
                    let color = regs.eax as u8;
                    let mut origin = Point::new(regs.esi as isize, regs.edi as isize);
                    for ch in text.bytes() {
                        origin.x += window.put_font(self, origin, ch, color, refreshing);
                    }
                });
            }
            7 => {
                // fill rect
                self.get_window(regs.ebx).map(|(window, refreshing)| {
                    window.fill_rect(
                        self,
                        regs.eax,
                        regs.ecx,
                        regs.esi,
                        regs.edi,
                        regs.ebp as u8,
                        refreshing,
                    );
                });
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
                // set pixel
                self.get_window(regs.ebx).map(|(window, refreshing)| {
                    window.set_pixel(self, regs.esi, regs.edi, regs.eax as u8, refreshing);
                });
            }
            12 => {
                // Refresh Window
                self.get_window(regs.ebx).map(|(window, _refreshing)| {
                    window.redraw_rect(self, regs.eax, regs.ecx, regs.esi, regs.edi);
                });
            }
            13 => {
                // draw line
                self.get_window(regs.ebx).map(|(window, refreshing)| {
                    let c0 = Point::new(regs.eax as isize, regs.ecx as isize);
                    let c1 = Point::new(regs.esi as isize, regs.edi as isize);
                    window.draw_line(self, c0, c1, regs.ebp as u8, refreshing);
                });
            }
            14 => {
                // TODO: Close Window
            }
            15 => {
                // Get Key
                let sleep = regs.eax != 0;
                regs.eax = self
                    .windows
                    .first()
                    .and_then(|window| window.get_message(sleep))
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
                    Some(v) => v.handle,
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
            21 => {
                // file open
                let name = self.load_cstring(regs.ebx);
                regs.eax = name.and_then(|name| self.alloc_file(name)).unwrap_or(0);
            }
            22 => {
                // TODO: file close
            }
            23 => {
                // seek
                self.get_file(regs.eax).map(|file| {
                    file.seek(regs.ebx as i32 as isize, Whence::from(regs.ecx as usize));
                });
            }
            24 => {
                // get size
                // SAFETY: Undefined return value when a handle is invalid
                self.get_file(regs.eax).map(|file| {
                    regs.eax = file.get_file_size(Whence::from(regs.ecx as usize)) as u32;
                });
            }
            25 => {
                // read file
                let size = regs.ecx;
                match self.safe_ptr(regs.ebx, size) {
                    Some(ptr) => {
                        self.get_file(regs.eax).map(|file| {
                            regs.eax = file.read(ptr, size);
                        });
                    }
                    None => self.raise_segv(),
                }
            }
            26 => {
                // command line
                match self.store_string(regs.ebx, regs.ecx, self.cmdline.as_ref()) {
                    Ok(v) => regs.eax = v,
                    Err(_) => self.raise_segv(),
                }
            }
            27 => {
                // langmode
                regs.eax = 0;
                regs.ecx = Self::OS_ID;
                regs.edx = Self::OS_VER;
            }
            28 => {
                // TODO: open file for write
                regs.eax = 0;
                // let name = self.load_cstring(regs.ebx);
                // regs.eax = name.and_then(|name| self.alloc_file(name)).unwrap_or(0);
            }
            29 => {
                // TODO: write file
                regs.eax = 0;
            }
            // 30 => {
            //     // void api_osselect(int i);
            // }
            // 31 => {
            //     // int api_sendkey(char *);
            // }
            // 32 => {
            //     // void api_semiFlat(void);
            //     // This API is unsupported because it violates our security policy.
            // }
            33 => {
                // extended API 33
                match regs.ecx {
                    // int api_getTimeCount(void);
                    1 => regs.eax = (Timer::monotonic().as_millis() / 10) as u32,
                    // int api_getkeyEx(int mode);
                    // 2 => {
                    //     let sleep = regs.eax != 0;
                    //     regs.eax = self
                    //         .windows
                    //         .first()
                    //         .and_then(|window| window.get_message(sleep))
                    //         .unwrap_or(0xFFFFFFFF);
                    // }
                    _ => regs.eax = 0,
                }
            }
            _ => {
                println!("Unimplemented syscall {}", regs.edx);
                self.abort();
            }
        }
    }

    /// Returns a safe pointer on the application data segment.
    fn safe_ptr(&self, offset: u32, size: u32) -> Option<usize> {
        if offset > 0 && (offset as u64 + size as u64) < self.context.size_of_data as u64 {
            Some((self.context.base_of_data + offset) as usize)
        } else {
            None
        }
    }

    /// Load an ASCIZ string from the application data segment
    fn load_cstring<'a>(&self, offset: u32) -> Option<&'a str> {
        if offset > 0 {
            unsafe {
                let base = self.context.base_of_data as usize as *const u8;
                let limit = base.add(self.context.size_of_data as usize);
                let base = base.add(offset as usize);

                let mut len = 0;
                loop {
                    let ptr = base.add(len);
                    if ptr.read_volatile() == 0 {
                        break;
                    }
                    if ptr >= limit {
                        return None;
                    }
                    len += 1;
                }

                Some(str::from_utf8_unchecked(slice::from_raw_parts(base, len)))
            }
        } else {
            None
        }
    }

    /// Load a string from the application data segment
    fn load_string<'a>(&self, offset: u32, len: u32) -> Option<&'a str> {
        self.safe_ptr(offset, len).map(|ptr| unsafe {
            str::from_utf8_unchecked(slice::from_raw_parts(ptr as *const u8, len as usize))
        })
    }

    /// Store a string into the application data segment
    fn store_string(&self, offset: u32, size: u32, s: &str) -> Result<u32, ()> {
        self.safe_ptr(offset, size)
            .map(|ptr| {
                let mut ptr = ptr as *mut u8;
                unsafe {
                    ptr.write_bytes(0, size as usize);
                }
                let len = u32::min(s.len() as u32, size);
                unsafe {
                    for c in s[..len as usize].bytes() {
                        ptr.write_volatile(c);
                        ptr = ptr.add(1);
                    }
                }
                len
            })
            .ok_or(())
    }

    fn get_color(index: u8) -> SomeColor {
        SomeColor::from_argb(Self::PALETTE[index as usize])
    }

    fn alloc_window(&mut self, title: &str, width: u32, height: u32, buffer: u32) -> u32 {
        let window = HoeWindow::new(self, title, width, height, buffer);
        self.windows.push(window);
        self.windows.len() as u32 * 2
    }

    fn get_window(&self, handle: u32) -> Option<(&HoeWindow, bool)> {
        let refreshing = (handle & 1) == 0;
        let index = handle as usize / 2 - 1;
        let window = self.windows.get(index);
        window.map(|window| (window, refreshing))
    }

    fn alloc_timer(&mut self) -> u32 {
        self.timers.push(HoeTimer::new());
        self.timers.len() as u32
    }

    fn get_timer(&mut self, handle: u32) -> Option<&mut HoeTimer> {
        self.timers.get_mut(handle as usize - 1)
    }

    fn alloc_file(&mut self, name: &str) -> Option<u32> {
        HoeFile::open(name).map(|file| {
            self.files.push(file);
            self.files.len() as u32
        })
    }

    fn get_file(&mut self, handle: u32) -> Option<&mut HoeFile> {
        self.files.get_mut(handle as usize - 1)
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
}

impl Personality for Hoe {
    fn context(&mut self) -> PersonalityContext {
        PersonalityContext::Hoe(self)
    }

    fn on_exit(&mut self) {
        for window in &self.windows {
            window.handle.close();
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
    const OFFSET_SIGN_1: usize = 4;
    const OFFSET_SIGN_2: usize = 8;
    const MINIMAL_BIN_SIZE: usize = 0x24;
}

pub struct HrbBinaryLoader {
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

    pub fn identity(blob: &[u8]) -> Option<Self> {
        if blob.len() > HrbExecutable::MINIMAL_BIN_SIZE
            && &blob[HrbExecutable::OFFSET_SIGN_1..HrbExecutable::OFFSET_SIGN_2]
                == HrbExecutable::SIGNATURE
        {
            Some(HrbBinaryLoader::new())
        } else {
            None
        }
    }

    fn start(_: usize) {
        let context = Scheduler::current_personality(|personality| {
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

    fn load(&mut self, blob: &[u8]) -> Result<(), ()> {
        unsafe {
            let blob_ptr = &blob[0] as *const u8;
            let header = (blob_ptr as *const HrbExecutable).as_ref().unwrap();
            let size_of_code = header.start_data as usize;
            let rva_data = (size_of_code + 0xFFF) & !0xFFF;
            let size_of_ds = header.size_of_ds as usize;
            let size_of_data = header.size_of_data as usize;
            let image_size = rva_data + size_of_ds;
            let stack_pointer = header.esp as usize;

            let image_base = MemoryManager::zalloc(Layout::from_size_align_unchecked(
                image_size,
                MemoryManager::PAGE_SIZE_MIN,
            ))
            .unwrap()
            .get() as *mut u8;
            image_base.write_bytes(0, image_size);

            let base_code = image_base;
            base_code.copy_from_nonoverlapping(blob_ptr, size_of_code);
            let base_data = image_base.add(rva_data);
            base_data
                .add(stack_pointer)
                .copy_from_nonoverlapping(blob_ptr.add(size_of_code), size_of_data);

            self.ctx.image_base = image_base as u32;
            self.ctx.image_size = image_size as u32;
            self.ctx.base_of_code = base_code as u32;
            self.ctx.size_of_code = size_of_code as u32;
            self.ctx.base_of_data = base_data as u32;
            self.ctx.size_of_data = size_of_ds as u32;
            self.ctx.start = HrbExecutable::START;
            self.ctx.stack_pointer = stack_pointer as u32;
        }
        Ok(())
    }

    fn invoke_start(self: Box<Self>) -> Option<ProcessId> {
        let cmdline = self.lio.argv.join(" ");
        SpawnOption::new()
            .personality(Hoe::new(self.ctx, cmdline))
            .start_process(Self::start, 0, self.lio.name.as_ref())
    }
}

struct HoeWindow {
    handle: WindowHandle,
    buffer: u32,
    width: u32,
    height: u32,
}

impl HoeWindow {
    const WINDOW_BGCOLOR: u8 = 7;
    const WINDOW_ADJUST_X: u32 = 2;
    const WINDOW_ADJUST_TOP: u32 = 22;
    const WINDOW_ADJUST_BOTTOM: u32 = 2;

    fn new(hoe: &Hoe, title: &str, width: u32, height: u32, buffer: u32) -> Self {
        let handle = WindowBuilder::new(title)
            .style_add(WindowStyle::NAKED)
            .size(Size::new(
                (width - Self::WINDOW_ADJUST_X * 2) as isize,
                (height - (Self::WINDOW_ADJUST_TOP + Self::WINDOW_ADJUST_BOTTOM)) as isize,
            ))
            .bg_color(Hoe::get_color(Self::WINDOW_BGCOLOR))
            .build();
        handle.make_active();
        let window = HoeWindow {
            handle,
            width,
            height,
            buffer,
        };
        window.fill_rect(hoe, 0, 0, width, height, Self::WINDOW_BGCOLOR, false);
        window
    }

    fn get_message(&self, sleep: bool) -> Option<u32> {
        let message_handler = |message| match message {
            WindowMessage::Char(c) => match c {
                '\x0D' => Some(0x0A),
                _ => Some(c as u8 as u32),
            },
            WindowMessage::Timer(timer_id) => Some(timer_id as u32),
            _ => {
                self.handle.handle_default_message(message);
                None
            }
        };
        if sleep {
            while let Some(message) = self.handle.wait_message() {
                match message_handler(message) {
                    Some(v) => return Some(v),
                    None => (),
                }
            }
            None
        } else {
            while let Some(message) = self.handle.read_message() {
                match message_handler(message) {
                    Some(v) => return Some(v),
                    None => (),
                }
            }
            None
        }
    }

    fn buffer<'a>(&self, hoe: &Hoe) -> &'a mut [u8] {
        let len = self.width as usize * self.height as usize;
        unsafe {
            slice_from_raw_parts_mut(
                (hoe.context.base_of_data as *mut u8).add(self.buffer as usize),
                len,
            )
            .as_mut()
            .unwrap()
        }
    }

    fn redraw_rect(&self, hoe: &Hoe, x0: u32, y0: u32, x1: u32, y1: u32) {
        let left = u32::max(Self::WINDOW_ADJUST_X, u32::min(x0, x1));
        let top = u32::max(Self::WINDOW_ADJUST_TOP, u32::min(y0, y1));
        let right = u32::min(self.width - Self::WINDOW_ADJUST_X, u32::max(x0, x1));
        let bottom = u32::min(self.height - Self::WINDOW_ADJUST_BOTTOM, u32::max(y0, y1));

        let coords = Coordinates::new(
            (left - Self::WINDOW_ADJUST_X) as isize,
            (top - Self::WINDOW_ADJUST_TOP) as isize,
            (right - Self::WINDOW_ADJUST_X) as isize + 1,
            (bottom - Self::WINDOW_ADJUST_TOP) as isize + 1,
        );

        self.handle
            .draw_in_rect(coords.into(), |bitmap| {
                let stride = self.width as usize;
                let width = bitmap.width() as usize;
                let height = bitmap.height() as usize;
                let buffer = self.buffer(hoe);
                for y in 0..height {
                    let cursor = left as usize + (y + top as usize) * stride;
                    let line = &buffer[cursor..cursor + width];
                    for x in 0..width {
                        let color = Hoe::get_color(line[x]);
                        unsafe {
                            bitmap.set_pixel_unchecked(Point::new(x as isize, y as isize), color);
                        }
                    }
                }
            })
            .unwrap();
        self.handle.set_needs_display();
    }

    fn fill_rect(&self, hoe: &Hoe, x0: u32, y0: u32, x1: u32, y1: u32, c: u8, refreshing: bool) {
        let left = u32::max(Self::WINDOW_ADJUST_X, u32::min(x0, x1));
        let top = u32::max(Self::WINDOW_ADJUST_TOP, u32::min(y0, y1));
        let right = u32::min(self.width - Self::WINDOW_ADJUST_X, u32::max(x0, x1));
        let bottom = u32::min(self.height - Self::WINDOW_ADJUST_BOTTOM, u32::max(y0, y1));

        let buffer = self.buffer(hoe);
        let stride = self.width;
        for y in top..=bottom {
            let line = y * stride;
            let line = &mut buffer[(line + left) as usize..=(line + right) as usize];
            for r in line {
                *r = c;
            }
        }

        if refreshing {
            self.redraw_rect(hoe, left, top, right, bottom);
        }
    }

    fn set_pixel(&self, hoe: &Hoe, x: u32, y: u32, c: u8, refreshing: bool) {
        if x < self.width && y < self.height {
            let buffer = self.buffer(hoe);
            let stride = self.width;
            buffer[(x + y * stride) as usize] = c;
            if refreshing {
                self.redraw_rect(hoe, x, y, x, y);
            }
        }
    }

    fn draw_line(&self, hoe: &Hoe, c0: Point, c1: Point, c: u8, refreshing: bool) {
        let buffer = self.buffer(hoe);
        let width = self.width as isize;
        let height = self.height as isize;
        let stride = self.width as usize;
        c0.line_to(c1, |p| {
            if p.x >= 0 && p.x < width && p.y >= 0 && p.y < height {
                buffer[p.x as usize + p.y as usize * stride] = c;
            }
        });
        if refreshing {
            self.redraw_rect(hoe, c0.x as u32, c0.y as u32, c1.x as u32, c1.y as u32);
        }
    }

    const BIT_MASKS: [u8; 8] = [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01];

    fn put_font(&self, hoe: &Hoe, origin: Point, ch: u8, color: u8, refreshing: bool) -> isize {
        if ch > 0x20
            && ch < 0x80
            && origin.x < self.width as isize - 8
            && origin.y < self.height as isize - 16
        {
            let buffer = self.buffer(hoe);
            let stride = self.width as usize;
            let font_stride = 16;
            let font_offset = (ch as usize - 0x20) * font_stride;
            let glyph = &fonts::FONT_HANKAKU_DATA[font_offset..font_offset + font_stride];
            for y in 0..16 {
                let data = glyph[y as usize];
                let cursor = origin.x as usize + (origin.y as usize + y) * stride;
                let line = &mut buffer[cursor..cursor + 8];
                for (index, bit) in Self::BIT_MASKS.iter().enumerate() {
                    if (data & bit) != 0 {
                        line[index] = color;
                    }
                }
            }
            if refreshing {
                self.redraw_rect(
                    hoe,
                    origin.x as u32,
                    origin.y as u32,
                    origin.x as u32 + 7,
                    origin.y as u32 + 15,
                );
            }
        }
        8
    }
}

struct HoeTimer {
    data: u32,
}

impl HoeTimer {
    fn new() -> Self {
        Self { data: u32::MAX }
    }
}

struct HoeFile(FsRawFileControlBlock);

impl HoeFile {
    fn open(name: &str) -> Option<Self> {
        FileManager::open(name).ok().map(Self)
    }

    fn seek(&mut self, offset: isize, whence: Whence) {
        self.0.lseek(offset as OffsetType, whence);
    }

    fn get_file_size(&mut self, whence: Whence) -> usize {
        let file_pos = self.0.lseek(0, Whence::SeekCur);
        let file_size = self.0.lseek(0, Whence::SeekEnd);
        self.0.lseek(file_pos, Whence::SeekSet);
        match whence {
            Whence::SeekSet => file_size as usize,
            Whence::SeekCur => file_pos as usize,
            Whence::SeekEnd => (file_pos - file_size) as usize,
        }
    }

    fn read(&mut self, ptr: usize, size: u32) -> u32 {
        let dest = unsafe { slice::from_raw_parts_mut(ptr as *mut u8, size as usize) };
        self.0.read(dest).map(|v| v as u32).unwrap_or(0)
    }
}
