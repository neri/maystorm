// WebAssembly
mod arle;
pub use arle::*;

use super::*;
use crate::window::*;
use crate::*;
use alloc::boxed::Box;
use alloc::string::String;
use arle::ArleBinaryLoader;
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
            Some(Box::new(ArleBinaryLoader::new()) as Box<dyn BinaryLoader>)
        } else {
            None
        }
    }
}
