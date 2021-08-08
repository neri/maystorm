// MEG-OS Arlequin Subsystem

use super::*;
use crate::{
    sync::Mutex,
    ui::theme::Theme,
    *,
    {io::hid::*, ui::text::*, ui::window::*},
};
use alloc::collections::BTreeMap;
use byteorder::*;
use core::{
    alloc::Layout, intrinsics::transmute, num::NonZeroU32, sync::atomic::*, time::Duration,
};
use megstd::drawing::*;
use megstd::rand::*;
use num_traits::FromPrimitive;
use wasm::{wasmintr::*, *};

pub struct ArleBinaryLoader {
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
        Scheduler::current_personality(|personality| match personality.context() {
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
            .load(blob, |mod_name, name, _type_ref| match mod_name {
                ArleRuntime::MOD_NAME => match name {
                    "svc0" | "svc1" | "svc2" | "svc3" | "svc4" | "svc5" | "svc6" => {
                        Ok(ArleRuntime::syscall)
                    }
                    _ => Err(WasmDecodeErrorKind::NoMethod),
                },
                _ => Err(WasmDecodeErrorKind::NoModule),
            })
            .map_err(|v| {
                println!("Load error: {:?}", v);
                ()
            })
    }

    fn invoke_start(self: Box<Self>) -> Option<ProcessId> {
        match self.loader.module().func(ArleRuntime::ENTRY_FUNC_NAME) {
            Ok(_) => {
                let module = self.loader.into_module();
                SpawnOption::new()
                    .personality(ArleRuntime::new(module))
                    .start_process(Self::start, 0, self.lio.name.as_ref())
            }
            Err(err) => {
                println!("error: {:?}", err);
                None
            }
        }
    }
}

/// Contextual structure of the MEG-OS Arlequin subsystem
#[allow(dead_code)]
pub struct ArleRuntime {
    // uuid: Uuid,
    module: WasmModule,
    next_handle: AtomicUsize,
    windows: Mutex<BTreeMap<usize, UnsafeCell<OsWindow>>>,
    rng32: XorShift32,
    key_buffer: Mutex<Vec<KeyEvent>>,
    malloc: Mutex<SimpleAllocator>,
    has_to_exit: AtomicBool,
}

impl ArleRuntime {
    const MOD_NAME: &'static str = "megos-canary";
    const ENTRY_FUNC_NAME: &'static str = "_start";

    const SIZE_KEYBUFFER: usize = 32;

    fn new(module: WasmModule) -> Box<Self> {
        Box::new(Self {
            // uuid: Uuid::generate().unwrap(),
            module,
            next_handle: AtomicUsize::new(1),
            windows: Mutex::new(BTreeMap::new()),
            rng32: XorShift32::default(),
            key_buffer: Mutex::new(Vec::with_capacity(Self::SIZE_KEYBUFFER)),
            malloc: Mutex::new(SimpleAllocator::default()),
            has_to_exit: AtomicBool::new(false),
        })
    }

    fn next_handle(&self) -> usize {
        let result = 1 + self.next_handle.load(Ordering::SeqCst);
        self.next_handle.swap(result, Ordering::SeqCst)
    }

    fn start(&self) -> ! {
        let function = match self.module.func(Self::ENTRY_FUNC_NAME) {
            Ok(v) => v,
            Err(err) => {
                println!("error: {:?}", err);
                RuntimeEnvironment::exit(1);
            }
        };

        match function.invoke(&[]) {
            Ok(_v) => (),
            Err(err) => match err.kind() {
                WasmRuntimeErrorKind::Exit => (),
                _ => println!("error: {:?}", err),
            },
        }

        RuntimeEnvironment::exit(0);
    }

    fn syscall(_: &WasmModule, params: &[WasmValue]) -> Result<WasmValue, WasmRuntimeErrorKind> {
        Scheduler::current_personality(|personality| match personality.context() {
            PersonalityContext::Arlequin(rt) => rt.dispatch_syscall(&params),
            _ => unreachable!(),
        })
        .unwrap()
    }

    fn dispatch_syscall(
        &mut self,
        params: &[WasmValue],
    ) -> Result<WasmValue, WasmRuntimeErrorKind> {
        use megosabi::svc::Function;
        let mut params = ParamsDecoder::new(params);
        let memory = self
            .module
            .memory(0)
            .ok_or(WasmRuntimeErrorKind::OutOfMemory)?;
        let func_no = params.get_u32().and_then(|v| {
            FromPrimitive::from_u32(v).ok_or(WasmRuntimeErrorKind::InvalidParameter)
        })?;
        if self.has_to_exit.load(Ordering::Relaxed) {
            return Err(WasmRuntimeErrorKind::Exit);
        }

        match func_no {
            Function::Exit => {
                return Err(WasmRuntimeErrorKind::Exit);
            }

            Function::Monotonic => {
                return Ok(WasmValue::I32(Timer::monotonic().as_micros() as i32));
            }
            Function::Time => {
                let sub_func_no = params.get_usize()?;
                match sub_func_no {
                    0 => {
                        let time = System::system_time();
                        return Ok(WasmValue::from((time.secs % 86400) as u32));
                    }
                    _ => (),
                }
            }
            Function::Usleep => {
                let us = params.get_u32()? as u64;
                Timer::sleep(Duration::from_micros(us));
            }

            Function::GetSystemInfo => {
                let sub_func_no = params.get_usize()?;
                match sub_func_no {
                    0 => return Ok(WasmValue::from(System::version().as_u32())),
                    _ => (),
                }
            }

            Function::PrintString => {
                params.get_string(memory).map(|s| print!("{}", s));
            }

            Function::NewWindow => {
                let title = params.get_string(memory).unwrap_or("");
                let size = params.get_size()?;
                let bg_color = params
                    .get_color()
                    .unwrap_or(Theme::shared().window_default_background());
                let window_option = params.get_u32().unwrap_or_default();

                let window = WindowBuilder::new()
                    .with_options(window_option)
                    .size(size)
                    .bg_color(bg_color)
                    .build(title);

                if window.as_usize() != 0 {
                    let handle = self.next_handle();
                    let window = UnsafeCell::new(OsWindow::new(handle, window));
                    self.windows.lock().unwrap().insert(handle, window);
                    return Ok(WasmValue::I32(handle as i32));
                }
            }
            Function::CloseWindow => {
                let handle = params.get_usize()?;
                self.windows.lock().unwrap().remove(&handle);
            }
            Function::BeginDraw => match params.get_window(self) {
                Ok(window) => {
                    window.begin_draw();
                    return Ok(WasmValue::from(window.handle() as u32));
                }
                Err(err) => return Err(err),
            },
            Function::EndDraw => match params.get_window(self) {
                Ok(window) => {
                    window.end_draw();
                }
                Err(err) => return Err(err),
            },

            Function::DrawString => {
                let window = params.get_window(self)?;

                let max_lines = 0;
                let origin = params.get_point()?;
                let text = params.get_string(memory).unwrap_or("");
                let color = params.get_color()?;
                let mut rect = Rect::from(window.content_rect().size());
                rect.origin = origin;
                rect.size.width -= origin.x;
                rect.size.height -= origin.y;
                window.draw_in_rect(rect, |bitmap| {
                    AttributedString::new()
                        .align(TextAlignment::Left)
                        .valign(VerticalAlignment::Top)
                        .color(color)
                        .text(text)
                        .draw_text(bitmap, rect.bounds(), max_lines);
                });
            }
            Function::FillRect => {
                let window = params.get_window(self)?;
                let origin = params.get_point()?;
                let size = params.get_size()?;
                let color = params.get_color()?;
                let rect = Rect { origin, size };
                window.draw_in_rect(rect, |bitmap| {
                    bitmap.fill_rect(rect.bounds(), color);
                });
            }
            Function::DrawRect => {
                let window = params.get_window(self)?;
                let origin = params.get_point()?;
                let size = params.get_size()?;
                let color = params.get_color()?;
                let rect = Rect { origin, size };
                window.draw_in_rect(rect, |bitmap| {
                    bitmap.draw_rect(rect.bounds(), color);
                });
            }
            Function::DrawLine => {
                let window = params.get_window(self)?;
                let c1 = params.get_point()?;
                let c2 = params.get_point()?;
                let color = params.get_color()?;
                let rect = Rect::from(Coordinates::from_diagonal(c1, c2)) + Size::new(1, 1);
                window.draw_in_rect(rect, |bitmap| {
                    bitmap.draw_line(c1 - rect.origin, c2 - rect.origin, color);
                });
            }
            Function::WaitChar => {
                let window = params.get_window(self)?;
                return self
                    .wait_key(window)
                    .map(|c| WasmValue::I32(c.unwrap_or('\0') as i32));
            }
            Function::ReadChar => {
                let window = params.get_window(self)?;
                let c = self.read_key(window);
                return Ok(WasmValue::from(
                    c.map(|v| v as u32).unwrap_or(megosabi::OPTION_CHAR_NONE),
                ));
            }

            Function::DrawView => {
                todo!()
            }

            Function::Blt8 => {
                let window = params.get_window(self)?;
                let origin = params.get_point()?;
                let src = params.get_bitmap8(memory)?;
                let rect = Rect {
                    origin,
                    size: src.size(),
                };
                window.draw_in_rect(rect, |bitmap| {
                    bitmap.blt_transparent(
                        &ConstBitmap::from(&src),
                        Point::default(),
                        src.size().into(),
                        IndexedColor::DEFAULT_KEY,
                    );
                });
            }
            Function::Blt32 => {
                let window = params.get_window(self)?;
                let origin = params.get_point()?;
                let src = params.get_bitmap32(memory)?;
                let rect = Rect {
                    origin,
                    size: src.size(),
                };
                window.draw_in_rect(rect, |bitmap| {
                    bitmap.blt(
                        &ConstBitmap::from(&src),
                        Point::default(),
                        src.size().into(),
                    );
                });
            }
            Function::BlendRect => {
                let bitmap = params.get_bitmap32(memory)?;
                let origin = params.get_point()?;
                let size = params.get_size()?;
                let color = params.get_u32().map(|v| TrueColor::from_argb(v))?;
                let rect = Rect { origin, size };
                let mut bitmap: Bitmap32 = unsafe { transmute(bitmap) };
                bitmap.blend_rect(rect, color);
            }
            Function::Blt1 => {
                let window = params.get_window(self)?;
                let origin = params.get_point()?;
                let os_bitmap = params.get_bitmap1(memory)?;
                let color = params.get_color()?;
                let mode = params.get_usize()?;
                window.draw_in_rect(os_bitmap.rect(origin, mode), |bitmap| {
                    os_bitmap.blt(bitmap, Point::default(), color, mode);
                });
            }

            Function::Rand => {
                return Ok(WasmValue::from(self.rng32.next()));
            }
            Function::Srand => {
                let seed = params.get_u32()?;
                NonZeroU32::new(seed).map(|v| self.rng32 = XorShift32::new(v));
            }

            Function::Alloc => {
                let size = params.get_usize()?;
                let align = params.get_usize()?;
                let layout = Layout::from_size_align(size, align)
                    .map_err(|_| WasmRuntimeErrorKind::InvalidParameter)?;
                let mut malloc = self.malloc.lock().unwrap();
                // return Err(WasmRuntimeErrorType::OutOfMemory);

                if let Some(result) = malloc.alloc(layout) {
                    println!("alloc1 {:?} => {:08x}", layout, result);
                    return Ok(WasmValue::from(result.get()));
                } else {
                    let min_alloc = WasmMemory::PAGE_SIZE;
                    let delta = (((layout.size() + min_alloc - 1) / min_alloc) * min_alloc
                        / WasmMemory::PAGE_SIZE) as i32;
                    let new_page = memory.grow(delta);
                    if new_page > 0 {
                        println!("grow {} => {}", delta, new_page);
                        malloc.append_block(
                            new_page as u32 * WasmMemory::PAGE_SIZE as u32,
                            delta as u32 * WasmMemory::PAGE_SIZE as u32,
                        );
                    } else {
                        return Err(WasmRuntimeErrorKind::OutOfMemory);
                    }

                    let result = match malloc.alloc(layout) {
                        Some(v) => v.get(),
                        None => 0,
                    };
                    println!("alloc2 {:?} => {:08x}", layout, result);
                    return Ok(WasmValue::from(result));
                }
            }

            Function::Dealloc => {
                let base = params.get_u32()?;
                let size = params.get_usize()?;
                let align = params.get_usize()?;
                let layout = Layout::from_size_align(size, align)
                    .map_err(|_| WasmRuntimeErrorKind::InvalidParameter)?;

                println!("dealloc {:08x} {:?}", base, layout);
                memory.write_bytes(base as usize, 0xCC, size)?;

                self.malloc.lock().unwrap().dealloc(base, layout);
            }
        }

        Ok(WasmValue::I32(0))
    }

    fn wait_key(&self, window: &OsWindow) -> Result<Option<char>, WasmRuntimeErrorKind> {
        let handle = window.native();
        while let Some(message) = handle.wait_message() {
            self.process_message(handle, message);
            if self.has_to_exit.load(Ordering::Relaxed) {
                return Err(WasmRuntimeErrorKind::Exit);
            }

            if let Some(c) = self
                .read_key_buffer()
                .and_then(|v| v.key_data().map(|v| v.into_char()))
            {
                return Ok(Some(c));
            }
        }
        Err(WasmRuntimeErrorKind::TypeMismatch)
    }

    fn read_key(&self, window: &OsWindow) -> Option<char> {
        let handle = window.native();
        while let Some(message) = handle.read_message() {
            self.process_message(handle, message);
        }
        self.read_key_buffer().map(|v| v.into_char())
    }

    fn read_key_buffer(&self) -> Option<KeyEvent> {
        let mut buffer = self.key_buffer.lock().unwrap();
        if buffer.len() > 0 {
            Some(buffer.remove(0))
        } else {
            None
        }
    }

    fn process_message(&self, window: WindowHandle, message: WindowMessage) {
        match message {
            WindowMessage::Close => {
                if self.windows.lock().unwrap().values().count() > 1 {
                    // todo:
                    window.close();
                } else {
                    self.has_to_exit.store(true, Ordering::SeqCst);
                }
            }
            WindowMessage::Key(event) => {
                event
                    .key_data()
                    .map(|data| self.key_buffer.lock().unwrap().push(data));
            }
            _ => window.handle_default_message(message),
        }
    }
}

impl Personality for ArleRuntime {
    fn context(&mut self) -> PersonalityContext {
        PersonalityContext::Arlequin(self)
    }

    fn on_exit(&mut self) {
        self.windows.lock().unwrap().clear();
    }
}

struct ParamsDecoder<'a> {
    params: &'a [WasmValue],
    index: usize,
}

impl<'a> ParamsDecoder<'a> {
    #[inline]
    pub const fn new(params: &'a [WasmValue]) -> Self {
        Self { params, index: 0 }
    }
}

impl ParamsDecoder<'_> {
    #[inline]
    fn get_u32(&mut self) -> Result<u32, WasmRuntimeErrorKind> {
        let index = self.index;
        self.params
            .get(index)
            .ok_or(WasmRuntimeErrorKind::InvalidParameter)
            .and_then(|v| v.get_u32())
            .map(|v| {
                self.index += 1;
                v
            })
    }

    #[inline]
    fn get_i32(&mut self) -> Result<i32, WasmRuntimeErrorKind> {
        let index = self.index;
        self.params
            .get(index)
            .ok_or(WasmRuntimeErrorKind::InvalidParameter)
            .and_then(|v| v.get_i32())
            .map(|v| {
                self.index += 1;
                v
            })
    }

    #[inline]
    fn get_usize(&mut self) -> Result<usize, WasmRuntimeErrorKind> {
        self.get_u32().map(|v| v as usize)
    }

    #[inline]
    fn get_memarg(&mut self) -> Result<MemArg, WasmRuntimeErrorKind> {
        let base = self.get_u32()? as usize;
        let len = self.get_u32()? as usize;
        Ok(MemArg::new(base, len))
    }

    #[inline]
    fn get_string<'a>(&mut self, memory: &'a WasmMemory) -> Option<&'a str> {
        self.get_memarg()
            .ok()
            .and_then(|memarg| memory.read_bytes(memarg.base(), memarg.len()).ok())
            .and_then(|v| core::str::from_utf8(v).ok())
    }

    #[allow(dead_code)]
    #[inline]
    fn get_string16(&mut self, memory: &WasmMemory) -> Option<String> {
        self.get_memarg()
            .ok()
            .and_then(|memarg| memory.read_bytes(memarg.base(), memarg.len() * 2).ok())
            .and_then(|v| unsafe { core::mem::transmute(v) })
            .and_then(|p| String::from_utf16(p).ok())
    }

    #[inline]
    fn get_point(&mut self) -> Result<Point, WasmRuntimeErrorKind> {
        let x = self.get_i32()? as isize;
        let y = self.get_i32()? as isize;
        Ok(Point::new(x, y))
    }

    #[inline]
    fn get_size(&mut self) -> Result<Size, WasmRuntimeErrorKind> {
        let width = self.get_i32()? as isize;
        let height = self.get_i32()? as isize;
        Ok(Size::new(width, height))
    }

    #[inline]
    fn get_color(&mut self) -> Result<Color, WasmRuntimeErrorKind> {
        self.get_u32().map(|v| PackedColor(v).into())
    }

    fn get_bitmap8<'a>(
        &mut self,
        memory: &'a WasmMemory,
    ) -> Result<ConstBitmap8<'a>, WasmRuntimeErrorKind> {
        const SIZE_OF_BITMAP: usize = 20;
        let base = self.get_u32()? as usize;
        let array = memory.read_bytes(base as usize, SIZE_OF_BITMAP)?;

        let width = LE::read_u32(&array[0..4]) as usize;
        let height = LE::read_u32(&array[4..8]) as usize;
        let _stride = LE::read_u32(&array[8..12]) as usize;
        let base = LE::read_u32(&array[12..16]) as usize;

        let len = width * height;
        let slice = memory.read_bytes(base, len)?;

        Ok(ConstBitmap8::from_bytes(
            slice,
            Size::new(width as isize, height as isize),
        ))
    }

    fn get_bitmap32<'a>(
        &mut self,
        memory: &'a WasmMemory,
    ) -> Result<ConstBitmap32<'a>, WasmRuntimeErrorKind> {
        const SIZE_OF_BITMAP: usize = 20;
        let base = self.get_u32()? as usize;
        let array = memory.read_bytes(base as usize, SIZE_OF_BITMAP)?;

        let width = LE::read_u32(&array[0..4]) as usize;
        let height = LE::read_u32(&array[4..8]) as usize;
        let _stride = LE::read_u32(&array[8..12]) as usize;
        let base = LE::read_u32(&array[12..16]) as usize;

        let len = width * height;
        let slice = memory.read_u32_array(base, len)?;

        Ok(ConstBitmap32::from_bytes(
            slice,
            Size::new(width as isize, height as isize),
        ))
    }

    fn get_bitmap1<'a>(
        &mut self,
        memory: &'a WasmMemory,
    ) -> Result<OsBitmap1<'a>, WasmRuntimeErrorKind> {
        let base = self.get_u32()?;
        OsBitmap1::from_memory(memory, base)
    }

    #[inline]
    fn get_window<'a>(
        &mut self,
        rt: &'a ArleRuntime,
    ) -> Result<&'a mut OsWindow, WasmRuntimeErrorKind> {
        match self.get_u32() {
            Ok(v) => rt
                .windows
                .lock()
                .unwrap()
                .get(&(v as usize))
                .map(|v| unsafe { &mut *v.get() })
                .ok_or(WasmRuntimeErrorKind::InvalidParameter),
            Err(err) => Err(err),
        }
    }
}

struct MemArg {
    base: usize,
    len: usize,
}

impl MemArg {
    #[inline]
    const fn new(base: usize, len: usize) -> Self {
        Self { base, len }
    }

    #[inline]
    const fn base(&self) -> usize {
        self.base
    }

    #[inline]
    const fn len(&self) -> usize {
        self.len
    }
}

struct OsBitmap1<'a> {
    slice: &'a [u8],
    dim: Size,
    stride: usize,
}

impl<'a> OsBitmap1<'a> {
    fn from_memory(memory: &'a WasmMemory, base: u32) -> Result<Self, WasmRuntimeErrorKind> {
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
    #[inline]
    const fn rect(&self, origin: Point, mode: usize) -> Rect {
        let scale = mode as isize;
        Rect {
            origin,
            size: Size::new(self.dim.width * scale, self.dim.height * scale),
        }
    }

    fn blt(&self, to: &mut Bitmap, origin: Point, color: Color, mode: usize) {
        // TODO: clipping
        let scale = mode as isize;
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
                        for offset in &[(0, 0), (0, 1), (1, 0), (1, 1)] {
                            let point =
                                Point::new(origin.x + x + offset.0, origin.y + y + offset.1);
                            unsafe { to.set_pixel_unchecked(point, color) };
                        }
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
                        for offset in &[(0, 0), (0, 1), (1, 0), (1, 1)] {
                            let point =
                                Point::new(origin.x + x + offset.0, origin.y + y + offset.1);
                            unsafe { to.set_pixel_unchecked(point, color) };
                        }
                    }
                }
            }
            cursor += stride;
        }
    }
}

struct OsWindow {
    native: WindowHandle,
    handle: usize,
    draw_region: Coordinates,
}

impl OsWindow {
    #[inline]
    const fn new(handle: usize, native: WindowHandle) -> Self {
        Self {
            native,
            handle,
            draw_region: Coordinates::void(),
        }
    }

    #[inline]
    const fn native(&self) -> WindowHandle {
        self.native
    }

    #[inline]
    const fn handle(&self) -> usize {
        self.handle
    }

    #[inline]
    fn content_rect(&self) -> Rect {
        self.native.content_rect()
    }

    #[inline]
    fn begin_draw(&mut self) {
        self.draw_region = Coordinates::void();
    }

    #[inline]
    fn end_draw(&self) {
        let coords = self.draw_region;
        if coords.left <= coords.right && coords.top <= coords.bottom {
            self.native.invalidate_rect(coords.into());
        }
    }

    #[inline]
    fn add_region(&mut self, rect: Rect) {
        let coords = Coordinates::from_rect(rect).unwrap();
        self.draw_region.merge(coords);
    }

    #[inline]
    fn draw_in_rect<F>(&mut self, rect: Rect, f: F)
    where
        F: FnOnce(&mut Bitmap) -> (),
    {
        let _ = self.native.draw_in_rect(rect, f);
        self.add_region(rect);
    }
}

impl Drop for OsWindow {
    #[inline]
    fn drop(&mut self) {
        self.native.close();
    }
}

pub struct SimpleAllocator {
    data: Vec<SimpleFreePair>,
    strategy: AllocationStrategy,
}

impl SimpleAllocator {
    const MIN_MASK: u32 = 0x0000_000F;

    #[inline]
    pub const fn new(strategy: AllocationStrategy) -> Self {
        Self {
            data: Vec::new(),
            strategy,
        }
    }

    fn merge(&mut self, new_data: Option<SimpleFreePair>) {
        if let Some(new_data) = new_data {
            self.data.push(new_data);
        }
        self.data.sort_by_key(|v| v.base);

        let mut do_retry = false;
        let mut retry_index = 0;
        loop {
            for index in retry_index..self.data.len() - 1 {
                let current = unsafe { self.data.get_unchecked(index) };
                let next = unsafe { self.data.get_unchecked(index + 1) };
                if current.next_base() == next.base {
                    let next = self.data.remove(index + 1);
                    self.data[index].size += next.size;
                    retry_index = index;
                    do_retry = true;
                    break;
                }
            }
            if do_retry == false {
                break;
            }
        }
    }

    pub fn append_block(&mut self, base: u32, size: u32) {
        self.merge(Some(SimpleFreePair::new(base, size)));
    }

    pub fn alloc(&mut self, layout: Layout) -> Option<NonZeroU32> {
        let layout_align = u32::max(layout.align() as u32, Self::MIN_MASK + 1);
        let layout_mask = layout_align - 1;
        let min_alloc =
            (u32::max(layout_align, layout.size() as u32) + Self::MIN_MASK) & !Self::MIN_MASK;
        let max_alloc = (min_alloc + layout_mask) & layout_mask;

        let mut result = 0;
        let mut extend = Vec::new();
        match self.strategy {
            AllocationStrategy::FirstFit => {
                for pair in &mut self.data {
                    if (pair.base & layout_mask) == 0 && pair.size >= min_alloc {
                        result = pair.base;
                        pair.size -= min_alloc;
                        pair.base += min_alloc;
                        break;
                    } else if pair.size >= max_alloc {
                        let redundant = pair.base & layout_mask;
                        extend.push(SimpleFreePair::new(pair.base, redundant));
                        pair.base -= redundant;
                        pair.size -= redundant;

                        result = pair.base;
                        pair.size -= min_alloc;
                        pair.base += min_alloc;
                        break;
                    }
                }
            }
            AllocationStrategy::BestFit => todo!(),
        }
        if extend.len() > 0 {
            self.data.extend_from_slice(extend.as_slice());
            self.merge(None);
        }

        NonZeroU32::new(result)
    }

    pub fn dealloc(&mut self, base: u32, layout: Layout) {
        let layout_align = u32::max(layout.align() as u32, Self::MIN_MASK + 1);
        // let layout_mask = layout_align - 1;
        let min_alloc =
            (u32::max(layout_align, layout.size() as u32) + Self::MIN_MASK) & !Self::MIN_MASK;

        let new_pair = SimpleFreePair::new(base, min_alloc);

        let mut cursor = None;
        for (index, pair) in self.data.iter_mut().enumerate() {
            if new_pair.next_base() == pair.base {
                pair.base = new_pair.base;
                pair.size += min_alloc;
                cursor = Some(index);
                break;
            } else if pair.next_base() == base {
                pair.size += min_alloc;
                cursor = Some(index);
                break;
            }
        }

        if let Some(index) = cursor {
            if index < self.data.len() - 1 {
                let current = unsafe { self.data.get_unchecked(index) };
                let next = unsafe { self.data.get_unchecked(index + 1) };
                if current.next_base() == next.base {
                    let next = self.data.remove(index + 1);
                    self.data[index].size += next.size;
                }
            }
        } else {
            self.merge(Some(new_pair));
        }
        for data in &self.data {
            println!("DATA {:08x} {}", data.base, data.size);
        }
    }
}

impl Default for SimpleAllocator {
    #[inline]
    fn default() -> Self {
        Self::new(AllocationStrategy::FirstFit)
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AllocationStrategy {
    FirstFit,
    BestFit,
}

#[derive(Clone, Copy)]
struct SimpleFreePair {
    base: u32,
    size: u32,
}

impl SimpleFreePair {
    #[inline]
    pub const fn new(base: u32, size: u32) -> Self {
        Self { base, size }
    }

    #[inline]
    pub const fn next_base(&self) -> u32 {
        self.base + self.size
    }
}
