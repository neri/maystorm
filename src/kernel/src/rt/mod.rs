// Runtime Environment

pub mod hoe;

use crate::arch::cpu::*;
use crate::system::*;
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

        shared.image_loaders.push(hoe::HrbRecognizer::new());
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
    Hoe(&'a mut hoe::Hoe),
}

pub trait BinaryRecognizer {
    fn recognize(&self, blob: &[u8]) -> Option<Box<dyn BinaryLoader>>;
}

pub trait BinaryLoader {
    fn option(&mut self) -> &mut LoadedImageOption;

    fn load(&mut self, blob: &[u8]);

    fn invoke_start(&mut self, name: &str) -> Option<ThreadHandle>;
}

#[derive(Debug, Default)]
pub struct LoadedImageOption {
    pub argv: Vec<String>,
    pub image_base: VirtualAddress,
    pub image_size: usize,
}

#[derive(Debug, Default, Copy, Clone)]
pub struct LegacyAppContext {
    pub base_of_image: u32,
    pub size_of_image: u32,
    pub base_of_code: u32,
    pub size_of_code: u32,
    pub base_of_data: u32,
    pub size_of_data: u32,
    pub start: u32,
    pub stack_pointer: u32,
}
