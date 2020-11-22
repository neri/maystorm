// WebAssembly Subsystem
mod wasm;

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
        WasmBinaryLoader::identity(blob).map(|v| Box::new(v) as Box<dyn BinaryLoader>)
    }
}
