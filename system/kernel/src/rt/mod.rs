//! Runtime Environment and Personalities

pub mod haribote;
pub mod megos;

use core::{cell::UnsafeCell, ffi::c_void};

use crate::arch::cpu::*;
use crate::task::scheduler::*;
use alloc::{boxed::Box, string::String, string::*, vec::Vec};
use megstd::uuid::{Identify, Uuid};

static mut RE: UnsafeCell<RuntimeEnvironment> = UnsafeCell::new(RuntimeEnvironment::new());

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
    pub unsafe fn init() {
        let shared = &mut *RE.get();
        shared.add_image("wasm", megos::WasmRecognizer::new());
        shared.add_image("hrb", haribote::HrbRecognizer::new());
    }

    #[inline]
    fn add_image(&mut self, ext: &str, loader: Box<dyn BinaryRecognizer>) {
        self.exts.push(ext.to_string());
        self.image_loaders.push(loader);
    }

    #[inline]
    fn shared<'a>() -> &'a Self {
        unsafe { &*RE.get() }
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

/// Contains a reference to the context of the current personality
pub struct PersonalityContext {
    uuid: Uuid,
    payload: Box<dyn Personality>,
}

pub trait Personality {
    /// Returns its own context
    fn context(&mut self) -> *mut c_void;

    /// Called to clean up resources before the process ends.
    fn on_exit(self: Box<Self>);
}

impl PersonalityContext {
    #[inline]
    pub fn new<T: Personality + Identify + 'static>(payload: T) -> Self {
        Self {
            uuid: T::UUID,
            payload: Box::new(payload),
        }
    }

    #[inline]
    pub fn get<'a, T: Identify>(&'a mut self) -> Result<&'a mut T, Uuid> {
        (T::UUID == self.uuid())
            .then(|| unsafe { &mut *(self.payload.context() as *mut T) })
            .ok_or(self.uuid())
    }

    #[inline]
    pub const fn uuid(&self) -> Uuid {
        self.uuid
    }

    #[inline]
    pub fn on_exit(self) {
        self.payload.on_exit();
    }
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
