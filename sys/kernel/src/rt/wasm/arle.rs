// Arlequin Subsystem

use super::*;
// use crate::io::fonts::*;
use crate::uuid::Uuid;
use alloc::collections::BTreeMap;
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
                    "syscall0" => Ok(ArleRuntime::syscall),
                    "syscall1" => Ok(ArleRuntime::syscall),
                    "syscall2" => Ok(ArleRuntime::syscall),
                    "syscall3" => Ok(ArleRuntime::syscall),
                    "syscall4" => Ok(ArleRuntime::syscall),
                    "syscall5" => Ok(ArleRuntime::syscall),
                    "syscall6" => Ok(ArleRuntime::syscall),
                    _ => Err(WasmDecodeError::DynamicLinkError),
                },
                _ => Err(WasmDecodeError::DynamicLinkError),
            })
            .map_err(|_| ())
    }

    fn invoke_start(self: Box<Self>) -> Option<ThreadHandle> {
        match self.loader.module().func(ArleRuntime::ENTRY_FUNC_NAME) {
            Ok(_) => {
                let module = self.loader.consume();
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
