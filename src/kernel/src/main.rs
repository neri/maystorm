// My OS Entry
#![no_std]
#![no_main]
#![feature(asm)]

// use arch::cpu::*;
use bootprot::*;
use core::ffi::c_void;
use core::fmt::Write;
use core::mem::transmute;
use io::console::*;
use io::fonts::*;
use io::graphics::*;
use io::hid::*;
use kernel::*;
use scheduler::*;
use system::*;
use window::*;

#[macro_use]
extern crate alloc;
extern crate rlibc;

// use expr::simple_executor::*;
// use expr::*;
// use futures_util::stream::StreamExt;

myos_entry!(main);

const STATUS_BAR_HEIGHT: isize = 24;
const STATUS_BAR_BG_COLOR: Color = Color::from_argb(0xC0EEEEEE);

fn main(info: &BootInfo) {
    System::init(info, sysinit);
}

fn sysinit() {
    let system = System::shared();

    {
        let screen_bounds = WindowManager::main_screen_bounds();
        // Status bar
        let window = WindowBuilder::new("Status Bar")
            .style(WindowStyle::CLIENT_RECT | WindowStyle::FLOATING)
            .frame(Rect::new(0, 0, screen_bounds.width(), STATUS_BAR_HEIGHT))
            .bg_color(STATUS_BAR_BG_COLOR)
            .build();
        MyScheduler::spawn_f(
            status_bar_thread,
            unsafe { transmute(window) },
            Priority::Normal,
        );
    }

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
            .style_add(WindowStyle::PINCHABLE | WindowStyle::FLOATING)
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

    println!("{} v{}", system.name(), system.version(),);

    loop {
        print!("# ");
        loop {
            stdout().set_cursor_enabled(true);
            match HidManager::get_key() {
                Some(key) => {
                    stdout().set_cursor_enabled(false);
                    let c: char = key.into();
                    match c {
                        '!' => unsafe {
                            asm!(
                                "mov rax, 0xdeadbeef
                            jmp rax"
                            );
                        },
                        //Cpu::breakpoint(),
                        '\0' => (),
                        '\r' => {
                            println!("\nBad command or file name - KERNEL PANIC!!!");
                            break;
                        }
                        _ => print!("{}", c),
                    }
                }
                None => Timer::usleep(10000),
            }
        }
    }
}

fn status_bar_thread(window: *mut c_void) {
    let window: WindowHandle = unsafe { transmute(window) };
    let font = FontDriver::system_font();

    window
        .draw(|bitmap| {
            let bounds = bitmap.bounds();
            let rect = Rect::new(
                0,
                (bounds.height() - font.line_height()) / 2,
                bounds.width(),
                font.line_height(),
            );
            bitmap.draw_string(font, rect, IndexedColor::DarkGray.into(), "  My OS  ");
        })
        .unwrap();
    window.show();
    WindowManager::add_screen_insets(EdgeInsets::new(STATUS_BAR_HEIGHT, 0, 0, 0));

    let mut time_val = 0;
    loop {
        time_val += 1; // TODO: true clock
        let sec = time_val % 60;
        let min = time_val / 60 % 60;
        let hour = time_val / 3600 % 24;
        let time_str = if sec % 2 == 0 {
            format!("{:02} {:02} {:02}", hour, min, sec)
        } else {
            format!("{:02}:{:02}:{:02}", hour, min, sec)
        };

        let bounds = WindowManager::main_screen_bounds();
        let width = font.width() * time_str.len() as isize;
        let rect = Rect::new(
            bounds.width() - width - font.width() * 2,
            (window.frame().height() - font.line_height()) / 2,
            width,
            font.line_height(),
        );
        let _ = window.draw(|bitmap| {
            bitmap.fill_rect(rect, STATUS_BAR_BG_COLOR);
            bitmap.draw_string(font, rect, IndexedColor::DarkGray.into(), &time_str);
        });
        Timer::usleep(500_000);
    }
}
