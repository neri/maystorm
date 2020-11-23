// WebAssembly Subsystem
pub mod opcode;
pub mod wasm;

use super::*;
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

impl BinaryLoader for WasmBinaryLoader {
    fn option(&mut self) -> &mut LoadedImageOption {
        &mut self.lio
    }

    fn load(&mut self, blob: &[u8]) -> Result<(), ()> {
        self.loader.load(blob).map_err(|_| ())
    }

    fn invoke_start(&mut self) -> Option<ThreadHandle> {
        self.loader.print_stat();
        // SpawnOption::new().spawn(Self::start, 0, self.lio.name.as_ref())
        None
    }
}
