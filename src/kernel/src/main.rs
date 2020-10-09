// My OS Entry
// (c) 2020 Nerry
// License: MIT

#![no_std]
#![no_main]
#![feature(asm)]

// use acpi;
use alloc::boxed::Box;
use alloc::vec::*;
use bootprot::*;
use core::fmt::Write;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, RawWaker, RawWakerVTable, Waker};
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
        {
            // Main Terminal
            let (console, window) =
                GraphicalConsole::new("Terminal", (40, 10), FontDriver::system_font(), 0, 0);
            window.move_to(Point::new(16, 40));
            window.set_active();
            System::set_stdout(console);
        }

        if false {
            // Test Window 1
            let window = WindowBuilder::new("Hello")
                .frame(Rect::new(isize::MIN, isize::MIN, 256, 120))
                .build();
            window
                .draw(|bitmap| {
                    bitmap.fill_round_rect(
                        Rect::new(0, 4, 104, 30),
                        8,
                        IndexedColor::LightBlue.into(),
                    );
                    bitmap.draw_string(
                        FontDriver::system_font(),
                        bitmap.bounds().insets_by(EdgeInsets::new(8, 8, 4, 8)),
                        Color::WHITE,
                        "Hello Rust!",
                    );
                })
                .unwrap();
            window.set_active();
        }
    }

    let mut tasks: Vec<Pin<Box<dyn Future<Output = ()>>>> = Vec::new();

    tasks.push(Box::pin(repl_main()));

    if System::is_headless() {
    } else {
        tasks.push(Box::pin(status_bar_main()));
        tasks.push(Box::pin(activity_monitor_main()));
    }

    let waker = dummy_waker();
    let mut cx = Context::from_waker(&waker);
    loop {
        for task in &mut tasks {
            let _ = task.as_mut().poll(&mut cx);
        }
        Timer::usleep(100_000);
    }
}

async fn repl_main() {
    println!("{} v{}", System::name(), System::version(),);

    loop {
        print!("# ");
        loop {
            stdout().set_cursor_enabled(true);
            if let Ok(c) = stdout().read_async().await {
                stdout().set_cursor_enabled(false);
                match c {
                    '\0' => (),
                    '\r' => {
                        println!("\nBad command or filename");
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

async fn status_bar_main() {
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
        Timer::sleep_async(Duration::from_millis(500)).await;

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
    }
}

async fn activity_monitor_main() {
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

        Timer::sleep_async(Duration::from_millis(1000)).await;
    }
}
