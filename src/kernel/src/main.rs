// My OS Entry
// (c) 2020 Nerry
// License: MIT

#![no_std]
#![no_main]
#![feature(asm)]

// use acpi;
use alloc::boxed::Box;
use bootprot::*;
use core::fmt::Write;
use core::future::Future;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use core::time::Duration;
use io::fonts::*;
use io::graphics::*;
use kernel::*;
use mem::string;
use system::*;
use task::scheduler::*;
use window::*;

extern crate alloc;
extern crate rlibc;

// use expr::simple_executor::*;
// use expr::*;
// use futures_util::stream::StreamExt;

myos_entry!(main);

const STATUS_BAR_HEIGHT: isize = 24;
const STATUS_BAR_BG_COLOR: Color = Color::from_argb(0xC0EEEEEE);

fn main() {
    if System::is_headless() {
        stdout().reset().unwrap();
    } else {
        // Status bar
        MyScheduler::spawn_f(status_bar_thread, 0, "status bar", SpawnOption::new());

        if false {
            // Test Window 1
            let window = WindowBuilder::new("Window 1")
                .frame(Rect::new(-128, 40, 120, 80))
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

        if false {
            // Test Window 2
            let window = WindowBuilder::new("Window 2")
                .style_add(WindowStyle::PINCHABLE)
                .frame(Rect::new(-128, 150, 120, 80))
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
            let console = GraphicalConsole::new("Terminal", (80, 24), None, 0);
            let window = console.window().unwrap();
            window.move_to(Point::new(16, 40));
            window.set_active();
            System::set_stdout(console);
        }

        MyScheduler::spawn_f(top_thread, 0, "top", SpawnOption::new());
    }

    println!("{} v{}", System::name(), System::version(),);

    let waker = dummy_waker();
    let mut repl = Box::pin(repl());
    let mut cx = Context::from_waker(&waker);
    loop {
        match repl.as_mut().poll(&mut cx) {
            Poll::Ready(_) => break,
            Poll::Pending => Timer::usleep(100000),
        }
    }
}

async fn repl() {
    loop {
        print!("# ");
        loop {
            stdout().set_cursor_enabled(true);
            if let Ok(c) = stdout().read_async().await {
                stdout().set_cursor_enabled(false);
                match c {
                    '\0' => (),
                    '\r' => {
                        println!("\nBad command or file name - KERNEL PANIC!!!");
                        break;
                    }
                    _ => print!("{}", c),
                }
            }
        }
    }
}

fn dummy_waker() -> Waker {
    unsafe { Waker::from_raw(dummy_raw_waker()) }
}

fn dummy_raw_waker() -> RawWaker {
    fn no_op(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker {
        dummy_raw_waker()
    }

    let vtable = &RawWakerVTable::new(clone, no_op, no_op, no_op);
    RawWaker::new(0 as *const (), vtable)
}

fn status_bar_thread(_args: usize) {
    let screen_bounds = WindowManager::main_screen_bounds();
    let window = WindowBuilder::new("Status Bar")
        .style(WindowStyle::CLIENT_RECT | WindowStyle::FLOATING)
        .style_add(WindowStyle::BORDER)
        .frame(Rect::new(0, 0, screen_bounds.width(), STATUS_BAR_HEIGHT))
        .bg_color(STATUS_BAR_BG_COLOR)
        .build();
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

    let mut sb = string::Sb255::new();
    loop {
        sb.clear();

        let usage = MyScheduler::usage();
        let usage0 = usage / 10;
        let usage1 = usage % 10;
        write!(sb, "{:3}.{:1}%  ", usage0, usage1).unwrap();

        let time = System::system_time();
        let tod = time.secs % 86400;
        let sec = tod % 60;
        let min = tod / 60 % 60;
        let hour = tod / 3600;
        if sec % 2 == 0 {
            write!(sb, "{:2} {:02} {:02}", hour, min, sec).unwrap();
        } else {
            write!(sb, "{:2}:{:02}:{:02}", hour, min, sec).unwrap();
        };

        let bounds = window.frame();
        let width = font.width() * sb.len() as isize;
        let rect = Rect::new(
            bounds.width() - width - font.width() * 2,
            (bounds.height() - font.line_height()) / 2,
            width,
            font.line_height(),
        );
        let _ = window.draw(|bitmap| {
            bitmap.fill_rect(rect, STATUS_BAR_BG_COLOR);
            bitmap.draw_string(font, rect, IndexedColor::DarkGray.into(), sb.as_str());
        });
        Timer::usleep(500_000);
    }
}

fn top_thread(_args: usize) {
    let bg_color = Color::from_argb(0x80000000);
    let fg_color = IndexedColor::Yellow.into();

    let window = WindowBuilder::new("Activity Monitor")
        .style_add(WindowStyle::CLIENT_RECT | WindowStyle::FLOATING)
        .frame(Rect::new(-330, -230, 320, 200))
        .bg_color(bg_color)
        .build();
    let font = FontDriver::small_font();

    window.show();

    let mut sb = string::StringBuffer::with_capacity(0x1000);
    loop {
        MyScheduler::print_statistics(&mut sb);

        window
            .draw(|bitmap| {
                let rect = bitmap.bounds().insets_by(EdgeInsets::padding_all(4));
                bitmap.fill_rect(rect, bg_color);
                bitmap.draw_string(font, rect, fg_color, sb.as_str());
            })
            .unwrap();

        Timer::sleep(Duration::from_secs(1));
    }
}
