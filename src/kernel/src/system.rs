// A Computer System

use crate::arch::cpu::*;
use crate::bus::uart::*;
use crate::scheduler::*;
use crate::*;
use alloc::boxed::Box;
use alloc::vec::*;
use bootprot::BootInfo;
use core::num::*;
use core::ptr::*;

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
    emergency_console: Option<Box<dyn Tty>>,
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
            emergency_console: None,
        }
    }

    pub fn init(info: &BootInfo, f: fn() -> ()) -> ! {
        unsafe {
            let shared = &mut SYSTEM;
            shared.boot_flags = info.flags;

            MemoryManager::init(&info);

            let screen = Bitmap::from(info);
            screen.reset();
            shared.boot_screen = Some(Box::new(screen));
            let stdout = Box::new(GraphicalConsole::from(Self::boot_screen()));
            shared.emergency_console = Some(stdout);

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
        unsafe {
            window::WindowManager::init();
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

    pub fn reset() -> ! {
        unsafe {
            Cpu::reset();
        }
    }

    pub fn shutdown() -> ! {
        // TODO:
        unsafe {
            Cpu::stop();
        }
    }

    #[inline]
    pub fn is_headless() -> bool {
        Self::shared().boot_flags.contains(BootFlags::HEADLESS)
    }

    #[inline]
    pub fn uarts<'a>() -> &'a [Box<dyn Uart>] {
        arch::Arch::uarts()
    }

    #[inline]
    #[track_caller]
    pub fn boot_screen() -> &'static Box<Bitmap> {
        &Self::shared().boot_screen.as_ref().unwrap()
    }

    pub fn set_em_console(value: bool) {
        let _shared = Self::shared();
        unsafe {
            USE_EMCONSOLE = value;
        }
    }

    pub fn stdout<'a>() -> &'a mut Box<dyn Tty> {
        let shared = Self::shared();
        unsafe {
            if USE_EMCONSOLE {
                shared.emergency_console.as_mut().unwrap()
            } else {
                STDOUT.as_mut().unwrap()
            }
        }
    }

    pub fn set_stdout(console: Box<dyn Tty>) {
        let _shared = Self::shared();

        unsafe {
            STDOUT = Some(console);
            Self::set_em_console(false);
        }
    }
}

static mut USE_EMCONSOLE: bool = true;

static mut STDOUT: Option<Box<dyn Tty>> = None;

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
