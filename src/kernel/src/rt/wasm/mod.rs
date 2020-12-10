// WebAssembly
pub mod opcode;
pub mod wasm;
pub mod wasmintr;

use super::*;
use crate::window::*;
use crate::*;
use alloc::boxed::Box;
use alloc::string::String;
use wasm::*;

pub(super) struct WasmRecognizer {
    _phantom: (),
}

impl WasmRecognizer {
    pub fn new() -> Box<Self> {
        Box::new(Self { _phantom: () })
    }
}

impl BinaryRecognizer for WasmRecognizer {
    fn recognize(&self, blob: &[u8]) -> Option<Box<dyn BinaryLoader>> {
        if WasmLoader::identity(blob) {
            Some(Box::new(WasmBinaryLoader {
                loader: WasmLoader::new(),
                lio: LoadedImageOption::default(),
            }) as Box<dyn BinaryLoader>)
        } else {
            None
        }
    }
}

struct WasmBinaryLoader {
    loader: WasmLoader,
    lio: LoadedImageOption,
}

impl WasmBinaryLoader {
    fn start(_: usize) {
        MyScheduler::current_personality(|personality| match personality.context() {
            PersonalityContext::Arlequin(rt) => rt.start(),
            _ => unreachable!(),
        });
    }
}

impl BinaryLoader for WasmBinaryLoader {
    fn option(&mut self) -> &mut LoadedImageOption {
        &mut self.lio
    }

    fn load(&mut self, blob: &[u8]) -> Result<(), ()> {
        self.loader
            .load(blob, &|_mod_name, name, _type_ref| match name {
                "syscall0" => Ok(ArleRuntime::syscall),
                "syscall1" => Ok(ArleRuntime::syscall),
                "syscall2" => Ok(ArleRuntime::syscall),
                "syscall3" => Ok(ArleRuntime::syscall),
                "syscall4" => Ok(ArleRuntime::syscall),
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
pub struct ArleRuntime {
    module: WasmModule,
}

impl ArleRuntime {
    const ENTRY_FUNC_NAME: &'static str = "_start";

    fn new(module: WasmModule) -> Box<Self> {
        Box::new(Self { module })
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

    /// Syscall (temp)
    fn syscall(_module: &WasmModule, params: &[WasmValue]) -> Result<WasmValue, WasmRuntimeError> {
        MyScheduler::current_personality(|personality| {
            match personality.context() {
                PersonalityContext::Arlequin(rt) => {
                    let module = &rt.module;
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
                            return Ok(WasmValue::I32(window.as_usize() as i32));
                        }
                        _ => return Err(WasmRuntimeError::InvalidParameter),
                    }

                    Ok(WasmValue::I32(0))
                }
                _ => unreachable!(),
            }
        })
        .unwrap()
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

    fn get_size(params: &[WasmValue], index: usize) -> Result<Size<isize>, WasmRuntimeError> {
        let width = Self::get_i32(&params, index)? as isize;
        let height = Self::get_i32(&params, index + 1)? as isize;
        Ok(Size::new(width, height))
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
}

impl Personality for ArleRuntime {
    fn context(&mut self) -> PersonalityContext {
        PersonalityContext::Arlequin(self)
    }

    fn on_exit(&mut self) {
        //
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
