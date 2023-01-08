//! Arlequin Subsystem

use super::*;
use megstd::*;

/// Recognize .bin file
pub struct ArleRecognizer {
    _phantom: (),
}

impl ArleRecognizer {
    pub fn new() -> Box<Self> {
        Box::new(Self { _phantom: () })
    }
}

impl BinaryRecognizer for ArleRecognizer {
    fn recognize(&self, blob: &[u8]) -> Option<Box<dyn BinaryLoader>> {
        ArleLoader::identity(blob).map(|v| Box::new(v) as Box<dyn BinaryLoader>)
    }
}

//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//
//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//
//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//
//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//
//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//
//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//
//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//
//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//--//

pub struct ArleLoader {
    lio: LoadedImageOption,
    option: LaunchOption,
}

#[derive(Default)]
pub struct LaunchOption {
    start: usize,
    stack_pointer: usize,
}

impl ArleLoader {
    #[inline]
    fn new(option: LaunchOption) -> Self {
        Self {
            lio: LoadedImageOption::default(),
            option,
        }
    }

    pub fn identity(blob: &[u8]) -> Option<Self> {
        if blob[0] == 0xC3 {
            Some(Self::new(LaunchOption {
                start: 0,
                stack_pointer: 0,
            }))
        } else {
            None
        }
    }

    pub fn start(_: usize) {
        let ctx = Scheduler::current_personality()
            .unwrap()
            .get::<ArleContext>()
            .unwrap();
        unsafe {
            Hal::cpu().invoke_user(ctx.option.start, ctx.option.stack_pointer);
        }
    }
}

impl BinaryLoader for ArleLoader {
    fn option(&mut self) -> &mut LoadedImageOption {
        &mut self.lio
    }

    fn load(&mut self, blob: &[u8]) -> Result<(), ()> {
        todo!()
    }

    fn invoke_start(self: Box<Self>) -> Option<ProcessId> {
        SpawnOption::new()
            .personality(ArleContext::new(self.option))
            .start_process(Self::start, 0, self.lio.name.as_str())
    }
}

pub struct ArleContext {
    option: LaunchOption,
}

impl ArleContext {
    pub fn new(option: LaunchOption) -> PersonalityContext {
        PersonalityContext::new(Self { option })
    }
}

unsafe impl Identify for ArleContext {
    #[rustfmt::skip]
    /// 9CC78BE3-57D3-4E66-BF7E-61B18E8C3C65
    const UUID: Uuid = Uuid::from_parts(0x9CC78BE3, 0x57D3, 0x4E66, 0xBF7E, [0x61, 0xB1, 0x8E, 0x8C, 0x3C, 0x65]);
}

impl Personality for ArleContext {
    fn context(&mut self) -> *mut c_void {
        self as *const _ as *mut c_void
    }

    fn on_exit(self: Box<Self>) {
        // self.windows.lock().unwrap().clear();
    }
}
