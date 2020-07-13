// My OS

#![feature(abi_efiapi)]
#![no_std]
#![no_main]
#![feature(asm)]

#[cfg(any(target_os = "uefi"))]
use ::uefi::prelude::*;

// use aml;
use core::fmt::Write;
use myos::boot::*;
use myos::kernel::arch::cpu::Cpu;
use myos::kernel::io::graphics::*;
use myos::kernel::io::hid;
use myos::kernel::io::hid::*;
use myos::kernel::scheduler::*;
use myos::kernel::system::*;
use myos::*;

myos_entry!(main);

fn main(info: &BootInfo) {
    System::init(info, sysinit);
}

fn sysinit() {
    let system = System::shared();

    let fb = stdout().fb();
    let size = fb.size();
    let center = Point::<isize>::new(size.width / 2, size.height / 2);

    fb.blend_rect(
        Rect::new(center.x - 85, center.y - 60, 80, 80),
        IndexedColor::LightRed.as_color() * 0.8,
    );
    fb.blend_rect(
        Rect::new(center.x - 40, center.y - 20, 80, 80),
        IndexedColor::LightGreen.as_color() * 0.8,
    );
    fb.blend_rect(
        Rect::new(center.x + 5, center.y - 60, 80, 80),
        IndexedColor::LightBlue.as_color() * 0.8,
    );

    GlobalScheduler::wait_for(None, TimeMeasure::from_millis(100));

    println!(
        "\n{} v{} CPU {} CORES, MEMORY {} MB",
        system.name(),
        system.version(),
        system.number_of_active_cpus(),
        system.total_memory_size() >> 20,
    );
    println!("Hello, {}!", "Rust");

    // Thread::spawn(|| {
    //     println!("Hello, thread!");
    // });

    loop {
        match HidManager::get_key() {
            Some(key) => {
                let c = hid::HidManager::usage_to_char_109(key.usage, key.modifier);
                print!("{}", c);
                match c {
                    'p' => GlobalScheduler::print_statistics(),
                    '!' => Cpu::breakpoint(),
                    _ => (),
                }
            }
            None => GlobalScheduler::wait_for(None, TimeMeasure::from_millis(10)),
        }
    }
}
