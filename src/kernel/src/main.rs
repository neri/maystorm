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
use mem::memory::*;
use mem::string;
use system::*;
use task::scheduler::*;
use window::view::*;
use window::*;

extern crate alloc;
extern crate rlibc;

// use expr::simple_executor::*;
// use expr::*;
// use futures_util::stream::StreamExt;

myos_entry!(main);

fn main() {
    let mut _main_window: Option<WindowHandle> = None;
    if System::is_headless() {
        stdout().reset().unwrap();
    } else {
        {
            // Main Terminal
            let (console, window) =
                GraphicalConsole::new("Terminal", (40, 10), FontManager::fixed_system_font(), 0, 0);
            window.move_to(Point::new(16, 40));
            window.set_active();
            System::set_stdout(console);
            _main_window = Some(window);
        }

        if true {
            // Test Window 1
            let window = WindowBuilder::new("Welcome")
                .size(Size::new(512, 384))
                .center()
                .build();

            if let Some(view) = window.view() {
                let mut rect = view.bounds();
                rect.size.height = 56;
                let mut shape = View::with_frame(rect);
                shape.set_background_color(Color::from_rgb(0x64B5F6));
                // shape.set_background_color(Color::from_rgb(0xFF9800));
                view.add_subview(shape);

                let mut rect = view.bounds().insets_by(EdgeInsets::new(16, 16, 0, 16));
                rect.size.height = 44;
                let mut text_view = TextView::with_text("Welcome to My OS !");
                FontDescriptor::new(FontFamily::Cursive, 32).map(|font| text_view.set_font(font));
                text_view.set_tint_color(IndexedColor::White.into());
                text_view.set_frame(rect);
                text_view.set_max_lines(1);
                view.add_subview(text_view);

                // rect.origin.y += rect.size.height + 10;
                // rect.size.height = 24;
                // let mut text_view = TextView::with_text("~ A toy that displays a picture ~");
                // FontDescriptor::new(FontFamily::Cursive, 20).map(|font| text_view.set_font(font));
                // text_view.set_tint_color(IndexedColor::Green.into());
                // text_view.set_frame(rect);
                // text_view.set_max_lines(2);
                // view.add_subview(text_view);

                rect.origin.y += rect.size.height + 10;
                let mut text_view = TextView::with_text("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.");
                // let mut text_view = TextView::with_text("The quick brown fox jumps over the lazy dog.");
                text_view.set_frame(rect);
                FontDescriptor::new(FontFamily::Serif, 24).map(|font| text_view.set_font(font));
                text_view.set_tint_color(IndexedColor::DarkGray.into());
                text_view.set_max_lines(2);
                text_view.set_bounds(
                    text_view
                        .size_that_fits(Size::new(rect.width(), isize::MAX))
                        .into(),
                );
                view.add_subview(text_view);

                rect.origin.y += rect.size.height + 10;
                let mut text_view = TextView::with_text("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.");
                text_view.set_frame(rect);
                FontDescriptor::new(FontFamily::SansSerif, 20).map(|font| text_view.set_font(font));
                text_view.set_tint_color(IndexedColor::DarkGray.into());
                text_view.set_max_lines(2);
                text_view.set_bounds(
                    text_view
                        .size_that_fits(Size::new(rect.width(), isize::MAX))
                        .into(),
                );
                view.add_subview(text_view);

                rect.origin.y += rect.size.height + 10;
                let mut text_view = TextView::with_text("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.");
                text_view.set_frame(rect);
                FontDescriptor::new(FontFamily::Cursive, 16).map(|font| text_view.set_font(font));
                text_view.set_tint_color(IndexedColor::DarkGray.into());
                text_view.set_max_lines(2);
                text_view.set_bounds(
                    text_view
                        .size_that_fits(Size::new(rect.width(), isize::MAX))
                        .into(),
                );
                view.add_subview(text_view);

                let vertical_base = Coordinates::from_rect(rect).unwrap().bottom + 20;

                let mut button = Button::new(ButtonType::Default);
                button.set_title("OK");
                button.set_frame(Rect::new(10, vertical_base, 120, 30));
                view.add_subview(button);

                let mut button = Button::new(ButtonType::Normal);
                button.set_title("Cancel");
                button.set_frame(Rect::new(140, vertical_base, 120, 30));
                view.add_subview(button);

                let mut button = Button::new(ButtonType::Destructive);
                button.set_title("Destructive");
                button.set_frame(Rect::new(270, vertical_base, 120, 30));
                view.add_subview(button);
            }

            window.set_active();
        }
    }

    let mut tasks: Vec<Pin<Box<dyn Future<Output = ()>>>> = Vec::new();

    if System::is_headless() {
    } else {
        tasks.push(Box::pin(status_bar_main()));
        tasks.push(Box::pin(activity_monitor_main()));
    }
    tasks.push(Box::pin(repl_main(_main_window)));

    let waker = dummy_waker();
    let mut cx = Context::from_waker(&waker);
    loop {
        for task in &mut tasks {
            let _ = task.as_mut().poll(&mut cx);
        }
        Timer::usleep(100_000);
    }
}

#[allow(dead_code)]
async fn repl_main(_main_window: Option<WindowHandle>) {
    println!("{} v{}", System::name(), System::version(),);

    let mut sb = string::StringBuffer::with_capacity(0x1000);
    loop {
        print!("# ");
        if let Some(cmdline) = stdout().read_line_async(126).await {
            if cmdline.len() > 0 {
                // TODO: A better way
                if cmdline == "cls" {
                    stdout().reset().unwrap();
                } else if cmdline == "memory" {
                    MemoryManager::statistics(&mut sb);
                    print!("{}", sb.as_str());
                } else if cmdline == "ver" {
                    println!("{} v{}", System::name(), System::version(),);
                } else {
                    println!("Command not found: {}", cmdline);
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

#[allow(dead_code)]
async fn status_bar_main() {
    const STATUS_BAR_HEIGHT: isize = 24;
    let bg_color = Color::from_argb(0xC0EEEEEE);
    let fg_color = IndexedColor::DarkGray.into();

    let screen_bounds = WindowManager::main_screen_bounds();
    let window = WindowBuilder::new("Status Bar")
        .style(WindowStyle::NAKED | WindowStyle::FLOATING)
        .style_add(WindowStyle::BORDER)
        .frame(Rect::new(0, 0, screen_bounds.width(), STATUS_BAR_HEIGHT))
        .bg_color(bg_color)
        .build();

    let mut ats = AttributedString::with("My OS", FontManager::title_font(), fg_color);

    window
        .draw(|bitmap| {
            let bounds = bitmap.bounds();
            let size = ats.bounding_size(Size::new(isize::MAX, isize::MAX));
            let rect = Rect::new(
                16,
                (bounds.height() - size.height) / 2,
                size.width,
                size.height,
            );
            ats.draw(&bitmap, rect);
        })
        .unwrap();
    window.show();
    WindowManager::add_screen_insets(EdgeInsets::new(STATUS_BAR_HEIGHT, 0, 0, 0));

    ats.set_font(FontManager::system_font());
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
        ats.set_text(sb.as_str());

        let bounds = window.frame();
        let width = ats.bounding_size(Size::new(isize::MAX, isize::MAX)).width;
        let rect = Rect::new(
            bounds.width() - width - 16,
            (bounds.height() - ats.font().line_height()) / 2,
            width,
            ats.font().line_height(),
        );
        let _ = window.draw(|bitmap| {
            bitmap.fill_rect(rect, bg_color);
            ats.draw(&bitmap, rect);
        });
    }
}

#[allow(dead_code)]
async fn activity_monitor_main() {
    let bg_color = Color::from(IndexedColor::Black).set_opacity(0xC0);
    let fg_color = IndexedColor::Yellow.into();
    let graph_sub_color = IndexedColor::LightGreen.into();
    let graph_main_color = IndexedColor::Yellow.into();
    let graph_border_color = IndexedColor::LightGray.into();

    Timer::sleep_async(Duration::from_millis(2000)).await;

    let window = WindowBuilder::new("Activity Monitor")
        .style_add(WindowStyle::NAKED | WindowStyle::FLOATING | WindowStyle::PINCHABLE)
        .frame(Rect::new(-330, -180, 320, 150))
        .bg_color(bg_color)
        .build();

    window.show();

    let mut ats = AttributedString::new("");
    FontDescriptor::new(FontFamily::SmallSystem, 8).map(|font| ats.set_font(font));
    ats.set_color(fg_color);

    let num_of_cpus = System::num_of_cpus();
    let n_items = 64;
    let mut usage_temp = Vec::with_capacity(num_of_cpus);
    let mut usage_cursor = 0;
    let mut usage_history = {
        let count = num_of_cpus * n_items;
        let mut vec = Vec::with_capacity(count);
        for _ in 0..count {
            vec.push(u8::MAX);
        }
        vec
    };

    let mut sb = string::StringBuffer::with_capacity(0x1000);
    loop {
        MyScheduler::get_idle_statistics(&mut usage_temp);
        for i in 0..num_of_cpus {
            usage_history[i * n_items + usage_cursor] =
                (u32::min(usage_temp[i], 999) * 254 / 999) as u8;
        }
        usage_cursor = (usage_cursor + 1) % n_items;

        MyScheduler::print_statistics(&mut sb, true);
        window
            .draw(|bitmap| {
                bitmap.fill_rect(bitmap.bounds(), bg_color);
                for cpu_index in 0..num_of_cpus {
                    let padding = 4;
                    let item_size = Size::new(
                        isize::min(
                            isize::max(
                                (bitmap.bounds().width() - padding) / num_of_cpus as isize
                                    - padding,
                                16,
                            ),
                            n_items as isize,
                        ),
                        40,
                    );
                    let rect = Rect::new(
                        padding + cpu_index as isize * (item_size.width + padding),
                        padding,
                        item_size.width,
                        item_size.height,
                    );
                    let h_lines = 4;
                    let v_lines = 4;
                    for i in 1..h_lines {
                        let point = Point::new(rect.x(), rect.y() + i * item_size.height / h_lines);
                        bitmap.draw_hline(point, item_size.width, graph_sub_color);
                    }
                    for i in 1..v_lines {
                        let point = Point::new(rect.x() + i * item_size.width / v_lines, rect.y());
                        bitmap.draw_vline(point, item_size.height, graph_sub_color);
                    }

                    let limit = item_size.width as usize - 2;
                    for i in 0..limit {
                        let scale = item_size.height - 2;
                        let value1 = usage_history
                            [cpu_index * n_items + ((usage_cursor + i - limit) % n_items)]
                            as isize
                            * scale
                            / 255;
                        let value2 = usage_history
                            [cpu_index * n_items + ((usage_cursor + i - 1 - limit) % n_items)]
                            as isize
                            * scale
                            / 255;
                        let c0 = Point::new(rect.x() + i as isize + 1, rect.y() + 1 + value1);
                        let c1 = Point::new(rect.x() + i as isize, rect.y() + 1 + value2);
                        bitmap.draw_line(c0, c1, graph_main_color);
                    }
                    bitmap.draw_rect(rect, graph_border_color);
                }
                let rect = bitmap.bounds().insets_by(EdgeInsets::new(48, 4, 4, 4));
                bitmap.fill_rect(rect, bg_color);
                ats.set_text(sb.as_str());
                ats.draw(&bitmap, rect);
            })
            .unwrap();

        Timer::sleep_async(Duration::from_millis(1000)).await;
    }
}
