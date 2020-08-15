// A Computer System

use crate::arch::cpu::*;
use crate::scheduler::*;
use crate::*;
use alloc::boxed::Box;
use alloc::vec::*;
use bootprot::BootInfo;
use core::ptr::NonNull;

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
}

impl<T> Into<Option<NonNull<T>>> for VirtualAddress {
    fn into(self) -> Option<NonNull<T>> {
        if self != Self::NULL {
            NonNull::new(self.0 as *const T as *mut T)
        } else {
            None
        }
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Default, PartialEq, PartialOrd)]
pub struct PhysicalAddress(pub usize);

pub struct System {
    total_memory_size: u64,
    number_of_cpus: usize,
    cpus: Vec<Box<Cpu>>,
    acpi: Option<Box<acpi::Acpi>>,
}

static mut SYSTEM: System = System::new();

unsafe impl Sync for System {}

impl System {
    const fn new() -> Self {
        System {
            total_memory_size: 0,
            number_of_cpus: 0,
            cpus: Vec::new(),
            acpi: None,
        }
    }

    pub fn init(info: &BootInfo, f: fn() -> ()) -> ! {
        unsafe {
            let mut my_handler = MyAcpiHandler::new();
            SYSTEM.acpi = Some(Box::new(
                ::acpi::parse_rsdp(&mut my_handler, info.acpi_rsdptr as usize).unwrap(),
            ));

            SYSTEM.number_of_cpus = SYSTEM.acpi().application_processors.len() + 1;
            SYSTEM.total_memory_size = info.total_memory_size;

            SYSTEM.cpus.push(Cpu::new(ProcessorId::from(
                SYSTEM.acpi().boot_processor.unwrap().local_apic_id,
            )));
            Cpu::init();

            GlobalScheduler::start(&SYSTEM, Self::late_init, f as *const c_void as *mut c_void);
        }
    }

    fn late_init(args: *mut c_void) {
        unsafe {
            io::window::WindowManager::init();
            io::hid::HidManager::init();
            arch::Arch::late_init();

            let f = core::mem::transmute::<*mut c_void, fn() -> ()>(args);
            f();
        }
    }

    #[inline]
    pub fn shared() -> &'static System {
        unsafe { &SYSTEM }
    }

    #[inline]
    pub fn number_of_cpus(&self) -> usize {
        self.number_of_cpus
    }

    #[inline]
    pub fn number_of_active_cpus(&self) -> usize {
        self.cpus.len()
    }

    #[inline]
    pub fn cpu(&self, index: usize) -> &Box<Cpu> {
        &self.cpus[index]
    }

    #[inline]
    pub fn total_memory_size(&self) -> u64 {
        self.total_memory_size
    }

    #[inline]
    pub fn acpi(&self) -> &acpi::Acpi {
        self.acpi.as_ref().unwrap()
    }

    #[inline]
    pub(crate) unsafe fn activate_cpu(&self, new_cpu: Box<Cpu>) -> ProcessorIndex {
        let new_index = SYSTEM.cpus.len();
        SYSTEM.cpus.push(new_cpu);
        ProcessorIndex(new_index)
    }

    pub fn version(&self) -> &'static Version {
        &Version::VERSION
    }

    pub fn name(&self) -> &str {
        &Version::SYSTEM_NAME
    }
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