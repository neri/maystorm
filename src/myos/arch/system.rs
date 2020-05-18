// A Computer System

use super::cpu::*;
use alloc::boxed::Box;
use alloc::vec::*;

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

    pub unsafe fn init(rsdptr: usize, total_memory_size: u64) {
        let mut my_handler = MyAcpiHandler::new();

        SYSTEM.total_memory_size = total_memory_size;
        SYSTEM.acpi = Some(Box::new(acpi::parse_rsdp(&mut my_handler, rsdptr).unwrap()));
        SYSTEM.number_of_cpus = SYSTEM.acpi().application_processors.len() + 1;

        SYSTEM
            .cpus
            .push(Cpu::new(SYSTEM.acpi().boot_processor.unwrap()));
        Cpu::init();
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
    pub unsafe fn acpi(&self) -> &acpi::Acpi {
        self.acpi.as_ref().unwrap()
    }
}

struct MyAcpiHandler {}

impl MyAcpiHandler {
    fn new() -> Self {
        MyAcpiHandler {}
    }
}

use acpi::handler::PhysicalMapping;
use core::ptr::NonNull;
impl acpi::handler::AcpiHandler for MyAcpiHandler {
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
