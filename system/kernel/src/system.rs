use crate::{arch::cpu::*, io::emcon::*, io::tty::*, task::scheduler::*, *};
use alloc::{boxed::Box, string::*, vec::Vec};
use bootprot::BootInfo;
use core::{cell::UnsafeCell, ffi::c_void, fmt, mem::transmute, sync::atomic::*};
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProcessorCoreType {
    /// Main Processor
    Main,
    /// Subprocessor of SMT enabled processor.
    Sub,
    /// High-efficiency processor
    Efficient,
}

/// A Kernel of MEG-OS codename Maystorm
#[allow(dead_code)]
pub struct System {
    /// Current device information
    current_device: DeviceInfo,

    /// Array of activated processor cores
    cpus: Vec<Box<Cpu>>,

    /// An instance of ACPI tables
    acpi: Option<&'static myacpi::RsdPtr>,

    /// An instance of SMBIOS
    smbios: Option<Box<fw::smbios::SmBios>>,

    // screens
    main_screen: Option<UnsafeCell<Bitmap32<'static>>>,
    em_console: EmConsole,
    stdout: Option<Box<dyn Tty>>,

    // copy of boot info
    boot_flags: BootFlags,
    initrd_base: PhysicalAddress,
    initrd_size: usize,
}

static mut SYSTEM: UnsafeCell<System> = UnsafeCell::new(System::new());

impl System {
    const SYSTEM_NAME: &'static str = "MEG-OS";
    const SYSTEM_CODENAME: &'static str = "Cherry";
    const SYSTEM_SHORT_NAME: &'static str = "myos";
    const RELEASE: &'static str = "alpha";
    const VERSION: Version<'static> = Version::new(0, 12, 0, Self::RELEASE);

    #[inline]
    const fn new() -> Self {
        System {
            current_device: DeviceInfo::new(),
            cpus: Vec::new(),
            acpi: None,
            smbios: None,
            boot_flags: BootFlags::empty(),
            main_screen: None,
            em_console: EmConsole::new(ui::font::FontManager::preferred_console_font()),
            stdout: None,
            initrd_base: PhysicalAddress::NULL,
            initrd_size: 0,
        }
    }

    /// Initialize the system
    pub unsafe fn init(info: &BootInfo, f: fn() -> ()) -> ! {
        assert_call_once!();

        let shared = SYSTEM.get_mut();
        shared.boot_flags = info.flags;
        shared.initrd_base = PhysicalAddress::new(info.initrd_base as u64);
        shared.initrd_size = info.initrd_size as usize;
        shared.current_device.total_memory_size = info.total_memory_size as usize;

        let main_screen = Bitmap32::from_static(
            PhysicalAddress::new(info.vram_base).direct_map(),
            Size::new(info.screen_width as isize, info.screen_height as isize),
            info.vram_stride as usize,
        );
        shared.main_screen = Some(UnsafeCell::new(main_screen));
        // Self::em_console().reset().unwrap();

        mem::MemoryManager::init_first(info);

        shared.acpi = unsafe { myacpi::RsdPtr::parse(info.acpi_rsdptr as usize as *const c_void) };

        if info.smbios != 0 {
            let device = &mut shared.current_device;
            let smbios = fw::smbios::SmBios::init(info.smbios.into());
            device.manufacturer_name = smbios.manufacturer_name();
            device.model_name = smbios.model_name();
            shared.smbios = Some(smbios);
        }

        arch::Arch::init();

        Scheduler::start(Self::late_init, f as usize);
    }

    /// The second half of the system initialization
    fn late_init(args: usize) {
        assert_call_once!();

        let shared = unsafe { Self::shared_mut() };

        if false {
            // banner
            let device = System::current_device();
            let bytes = device.total_memory_size();
            let gb = bytes >> 30;
            let mb = (100 * (bytes & 0x3FFF_FFFF)) / 0x4000_0000;

            writeln!(
                System::em_console(),
                "{} v{} [{} Processor cores, Memory {}.{:02} GB]",
                System::name(),
                System::version(),
                device.num_of_active_cpus(),
                gb,
                mb
            )
            .unwrap();
        }

        unsafe {
            Scheduler::late_init();
            mem::MemoryManager::late_init();

            log::EventManager::init();

            fs::FileManager::init(shared.initrd_base.direct_map(), shared.initrd_size);

            io::audio::AudioManager::init();
            io::hid_mgr::HidManager::init();
            drivers::usb::UsbManager::init();
            drivers::pci::Pci::init();

            if let Some(main_screen) = shared.main_screen.as_mut() {
                ui::font::FontManager::init();
                ui::window::WindowManager::init(main_screen.get_mut().clone());
            }

            arch::Arch::late_init();

            rt::RuntimeEnvironment::init();

            user::userenv::UserEnv::start(transmute(args));
        }
    }

    #[inline]
    unsafe fn shared_mut() -> &'static mut System {
        SYSTEM.get_mut()
    }

    #[inline]
    fn shared() -> &'static System {
        unsafe { &*SYSTEM.get() }
    }

    /// Returns the name of the current system.
    #[inline]
    pub const fn name() -> &'static str {
        &Self::SYSTEM_NAME
    }

    /// Returns the codename of the current system.
    #[inline]
    pub const fn codename() -> &'static str {
        &Self::SYSTEM_CODENAME
    }

    /// Returns abbreviated name of the current system.
    #[inline]
    pub const fn short_name() -> &'static str {
        &Self::SYSTEM_SHORT_NAME
    }

    /// Returns the version of the current system.
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
        let shared = Self::shared_mut();
        let processor_type = new_cpu.processor_type();
        shared.cpus.push(new_cpu);
        let device = &shared.current_device;
        device.num_of_active_cpus.fetch_add(1, Ordering::AcqRel);
        if processor_type == ProcessorCoreType::Main {
            device.num_of_main_cpus.fetch_add(1, Ordering::AcqRel);
        }
        fence(Ordering::SeqCst);
    }

    /// Returns an instance of the current processor.
    #[inline]
    #[track_caller]
    pub fn current_processor<'a>() -> &'a Cpu {
        Self::shared()
            .cpus
            .get(Hal::cpu().current_processor_index().0)
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
        Self::shared_mut().cpus.get_mut(index.0).unwrap()
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
    pub fn acpi<'a>() -> Option<&'a myacpi::Xsdt> {
        Self::shared().acpi.as_ref().map(|v| v.xsdt())
    }

    /// Get main screen
    pub fn main_screen() -> Bitmap<'static> {
        unsafe { &mut *Self::shared_mut().main_screen.as_mut().unwrap().get() }.into()
    }

    pub fn em_console<'a>() -> &'a mut EmConsole {
        unsafe { &mut Self::shared_mut().em_console }
    }

    pub fn set_stdout(stdout: Box<dyn Tty>) {
        let shared = unsafe { Self::shared_mut() };
        shared.stdout = Some(stdout);
    }

    pub fn stdout<'a>() -> &'a mut dyn Tty {
        let shared = unsafe { Self::shared_mut() };
        match shared.stdout.as_mut() {
            Some(v) => v.as_mut(),
            None => io::null::Null::null(),
        }
    }

    #[track_caller]
    pub fn assert_call_once(mutex: &AtomicBool) {
        if mutex
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
            .is_err()
        {
            panic!("Multiple calls are not allowed");
        }
    }
}

#[macro_export]
macro_rules! assert_call_once {
    () => {
        static MUTEX: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
        System::assert_call_once(&MUTEX);
    };
}

pub struct DeviceInfo {
    manufacturer_name: Option<String>,
    model_name: Option<String>,
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
