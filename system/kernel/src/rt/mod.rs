//! Runtime Environment and Personalities

pub mod haribote;
pub mod megos;

use crate::arch::cpu::*;
use crate::task::scheduler::*;
use alloc::{boxed::Box, string::String, string::*, vec::Vec};

static mut RE: RuntimeEnvironment = RuntimeEnvironment::new();

pub struct RuntimeEnvironment {
    exts: Vec<String>,
    image_loaders: Vec<Box<dyn BinaryRecognizer>>,
}

impl RuntimeEnvironment {
    #[inline]
    const fn new() -> Self {
        Self {
            exts: Vec::new(),
            image_loaders: Vec::new(),
        }
    }

    #[inline]
    pub(crate) unsafe fn init() {
        let shared = Self::shared();
        shared.add_image("wasm", megos::WasmRecognizer::new());
        shared.add_image("hrb", haribote::HrbRecognizer::new());
    }

    #[inline]
    fn add_image(&mut self, ext: &str, loader: Box<dyn BinaryRecognizer>) {
        self.exts.push(ext.to_string());
        self.image_loaders.push(loader);
    }

    #[inline]
    fn shared() -> &'static mut Self {
        unsafe { &mut RE }
    }

    #[inline]
    pub fn supported_extensions<'a>() -> &'a [String] {
        let shared = Self::shared();
        shared.exts.as_slice()
    }

    #[inline]
    pub fn recognize(blob: &[u8]) -> Option<Box<dyn BinaryLoader>> {
        let shared = Self::shared();
        for recognizer in &shared.image_loaders {
            if let Some(loader) = recognizer.recognize(blob) {
                return Some(loader);
            }
        }
        None
    }

    #[inline]
    pub fn exit(_exit_code: usize) -> ! {
        Scheduler::exit();
    }

    #[inline]
    pub unsafe fn invoke_legacy(context: &LegacyAppContext) -> ! {
        Cpu::invoke_legacy(context);
    }
}

pub trait Personality {
    /// Gets the current personality context
    fn context(&mut self) -> PersonalityContext;
    /// Called to clean up resources before the process ends.
    fn on_exit(&mut self);
}

#[non_exhaustive]
/// Contains a reference to the context of the current personality
pub enum PersonalityContext<'a> {
    /// Kernel native process
    Native,
    /// MEG-OS Arlequin subsystem
    Arlequin(&'a mut megos::ArleRuntime),
    /// Haribote OS Emulation subsystem
    Hoe(&'a mut haribote::Hoe),
}

pub trait BinaryRecognizer {
    /// Recognizes the binary format and returns the corresponding binary loader.
    fn recognize(&self, blob: &[u8]) -> Option<Box<dyn BinaryLoader>>;
}

pub trait BinaryLoader {
    fn option(&mut self) -> &mut LoadedImageOption;

    fn load(&mut self, blob: &[u8]) -> Result<(), ()>;

    fn invoke_start(self: Box<Self>) -> Option<ProcessId>;
}

#[derive(Debug, Default)]
pub struct LoadedImageOption {
    pub name: String,
    pub argv: Vec<String>,
}

/// Contextual data for legacy applications
#[derive(Debug, Default, Copy, Clone)]
pub struct LegacyAppContext {
    /// Base address of the application image
    pub image_base: u32,
    /// Size of the application image
    pub image_size: u32,
    /// Base address of the code segment
    pub base_of_code: u32,
    /// Size of the code segment
    pub size_of_code: u32,
    /// Base address of the data segment
    pub base_of_data: u32,
    /// Size of the data segment
    pub size_of_data: u32,
    /// Application entry point
    pub start: u32,
    /// Initial stack pointer
    pub stack_pointer: u32,
}
