// Arlequin Subsystem

use super::*;
use crate::dev::rng::*;
use crate::num::*;
use crate::uuid::Uuid;
use alloc::collections::BTreeMap;
use byteorder::*;
use core::mem::size_of;
use core::sync::atomic::*;

pub(super) struct ArleBinaryLoader {
    loader: WasmLoader,
    lio: LoadedImageOption,
}

impl ArleBinaryLoader {
    pub fn new() -> Self {
        Self {
            loader: WasmLoader::new(),
            lio: LoadedImageOption::default(),
        }
    }

    fn start(_: usize) {
        MyScheduler::current_personality(|personality| match personality.context() {
            PersonalityContext::Arlequin(rt) => rt.start(),
            _ => unreachable!(),
        });
    }
}

impl BinaryLoader for ArleBinaryLoader {
    fn option(&mut self) -> &mut LoadedImageOption {
        &mut self.lio
    }

    fn load(&mut self, blob: &[u8]) -> Result<(), ()> {
        self.loader
            .load(blob, &|mod_name, name, _type_ref| match mod_name {
                ArleRuntime::MOD_NAME => match name {
                    "svc0" => Ok(ArleRuntime::syscall),
                    "svc1" => Ok(ArleRuntime::syscall),
                    "svc2" => Ok(ArleRuntime::syscall),
                    "svc3" => Ok(ArleRuntime::syscall),
                    "svc4" => Ok(ArleRuntime::syscall),
                    "svc5" => Ok(ArleRuntime::syscall),
                    "svc6" => Ok(ArleRuntime::syscall),
                    _ => Err(WasmDecodeError::DynamicLinkError),
                },
                _ => Err(WasmDecodeError::DynamicLinkError),
            })
            .map_err(|_| ())
    }

    fn invoke_start(self: Box<Self>) -> Option<ThreadHandle> {
        match self.loader.module().func(ArleRuntime::ENTRY_FUNC_NAME) {
            Ok(_) => {
                let module = self.loader.into_module();
                SpawnOption::new()
                    .personality(ArleRuntime::new(module))
                    .spawn(Self::start, 0, self.lio.name.as_ref())
            }
            Err(err) => {
                println!("error: {:?}", err);
                None
            }
        }
    }
}

/// Arlequin subsystem
#[allow(dead_code)]
pub struct ArleRuntime {
    uuid: Uuid,
    module: WasmModule,
    next_handle: AtomicUsize,
    windows: BTreeMap<usize, WindowHandle>,
    rng32: XorShift32,
}

impl ArleRuntime {
    const MOD_NAME: &'static str = "arl";
    const ENTRY_FUNC_NAME: &'static str = "_start";

    fn new(module: WasmModule) -> Box<Self> {
        Box::new(Self {
            uuid: Uuid::generate().unwrap(),
            module,
            next_handle: AtomicUsize::new(1),
            windows: BTreeMap::new(),
            rng32: XorShift32::default(),
        })
    }

    fn next_handle(&self) -> usize {
        self.next_handle.fetch_add(1, Ordering::SeqCst)
    }

    fn start(&self) -> ! {
        match self
            .module
            .func(Self::ENTRY_FUNC_NAME)
            .map(|v| v.invoke(&[]))
        {
            Ok(_) => RuntimeEnvironment::exit(0),
            Err(err) => {
                println!("error: {:?}", err);
                RuntimeEnvironment::exit(1);
            }
        }
    }

    fn syscall(_: &WasmModule, params: &[WasmValue]) -> Result<WasmValue, WasmRuntimeError> {
        MyScheduler::current_personality(|personality| match personality.context() {
            PersonalityContext::Arlequin(rt) => rt.dispatch_syscall(&params),
            _ => unreachable!(),
        })
        .unwrap()
    }

    fn dispatch_syscall(&mut self, params: &[WasmValue]) -> Result<WasmValue, WasmRuntimeError> {
        let module = &self.module;
        let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
        let func_no = Self::get_u32(&params, 0)?;
        match func_no {
            0 => {
                // exit
                let v = Self::get_u32(&params, 1)? as usize;
                RuntimeEnvironment::exit(v);
            }
            1 => {
                // puts_utf8
                let m = Self::get_memarg(&params, 1)?;
                Self::get_string(memory, m).map(|s| print!("{}", s));
            }
            2 => {
                // puts_utf16
                let m = Self::get_memarg(&params, 1)?;
                Self::get_string16(memory, m).map(|s| print!("{}", s));
            }
            3 => {
                // new window
                let m = Self::get_memarg(&params, 1)?;
                let size = Self::get_size(&params, 3)?;
                let title = Self::get_string(memory, m).unwrap_or("");
                let window = WindowBuilder::new(title)
                    .style_add(WindowStyle::NAKED)
                    .size(size)
                    .build();
                window.make_active();

                if window.as_usize() != 0 {
                    let handle = self.next_handle();
                    self.windows.insert(handle, window);
                    return Ok(WasmValue::I32(handle as i32));
                }
            }
            4 => {
                // draw text
                if let Some(window) = self.get_window(&params, 1)? {
                    let origin = Self::get_point(&params, 2)?;
                    let m = Self::get_memarg(&params, 4)?;
                    let color = Self::get_color(&params, 6)?;
                    let text = Self::get_string(memory, m).unwrap_or("");
                    let mut rect = window.frame();
                    rect.origin = origin;
                    rect.size.width -= origin.x * 2;
                    rect.size.height -= origin.y;
                    let mut ats = AttributedString::new(text);
                    // ats.font(FontDescriptor::new(FontFamily::Serif, 16).unwrap());
                    ats.color(color);
                    let _ = window.draw_in_rect(rect, |bitmap| {
                        ats.draw(bitmap, rect.size.into());
                    });
                    window.set_needs_display();
                }
            }
            5 => {
                // fill rect
                if let Some(window) = self.get_window(&params, 1)? {
                    let origin = Self::get_point(&params, 2)?;
                    let size = Self::get_size(&params, 4)?;
                    let color = Self::get_color(&params, 6)?;
                    let rect = Rect { origin, size };
                    let _ = window.draw_in_rect(rect, |bitmap| {
                        bitmap.fill_rect(rect.size.into(), color);
                    });
                    window.set_needs_display();
                }
            }
            6 => {
                // wait key
                if let Some(window) = self.get_window(&params, 1)? {
                    let c = Self::wait_key(window);
                    return Ok(WasmValue::I32(c.unwrap_or('\0') as i32));
                }
            }
            7 => {
                // Blt bitmap8
                if let Some(window) = self.get_window(&params, 1)? {
                    let origin = Self::get_point(&params, 2)?;
                    let os_bitmap = Self::get_bitmap8(&memory, &params, 4)?;
                    let _ = window.draw_in_rect(os_bitmap.rect(origin), |bitmap| {
                        os_bitmap.blt(bitmap, Point::zero());
                    });
                    window.set_needs_display();
                }
            }
            8 => {
                // Blt bitmap1
                if let Some(window) = self.get_window(&params, 1)? {
                    let origin = Self::get_point(&params, 2)?;
                    let os_bitmap = Self::get_bitmap1(&memory, &params, 4)?;
                    let color = Self::get_color(&params, 5)?;
                    let scale = Self::get_i32(&params, 6)? as isize;
                    let _ = window.draw_in_rect(os_bitmap.rect(origin, scale), |bitmap| {
                        os_bitmap.blt(bitmap, Point::zero(), color, scale);
                    });
                    window.set_needs_display();
                }
            }
            // 9=> {}
            10 => {
                // Flip
                if let Some(window) = self.get_window(&params, 1)? {
                    window.refresh_if_needed();
                }
            }
            50 => {
                // monotonic
                return Ok(WasmValue::I32(Timer::monotonic().as_micros() as i32));
            }
            51 => {
                // rand
                return Ok(WasmValue::from(self.rng32.next()));
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }

        Ok(WasmValue::I32(0))
    }

    fn get_u32(params: &[WasmValue], index: usize) -> Result<u32, WasmRuntimeError> {
        params
            .get(index)
            .ok_or(WasmRuntimeError::InvalidParameter)
            .and_then(|v| v.get_u32())
    }

    fn get_i32(params: &[WasmValue], index: usize) -> Result<i32, WasmRuntimeError> {
        params
            .get(index)
            .ok_or(WasmRuntimeError::InvalidParameter)
            .and_then(|v| v.get_i32())
    }

    fn get_memarg(params: &[WasmValue], index: usize) -> Result<MemArg, WasmRuntimeError> {
        let base = Self::get_u32(&params, index)? as usize;
        let len = Self::get_u32(&params, index + 1)? as usize;
        Ok(MemArg::new(base, len))
    }

    fn get_point(params: &[WasmValue], index: usize) -> Result<Point<isize>, WasmRuntimeError> {
        let x = Self::get_i32(&params, index)? as isize;
        let y = Self::get_i32(&params, index + 1)? as isize;
        Ok(Point::new(x, y))
    }

    fn get_size(params: &[WasmValue], index: usize) -> Result<Size<isize>, WasmRuntimeError> {
        let width = Self::get_i32(&params, index)? as isize;
        let height = Self::get_i32(&params, index + 1)? as isize;
        Ok(Size::new(width, height))
    }

    fn get_color(params: &[WasmValue], index: usize) -> Result<Color, WasmRuntimeError> {
        params
            .get(index)
            .ok_or(WasmRuntimeError::InvalidParameter)
            .and_then(|v| v.get_u32())
            .map(|v| Color::from_argb(v))
    }

    fn get_window(
        &self,
        params: &[WasmValue],
        index: usize,
    ) -> Result<Option<WindowHandle>, WasmRuntimeError> {
        params
            .get(index)
            .ok_or(WasmRuntimeError::InvalidParameter)
            .and_then(|v| v.get_u32())
            .map(|v| self.windows.get(&(v as usize)).map(|v| *v))
    }

    fn get_string(memory: &WasmMemory, memarg: MemArg) -> Option<&str> {
        memory
            .read_bytes(memarg.base(), memarg.len())
            .ok()
            .and_then(|v| core::str::from_utf8(v).ok())
    }

    fn get_string16(memory: &WasmMemory, memarg: MemArg) -> Option<String> {
        memory
            .read_bytes(memarg.base(), memarg.len() * 2)
            .ok()
            .and_then(|v| unsafe { core::mem::transmute(v) })
            .and_then(|p| String::from_utf16(p).ok())
    }

    fn get_bitmap8<'a>(
        memory: &'a WasmMemory,
        params: &[WasmValue],
        index: usize,
    ) -> Result<OsBitmap8<'a>, WasmRuntimeError> {
        let base = Self::get_u32(&params, index)?;
        OsBitmap8::from_memory(memory, base)
    }

    fn get_bitmap1<'a>(
        memory: &'a WasmMemory,
        params: &[WasmValue],
        index: usize,
    ) -> Result<OsBitmap1<'a>, WasmRuntimeError> {
        let base = Self::get_u32(&params, index)?;
        OsBitmap1::from_memory(memory, base)
    }

    fn wait_key(window: WindowHandle) -> Option<char> {
        while let Some(message) = window.wait_message() {
            match message {
                WindowMessage::Char(c) => return Some(c),
                _ => window.handle_default_message(message),
            }
        }
        None
    }
}

impl Personality for ArleRuntime {
    fn info(&self) -> PersonalityInfo {
        PersonalityInfo {
            is_native: false,
            cpu_mode: size_of::<usize>(),
            address_size: 4,
        }
    }

    fn context(&mut self) -> PersonalityContext {
        PersonalityContext::Arlequin(self)
    }

    fn on_exit(&mut self) {
        for window in self.windows.values() {
            window.close();
        }
    }
}

struct MemArg {
    base: usize,
    len: usize,
}

impl MemArg {
    const fn new(base: usize, len: usize) -> Self {
        Self { base, len }
    }

    const fn base(&self) -> usize {
        self.base
    }

    const fn len(&self) -> usize {
        self.len
    }
}

const PALETTE: [u32; 256] = [
    0xFF212121, 0xFF0D47A1, 0xFF1B5E20, 0xFF006064, 0xFFb71c1c, 0xFF4A148C, 0xFF795548, 0xFF9E9E9E,
    0xFF616161, 0xFF2196F3, 0xFF4CAF50, 0xFF00BCD4, 0xFFf44336, 0xFF9C27B0, 0xFFFFEB3B, 0xFFFFFFFF,
    0xFF000000, 0xFF330000, 0xFF660000, 0xFF990000, 0xFFCC0000, 0xFFFF0000, 0xFF003300, 0xFF333300,
    0xFF663300, 0xFF993300, 0xFFCC3300, 0xFFFF3300, 0xFF006600, 0xFF336600, 0xFF666600, 0xFF996600,
    0xFFCC6600, 0xFFFF6600, 0xFF009900, 0xFF339900, 0xFF669900, 0xFF999900, 0xFFCC9900, 0xFFFF9900,
    0xFF00CC00, 0xFF33CC00, 0xFF66CC00, 0xFF99CC00, 0xFFCCCC00, 0xFFFFCC00, 0xFF00FF00, 0xFF33FF00,
    0xFF66FF00, 0xFF99FF00, 0xFFCCFF00, 0xFFFFFF00, 0xFF000033, 0xFF330033, 0xFF660033, 0xFF990033,
    0xFFCC0033, 0xFFFF0033, 0xFF003333, 0xFF333333, 0xFF663333, 0xFF993333, 0xFFCC3333, 0xFFFF3333,
    0xFF006633, 0xFF336633, 0xFF666633, 0xFF996633, 0xFFCC6633, 0xFFFF6633, 0xFF009933, 0xFF339933,
    0xFF669933, 0xFF999933, 0xFFCC9933, 0xFFFF9933, 0xFF00CC33, 0xFF33CC33, 0xFF66CC33, 0xFF99CC33,
    0xFFCCCC33, 0xFFFFCC33, 0xFF00FF33, 0xFF33FF33, 0xFF66FF33, 0xFF99FF33, 0xFFCCFF33, 0xFFFFFF33,
    0xFF000066, 0xFF330066, 0xFF660066, 0xFF990066, 0xFFCC0066, 0xFFFF0066, 0xFF003366, 0xFF333366,
    0xFF663366, 0xFF993366, 0xFFCC3366, 0xFFFF3366, 0xFF006666, 0xFF336666, 0xFF666666, 0xFF996666,
    0xFFCC6666, 0xFFFF6666, 0xFF009966, 0xFF339966, 0xFF669966, 0xFF999966, 0xFFCC9966, 0xFFFF9966,
    0xFF00CC66, 0xFF33CC66, 0xFF66CC66, 0xFF99CC66, 0xFFCCCC66, 0xFFFFCC66, 0xFF00FF66, 0xFF33FF66,
    0xFF66FF66, 0xFF99FF66, 0xFFCCFF66, 0xFFFFFF66, 0xFF000099, 0xFF330099, 0xFF660099, 0xFF990099,
    0xFFCC0099, 0xFFFF0099, 0xFF003399, 0xFF333399, 0xFF663399, 0xFF993399, 0xFFCC3399, 0xFFFF3399,
    0xFF006699, 0xFF336699, 0xFF666699, 0xFF996699, 0xFFCC6699, 0xFFFF6699, 0xFF009999, 0xFF339999,
    0xFF669999, 0xFF999999, 0xFFCC9999, 0xFFFF9999, 0xFF00CC99, 0xFF33CC99, 0xFF66CC99, 0xFF99CC99,
    0xFFCCCC99, 0xFFFFCC99, 0xFF00FF99, 0xFF33FF99, 0xFF66FF99, 0xFF99FF99, 0xFFCCFF99, 0xFFFFFF99,
    0xFF0000CC, 0xFF3300CC, 0xFF6600CC, 0xFF9900CC, 0xFFCC00CC, 0xFFFF00CC, 0xFF0033CC, 0xFF3333CC,
    0xFF6633CC, 0xFF9933CC, 0xFFCC33CC, 0xFFFF33CC, 0xFF0066CC, 0xFF3366CC, 0xFF6666CC, 0xFF9966CC,
    0xFFCC66CC, 0xFFFF66CC, 0xFF0099CC, 0xFF3399CC, 0xFF6699CC, 0xFF9999CC, 0xFFCC99CC, 0xFFFF99CC,
    0xFF00CCCC, 0xFF33CCCC, 0xFF66CCCC, 0xFF99CCCC, 0xFFCCCCCC, 0xFFFFCCCC, 0xFF00FFCC, 0xFF33FFCC,
    0xFF66FFCC, 0xFF99FFCC, 0xFFCCFFCC, 0xFFFFFFCC, 0xFF0000FF, 0xFF3300FF, 0xFF6600FF, 0xFF9900FF,
    0xFFCC00FF, 0xFFFF00FF, 0xFF0033FF, 0xFF3333FF, 0xFF6633FF, 0xFF9933FF, 0xFFCC33FF, 0xFFFF33FF,
    0xFF0066FF, 0xFF3366FF, 0xFF6666FF, 0xFF9966FF, 0xFFCC66FF, 0xFFFF66FF, 0xFF0099FF, 0xFF3399FF,
    0xFF6699FF, 0xFF9999FF, 0xFFCC99FF, 0xFFFF99FF, 0xFF00CCFF, 0xFF33CCFF, 0xFF66CCFF, 0xFF99CCFF,
    0xFFCCCCFF, 0xFFFFCCFF, 0xFF00FFFF, 0xFF33FFFF, 0xFF66FFFF, 0xFF99FFFF, 0xFFCCFFFF, 0xFFFFFFFF,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

struct OsBitmap8<'a> {
    slice: &'a [u8],
    dim: Size<isize>,
}

impl<'a> OsBitmap8<'a> {
    fn from_memory(memory: &'a WasmMemory, base: u32) -> Result<Self, WasmRuntimeError> {
        const SIZE_OF_BITMAP: usize = 16;
        let array = memory.read_bytes(base as usize, SIZE_OF_BITMAP)?;

        let width = LE::read_u32(&array[0..4]) as usize;
        let height = LE::read_u32(&array[4..8]) as usize;
        let base = LE::read_u32(&array[8..12]) as usize;

        let dim = Size::new(width as isize, height as isize);
        let size = width * height;
        let slice = memory.read_bytes(base, size)?;

        Ok(Self { slice, dim })
    }
}

impl OsBitmap8<'_> {
    const fn rect(&self, origin: Point<isize>) -> Rect<isize> {
        Rect {
            origin,
            size: self.dim,
        }
    }

    fn blt(&self, to: &Bitmap, origin: Point<isize>) {
        // TODO: clipping
        let stride = self.dim.width as usize;
        let mut cursor = 0;
        for y in 0..self.dim.height {
            for x in 0..self.dim.width {
                let point = Point::new(origin.x + x, origin.y + y);
                let color = Color::from_argb(unsafe {
                    *PALETTE.get_unchecked(*self.slice.get_unchecked(cursor + x as usize) as usize)
                });
                if !color.is_transparent() {
                    to.set_pixel_unchecked(point, color);
                }
            }
            cursor += stride;
        }
    }
}

struct OsBitmap1<'a> {
    slice: &'a [u8],
    dim: Size<isize>,
    stride: usize,
}

impl<'a> OsBitmap1<'a> {
    fn from_memory(memory: &'a WasmMemory, base: u32) -> Result<Self, WasmRuntimeError> {
        const SIZE_OF_BITMAP: usize = 16;
        let array = memory.read_bytes(base as usize, SIZE_OF_BITMAP)?;

        let width = LE::read_u32(&array[0..4]) as usize;
        let height = LE::read_u32(&array[4..8]) as usize;
        let stride = LE::read_u32(&array[8..12]) as usize;
        let base = LE::read_u32(&array[12..16]) as usize;

        let dim = Size::new(width as isize, height as isize);
        let size = stride * height;
        let slice = memory.read_bytes(base, size)?;

        Ok(Self { slice, dim, stride })
    }
}

impl OsBitmap1<'_> {
    const fn rect(&self, origin: Point<isize>, scale: isize) -> Rect<isize> {
        Rect {
            origin,
            size: Size::new(self.dim.width * scale, self.dim.height * scale),
        }
    }

    fn blt(&self, to: &Bitmap, origin: Point<isize>, color: Color, scale: isize) {
        // TODO: clipping
        let stride = self.stride;
        let mut cursor = 0;
        let w8 = self.dim.width as usize / 8;
        let w7 = self.dim.width as usize & 7;
        for y in 0..self.dim.height {
            for i in 0..w8 {
                let data = unsafe { self.slice.get_unchecked(cursor + i) };
                for j in 0..8 {
                    let position = 0x80u8 >> j;
                    if (data & position) != 0 {
                        let x = scale * (i * 8 + j) as isize;
                        let y = y * scale;
                        let point = Point::new(origin.x + x, origin.y + y);
                        to.set_pixel_unchecked(point, color);
                    }
                }
            }
            if w7 > 0 {
                let data = unsafe { self.slice.get_unchecked(cursor + w8) };
                let base_x = w8 * 8;
                for i in 0..w7 {
                    let position = 0x80u8 >> i;
                    if (data & position) != 0 {
                        let x = scale * (i + base_x) as isize;
                        let y = y * scale;
                        let point = Point::new(origin.x + x, origin.y + y);
                        to.set_pixel_unchecked(point, color);
                    }
                }
            }
            cursor += stride;
        }
    }
}
