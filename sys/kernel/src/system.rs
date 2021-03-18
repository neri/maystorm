// A Computer System

use crate::io::emcon::*;
use crate::task::scheduler::*;
use crate::*;
use crate::{arch::cpu::*, fonts::*};
use crate::{drawing::*, io::tty::Tty};
use alloc::boxed::Box;
use alloc::vec::*;
use bootprot::BootInfo;
use core::fmt;
// use core::fmt::Write;
use core::ptr::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    versions: u32,
    rel: &'static str,
}

impl Version {
    const SYSTEM_NAME: &'static str = "codename MYOS";
    const SYSTEM_SHORT_NAME: &'static str = "myos";
    const RELEASE: &'static str = "";
    const VERSION: Version = Version::new(0, 0, 1, Self::RELEASE);

    const fn new(maj: u8, min: u8, patch: u16, rel: &'static str) -> Self {
        let versions = ((maj as u32) << 24) | ((min as u32) << 16) | (patch as u32);
        Version { versions, rel }
    }

    pub const fn as_u32(&self) -> u32 {
        self.versions
    }

    pub const fn maj(&self) -> usize {
        ((self.versions >> 24) & 0xFF) as usize
    }

    pub const fn min(&self) -> usize {
        ((self.versions >> 16) & 0xFF) as usize
    }

    pub const fn patch(&self) -> usize {
        (self.versions & 0xFFFF) as usize
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.rel.len() > 0 {
            write!(
                f,
                "{}.{}.{}-{}",
                self.maj(),
                self.min(),
                self.patch(),
                self.rel
            )
        } else {
            write!(f, "{}.{}.{}", self.maj(), self.min(), self.patch())
        }
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

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ProcessorCoreType {
    Main,
    Sub,
}

#[repr(transparent)]
#[derive(Copy, Clone, Default, PartialEq, PartialOrd)]
pub struct PhysicalAddress(pub usize);

pub struct System {
    /// Number of cpu cores
    num_of_cpus: usize,
    /// Number of physical cpu cores
    num_of_performance_cpus: usize,
    /// Vector of cpu cores
    cpus: Vec<Box<Cpu>>,

    /// An instance of ACPI tables
    acpi: Option<Box<acpi::AcpiTables<MyAcpiHandler>>>,

    // screens
    main_screen: Option<Bitmap32<'static>>,
    em_console: EmConsole,
    stdout: Option<Box<dyn Tty>>,

    // copy of boot info
    boot_flags: BootFlags,
    boot_vram: usize,
    boot_vram_stride: usize,
    initrd_base: usize,
    initrd_size: usize,
}

static mut SYSTEM: System = System::new();

impl System {
    const fn new() -> Self {
        System {
            num_of_cpus: 0,
            num_of_performance_cpus: 1,
            cpus: Vec::new(),
            acpi: None,
            boot_flags: BootFlags::empty(),
            main_screen: None,
            em_console: EmConsole::new(),
            stdout: None,
            boot_vram: 0,
            boot_vram_stride: 0,
            initrd_base: 0,
            initrd_size: 0,
        }
    }

    pub unsafe fn init(info: &BootInfo, f: fn() -> ()) -> ! {
        let shared = &mut SYSTEM;
        shared.boot_vram = info.vram_base as usize;
        shared.boot_vram_stride = info.vram_stride as usize;
        shared.boot_flags = info.flags;
        if shared.boot_flags.contains(BootFlags::INITRD_EXISTS) {
            shared.initrd_base = info.initrd_base as usize;
            shared.initrd_size = info.initrd_size as usize;
        }
        // shared.boot_flags.insert(BootFlags::HEADLESS);

        mem::MemoryManager::init_first(&info);

        let width = info.screen_width as isize;
        let height = info.screen_height as isize;
        let stride = info.vram_stride as usize;
        shared.main_screen = Some(Bitmap32::from_static(
            info.vram_base as usize as *mut TrueColor,
            Size::new(width, height),
            stride,
        ));

        shared.acpi = Some(Box::new(
            acpi::AcpiTables::from_rsdp(MyAcpiHandler::new(), info.acpi_rsdptr as usize).unwrap(),
        ));

        let pi = Self::acpi_platform().processor_info.unwrap();
        shared.num_of_cpus = pi.application_processors.len() + 1;
        shared.cpus.reserve(shared.num_of_cpus);
        shared
            .cpus
            .push(Cpu::new(ProcessorId(pi.boot_processor.local_apic_id)));

        arch::Arch::init();

        bus::pci::Pci::init();

        Scheduler::start(Self::init_late, f as usize);
    }

    fn init_late(args: usize) {
        let shared = Self::shared();
        unsafe {
            mem::MemoryManager::init_late();

            fs::Fs::init(shared.initrd_base, shared.initrd_size);

            rt::RuntimeEnvironment::init();

            if let Some(main_screen) = shared.main_screen.as_mut() {
                fonts::FontManager::init();
                window::WindowManager::init(main_screen.clone());
            }

            io::hid::HidManager::init();
            arch::Arch::init_late();

            user::userenv::UserEnv::start(core::mem::transmute(args));
        }
    }

    #[inline]
    pub fn debug_tick() {
        let shared = Self::shared();
        static mut DEBUG_PTR: usize = 0;
        if shared.boot_flags.contains(BootFlags::DEBUG_MODE) {
            unsafe {
                if DEBUG_PTR == 0 {
                    DEBUG_PTR = shared.boot_vram_stride * 6 + 6;
                }
                let vram = shared.boot_vram as *mut u32;
                vram.add(DEBUG_PTR).write_volatile(0xCCCCCC);
                DEBUG_PTR += 4;
            }
        }
    }

    /// Returns an internal shared instance
    #[inline]
    fn shared() -> &'static mut System {
        unsafe { &mut SYSTEM }
    }

    /// Returns the name of current system.
    #[inline]
    pub const fn name() -> &'static str {
        &Version::SYSTEM_NAME
    }

    /// Returns abbreviated name of current system.
    #[inline]
    pub const fn short_name() -> &'static str {
        &Version::SYSTEM_SHORT_NAME
    }

    /// Returns the version of current system.
    #[inline]
    pub const fn version() -> &'static Version {
        &Version::VERSION
    }

    /// Returns the current system time.
    #[inline]
    pub fn system_time() -> SystemTime {
        arch::Arch::system_time()
    }

    /// Returns whether the current system is in headless mode.
    #[inline]
    pub fn is_headless() -> bool {
        Self::shared().boot_flags.contains(BootFlags::HEADLESS)
    }

    /// Returns the number of logical CPU cores.
    #[inline]
    pub fn num_of_cpus() -> usize {
        Self::shared().num_of_cpus
    }

    /// Returns the number of performance CPU cores.
    /// Returns less than `num_of_cpus` for SMT-enabled processors or heterogeneous computing.
    #[inline]
    pub fn num_of_performance_cpus() -> usize {
        Self::shared().num_of_performance_cpus
    }

    /// Returns the number of active logical CPU cores.
    /// Returns the same value as `num_of_cpus` except during SMP initialization.
    #[inline]
    pub fn num_of_active_cpus() -> usize {
        Self::shared().cpus.len()
    }

    /// Add SMP-initialized CPU cores to the list of enabled cores.
    ///
    /// SAFETY: THREAD UNSAFE. DO NOT CALL IT EXCEPT FOR SMP INITIALIZATION.
    #[inline]
    pub(crate) unsafe fn activate_cpu(new_cpu: Box<Cpu>) {
        let shared = Self::shared();
        if new_cpu.processor_type() == ProcessorCoreType::Main {
            shared.num_of_performance_cpus += 1;
        }
        shared.cpus.push(new_cpu);
    }

    #[inline]
    pub fn cpu<'a>(index: usize) -> &'a Box<Cpu> {
        &Self::shared().cpus[index]
    }

    #[inline]
    pub(crate) unsafe fn cpu_mut<'a>(index: usize) -> &'a mut Box<Cpu> {
        &mut Self::shared().cpus[index]
    }

    /// SAFETY: THREAD UNSAFE. DO NOT CALL IT EXCEPT FOR SMP INITIALIZATION.
    #[inline]
    pub(crate) unsafe fn sort_cpus<F>(compare: F)
    where
        F: FnMut(&Box<Cpu>, &Box<Cpu>) -> core::cmp::Ordering,
    {
        Self::shared().cpus.sort_by(compare);
        let mut i = 0;
        for cpu in &mut Self::shared().cpus {
            cpu.cpu_index = ProcessorIndex(i);
            i += 1;
        }
    }

    #[inline]
    #[track_caller]
    pub fn acpi() -> &'static acpi::AcpiTables<MyAcpiHandler> {
        Self::shared().acpi.as_ref().unwrap()
    }

    #[inline]
    #[track_caller]
    pub fn acpi_platform() -> acpi::PlatformInfo {
        Self::acpi().platform_info().unwrap()
    }

    /// SAFETY: IT DESTROYS EVERYTHING.
    pub unsafe fn reset() -> ! {
        Cpu::reset();
    }

    /// SAFETY: IT DESTROYS EVERYTHING.
    pub unsafe fn shutdown() -> ! {
        todo!();
    }

    /// Get main screen
    pub fn main_screen() -> Bitmap<'static> {
        let shared = Self::shared();
        shared.main_screen.as_mut().unwrap().into()
    }

    #[inline]
    pub const fn em_console_font() -> &'static FixedFontDriver<'static> {
        FontManager::fixed_system_font()
        // FontManager::fixed_small_font()
    }

    pub fn em_console<'a>() -> &'a mut EmConsole {
        let shared = Self::shared();
        &mut shared.em_console
    }

    pub fn set_stdout(stdout: Box<dyn Tty>) {
        let shared = Self::shared();
        shared.stdout = Some(stdout);
    }

    pub fn stdout<'a>() -> &'a mut Box<dyn Tty> {
        let shared = Self::shared();
        shared.stdout.as_mut().unwrap()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct SystemTime {
    pub secs: u64,
    pub nanos: u32,
}

//-//-//-//-//

#[derive(Clone)]
pub struct MyAcpiHandler {}

impl MyAcpiHandler {
    const fn new() -> Self {
        MyAcpiHandler {}
    }
}

use ::acpi::PhysicalMapping;
impl ::acpi::AcpiHandler for MyAcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<Self, T> {
        PhysicalMapping {
            physical_start: physical_address,
            virtual_start: NonNull::new(physical_address as *mut T).unwrap(),
            region_length: size,
            mapped_length: size,
            handler: Self::new(),
        }
    }
    fn unmap_physical_region<T>(&self, _region: &PhysicalMapping<Self, T>) {}
}
