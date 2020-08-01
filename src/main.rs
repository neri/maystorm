// My OS

#![feature(abi_efiapi)]
#![no_std]
#![no_main]
#![feature(asm)]

#[cfg(any(target_os = "uefi"))]
use ::uefi::prelude::*;

// use aml;
use alloc::boxed::Box;
use core::fmt::Write;
use myos::boot::*;
use myos::kernel::io::console::*;
use myos::kernel::io::fonts::*;
use myos::kernel::io::graphics::*;
use myos::kernel::io::hid::*;
use myos::kernel::io::window::*;
use myos::kernel::scheduler::*;
use myos::kernel::system::*;
use myos::*;
extern crate alloc;

myos_entry!(main);

fn main(info: &BootInfo) {
    System::init(info, sysinit);
}

fn sysinit() {
    let system = System::shared();

    GlobalScheduler::wait_for(None, TimeMeasure::from_millis(300));

    {
        let window = WindowBuilder::new("Test 1")
            .frame(Rect::new(640, 40, 120, 80))
            .build();
        window
            .draw(|bitmap| {
                bitmap.draw_string(
                    FontDriver::small_font(),
                    bitmap.bounds(),
                    Color::BLACK,
                    "The quick brown fox jumps over the lazy dog.",
                );
            })
            .unwrap();
        window.show();
    }

    {
        let window = WindowBuilder::new("Test 2")
            .frame(Rect::new(640, 136, 120, 80))
            .bg_color(Color::from_argb(0x80000000))
            .build();
        window
            .draw(|bitmap| {
                bitmap.draw_string(
                    FontDriver::small_font(),
                    bitmap.bounds(),
                    IndexedColor::Yellow.into(),
                    "ETAOIN SHRDLU CMFWYP VBGKQJ XZ 1234567890",
                );
            })
            .unwrap();
        window.show();
    }

    {
        // Main Terminal
        let window = WindowBuilder::new("Terminal")
            .frame(Rect::new(8, 32, 640, 480))
            .build();
        window.show();
        window
            .draw(|bitmap| {
                let center = Point::<isize>::new(bitmap.width() / 2, bitmap.height() / 2);
                bitmap.blend_rect(
                    Rect::new(center.x - 85, center.y - 60, 80, 80),
                    IndexedColor::LightRed.as_color() * 0.8,
                );
                bitmap.blend_rect(
                    Rect::new(center.x - 40, center.y - 20, 80, 80),
                    IndexedColor::LightGreen.as_color() * 0.8,
                );
                bitmap.blend_rect(
                    Rect::new(center.x + 5, center.y - 60, 80, 80),
                    IndexedColor::LightBlue.as_color() * 0.8,
                );
            })
            .unwrap();
        set_stdout(Box::new(GraphicalConsole::from(window)));
    }

    println!(
        "{} v{} CPU {} CORES, MEMORY {} MB",
        system.name(),
        system.version(),
        system.number_of_active_cpus(),
        system.total_memory_size() >> 20,
    );
    println!("Hello, {}!", "Rust");

    loop {
        stdout().set_cursor_enabled(true);
        match HidManager::get_key() {
            Some(key) => {
                stdout().set_cursor_enabled(false);
                let c: char = key.into();
                if c != '\0' {
                    print!("{}", c);
                }
            }
            None => GlobalScheduler::wait_for(None, TimeMeasure::from_millis(10)),
        }
    }
}
