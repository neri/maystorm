// Runtime Environment

pub mod haribote;
pub mod wasm;

use crate::arch::cpu::*;
use crate::task::scheduler::*;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::*;

static mut RE: RuntimeEnvironment = RuntimeEnvironment::new();

#[derive(Debug, Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct ProcessId(pub usize);

pub struct RuntimeEnvironment {
    image_loaders: Vec<Box<dyn BinaryRecognizer>>,
}

impl RuntimeEnvironment {
    const fn new() -> Self {
        Self {
            image_loaders: Vec::new(),
        }
    }

    pub(crate) unsafe fn init() {
        let shared = Self::shared();

        shared.image_loaders.push(wasm::WasmRecognizer::new());
        shared.image_loaders.push(haribote::HrbRecognizer::new());
    }

    fn shared() -> &'static mut Self {
        unsafe { &mut RE }
    }

    pub(crate) fn raise_pid() -> ProcessId {
        static NEXT_PID: AtomicUsize = AtomicUsize::new(1);
        let pid = ProcessId(NEXT_PID.fetch_add(1, Ordering::SeqCst));

        // TODO:

        pid
    }

    pub fn recognize(blob: &[u8]) -> Option<Box<dyn BinaryLoader>> {
        let shared = Self::shared();
        for recognizer in &shared.image_loaders {
            if let Some(loader) = recognizer.recognize(blob) {
                return Some(loader);
            }
        }
        None
    }

    pub fn exit(exit_code: usize) -> ! {
        let _ = exit_code;
        MyScheduler::exit();
    }

    pub unsafe fn invoke_legacy(context: &LegacyAppContext) -> ! {
        Cpu::invoke_legacy(context);
    }
}

pub trait Personality {
    fn context(&mut self) -> PersonalityContext;

    fn on_exit(&mut self);
}

pub enum PersonalityContext<'a> {
    Native,
    Arlequin(&'a mut wasm::ArleRuntime),
    Hoe(&'a mut haribote::hoe::Hoe),
}

pub trait BinaryRecognizer {
    fn recognize(&self, blob: &[u8]) -> Option<Box<dyn BinaryLoader>>;
}

pub trait BinaryLoader {
    fn option(&mut self) -> &mut LoadedImageOption;

    fn load(&mut self, blob: &[u8]) -> Result<(), ()>;

    fn invoke_start(self: Box<Self>) -> Option<ThreadHandle>;
}

#[derive(Debug, Default)]
pub struct LoadedImageOption {
    pub name: String,
    pub argv: Vec<String>,
}

#[derive(Debug, Default, Copy, Clone)]
pub struct LegacyAppContext {
    pub image_base: u32,
    pub image_size: u32,
    pub base_of_code: u32,
    pub size_of_code: u32,
    pub base_of_data: u32,
    pub size_of_data: u32,
    pub start: u32,
    pub stack_pointer: u32,
}
