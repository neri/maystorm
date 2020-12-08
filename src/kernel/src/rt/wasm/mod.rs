// WebAssembly Subsystem
pub mod opcode;
pub mod wasm;
pub mod wasmintr;

use super::*;
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
    // fn start(_: usize) {}
}

impl BinaryLoader for WasmBinaryLoader {
    fn option(&mut self) -> &mut LoadedImageOption {
        &mut self.lio
    }

    fn load(&mut self, blob: &[u8]) -> Result<(), ()> {
        self.loader
            .load(blob, &|_mod_name, name, _type_ref| match name {
                "syscall0" => Ok(WasmRuntime::wasm_syscall),
                "syscall1" => Ok(WasmRuntime::wasm_syscall),
                "syscall2" => Ok(WasmRuntime::wasm_syscall),
                "syscall3" => Ok(WasmRuntime::wasm_syscall),
                "syscall4" => Ok(WasmRuntime::wasm_syscall),
                _ => Err(WasmDecodeError::DynamicLinkError),
            })
            .map_err(|_| ())
    }

    fn invoke_start(&mut self) -> Option<ThreadHandle> {
        match self
            .loader
            .module()
            .func("_start")
            .and_then(|v| v.invoke(&[]))
        {
            Ok(result) => {
                println!("result: {}", result);
            }
            Err(err) => {
                println!("error: {:?}", err);
            }
        }

        // SpawnOption::new().spawn(Self::start, 0, self.lio.name.as_ref())
        None
    }
}

struct WasmRuntime {}

impl WasmRuntime {
    /// Syscall (temp)
    fn wasm_syscall(
        module: &WasmModule,
        params: &[WasmValue],
    ) -> Result<WasmValue, WasmRuntimeError> {
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

    fn get_memarg(params: &[WasmValue], index: usize) -> Result<MemArg, WasmRuntimeError> {
        let base = Self::get_u32(&params, index)? as usize;
        let len = Self::get_u32(&params, index + 1)? as usize;
        Ok(MemArg::new(base, len))
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
