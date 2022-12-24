use super::*;
use crate::{
    fs::{FileManager, FsRawFileControlBlock, OffsetType, Whence},
    sync::Mutex,
    ui::theme::Theme,
    *,
    {io::hid_mgr::*, ui::text::*, ui::window::*},
};
use alloc::{borrow::ToOwned, collections::BTreeMap, sync::Arc};
use byteorder::*;
use core::{
    alloc::Layout, intrinsics::transmute, num::NonZeroU32, sync::atomic::*, time::Duration,
};
use megstd::{
    drawing::*,
    game::v0,
    io::{Read, Write},
    rand::*,
};
use wasm::{intr::*, *};

include!("megg0808.rs");

pub struct MyosBinaryLoader {
    loader: WasmLoader,
    lio: LoadedImageOption,
}

impl MyosBinaryLoader {
    pub fn new() -> Self {
        Self {
            loader: WasmLoader::new(),
            lio: LoadedImageOption::default(),
        }
    }

    fn start(_: usize) {
        Scheduler::current_personality()
            .unwrap()
            .get::<MyosRuntime>()
            .unwrap()
            .start();
    }
}

impl BinaryLoader for MyosBinaryLoader {
    fn option(&mut self) -> &mut LoadedImageOption {
        &mut self.lio
    }

    fn load(&mut self, blob: &[u8]) -> Result<(), ()> {
        self.loader
            .load(blob, |mod_name, name, type_ref| {
                let signature = type_ref.signature();
                match mod_name {
                    MyosRuntime::MOD_NAME => match (name, signature.as_ref()) {
                        ("svc0", "ii") => TriState::Ok(MyosRuntime::syscall),
                        ("svc1", "iii") => TriState::Ok(MyosRuntime::syscall),
                        ("svc2", "iiii") => TriState::Ok(MyosRuntime::syscall),
                        ("svc3", "iiiii") => TriState::Ok(MyosRuntime::syscall),
                        ("svc4", "iiiiii") => TriState::Ok(MyosRuntime::syscall),
                        ("svc5", "iiiiiii") => TriState::Ok(MyosRuntime::syscall),
                        ("svc6", "iiiiiiii") => TriState::Ok(MyosRuntime::syscall),
                        _ => TriState::Err(WasmDecodeErrorKind::NoMethod(name.to_owned())),
                    },
                    _ => TriState::Err(WasmDecodeErrorKind::NoModule(mod_name.to_owned())),
                }
            })
            .map_err(|v| {
                println!("Load error: {:?}", v);
                ()
            })
    }

    fn invoke_start(self: Box<Self>) -> Option<ProcessId> {
        match self.loader.module().func(MyosRuntime::ENTRY_FUNC_NAME) {
            Ok(_) => {
                let module = self.loader.into_module();
                SpawnOption::new()
                    .personality(MyosRuntime::new(module))
                    .start_process(Self::start, 0, self.lio.name.as_ref())
            }
            Err(err) => {
                println!("error: {:?}", err);
                None
            }
        }
    }
}

#[allow(dead_code)]
pub struct MyosRuntime {
    module: WasmModule,
    next_handle: AtomicUsize,
    windows: Mutex<BTreeMap<usize, UnsafeCell<OsWindow>>>,
    files: Mutex<Vec<Option<Arc<Mutex<FsRawFileControlBlock>>>>>,
    rng32: XorShift32,
    key_buffer: Mutex<Vec<KeyEvent>>,
    game_presenter: Option<UnsafeCell<Box<OsGamePresenter>>>,
    malloc: Mutex<SimpleAllocator>,
    has_to_exit: AtomicBool,
}

unsafe impl Identify for MyosRuntime {
    #[rustfmt::skip]
    /// 57392D77-D199-486E-9A2C-47D15BA6DFCA
    const UUID: Uuid = Uuid::from_parts(0x57392D77, 0xD199, 0x486E, 0x9A2C, [0x47, 0xD1, 0x5B, 0xA6, 0xDF, 0xCA]);
}

impl Personality for MyosRuntime {
    fn context(&mut self) -> *mut c_void {
        self as *const _ as *mut c_void
    }

    fn on_exit(self: Box<Self>) {
        self.windows.lock().unwrap().clear();
    }
}

impl MyosRuntime {
    const MAX_FILES: usize = 20;
    const MOD_NAME: &'static str = "megos-canary";
    const ENTRY_FUNC_NAME: &'static str = "_start";

    const SIZE_KEYBUFFER: usize = 32;

    fn new(module: WasmModule) -> PersonalityContext {
        PersonalityContext::new(Self {
            module,
            next_handle: AtomicUsize::new(1),
            windows: Mutex::new(BTreeMap::new()),
            files: Mutex::new(Vec::new()),
            rng32: XorShift32::default(),
            key_buffer: Mutex::new(Vec::with_capacity(Self::SIZE_KEYBUFFER)),
            game_presenter: None,
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

    fn syscall(
        _: &WasmModule,
        params: &[WasmUnsafeValue],
    ) -> Result<WasmValue, WasmRuntimeErrorKind> {
        Scheduler::current_personality()
            .unwrap()
            .get::<Self>()
            .unwrap()
            .dispatch_syscall(&params)
    }

    fn dispatch_syscall(
        &mut self,
        params: &[WasmUnsafeValue],
    ) -> Result<WasmValue, WasmRuntimeErrorKind> {
        use megstd::sys::megos::svc::Function;

        let mut params = ParamsDecoder::new(params);
        let memory = self
            .module
            .memory(0)
            .ok_or(WasmRuntimeErrorKind::OutOfMemory)?;
        let func_no = params
            .get_u32()
            .map(|v| unsafe { transmute::<u32, Function>(v) })?;

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

            Function::Open => {
                let path = params
                    .get_string(memory)
                    .ok_or(WasmRuntimeErrorKind::InvalidParameter)?;
                let _options = params.get_u32()?;
                return Self::encode_io_result(
                    FileManager::open(path).and_then(|file| self.alloc_file(file)),
                );
            }
            Function::Close => {
                let handle = params.get_usize()?;
                self.close_file(handle);
            }
            Function::Read => {
                let file = params.get_file(self)?;
                let buf = params.get_buffer(memory)?;
                return Self::encode_io_result(file.lock().unwrap().read(buf));
            }
            Function::Write => {
                let file = params.get_file(self)?;
                let buf = params.get_buffer(memory)?;
                return Self::encode_io_result(file.lock().unwrap().write(buf));
            }
            Function::LSeek => {
                let file = params.get_file(self)?;
                let offset = params.get_i32()? as OffsetType;
                let whence = Whence::try_from(params.get_usize()?)
                    .map_err(|_| WasmRuntimeErrorKind::InvalidParameter)?;
                return Self::encode_io_result(
                    file.lock()
                        .unwrap()
                        .lseek(offset, whence)
                        .map(|v| v as usize),
                );
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
                let origin = params.get_point()?;
                let text = params.get_string(memory).unwrap_or("");
                let color = params.get_color()?;
                let mut rect = window.content_rect().bounds();
                rect.origin = origin;
                rect.size.width -= origin.x;
                rect.size.height -= origin.y;
                window.draw_in_rect(rect, |bitmap| {
                    AttributedString::new()
                        .align(TextAlignment::Left)
                        .valign(VerticalAlignment::Top)
                        .color(color)
                        .text(text)
                        .draw_text(bitmap, rect.bounds(), 0);
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
                let offset = Movement::from(rect.origin());
                window.draw_in_rect(rect, |bitmap| {
                    bitmap.draw_line(c1 - offset, c2 - offset, color);
                });
            }
            Function::DrawShape => {
                let window = params.get_window(self)?;
                let origin = params.get_point()?;
                let size = params.get_size()?;

                let offset = params.get_usize()?;
                let len = 3;
                let params = memory.read_u32_array(offset, len)?;
                let radius = unsafe { *params.get_unchecked(0) as isize };
                let bg_color = PackedColor(unsafe { *params.get_unchecked(1) }).as_color();
                let border_color = PackedColor(unsafe { *params.get_unchecked(2) }).as_color();

                let rect = Rect { origin, size };
                window.draw_in_rect(rect, |bitmap| {
                    if bg_color != Color::TRANSPARENT {
                        bitmap.fill_round_rect(bitmap.bounds(), radius, bg_color);
                    }
                    if border_color != Color::TRANSPARENT {
                        bitmap.draw_round_rect(bitmap.bounds(), radius, border_color);
                    }
                });
            }
            Function::WaitChar => {
                let window = params.get_window(self)?;
                return self
                    .wait_key(window.native())
                    .map(|c| WasmValue::I32(c.unwrap_or('\0') as i32));
            }
            Function::ReadChar => {
                let window = params.get_window(self)?;
                let c = self.read_key(window.native());
                return Ok(WasmValue::from(
                    c.map(|v| v as u32)
                        .unwrap_or(megstd::sys::megos::OPTION_CHAR_NONE),
                ));
            }

            Function::GameV0Init => {
                let window = params.get_window(self)?;
                let screen = params.get_usize()?;
                let fps = params.get_usize().unwrap_or(0);

                self.game_presenter =
                    match OsGamePresenter::new(memory, window.native(), screen, fps) {
                        Ok(v) => Some(UnsafeCell::new(Box::new(v))),
                        Err(err) => return Err(err),
                    };
                return Ok(WasmValue::from(1i32));
            }
            Function::GameV0Sync => {
                let _handle = params.get_usize()?;
                let presenter = self
                    .game_presenter
                    .as_ref()
                    .map(|v| unsafe { &mut *v.get() })
                    .ok_or(WasmRuntimeErrorKind::InvalidParameter)?;
                self.sense_message(presenter.window);
                return Ok(WasmValue::from(presenter.sync(memory)));
            }
            Function::GameV0Rect => {
                let _handle = params.get_usize()?;
                let origin = params.get_point()?;
                let size = params.get_size()?;
                let rect = Rect { origin, size };
                let presenter = self
                    .game_presenter
                    .as_ref()
                    .map(|v| unsafe { &mut *v.get() })
                    .ok_or(WasmRuntimeErrorKind::InvalidParameter)?;
                presenter.add_region(rect);
            }
            Function::GameV0MoveSprite => {
                let _handle = params.get_usize()?;
                let index = params.get_usize()?;
                let x = params.get_usize()?;
                let y = params.get_usize()?;
                let presenter = self
                    .game_presenter
                    .as_ref()
                    .map(|v| unsafe { &mut *v.get() })
                    .ok_or(WasmRuntimeErrorKind::InvalidParameter)?;
                presenter.move_sprite(memory, index, x, y);
            }
            Function::GameV0Button => {
                let _handle = params.get_usize()?;
                return Ok(WasmValue::from(
                    GameInputManager::current_input().buttons() as u32
                ));
            }
            Function::GameV0LoadFont => {
                let _handle = params.get_usize()?;
                let start_index = params.get_u32()? as u8;
                let start_char = params.get_u32()? as u8;
                let end_char = params.get_u32()? as u8;

                let presenter = self
                    .game_presenter
                    .as_ref()
                    .map(|v| unsafe { &mut *v.get() })
                    .ok_or(WasmRuntimeErrorKind::InvalidParameter)?;

                presenter.load_font(memory, start_index, start_char, end_char);
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

                return self.alloc(memory, layout).map(|v| WasmValue::from(v.get()));
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

            #[allow(unreachable_patterns)]
            _ => return Err(WasmRuntimeErrorKind::NotSupprted),
        }

        Ok(WasmValue::I32(0))
    }

    fn encode_io_result(
        val: Result<usize, megstd::io::Error>,
    ) -> Result<WasmValue, WasmRuntimeErrorKind> {
        match val {
            Ok(v) => Ok((v as u32).into()),
            Err(_err) => {
                // TODO
                Ok((-1).into())
            }
        }
    }

    fn alloc_file(&self, file: FsRawFileControlBlock) -> Result<usize, megstd::io::Error> {
        let mut vec = self.files.lock().unwrap();
        for (handle, entry) in vec.iter_mut().enumerate() {
            if entry.is_none() {
                *entry = Some(Arc::new(Mutex::new(file)));
                return Ok(handle);
            }
        }
        let handle = vec.len();
        if handle >= Self::MAX_FILES {
            return Err(megstd::io::ErrorKind::OutOfMemory.into());
        }
        vec.push(Some(Arc::new(Mutex::new(file))));
        Ok(handle)
    }

    fn close_file(&self, handle: usize) {
        let mut vec = self.files.lock().unwrap();
        if let Some(entry) = vec.get_mut(handle) {
            *entry = None;
        }
    }

    fn alloc(
        &self,
        memory: &WasmMemory,
        layout: Layout,
    ) -> Result<NonZeroU32, WasmRuntimeErrorKind> {
        let mut malloc = self.malloc.lock().unwrap();

        if let Some(result) = malloc.alloc(layout) {
            println!("alloc1 {:?} => {:08x}", layout, result);
            return Ok(result);
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
            NonZeroU32::new(result).ok_or(WasmRuntimeErrorKind::OutOfMemory)
        }
    }

    fn wait_key(&self, window: WindowHandle) -> Result<Option<char>, WasmRuntimeErrorKind> {
        while let Some(message) = window.wait_message() {
            self.process_message(window, message);
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

    fn read_key(&self, window: WindowHandle) -> Option<char> {
        while let Some(message) = window.read_message() {
            self.process_message(window, message);
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

    fn sense_message(&self, window: WindowHandle) {
        while let Some(message) = window.read_message() {
            self.process_message(window, message);
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
                match self
                    .game_presenter
                    .as_ref()
                    .map(|v| unsafe { &mut *v.get() })
                {
                    Some(_presenter) => {
                        GameInputManager::send_key(event);
                    }
                    None => {
                        event
                            .key_data()
                            .map(|data| self.key_buffer.lock().unwrap().push(data));
                    }
                }
            }
            _ => window.handle_default_message(message),
        }
    }
}

struct ParamsDecoder<'a> {
    params: &'a [WasmUnsafeValue],
    index: usize,
}

impl<'a> ParamsDecoder<'a> {
    #[inline]
    pub const fn new(params: &'a [WasmUnsafeValue]) -> Self {
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
            .map(|v| unsafe { v.get_u32() })
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
            .map(|v| unsafe { v.get_i32() })
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
    fn get_buffer<'a>(
        &mut self,
        memory: &'a WasmMemory,
    ) -> Result<&'a mut [u8], WasmRuntimeErrorKind> {
        self.get_memarg()
            .and_then(|memarg| unsafe { memory.slice_mut(memarg.base(), memarg.len()) })
    }

    #[inline]
    fn get_string<'a>(&mut self, memory: &'a WasmMemory) -> Option<&'a str> {
        self.get_memarg()
            .ok()
            .and_then(|memarg| unsafe { memory.slice(memarg.base(), memarg.len()) }.ok())
            .and_then(|v| core::str::from_utf8(v).ok())
    }

    #[allow(dead_code)]
    #[inline]
    fn get_string16(&mut self, memory: &WasmMemory) -> Option<String> {
        self.get_memarg()
            .ok()
            .and_then(|memarg| unsafe { memory.slice(memarg.base(), memarg.len() * 2) }.ok())
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
        let array = unsafe { memory.slice(base as usize, SIZE_OF_BITMAP) }?;

        let width = LE::read_u32(&array[0..4]) as usize;
        let height = LE::read_u32(&array[4..8]) as usize;
        let _stride = LE::read_u32(&array[8..12]) as usize;
        let base = LE::read_u32(&array[12..16]) as usize;

        let len = width * height;
        let slice = unsafe { memory.slice(base, len) }?;

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
        let array = unsafe { memory.slice(base as usize, SIZE_OF_BITMAP) }?;

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

    fn get_window<'a>(
        &mut self,
        rt: &'a MyosRuntime,
    ) -> Result<&'a mut OsWindow, WasmRuntimeErrorKind> {
        let handle = self.get_usize()?;
        rt.windows
            .lock()
            .unwrap()
            .get(&handle)
            .map(|v| unsafe { &mut *v.get() })
            .ok_or(WasmRuntimeErrorKind::InvalidParameter)
    }

    fn get_file(
        &mut self,
        rt: &MyosRuntime,
    ) -> Result<Arc<Mutex<FsRawFileControlBlock>>, WasmRuntimeErrorKind> {
        let handle = self.get_usize()?;
        rt.files
            .lock()
            .unwrap()
            .get(handle)
            .and_then(|v| v.as_ref())
            .map(|v| v.clone())
            .ok_or(WasmRuntimeErrorKind::InvalidParameter)
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
        let array = unsafe { memory.slice(base as usize, SIZE_OF_BITMAP) }?;

        let width = LE::read_u32(&array[0..4]) as usize;
        let height = LE::read_u32(&array[4..8]) as usize;
        let stride = LE::read_u32(&array[8..12]) as usize;
        let base = LE::read_u32(&array[12..16]) as usize;

        let dim = Size::new(width as isize, height as isize);
        let size = stride * height;
        let slice = unsafe { memory.slice(base, size) }?;

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

struct OsGamePresenter {
    window: WindowHandle,
    screen: usize,
    size: Size,
    buffer: OwnedBitmap32,
    draw_region: Coordinates,
    timer_div: u64,
    expected_time: u64,
}

impl OsGamePresenter {
    const SCALE_FACTOR: isize = 2;

    #[inline]
    fn new(
        memory: &WasmMemory,
        window: WindowHandle,
        screen: usize,
        fps: usize,
    ) -> Result<Self, WasmRuntimeErrorKind> {
        let fps = if fps > 0 && fps <= 1000 { fps } else { 60 };
        let timer_div = (1000_000 / fps) as u64;
        let size = window.content_rect().size() / Self::SCALE_FACTOR;
        let buffer = OwnedBitmap32::new(size, TrueColor::TRANSPARENT);

        let result = Self {
            window,
            screen,
            size,
            buffer,
            draw_region: unsafe { Coordinates::from_rect_unchecked(Rect::from(size)) },
            timer_div,
            expected_time: timer_div + Timer::monotonic().as_micros() as u64,
        };

        let screen = unsafe { &mut *result.screen(memory).unwrap().get() };
        screen.control_mut().reset();
        for i in 0..v0::MAX_PALETTES / 4 {
            if i < v0::MAX_PALETTES / 8 {
                screen.set_palette(i * 4, PackedColor::WHITE);
            } else {
                screen.set_palette(i * 4, PackedColor::TRANSPARENT);
            }
            screen.set_palette(i * 4 + 1, PackedColor::LIGHT_GRAY);
            screen.set_palette(i * 4 + 2, PackedColor::DARK_GRAY);
            screen.set_palette(i * 4 + 3, PackedColor::BLACK);
        }
        result.load_font(memory, 0, 0, 0xFF);
        for i in 0..v0::MAX_SPRITES {
            screen.set_sprite(i, 0, 0);
        }

        Ok(result)
    }

    #[inline]
    unsafe fn screen<'a>(
        &self,
        memory: &'a WasmMemory,
    ) -> Result<&'a UnsafeCell<v0::Screen>, WasmRuntimeErrorKind> {
        memory.transmute(self.screen)
    }

    fn load_font(&self, memory: &WasmMemory, start_index: u8, start_char: u8, end_char: u8) {
        let screen = match unsafe { self.screen(memory) } {
            Ok(v) => unsafe { &mut *v.get() },
            Err(_) => return,
        };
        let adjust = start_index - start_char;
        let char_min = 0x20;
        let char_max = 0x80;
        let start_char = u8::max(char_min, start_char);
        let end_char = u8::min(char_max, end_char);
        let stride = v0::TILE_SIZE as usize;
        for index in start_char..=end_char {
            let base = (index - char_min) as usize * stride;
            let data8 = unsafe { FONT_MEGG0808_DATA.get_unchecked(base..base + stride) };
            let mut data = [0u8; v0::TILE_DATA_LEN];
            for (index, byte) in data8.iter().enumerate() {
                data[index] = *byte;
                data[index + stride] = *byte;
            }
            screen.set_tile_data(index + adjust, &data);
        }
    }

    #[inline]
    fn sync(&mut self, memory: &WasmMemory) -> u32 {
        self.redraw(memory);
        let actual = Timer::monotonic().as_micros() as u64;
        if actual < self.expected_time {
            let diff = self.expected_time - actual;
            self.expected_time += self.timer_div;
            Timer::sleep(Duration::from_micros(diff));
            0
        } else {
            let diff = actual - self.expected_time;
            let lost_frames = 1 + diff / self.timer_div;
            self.expected_time += lost_frames * self.timer_div;
            Timer::sleep(Duration::from_micros(1));
            lost_frames as u32
        }
    }

    fn add_region(&mut self, rect: Rect) {
        let coords = match Coordinates::from_rect(rect) {
            Ok(v) => v,
            Err(_) => return,
        };
        if coords.right < 0
            || coords.bottom < 0
            || coords.left >= self.size.width()
            || coords.top >= self.size.height()
        {
            return;
        }
        self.draw_region.merge(coords.trimmed(self.size.into()));
    }

    fn move_sprite(&mut self, memory: &WasmMemory, index: usize, x: usize, y: usize) {
        let screen = match unsafe { self.screen(memory) } {
            Ok(v) => unsafe { &mut *v.get() },
            Err(_) => return,
        };
        let oam = screen.get_sprite(index);
        let old_rect = oam.frame();

        let oam = screen.get_sprite_mut(index);
        oam.x = x as u8;
        oam.y = y as u8;
        let new_rect = oam.frame();
        drop(oam);

        self.add_region(old_rect);
        self.add_region(new_rect);
    }

    fn redraw(&mut self, memory: &WasmMemory) {
        let coords = self.draw_region;
        let coords = Coordinates::new(
            isize::max(0, coords.left),
            isize::max(0, coords.top),
            isize::min(self.size.width, coords.right),
            isize::min(self.size.height, coords.bottom),
        );
        if coords.left <= coords.right && coords.top <= coords.bottom {
            let rect = Rect::from(coords);
            self.redraw_rect(memory, rect);
            self.window.invalidate_rect(rect * Self::SCALE_FACTOR);
        }
        self.draw_region = Coordinates::void();
    }

    fn redraw_rect(&mut self, memory: &WasmMemory, rect: Rect) {
        let screen = match unsafe { self.screen(memory) } {
            Ok(v) => unsafe { &*v.get() },
            Err(_) => return,
        };
        let limit_x = self.size.width() / v0::TILE_SIZE;
        let limit_y = self.size.height() / v0::TILE_SIZE;
        let min_x = isize::max(0, rect.min_x() / v0::TILE_SIZE);
        let max_x = isize::min(limit_x, (rect.max_x() + 7) / v0::TILE_SIZE);
        let min_y = isize::max(0, rect.min_y() / v0::TILE_SIZE);
        let max_y = isize::min(limit_y, (rect.max_y() + 7) / v0::TILE_SIZE);
        if max_x < 0 || min_y < 0 || min_x >= limit_x || min_y >= limit_y {
            return;
        }

        let bitmap = self.buffer.as_mut();
        // bitmap.fill_rect(rect, screen.get_palette(0).into_true_color());

        // BG1
        {
            let scroll = screen.control().get_scroll();
            let sx = scroll.x;
            let sy = scroll.y;
            let sx7 = sx & 7;
            let sy7 = sy & 7;
            let sx8 = ((sx as u8) >> 3) as isize;
            let sy8 = ((sy as u8) >> 3) as isize;
            let min_x = if sx7 != 0 { min_x - 1 } else { min_x };
            let min_y = if sy7 != 0 { min_y - 1 } else { min_y };
            let hmask = v0::MAX_VWIDTH as isize / v0::TILE_SIZE - 1;
            let vmask = v0::MAX_VHEIGHT as isize / v0::TILE_SIZE - 1;

            for y in min_y..max_y {
                for x in min_x..max_x {
                    let name = screen.get_name((x - sx8) & hmask, (y - sy8) & vmask);
                    let pattern = screen.get_tile_data(name.index());
                    let origin = Point::new(x * v0::TILE_SIZE + sx7, y * v0::TILE_SIZE + sy7);
                    Self::render_tile(bitmap, name.attr(), pattern, origin, screen.palettes());
                }
            }
        }

        let rect_min_x = rect.min_x();
        let rect_min_y = rect.min_y();
        let rect_max_x = rect.max_x();
        let rect_max_y = rect.max_y();
        let sprite_min = screen.control().sprite_min as usize;
        let sprite_max = screen.control().sprite_max as usize;
        for oam in unsafe {
            screen
                .sprites()
                .get_unchecked(sprite_min..=sprite_max)
                .iter()
                .rev()
        } {
            if oam.is_gone() {
                continue;
            }
            let oam_attr = oam.attr;
            let oam_rect = oam.frame();

            let oam_min_x = oam_rect.min_x();
            let oam_min_y = oam_rect.min_y();
            let oam_max_x = oam_rect.max_x();
            let oam_max_y = oam_rect.max_y();

            if (oam_min_x >= rect_min_x && oam_min_x < rect_max_x
                || oam_max_x >= rect_min_x && oam_max_x < rect_max_x)
                && (oam_min_y >= rect_min_y && oam_min_y < rect_max_y
                    || oam_max_y >= rect_min_y && oam_max_y < rect_max_y)
            {
                let base_x = if oam_attr & (v0::OAM_ATTR_W16 | v0::OAM_ATTR_HFLIP)
                    == (v0::OAM_ATTR_W16 | v0::OAM_ATTR_HFLIP)
                {
                    v0::TILE_SIZE
                } else {
                    0
                };
                let base_y = if oam_attr & (v0::OAM_ATTR_H16 | v0::OAM_ATTR_VFLIP)
                    == (v0::OAM_ATTR_H16 | v0::OAM_ATTR_VFLIP)
                {
                    v0::TILE_SIZE
                } else {
                    0
                };

                let pattern = screen.get_tile_data(oam.index());
                Self::render_sprite(
                    bitmap,
                    oam_attr,
                    pattern,
                    oam_rect.origin() + Movement::new(base_x, base_y),
                    screen.palettes(),
                );
                if (oam_attr & v0::OAM_ATTR_W16) != 0 {
                    let pattern = screen.get_tile_data(oam.index() | 0x01);
                    Self::render_sprite(
                        bitmap,
                        oam_attr,
                        pattern,
                        oam_rect.origin() + Movement::new(base_x ^ v0::TILE_SIZE, base_y),
                        screen.palettes(),
                    );
                }
                if (oam_attr & v0::OAM_ATTR_H16) != 0 {
                    let pattern = screen.get_tile_data(oam.index() | 0x10);
                    Self::render_sprite(
                        bitmap,
                        oam_attr,
                        pattern,
                        oam_rect.origin() + Movement::new(base_x, base_y ^ v0::TILE_SIZE),
                        screen.palettes(),
                    );
                    if oam_attr & (v0::OAM_ATTR_W16) != 0 {
                        let pattern = screen.get_tile_data(oam.index() | 0x11);
                        Self::render_sprite(
                            bitmap,
                            oam_attr,
                            pattern,
                            oam_rect.origin()
                                + Movement::new(base_x ^ v0::TILE_SIZE, base_y ^ v0::TILE_SIZE),
                            screen.palettes(),
                        );
                    }
                }
            }
        }

        // BG2
        if false {
            //
        }

        let rect = rect * Self::SCALE_FACTOR;
        let _ = self.window.draw_in_rect(rect, |bitmap| match bitmap {
            Bitmap::Indexed(_) => todo!(),
            Bitmap::Argb32(bitmap) => {
                let origin = Point::new(rect.min_x() / 2, rect.min_y() / 2);
                for y in 0..bitmap.height() / 2 {
                    for x in 0..bitmap.width() / 2 {
                        unsafe {
                            let sp = origin + Movement::new(x as isize, y as isize);
                            let dp = Point::new(x as isize * 2, y as isize * 2);
                            let pixel = self.buffer.get_pixel_unchecked(sp);
                            bitmap.set_pixel_unchecked(dp, pixel);
                            bitmap.set_pixel_unchecked(dp + Movement::new(1, 0), pixel);
                            bitmap.set_pixel_unchecked(dp + Movement::new(0, 1), pixel);
                            bitmap.set_pixel_unchecked(dp + Movement::new(1, 1), pixel);
                        }
                    }
                }
            }
        });
    }

    #[inline]
    fn render_tile(
        bitmap: &mut Bitmap32,
        attr: u8,
        pattern: &v0::TileData,
        origin: Point,
        palette: &[v0::PaletteEntry],
    ) {
        Self::render_tile_raw(bitmap, attr & v0::TILE_ATTR_MASK, pattern, origin, palette);
    }

    #[inline]
    fn render_sprite(
        bitmap: &mut Bitmap32,
        attr: u8,
        pattern: &v0::TileData,
        origin: Point,
        palette: &[v0::PaletteEntry],
    ) {
        Self::render_tile_raw(
            bitmap,
            (attr & v0::OAM_DRAW_ATTR_MASK) | v0::OAM_PALETTE_BASE,
            pattern,
            origin,
            palette,
        );
    }

    fn render_tile_raw(
        bitmap: &mut Bitmap32,
        attr: u8,
        pattern: &v0::TileData,
        origin: Point,
        palette: &[v0::PaletteEntry],
    ) {
        let mut dx = origin.x();
        let mut dy = origin.y();
        let mut bx = 0;
        let mut by = 0;
        let mut dw = v0::TILE_SIZE;
        let mut dh = v0::TILE_SIZE;
        if dx + dw > bitmap.width() as isize {
            dw = bitmap.width() as isize - dx;
        }
        if dy + dh > bitmap.height() as isize {
            dh = bitmap.height() as isize - dy;
        }
        if dx < 0 {
            bx -= dx;
            dx = 0;
        }
        if dy < 0 {
            by -= dy;
            dy = 0;
        }
        if dw <= 0 || dh <= 0 {
            return;
        }

        let palette_base = ((attr & v0::TILE_ATTR_PAL_MASK) / v0::PALETTE_1) as usize * 4;
        let color0 = unsafe { palette.get_unchecked(palette_base + 0) }.into_true_color();
        let color1 = unsafe { palette.get_unchecked(palette_base + 1) }.into_true_color();
        let color2 = unsafe { palette.get_unchecked(palette_base + 2) }.into_true_color();
        let color3 = unsafe { palette.get_unchecked(palette_base + 3) }.into_true_color();
        let color0_transparent = color0.is_transparent();

        let delta1 = 1;
        let delta2 = 8;
        let bit_pattern = if (attr & v0::TILE_ATTR_HFLIP) == 0 {
            0x0102040810204080u64
        } else {
            0x8040201008040201u64
        };

        if (attr & v0::TILE_ATTR_VFLIP) == 0 {
            let dy = dy - by;
            let dx = dx - bx;
            for y in by as usize..dh as usize {
                let p1 = unsafe { *pattern.get_unchecked(y * delta1) };
                let p2 = unsafe { *pattern.get_unchecked(y * delta1 + delta2) };
                let p3 = p1 & p2;
                let oy = dy + y as isize;
                for x in bx as usize..dw as usize {
                    let point = Point::new(dx + x as isize, oy);
                    let bits = (bit_pattern >> (x * 8)) as u8;
                    if (p3 & bits) != 0 {
                        unsafe {
                            bitmap.set_pixel_unchecked(point, color3);
                        }
                    } else if (p2 & bits) != 0 {
                        unsafe {
                            bitmap.set_pixel_unchecked(point, color2);
                        }
                    } else if (p1 & bits) != 0 {
                        unsafe {
                            bitmap.set_pixel_unchecked(point, color1);
                        }
                    } else if !color0_transparent {
                        unsafe {
                            bitmap.set_pixel_unchecked(point, color0);
                        }
                    }
                }
            }
        } else {
            let eh = 7 - by;
            let ch = dh - by;
            let dx = dx - bx;
            for y in 0..ch as usize {
                let p1 = unsafe { *pattern.get_unchecked((eh as usize - y) * delta1) };
                let p2 = unsafe { *pattern.get_unchecked((eh as usize - y) * delta1 + delta2) };
                let p3 = p1 & p2;
                let oy = dy + y as isize;
                for x in bx as usize..dw as usize {
                    let point = Point::new(dx + x as isize, oy);
                    let bits = (bit_pattern >> (x * 8)) as u8;
                    if (p3 & bits) != 0 {
                        unsafe {
                            bitmap.set_pixel_unchecked(point, color3);
                        }
                    } else if (p2 & bits) != 0 {
                        unsafe {
                            bitmap.set_pixel_unchecked(point, color2);
                        }
                    } else if (p1 & bits) != 0 {
                        unsafe {
                            bitmap.set_pixel_unchecked(point, color1);
                        }
                    } else if !color0_transparent {
                        unsafe {
                            bitmap.set_pixel_unchecked(point, color0);
                        }
                    }
                }
            }
        }
    }
}
