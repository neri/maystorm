// WebAssembly Subsystem
pub mod opcode;
pub mod wasm;
pub mod wasmintr;

use super::*;
use crate::*;
use alloc::boxed::Box;
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
                "fd_write" => Ok(Box::new(FdWrite::new()) as Box<dyn WasmInvocation>),
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

struct FdWrite {}

impl FdWrite {
    const fn new() -> Self {
        Self {}
    }
}

impl WasmInvocation for FdWrite {
    fn invoke(
        &self,
        module: &WasmModule,
        params: &[WasmValue],
    ) -> Result<WasmValue, WasmRuntimeError> {
        // fd_write (i32 i32 i32 i32) -> i32

        let memory = module.memory(0).unwrap();

        let iovs = params
            .get(1)
            .ok_or(WasmRuntimeError::InvalidParameter)
            .and_then(|v| v.get_u32())? as usize;
        // let iovs_len = params
        //     .get(2)
        //     .ok_or(WasmRuntimeError::InvalidParameter)
        //     .and_then(|v| v.get_i32())?;

        let iov_base = memory.read_u32(iovs)? as usize;
        let iov_len = memory.read_u32(iovs + 4)? as usize;

        let slice = memory.read_bytes(iov_base, iov_len)?;
        let s = core::str::from_utf8(slice).map_err(|_| WasmRuntimeError::InvalidParameter)?;
        print!("{}", s);

        Ok(WasmValue::I32(s.len() as i32))
    }
}
