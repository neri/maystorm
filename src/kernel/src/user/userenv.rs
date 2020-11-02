// User Environment Manager

use crate::arch::cpu::*;
use crate::io::fonts::*;
use crate::io::graphics::*;
use crate::*;
// use crate::kernel::*;
// use crate::mem::memory::*;
use crate::mem::string;
use crate::system::*;
use crate::task::scheduler::*;
use crate::window::*;
use alloc::boxed::Box;
use alloc::vec::*;
use core::fmt::Write;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, RawWaker, RawWakerVTable, Waker};
use core::time::Duration;

const DESKTOP_COLOR: Color = Color::from_argb(0x802196F3);

pub struct UserEnv {
    _phantom: (),
}

impl UserEnv {
    pub(crate) fn start(f: fn()) {
        if System::is_headless() {
            stdout().reset().unwrap();
            f();
        } else {
            {
                let logo_bmp = include_bytes!("logo.bmp");
                let logo = Bitmap::from_msdib(logo_bmp).unwrap();

                let main_text = AttributedString::with(
                    "Starting my OS",
                    FontDescriptor::new(FontFamily::SansSerif, 24).unwrap(),
                    IndexedColor::White.into(),
                );
                let text_size = main_text.bounding_size(Size::new(isize::MAX, isize::MAX));

                let padding = 8;
                let size = Size::new(
                    isize::max(logo.width(), text_size.width) + padding * 2,
                    logo.height() + text_size.height + padding * 3,
                );

                let window = WindowBuilder::new("")
                    .style(WindowStyle::NAKED)
                    .size(size)
                    .bg_color(Color::TRANSPARENT)
                    .build();

                window
                    .draw(|bitmap| {
                        // bitmap.draw_rect(bitmap.bounds(), Color::WHITE);

                        let origin = Point::new(
                            bitmap.bounds().center().x - logo.bounds().center().x,
                            padding,
                        );
                        bitmap.blt(&logo, origin, logo.bounds(), BltOption::COPY);

                        let rect = Rect::new(
                            (bitmap.width() - text_size.width) / 2,
                            logo.height() + padding * 2,
                            text_size.width,
                            text_size.height,
                        );
                        main_text.draw(bitmap, rect);
                    })
                    .unwrap();

                window.set_active();

                Timer::sleep(Duration::from_millis(1000));
                // let max = 10;
                // for i in 0..max {
                //     let color = DESKTOP_COLOR
                //         .blend_each(Color::TRANSPARENT, |a, _b| (a as usize * i / max) as u8);
                //     WindowManager::set_desktop_color(color);
                //     Timer::sleep(Duration::from_millis(50));
                // }

                {
                    let screen_bounds = WindowManager::main_screen_bounds();
                    let bitmap = Bitmap::new(
                        screen_bounds.width() as usize,
                        screen_bounds.height() as usize,
                        false,
                    );

                    bitmap
                        .update_bitmap(|slice| {
                            let rng = XorShift64::default();
                            for color in slice.iter_mut() {
                                *color = if (rng.next() & 1) > 0 {
                                    Color::WHITE
                                } else {
                                    Color::TRANSPARENT
                                }
                            }
                        })
                        .unwrap();
                    bitmap.blur(&bitmap, 4);
                    bitmap.blend_rect(bitmap.bounds(), DESKTOP_COLOR);

                    WindowManager::set_desktop_bitmap(Some(Box::new(bitmap)));
                }

                WindowManager::set_pointer_visible(true);
                Timer::sleep(Duration::from_millis(500));

                // panic!();
                window.hide();
            }

            {
                // Main Terminal
                let (console, window) = GraphicalConsole::new(
                    "Terminal",
                    (80, 24),
                    FontManager::fixed_system_font(),
                    0,
                    0,
                );
                window.move_to(Point::new(16, 40));
                window.set_active();
                System::set_stdout(console);
            }

            let mut tasks: Vec<Pin<Box<dyn Future<Output = ()>>>> = Vec::new();

            tasks.push(Box::pin(status_bar_main()));
            tasks.push(Box::pin(activity_monitor_main()));
            tasks.push(Box::pin(menu_main()));

            SpawnOption::new().spawn(unsafe { core::mem::transmute(f) }, 0, "shell");

            let waker = dummy_waker();
            let mut cx = Context::from_waker(&waker);
            loop {
                for task in &mut tasks {
                    let _ = task.as_mut().poll(&mut cx);
                }
                Timer::usleep(100_000);
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
    let fg_color = IndexedColor::Black.into();

    let screen_bounds = WindowManager::main_screen_bounds();
    let window = WindowBuilder::new("Status Bar")
        .style(WindowStyle::NAKED | WindowStyle::FLOATING)
        .style_add(WindowStyle::BORDER)
        .frame(Rect::new(0, 0, screen_bounds.width(), STATUS_BAR_HEIGHT))
        .bg_color(bg_color)
        .default_message_queue()
        .build();

    window
        .draw(|bitmap| {
            let ats = AttributedString::with("My OS", FontManager::title_font(), fg_color);
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

    let font = FontManager::system_font();
    let mut sb = string::Sb255::new();
    loop {
        window.set_needs_display();
        while let Some(message) = window.consume_message() {
            match message {
                WindowMessage::Draw => {
                    sb.clear();

                    // let usage = MyScheduler::usage_per_cpu();
                    // let usage0 = usage / 10;
                    // let usage1 = usage % 10;
                    // write!(sb, "{:3}.{:1}%  ", usage0, usage1).unwrap();

                    let time = System::system_time();
                    let tod = time.secs % 86400;
                    // let sec = tod % 60;
                    let min = tod / 60 % 60;
                    let hour = tod / 3600;
                    // if sec % 2 == 0 {
                    //     write!(sb, "{:2} {:02} {:02}", hour, min, sec).unwrap();
                    // } else {
                    //     write!(sb, "{:2}:{:02}:{:02}", hour, min, sec).unwrap();
                    // };
                    write!(sb, "{:2}:{:02}", hour, min).unwrap();
                    let ats = AttributedString::with(sb.as_str(), font, fg_color);

                    let bounds = window.frame();
                    let width = ats.bounding_size(Size::new(isize::MAX, isize::MAX)).width;
                    let rect = Rect::new(
                        bounds.width() - width - 16,
                        (bounds.height() - font.line_height()) / 2,
                        width,
                        font.line_height(),
                    );
                    let _ = window.draw(|bitmap| {
                        bitmap.fill_rect(rect, bg_color);
                        ats.draw(&bitmap, rect);
                    });
                }
                WindowMessage::MouseDown(_) => {
                    if let Some(menu) = unsafe { MENU_WINDOW } {
                        let _ =
                            menu.post(WindowMessage::User(if menu.is_visible() { 0 } else { 1 }));
                    }
                }
                _ => window.handle_default_message(message),
            }
        }

        Timer::sleep_async(Duration::from_millis(500)).await;
    }
}

static mut MENU_WINDOW: Option<WindowHandle> = None;

use crate::dev::rng::*;
async fn menu_main() {
    let bg_color = Color::from(IndexedColor::Blue).set_opacity(0x80);
    //Color::from_argb(0x40000000);
    let fg_color = IndexedColor::White.into();

    let screen_bounds = WindowManager::main_screen_bounds();
    let window = WindowBuilder::new("Status Bar")
        .style(WindowStyle::NAKED | WindowStyle::FLOATING)
        .style_add(WindowStyle::OPAQUE)
        // .style_add(WindowStyle::BORDER)
        .frame(screen_bounds.into())
        .bg_color(Color::TRANSPARENT)
        .default_message_queue()
        .build();

    let buffer = Bitmap::new(
        screen_bounds.width() as usize,
        screen_bounds.height() as usize,
        false,
    );
    buffer.reset();
    buffer
        .update_bitmap(|slice| {
            let rng = XorShift64::default();
            for color in slice.iter_mut() {
                // if (rng.next() & 1) > 0 {
                //     *color = Color::WHITE;
                // }
                *color = Color::from_rgb(rng.next() as u32);
            }
        })
        .unwrap();
    buffer.blur(&buffer, 4);
    buffer.blend_rect(buffer.bounds(), bg_color);

    unsafe {
        MENU_WINDOW = Some(window);
    }
    loop {
        while let Some(message) = window.get_message().await {
            match message {
                WindowMessage::Draw => {
                    window
                        .draw(|bitmap| {
                            bitmap.copy_from(&buffer);

                            AttributedString::with(
                                "MyOS Launcher",
                                FontDescriptor::new(FontFamily::SansSerif, 24).unwrap(),
                                fg_color,
                            )
                            .draw(
                                bitmap,
                                bitmap.bounds().insets_by(EdgeInsets::padding_each(32)),
                            );
                            AttributedString::with(
                                "Command not found\n\nPress any key to restart",
                                FontDescriptor::new(FontFamily::SansSerif, 20).unwrap(),
                                fg_color,
                            )
                            .draw(
                                bitmap,
                                bitmap.bounds().insets_by(EdgeInsets::new(120, 64, 64, 32)),
                            );

                            // for i in 0..5 {
                            //     let point = Point::new(48, 72 + 48 * i);
                            //     bitmap.fill_circle(point, 16, IndexedColor::LightBlue.into());
                            //     bitmap.draw_circle(point, 16, Color::WHITE);
                            // }
                        })
                        .unwrap();
                }
                WindowMessage::Char(_) => {
                    let _ = window.post(WindowMessage::User(0));
                }
                WindowMessage::MouseUp(_) => {
                    let _ = window.post(WindowMessage::User(0));
                }
                WindowMessage::User(flag) => {
                    let become_active = flag != 0;
                    if become_active {
                        // WindowManager::save_screen_to(&buffer, buffer.bounds());
                        // buffer.blur(&buffer, 32);
                        window.set_active();
                    } else {
                        window.hide();
                    }
                }
                _ => window.handle_default_message(message),
            }
        }
    }
}

#[allow(dead_code)]
async fn activity_monitor_main() {
    let bg_color = Color::from(IndexedColor::Black).set_opacity(0xC0);
    let fg_color = IndexedColor::Yellow.into();
    let graph_sub_color = IndexedColor::LightGreen.into();
    let graph_main_color = IndexedColor::Yellow.into();
    let graph_border_color = IndexedColor::LightGray.into();

    let window = WindowBuilder::new("Activity Monitor")
        .style_add(WindowStyle::NAKED | WindowStyle::FLOATING | WindowStyle::PINCHABLE)
        .frame(Rect::new(-330, -180, 320, 150))
        .bg_color(bg_color)
        .build();

    window.show();

    let font = FontDescriptor::new(FontFamily::SmallFixed, 8).unwrap_or(FontManager::system_font());

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
    let mut time0 = Timer::measure();
    let mut tsc0 = unsafe { Cpu::read_tsc() };
    loop {
        Timer::sleep_async(Duration::from_millis(1000)).await;
        let time1 = Timer::measure();
        let tsc1 = unsafe { Cpu::read_tsc() };

        MyScheduler::get_idle_statistics(&mut usage_temp);
        for i in 0..num_of_cpus {
            usage_history[i * n_items + usage_cursor] =
                (u32::min(usage_temp[i], 999) * 254 / 999) as u8;
        }
        usage_cursor = (usage_cursor + 1) % n_items;

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
                        32,
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

                sb.clear();
                let hz = ((tsc1 - tsc0) / (time1 - time0) + 5) / 10;
                let hz0 = hz % 100;
                let hz1 = hz / 100;
                let usage = MyScheduler::usage_per_cpu();
                let usage0 = usage % 10;
                let usage1 = usage / 10;
                write!(
                    sb,
                    "CPU: {}.{:02} GHz {:3}.{}% {} Cores {} Threads",
                    hz1,
                    hz0,
                    usage1,
                    usage0,
                    System::num_of_physical_cpus(),
                    System::num_of_cpus(),
                )
                .unwrap();
                let rect = bitmap.bounds().insets_by(EdgeInsets::new(38, 4, 4, 4));
                AttributedString::with(sb.as_str(), font, fg_color).draw(&bitmap, rect);

                MyScheduler::print_statistics(&mut sb, true);
                let rect = bitmap.bounds().insets_by(EdgeInsets::new(48, 4, 4, 4));
                AttributedString::with(sb.as_str(), font, fg_color).draw(&bitmap, rect);
            })
            .unwrap();

        tsc0 = tsc1;
        time0 = time1;
    }
}
