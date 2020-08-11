// My OS Entry
#![no_std]
#![no_main]

use bootinfo::*;
use core::fmt::Write;
use io::console::*;
use io::fonts::*;
use io::graphics::*;
use io::hid::*;
use io::window::*;
use kernel::*;
use scheduler::*;
use system::*;
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
            .frame(Rect::new(670, 40, 120, 80))
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
            .style(WindowStyle::DEFAULT | WindowStyle::PINCHABLE)
            .frame(Rect::new(670, 136, 120, 80))
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
        let console = GraphicalConsole::new("Terminal", (80, 24), None);
        let window = console.window().unwrap();
        window.set_active();
        // window
        //     .draw(|bitmap| {
        //         let center = Point::new(bitmap.width() / 2, bitmap.height() / 2);
        //         bitmap.blend_rect(
        //             Rect::new(center.x - 85, center.y - 60, 80, 80),
        //             IndexedColor::LightRed.as_color() * 0.8,
        //         );
        //         bitmap.blend_rect(
        //             Rect::new(center.x - 40, center.y - 20, 80, 80),
        //             IndexedColor::LightGreen.as_color() * 0.8,
        //         );
        //         bitmap.blend_rect(
        //             Rect::new(center.x + 5, center.y - 60, 80, 80),
        //             IndexedColor::LightBlue.as_color() * 0.8,
        //         );
        //     })
        //     .unwrap();
        set_stdout(console);
    }

    println!(
        "{} v{} CPU {} CORES, MEMORY {} MB",
        system.name(),
        system.version(),
        system.number_of_active_cpus(),
        system.total_memory_size() >> 20,
    );

    loop {
        print!("# ");
        loop {
            stdout().set_cursor_enabled(true);
            match HidManager::get_key() {
                Some(key) => {
                    stdout().set_cursor_enabled(false);
                    let c: char = key.into();
                    match c {
                        '\0' => (),
                        '\r' => {
                            println!("\nBad command or file name - KERNEL PANIC!!!");
                            break;
                        }
                        _ => print!("{}", c),
                    }
                }
                None => GlobalScheduler::wait_for(None, TimeMeasure::from_millis(10)),
            }
        }
    }
}
