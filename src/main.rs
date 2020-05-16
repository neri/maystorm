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
            write!(st.stdout(), "Error: ACPI Table Not Found\n").unwrap();
            return Status::LOAD_ERROR;
        }
    };

    let (_st, _mm) = exit_boot_services(st, handle);

    let fb = stdout().fb();
    // fb.reset();
    fb.fill_rect(
        Rect::new((50, 50, 200, 200)),
        IndexedColor::LightBlue.as_color(),
    );
    fb.fill_rect(
        Rect::new((100, 100, 200, 200)),
        IndexedColor::LightRed.as_color(),
    );
    fb.fill_rect(
        Rect::new((150, 150, 200, 200)),
        IndexedColor::LightGreen.as_color(),
    );

    println!("My Practice OS version {}.{}.{}", 0, 0, 114514);
    println!("Hello, {:#}!", "Rust");

    println!("RSDPtr: {:#?}", rsdptr);
    // let mut my_handler = MyAcpiHandler::new();
    // let acpi = unsafe { acpi::parse_rsdp(&mut my_handler, rsdptr as usize).unwrap() };
    // println!("ACPI {:#?}", acpi);
    // dump_cpu(&acpi.boot_processor.unwrap());
    // for cpu in acpi.application_processors {
    //     dump_cpu(&cpu);
    // }

    panic!("Hoge");
    // loop {}
    // Status::SUCCESS
}

// fn dump_cpu(cpu: &acpi::Processor) {
//     println!(
//         "CPU {} apic id {} is_ap {} state {:#?}",
//         cpu.processor_uid, cpu.local_apic_id, cpu.is_ap, cpu.state
//     );
// }

// struct MyAcpiHandler {}

// impl MyAcpiHandler {
//     fn new() -> Self {
//         MyAcpiHandler {}
//     }
// }

// use acpi::handler::PhysicalMapping;
// use core::ptr::NonNull;
// impl acpi::handler::AcpiHandler for MyAcpiHandler {
//     unsafe fn map_physical_region<T>(
//         &mut self,
//         physical_address: usize,
//         size: usize,
//     ) -> PhysicalMapping<T> {
//         PhysicalMapping::<T> {
//             physical_start: physical_address,
//             virtual_start: NonNull::new(physical_address as *mut T).unwrap(),
//             region_length: size,
//             mapped_length: size,
//         }
//     }
//     fn unmap_physical_region<T>(&mut self, _region: PhysicalMapping<T>) {}
// }
