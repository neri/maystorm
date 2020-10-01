// A Computer System

use crate::arch::cpu::*;
use crate::dev::uart::*;
use crate::dev::vt100::*;
use crate::io::tty::*;
use crate::task::scheduler::*;
use crate::*;
use alloc::boxed::Box;
use alloc::vec::*;
use bootprot::BootInfo;
use core::fmt::*;
use core::num::*;
use core::ptr::*;
use core::sync::atomic::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub maj: usize,
    pub min: usize,
    pub rel: usize,
}

impl Version {
    const SYSTEM_NAME: &'static str = "my OS";
    const VERSION: Version = Version::new(0, 0, 1);

    const fn new(maj: usize, min: usize, rel: usize) -> Self {
        Version { maj, min, rel }
    }
}

use core::fmt;
impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.maj, self.min, self.rel)
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct ProcessorId(pub u8);

impl ProcessorId {
    pub const fn as_u32(self) -> u32 {
        self.0 as u32
    }
}

impl From<u8> for ProcessorId {
    fn from(val: u8) -> Self {
        Self(val)
    }
}

impl From<u32> for ProcessorId {
    fn from(val: u32) -> Self {
        Self(val as u8)
    }
}

impl From<usize> for ProcessorId {
    fn from(val: usize) -> Self {
        Self(val as u8)
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ProcessorIndex(pub usize);

#[repr(transparent)]
#[derive(Copy, Clone, Default, PartialEq, PartialOrd)]
pub struct VirtualAddress(pub usize);

impl VirtualAddress {
    pub const NULL: VirtualAddress = VirtualAddress(0);

    pub fn into_nonnull<T>(self) -> Option<NonNull<T>> {
        self.into()
    }

    pub fn into_nonzero(self) -> Option<NonZeroUsize> {
        self.into()
    }
}

impl<T> Into<Option<NonNull<T>>> for VirtualAddress {
    fn into(self) -> Option<NonNull<T>> {
        NonNull::new(self.0 as *const T as *mut T)
    }
}

impl Into<Option<NonZeroUsize>> for VirtualAddress {
    fn into(self) -> Option<NonZeroUsize> {
        NonZeroUsize::new(self.0)
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Default, PartialEq, PartialOrd)]
pub struct PhysicalAddress(pub usize);

pub struct System {
    num_of_cpus: usize,
    cpus: Vec<Box<Cpu>>,
    acpi: Option<Box<acpi::Acpi>>,
    boot_flags: BootFlags,
    boot_screen: Option<Box<Bitmap>>,
    stdout: Option<Box<dyn Tty>>,
    emergency_console: Option<Box<dyn Tty>>,
    use_emergency_console: AtomicBool,
}

static mut SYSTEM: System = System::new();

unsafe impl Sync for System {}

impl System {
    const fn new() -> Self {
        System {
            num_of_cpus: 0,
            cpus: Vec::new(),
            acpi: None,
            boot_flags: BootFlags::empty(),
            boot_screen: None,
            stdout: None,
            emergency_console: None,
            use_emergency_console: AtomicBool::new(true),
        }
    }

    pub fn init(info: &BootInfo, f: fn() -> ()) -> ! {
        unsafe {
            let shared = &mut SYSTEM;
            shared.boot_flags = info.flags;
            // shared.boot_flags.insert(BootFlags::HEADLESS);

            MemoryManager::init(&info);

            if System::is_headless() {
                let uart = arch::Arch::master_uart().unwrap();
                let stdout = Box::new(Vt100::with_uart(&uart));
                shared.emergency_console = Some(stdout);
            } else {
                let screen = Bitmap::from(info);
                screen.reset();
                shared.boot_screen = Some(Box::new(screen));
                let stdout = Box::new(GraphicalConsole::from(shared.boot_screen.as_ref().unwrap()));
                shared.emergency_console = Some(stdout);
            }

            MemoryManager::init_late();

            let mut my_handler = MyAcpiHandler::new();
            shared.acpi = Some(Box::new(
                ::acpi::parse_rsdp(&mut my_handler, info.acpi_rsdptr as usize).unwrap(),
            ));

            shared.num_of_cpus = Self::acpi().application_processors.len() + 1;

            shared.cpus.push(Cpu::new(ProcessorId::from(
                Self::acpi().boot_processor.unwrap().local_apic_id,
            )));

            arch::Arch::init();

            MyScheduler::start(Self::init_late, f as *const c_void as usize);
        }
    }

    fn init_late(args: usize) {
        let shared = Self::shared();
        unsafe {
            if let Some(main_screen) = shared.boot_screen.as_ref() {
                window::WindowManager::init(main_screen);
            }
            io::hid::HidManager::init();
            arch::Arch::init_late();

            let f: fn() = core::mem::transmute(args);
            f();
        }
    }

    #[inline]
    fn shared() -> &'static mut System {
        unsafe { &mut SYSTEM }
    }

    #[inline]
    pub fn num_of_cpus() -> usize {
        Self::shared().num_of_cpus
    }

    #[inline]
    pub fn num_of_active_cpus() -> usize {
        Self::shared().cpus.len()
    }

    #[inline]
    pub fn cpu<'a>(index: usize) -> &'a Box<Cpu> {
        &Self::shared().cpus[index]
    }

    #[inline]
    pub fn acpi() -> &'static acpi::Acpi {
        Self::shared().acpi.as_ref().unwrap()
    }

    #[inline]
    pub(crate) unsafe fn activate_cpu(new_cpu: Box<Cpu>) -> ProcessorIndex {
        let shared = Self::shared();
        let new_index = shared.cpus.len();
        shared.cpus.push(new_cpu);
        ProcessorIndex(new_index)
    }

    pub fn version<'a>() -> &'a Version {
        &Version::VERSION
    }

    pub fn name<'a>() -> &'a str {
        &Version::SYSTEM_NAME
    }

    pub unsafe fn reset() -> ! {
        Cpu::reset();
    }

    pub unsafe fn shutdown() -> ! {
        todo!();
    }

    pub fn system_time() -> SystemTime {
        arch::Arch::system_time()
    }

    #[inline]
    pub fn is_headless() -> bool {
        Self::shared().boot_flags.contains(BootFlags::HEADLESS)
    }

    #[inline]
    pub fn uarts<'a>() -> &'a [Box<dyn Uart>] {
        arch::Arch::uarts()
    }

    pub fn set_em_console(value: bool) {
        let shared = Self::shared();
        shared.use_emergency_console.store(value, Ordering::SeqCst);
    }

    pub fn stdout<'a>() -> &'a mut Box<dyn Tty> {
        let shared = Self::shared();
        if shared.use_emergency_console.load(Ordering::SeqCst) {
            shared.emergency_console.as_mut().unwrap()
        } else {
            shared.stdout.as_mut().unwrap()
        }
    }

    pub fn set_stdout(console: Box<dyn Tty>) {
        let shared = Self::shared();
        shared.stdout = Some(console);
        Self::set_em_console(false);
    }
}

pub struct SystemTime {
    pub secs: u64,
    pub nanos: u32,
}

struct MyAcpiHandler {}

impl MyAcpiHandler {
    fn new() -> Self {
        MyAcpiHandler {}
    }
}

use acpi::handler::PhysicalMapping;
impl ::acpi::handler::AcpiHandler for MyAcpiHandler {
    unsafe fn map_physical_region<T>(
        &mut self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<T> {
        PhysicalMapping::<T> {
            physical_start: physical_address,
            virtual_start: NonNull::new(physical_address as *mut T).unwrap(),
            region_length: size,
            mapped_length: size,
        }
    }
    fn unmap_physical_region<T>(&mut self, _region: PhysicalMapping<T>) {}
}
