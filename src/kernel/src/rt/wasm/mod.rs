// WebAssembly Subsystem
pub mod opcode;
pub mod wart;
pub mod wasm;

use super::*;
use crate::*;
use alloc::boxed::Box;
use wart::*;
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

impl BinaryLoader for WasmBinaryLoader {
    fn option(&mut self) -> &mut LoadedImageOption {
        &mut self.lio
    }

    fn load(&mut self, blob: &[u8]) -> Result<(), ()> {
        self.loader.load(blob).map_err(|_| ())
    }

    fn invoke_start(&mut self) -> Option<ThreadHandle> {
        // self.loader.print_stat();

        let mut rt = WasmRuntimeContext::from_module(self.loader.module());
        match rt.start() {
            Ok(result) => {
                println!("result: {}", result);
            }
            Err(err) => {
                println!("error: {:?}", err);
            }
        }

        // let params = [WasmValue::I32(123), WasmValue::I32(456)];
        // match rt.invoke_function("add", &params) {
        //     Ok(result) => {
        //         println!("result: {}", result);
        //     }
        //     Err(err) => {
        //         println!("error: {:?}", err);
        //     }
        // }

        // SpawnOption::new().spawn(Self::start, 0, self.lio.name.as_ref())
        None
    }
}
