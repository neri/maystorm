// My UEFI-Rust Playground
#![feature(abi_efiapi)]
#![no_std]
#![no_main]
use core::fmt::Write;
use uefi::prelude::*;
use uefi_pg::myos::io::graphics::*;
use uefi_pg::*;

uefi_pg_entry!(main);

fn main(handle: Handle, st: SystemTable<Boot>) -> Status {
    let rsdptr = match st.find_config_table(uefi::table::cfg::ACPI2_GUID) {
        Some(val) => val,
        None => {
            writeln!(st.stdout(), "Error: ACPI Table Not Found").unwrap();
            return Status::LOAD_ERROR;
        }
    };

    // TODO: init custom allocator
    let buf_size = 0x1000000;
    let buf_ptr = st
        .boot_services()
        .allocate_pool(uefi::table::boot::MemoryType::LOADER_DATA, buf_size)
        .unwrap()
        .unwrap();
    myos::mem::alloc::init(buf_ptr as usize, buf_size);

    //////// GUARD //////// exit_boot_services //////// GUARD ////////
    let (_st, mm) = exit_boot_services(st, handle);

    let fb = stdout().fb();
    // fb.reset();
    fb.fill_rect(
        Rect::new(50, 50, 200, 200),
        IndexedColor::LightRed.as_color(),
    );
    fb.fill_rect(
        Rect::new(100, 100, 200, 200),
        IndexedColor::LightGreen.as_color(),
    );
    fb.fill_rect(
        Rect::new(150, 150, 200, 200),
        IndexedColor::LightBlue.as_color(),
    );

    let mut total_memory_size: u64 = 0;
    for mem_desc in mm {
        if mem_desc.ty.is_countable() {
            total_memory_size += mem_desc.page_count << 12;
        }
    }

    let mut my_handler = MyAcpiHandler::new();
    let acpi = unsafe { acpi::parse_rsdp(&mut my_handler, rsdptr as usize).unwrap() };
    unsafe {
        myos::arch::system::System::init(acpi.application_processors.len() + 1, total_memory_size);
    }

    let system = myos::arch::system::System::shared();

    println!(
        "My practice OS version {} Total {} / {} CPU Cores, {} MB System Memory",
        myos::MyOs::version(),
        system.number_of_active_cpus(),
        system.number_of_cpus(),
        system.total_memory_size() >> 20,
    );
    println!("Hello, {:#}!", "Rust");

    unsafe {
        myos::arch::cpu::Cpu::debug_assert();
    }
    panic!("System has halted");
    // Status::SUCCESS
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
