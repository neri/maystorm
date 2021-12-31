//! A Computer System

use crate::{
    arch::cpu::*,
    arch::page::{PageManager, PhysicalAddress},
    io::emcon::*,
    io::tty::*,
    task::scheduler::*,
    ui::font::FontManager,
    *,
};
use alloc::{boxed::Box, string::*, vec::Vec};
use bootprot::BootInfo;
use core::{fmt, ptr::*, sync::atomic::*};
use megstd::{drawing::*, time::SystemTime};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version<'a> {
    versions: u32,
    rel: &'a str,
}

impl Version<'_> {
    #[inline]
    pub const fn new<'a>(maj: u8, min: u8, patch: u16, rel: &'a str) -> Version<'a> {
        let versions = ((maj as u32) << 24) | ((min as u32) << 16) | (patch as u32);
        Version { versions, rel }
    }

    #[inline]
    pub const fn as_u32(&self) -> u32 {
        self.versions
    }

    #[inline]
    pub const fn maj(&self) -> usize {
        ((self.versions >> 24) & 0xFF) as usize
    }

    #[inline]
    pub const fn min(&self) -> usize {
        ((self.versions >> 16) & 0xFF) as usize
    }

    #[inline]
    pub const fn patch(&self) -> usize {
        (self.versions & 0xFFFF) as usize
    }

    #[inline]
    pub const fn rel(&self) -> &str {
        &self.rel
    }
}

impl fmt::Display for Version<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.rel().len() > 0 {
            write!(
                f,
                "{}.{}.{}-{}",
                self.maj(),
                self.min(),
                self.patch(),
                self.rel(),
            )
        } else {
            write!(f, "{}.{}.{}", self.maj(), self.min(), self.patch())
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessorIndex(pub usize);

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ProcessorCoreType {
    /// Main Processor
    Main,
    /// Subprocessor of SMT enabled processor.
    Sub,
    /// High-efficiency processor
    Efficient,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProcessorSystemType {
    /// The system is equipped with a uniprocessor. (deprecated)
    UP,
    /// The system is equipped with a symmetric multiprocessing processor.
    SMP,
    /// The system is equipped with one or more simultaneous multi-threading processors.
    SMT,
}

impl ToString for ProcessorSystemType {
    fn to_string(&self) -> String {
        let s = match self {
            ProcessorSystemType::UP => "UP",
            ProcessorSystemType::SMP => "SMP",
            ProcessorSystemType::SMT => "SMT",
        };
        s.to_string()
    }
}

#[allow(dead_code)]
pub struct System {
    /// Current device information
    current_device: DeviceInfo,

    /// Array of activated processor cores
    cpus: Vec<Box<Cpu>>,

    /// An instance of ACPI tables
    acpi: Option<Box<acpi::AcpiTables<MyAcpiHandler>>>,

    /// An instance of SMBIOS
    smbios: Option<Box<fw::smbios::SmBios>>,

    // screens
    main_screen: Option<Bitmap32<'static>>,
    em_console: EmConsole,
    stdout: Option<Box<dyn Tty>>,

    // copy of boot info
    boot_flags: BootFlags,
    initrd_base: usize,
    initrd_size: usize,
}

static mut SYSTEM: System = System::new();

impl System {
    const SYSTEM_NAME: &'static str = "MEG-OS codename Azalea";
    const SYSTEM_SHORT_NAME: &'static str = "megos";
    const RELEASE: &'static str = "";
    const VERSION: Version<'static> = Version::new(0, 10, 0, Self::RELEASE);

    #[inline]
    const fn new() -> Self {
        System {
            current_device: DeviceInfo::new(),
            cpus: Vec::new(),
            acpi: None,
            smbios: None,
            boot_flags: BootFlags::empty(),
            main_screen: None,
            em_console: EmConsole::new(FontManager::preferred_console_font()),
            stdout: None,
            initrd_base: 0,
            initrd_size: 0,
        }
    }

    /// Initialize the system
    pub unsafe fn init(info: &BootInfo, f: fn() -> ()) -> ! {
        let shared = &mut SYSTEM;
        shared.boot_flags = info.flags;
        shared.initrd_base = info.initrd_base as usize;
        shared.initrd_size = info.initrd_size as usize;
        shared.current_device.total_memory_size = info.total_memory_size as usize;

        let main_screen = Bitmap32::from_static(
            PageManager::direct_map(info.vram_base) as *mut TrueColor,
            Size::new(info.screen_width as isize, info.screen_height as isize),
            info.vram_stride as usize,
        );
        shared.main_screen = Some(main_screen);
        // Self::em_console().reset().unwrap();

        mem::MemoryManager::init_first(info);

        shared.acpi = Some(Box::new(
            ::acpi::AcpiTables::from_rsdp(MyAcpiHandler::new(), info.acpi_rsdptr as usize).unwrap(),
        ));

        if info.smbios != 0 {
            let device = &mut shared.current_device;
            let smbios = fw::smbios::SmBios::init(info.smbios);
            device.manufacturer_name = smbios.manufacturer_name().map(|v| v.to_string());
            device.model_name = smbios.model_name().map(|v| v.to_string());
            shared.smbios = Some(smbios);
        }

        arch::Arch::init();

        Scheduler::start(Self::late_init, f as usize);
    }

    /// The second half of the system initialization
    fn late_init(args: usize) {
        let shared = Self::shared();

        // banner
        if false {
            let device = System::current_device();

            writeln!(
                System::em_console(),
                "{} v{} Processor {} / {} {:?}, Memory {} MB",
                System::name(),
                System::version(),
                device.num_of_performance_cpus(),
                device.num_of_active_cpus(),
                device.processor_system_type(),
                device.total_memory_size() >> 20,
            )
            .unwrap();
        }

        unsafe {
            mem::MemoryManager::late_init();

            log::EventManager::init();

            io::hid_mgr::HidManager::init();
            bus::usb::UsbManager::init();
            bus::pci::Pci::init();

            fs::FileManager::init(
                PageManager::direct_map(shared.initrd_base as PhysicalAddress),
                shared.initrd_size,
            );

            if let Some(main_screen) = shared.main_screen.as_mut() {
                FontManager::init();
                ui::window::WindowManager::init(main_screen.clone());
            }

            arch::Arch::late_init();

            rt::RuntimeEnvironment::init();

            user::userenv::UserEnv::start(core::mem::transmute(args));
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
        &Self::SYSTEM_NAME
    }

    /// Returns abbreviated name of current system.
    #[inline]
    pub const fn short_name() -> &'static str {
        &Self::SYSTEM_SHORT_NAME
    }

    /// Returns the version of current system.
    #[inline]
    pub const fn version<'a>() -> &'a Version<'a> {
        &Self::VERSION
    }

    #[inline]
    pub fn boot_flags() -> BootFlags {
        Self::shared().boot_flags
    }

    /// Returns the current system time.
    #[inline]
    pub fn system_time() -> SystemTime {
        arch::Arch::system_time()
    }

    /// Returns whether the kernel is multiprocessor-capable.
    #[inline]
    pub const fn is_multi_processor_capable_kernel() -> bool {
        true
    }

    /// Add SMP-initialized CPU cores to the list of activated cores.
    ///
    /// # Safety
    ///
    /// THREAD UNSAFE.
    /// Do not call this function except when initializing the SMP.
    #[inline]
    pub unsafe fn activate_cpu(new_cpu: Box<Cpu>) {
        let shared = Self::shared();
        let processor_type = new_cpu.processor_type();
        shared.cpus.push(new_cpu);
        let device = &shared.current_device;
        device.num_of_active_cpus.fetch_add(1, Ordering::SeqCst);
        if processor_type == ProcessorCoreType::Main {
            device.num_of_main_cpus.fetch_add(1, Ordering::SeqCst);
        }
        fence(Ordering::SeqCst);
    }

    /// Sorts the list of CPUs by processor ID.
    ///
    /// # Safety
    ///
    /// THREAD UNSAFE.
    /// Do not call this function except when initializing the SMP.
    #[inline]
    pub unsafe fn sort_cpus_by<F>(key: F)
    where
        F: FnMut(&Box<Cpu>) -> usize,
    {
        Self::shared().cpus.sort_by_key(key);
        for (index, cpu) in Self::shared().cpus.iter_mut().enumerate() {
            cpu.cpu_index = ProcessorIndex(index);
        }
        fence(Ordering::SeqCst);
    }

    /// Returns an instance of the current processor.
    #[inline]
    #[track_caller]
    pub fn current_processor<'a>() -> &'a Cpu {
        Self::shared()
            .cpus
            .get(Cpu::current_processor_index().0)
            .unwrap()
    }

    /// Returns a reference to the processor at the specified index.
    ///
    /// # Panics
    ///
    /// Panics if specified index is larger than the number of processors.
    #[inline]
    #[track_caller]
    pub fn cpu<'a>(index: ProcessorIndex) -> &'a Cpu {
        Self::shared().cpus.get(index.0).unwrap()
    }

    /// Returns a mutable reference to the processor at the specified index.
    ///
    /// # Panics
    ///
    /// Panics if specified index is larger than the number of processors.
    #[inline]
    #[track_caller]
    pub unsafe fn cpu_mut<'a>(index: ProcessorIndex) -> &'a mut Cpu {
        Self::shared().cpus.get_mut(index.0).unwrap()
    }

    #[inline]
    pub unsafe fn set_processor_systsm_type(value: ProcessorSystemType) {
        Self::shared().current_device.processor_system_type = value;
    }

    #[inline]
    #[track_caller]
    pub(crate) fn acpi() -> &'static acpi::AcpiTables<MyAcpiHandler> {
        Self::shared().acpi.as_ref().unwrap()
    }

    #[inline]
    pub fn smbios<'a>() -> Option<&'a fw::smbios::SmBios> {
        Self::shared().smbios.as_ref().map(|v| v.as_ref())
    }

    /// Returns the current device information.
    #[inline]
    pub fn current_device<'a>() -> &'a DeviceInfo {
        &Self::shared().current_device
    }

    #[inline]
    #[track_caller]
    pub fn acpi_platform() -> acpi::PlatformInfo {
        Self::acpi().platform_info().unwrap()
    }

    #[track_caller]
    pub fn reset() -> ! {
        Cpu::reset();
    }

    #[track_caller]
    pub fn shutdown() -> ! {
        todo!();
    }

    /// Get main screen
    pub fn main_screen() -> Bitmap<'static> {
        let shared = Self::shared();
        shared.main_screen.as_mut().unwrap().into()
    }

    pub fn em_console<'a>() -> &'a mut EmConsole {
        let shared = Self::shared();
        &mut shared.em_console
    }

    pub fn set_stdout(stdout: Box<dyn Tty>) {
        let shared = Self::shared();
        shared.stdout = Some(stdout);
    }

    pub fn stdout<'a>() -> &'a mut dyn Tty {
        let shared = Self::shared();
        match shared.stdout.as_mut() {
            Some(v) => v.as_mut(),
            None => io::null::Null::null(),
        }
    }
}

pub struct DeviceInfo {
    manufacturer_name: Option<String>,
    model_name: Option<String>,
    processor_system_type: ProcessorSystemType,
    num_of_active_cpus: AtomicUsize,
    num_of_main_cpus: AtomicUsize,
    total_memory_size: usize,
}

impl DeviceInfo {
    #[inline]
    const fn new() -> Self {
        Self {
            manufacturer_name: None,
            model_name: None,
            processor_system_type: ProcessorSystemType::UP,
            num_of_active_cpus: AtomicUsize::new(0),
            num_of_main_cpus: AtomicUsize::new(0),
            total_memory_size: 0,
        }
    }

    /// Returns the name of the manufacturer of the system, if available.
    #[inline]
    pub fn manufacturer_name(&self) -> Option<&str> {
        self.manufacturer_name.as_ref().map(|v| v.as_str())
    }

    /// Returns the model name of the system, if available.
    #[inline]
    pub fn model_name(&self) -> Option<&str> {
        self.model_name.as_ref().map(|v| v.as_str())
    }

    /// Returns the total amount of memory size in bytes.
    #[inline]
    pub const fn total_memory_size(&self) -> usize {
        self.total_memory_size
    }

    #[inline]
    pub const fn processor_system_type(&self) -> ProcessorSystemType {
        self.processor_system_type
    }

    /// Returns the number of active logical CPU cores.
    #[inline]
    pub fn num_of_active_cpus(&self) -> usize {
        self.num_of_active_cpus.load(Ordering::SeqCst)
    }

    /// Returns the number of performance CPU cores.
    /// Returns less than `num_of_active_cpus` for SMT-enabled processors.
    #[inline]
    pub fn num_of_performance_cpus(&self) -> usize {
        self.num_of_main_cpus.load(Ordering::SeqCst)
    }
}

//-//-//-//-//

#[doc(hidden)]
#[derive(Clone)]
pub(crate) struct MyAcpiHandler {}

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
        PhysicalMapping::new(
            physical_address,
            NonNull::new_unchecked(
                PageManager::direct_map(physical_address as PhysicalAddress) as *mut T
            ),
            size,
            size,
            Self::new(),
        )
    }
    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {}
}
