// MEG-OS Arlequin Subsystem

use super::*;
use crate::io::hid::*;
use crate::util::rng::*;
use crate::util::text::*;
use alloc::collections::BTreeMap;
use byteorder::*;
use core::{
    convert::TryFrom, intrinsics::transmute, num::NonZeroU32, sync::atomic::*, time::Duration,
};
use megstd::drawing::*;
use myosabi::*;
use wasm::{wasmintr::*, *};

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
    // uuid: Uuid,
    module: WasmModule,
    next_handle: AtomicUsize,
    windows: BTreeMap<usize, WindowHandle>,
    rng32: XorShift32,
    key_buffer: Vec<KeyEvent>,
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
            windows: BTreeMap::new(),
            rng32: XorShift32::default(),
            key_buffer: Vec::with_capacity(Self::SIZE_KEYBUFFER),
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
            Ok(_v) => {
                // println!("exit: {}", v.get_u32().unwrap_or(0));
                RuntimeEnvironment::exit(0);
            }
            Err(err) => {
                println!("error: {:?}", err);
                RuntimeEnvironment::exit(1);
            }
        }
    }

    fn syscall(_: &WasmModule, params: &[WasmValue]) -> Result<WasmValue, WasmRuntimeError> {
        Scheduler::current_personality(|personality| match personality.context() {
            PersonalityContext::Arlequin(rt) => rt.dispatch_syscall(&params),
            _ => unreachable!(),
        })
        .unwrap()
    }

    fn dispatch_syscall(&mut self, params: &[WasmValue]) -> Result<WasmValue, WasmRuntimeError> {
        let mut params = ParamsDecoder::new(params);
        let memory = self.module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
        let func_no = params.get_u32().and_then(|v| {
            svc::Function::try_from(v).map_err(|_| WasmRuntimeError::InvalidParameter)
        })?;

        match func_no {
            svc::Function::Exit => {
                let v = params.get_usize()?;
                RuntimeEnvironment::exit(v);
            }

            svc::Function::Monotonic => {
                return Ok(WasmValue::I32(Timer::monotonic().as_micros() as i32));
            }
            svc::Function::Time => {
                let sub_func_no = params.get_usize()?;
                match sub_func_no {
                    0 => {
                        let time = System::system_time();
                        return Ok(WasmValue::from((time.secs % 86400) as u32));
                    }
                    _ => (),
                }
            }
            svc::Function::Usleep => {
                let us = params.get_u32()? as u64;
                Timer::sleep(Duration::from_micros(us));
            }

            svc::Function::GetSystemInfo => {
                let sub_func_no = params.get_usize()?;
                match sub_func_no {
                    0 => return Ok(WasmValue::from(System::version().as_u32())),
                    _ => (),
                }
            }

            svc::Function::PrintString => {
                params.get_string(memory).map(|s| print!("{}", s));
            }

            svc::Function::NewWindow => {
                let title = params.get_string(memory).unwrap_or("");
                let size = params.get_size()?;
                let bg_color = params.get_color().unwrap_or(WindowManager::DEFAULT_BGCOLOR);
                let window_option = params.get_u32().unwrap_or(0);

                let window = WindowBuilder::new(title)
                    .style_add(WindowStyle::NAKED)
                    .size(size)
                    .bg_color(bg_color)
                    .bitmap_strategy(if (window_option & MyOsAbi::WINDOW_32BIT_BITMAP) != 0 {
                        BitmapStrategy::Expressive
                    } else {
                        BitmapStrategy::Compact
                    })
                    .build();
                window.make_active();

                if window.as_usize() != 0 {
                    let handle = self.next_handle();
                    self.windows.insert(handle, window);
                    return Ok(WasmValue::I32(handle as i32));
                }
            }
            svc::Function::CloseWindow => {
                let handle = params.get_usize()?;
                if let Some(window) = self.windows.get(&handle) {
                    window.close();
                    self.windows.remove(&handle);
                }
            }
            svc::Function::DrawString => {
                if let Some(window) = params.get_window(self)? {
                    let max_lines = 0;
                    let origin = params.get_point()?;
                    let text = params.get_string(memory).unwrap_or("");
                    let color = params.get_color()?;
                    let mut rect = Rect::from(window.content_rect().size());
                    rect.origin = origin;
                    rect.size.width -= origin.x;
                    rect.size.height -= origin.y;
                    let _ = window.draw_in_rect(rect, |bitmap| {
                        AttributedString::props()
                            .align(TextAlignment::Left)
                            .valign(VerticalAlignment::Top)
                            .color(color)
                            .text(text)
                            .draw_text(bitmap, rect.size.into(), max_lines);
                    });
                    window.set_needs_display();
                }
            }
            svc::Function::FillRect => {
                if let Some(window) = params.get_window(self)? {
                    let origin = params.get_point()?;
                    let size = params.get_size()?;
                    let color = params.get_color()?;
                    let rect = Rect { origin, size };
                    let _ = window.draw_in_rect(rect, |bitmap| {
                        bitmap.fill_rect(rect.size.into(), color);
                    });
                    window.set_needs_display();
                }
            }
            svc::Function::DrawRect => {
                if let Some(window) = params.get_window(self)? {
                    let origin = params.get_point()?;
                    let size = params.get_size()?;
                    let color = params.get_color()?;
                    let rect = Rect { origin, size };
                    let _ = window.draw_in_rect(rect, |bitmap| {
                        bitmap.draw_rect(rect.size.into(), color);
                    });
                    window.set_needs_display();
                }
            }
            svc::Function::DrawLine => {
                if let Some(window) = params.get_window(self)? {
                    let c1 = params.get_point()?;
                    let c2 = params.get_point()?;
                    let color = params.get_color()?;
                    let rect = Rect::from(Coordinates::from_two(c1, c2)) + Size::new(1, 1);
                    let _ = window.draw_in_rect(rect, |bitmap| {
                        bitmap.draw_line(c1 - rect.origin, c2 - rect.origin, color);
                    });
                    window.set_needs_display();
                }
            }
            svc::Function::WaitChar => {
                if let Some(window) = params.get_window(self)? {
                    let c = self.wait_key(window);
                    return Ok(WasmValue::I32(c.unwrap_or('\0') as i32));
                }
            }
            svc::Function::ReadChar => {
                if let Some(window) = params.get_window(self)? {
                    let c = self.read_key(window);
                    return Ok(WasmValue::from(
                        c.map(|v| v as u32).unwrap_or(MyOsAbi::OPTION_CHAR_NONE),
                    ));
                }
            }
            svc::Function::Blt8 => {
                if let Some(window) = params.get_window(self)? {
                    let origin = params.get_point()?;
                    let src = params.get_bitmap8(memory)?;
                    let rect = Rect {
                        origin,
                        size: src.size(),
                    };
                    let _ = window.draw_in_rect(rect, |bitmap| {
                        bitmap.blt_transparent(
                            &ConstBitmap::from(&src),
                            Point::default(),
                            src.size().into(),
                            IndexedColor::DEFAULT_KEY,
                        );
                    });
                    window.set_needs_display();
                }
            }
            svc::Function::Blt32 => {
                if let Some(window) = params.get_window(self)? {
                    let origin = params.get_point()?;
                    let src = params.get_bitmap32(memory)?;
                    let rect = Rect {
                        origin,
                        size: src.size(),
                    };
                    let _ = window.draw_in_rect(rect, |bitmap| {
                        bitmap.blt(
                            &ConstBitmap::from(&src),
                            Point::default(),
                            src.size().into(),
                        );
                    });
                    window.set_needs_display();
                }
            }
            svc::Function::BlendRect => {
                let bitmap = params.get_bitmap32(memory)?;
                let origin = params.get_point()?;
                let size = params.get_size()?;
                let color = params.get_u32().map(|v| TrueColor::from_argb(v))?;
                let rect = Rect { origin, size };
                let mut bitmap: Bitmap32 = unsafe { transmute(bitmap) };
                bitmap.blend_rect(rect, color);
            }
            svc::Function::Blt1 => {
                if let Some(window) = params.get_window(self)? {
                    let origin = params.get_point()?;
                    let os_bitmap = params.get_bitmap1(memory)?;
                    let color = params.get_color()?;
                    let mode = params.get_usize()?;
                    let _ = window.draw_in_rect(os_bitmap.rect(origin, mode), |bitmap| {
                        os_bitmap.blt(bitmap, Point::default(), color, mode);
                    });
                    window.set_needs_display();
                }
            }
            svc::Function::RefreshWindow => {
                if let Some(window) = params.get_window(self)? {
                    window.refresh_if_needed();
                }
            }

            svc::Function::Rand => {
                return Ok(WasmValue::from(self.rng32.next()));
            }
            svc::Function::Srand => {
                let seed = params.get_u32()?;
                NonZeroU32::new(seed).map(|v| self.rng32 = XorShift32::new(v));
            }

            svc::Function::Alloc | svc::Function::Free => {
                // TODO:
            }

            svc::Function::Test => {
                let val = params.get_u32()?;
                println!("val: {}", val);
            }
        }

        Ok(WasmValue::I32(0))
    }

    fn wait_key(&mut self, window: WindowHandle) -> Option<char> {
        while let Some(message) = window.wait_message() {
            self.process_message(window, message);

            if let Some(c) = self
                .read_key_buffer()
                .and_then(|v| v.key_data().map(|v| v.into_char()))
            {
                return Some(c);
            }
        }
        None
    }

    fn read_key(&mut self, window: WindowHandle) -> Option<char> {
        while let Some(message) = window.read_message() {
            self.process_message(window, message);
        }
        self.read_key_buffer()
            .and_then(|v| v.key_data().map(|v| v.into_char()))
    }

    fn read_key_buffer(&mut self) -> Option<KeyEvent> {
        if self.key_buffer.len() > 0 {
            return Some(self.key_buffer.remove(0));
        }
        None
    }

    fn process_message(&mut self, window: WindowHandle, message: WindowMessage) {
        match message {
            WindowMessage::Key(event) => {
                self.key_buffer.push(event);
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
        for window in self.windows.values() {
            window.close();
        }
    }
}

struct ParamsDecoder<'a> {
    params: &'a [WasmValue],
    index: usize,
}

impl<'a> ParamsDecoder<'a> {
    pub const fn new(params: &'a [WasmValue]) -> Self {
        Self { params, index: 0 }
    }
}

impl ParamsDecoder<'_> {
    fn get_u32(&mut self) -> Result<u32, WasmRuntimeError> {
        let index = self.index;
        self.params
            .get(index)
            .ok_or(WasmRuntimeError::InvalidParameter)
            .and_then(|v| v.get_u32())
            .map(|v| {
                self.index += 1;
                v
            })
    }

    fn get_i32(&mut self) -> Result<i32, WasmRuntimeError> {
        let index = self.index;
        self.params
            .get(index)
            .ok_or(WasmRuntimeError::InvalidParameter)
            .and_then(|v| v.get_i32())
            .map(|v| {
                self.index += 1;
                v
            })
    }

    fn get_usize(&mut self) -> Result<usize, WasmRuntimeError> {
        self.get_u32().map(|v| v as usize)
    }

    fn get_memarg(&mut self) -> Result<MemArg, WasmRuntimeError> {
        let base = self.get_u32()? as usize;
        let len = self.get_u32()? as usize;
        Ok(MemArg::new(base, len))
    }

    fn get_string<'a>(&mut self, memory: &'a WasmMemory) -> Option<&'a str> {
        self.get_memarg()
            .ok()
            .and_then(|memarg| memory.read_bytes(memarg.base(), memarg.len()).ok())
            .and_then(|v| core::str::from_utf8(v).ok())
    }

    #[allow(dead_code)]
    fn get_string16(&mut self, memory: &WasmMemory) -> Option<String> {
        self.get_memarg()
            .ok()
            .and_then(|memarg| memory.read_bytes(memarg.base(), memarg.len() * 2).ok())
            .and_then(|v| unsafe { core::mem::transmute(v) })
            .and_then(|p| String::from_utf16(p).ok())
    }

    fn get_point(&mut self) -> Result<Point, WasmRuntimeError> {
        let x = self.get_i32()? as isize;
        let y = self.get_i32()? as isize;
        Ok(Point::new(x, y))
    }

    fn get_size(&mut self) -> Result<Size, WasmRuntimeError> {
        let width = self.get_i32()? as isize;
        let height = self.get_i32()? as isize;
        Ok(Size::new(width, height))
    }

    fn get_color(&mut self) -> Result<AmbiguousColor, WasmRuntimeError> {
        self.get_u32().map(|v| IndexedColor::from(v as u8).into())
    }

    fn get_bitmap8<'a>(
        &mut self,
        memory: &'a WasmMemory,
    ) -> Result<ConstBitmap8<'a>, WasmRuntimeError> {
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
    ) -> Result<ConstBitmap32<'a>, WasmRuntimeError> {
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
    ) -> Result<OsBitmap1<'a>, WasmRuntimeError> {
        let base = self.get_u32()?;
        OsBitmap1::from_memory(memory, base)
    }

    fn get_window(&mut self, rt: &ArleRuntime) -> Result<Option<WindowHandle>, WasmRuntimeError> {
        self.get_u32()
            .map(|v| rt.windows.get(&(v as usize)).map(|v| *v))
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
    #[inline]
    const fn rect(&self, origin: Point, mode: usize) -> Rect {
        let scale = mode as isize;
        Rect {
            origin,
            size: Size::new(self.dim.width * scale, self.dim.height * scale),
        }
    }

    fn blt(&self, to: &mut Bitmap, origin: Point, color: AmbiguousColor, mode: usize) {
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
