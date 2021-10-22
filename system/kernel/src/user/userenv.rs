// User Environment

use crate::log::EventManager;
use crate::sync::fifo::ConcurrentFifo;
use crate::{
    arch::cpu::*, fs::*, mem::*, system::*, task::scheduler::*, task::*, ui::font::*,
    ui::terminal::Terminal, ui::text::*, ui::theme::Theme, ui::window::*, *,
};
use ::alloc::string::String;
use ::alloc::sync::Arc;
use ::alloc::vec::*;
use core::{fmt::Write, time::Duration};
use megstd::drawing::img::*;
use megstd::drawing::*;
use megstd::string::*;

pub struct UserEnv;

impl UserEnv {
    pub fn start(f: fn()) {
        Scheduler::spawn_async(Task::new(logo_task(f)));
        Scheduler::perform_tasks();
    }
}

#[allow(dead_code)]
async fn logo_task(f: fn()) {
    let width = 320;
    let height = 200;

    WindowManager::set_desktop_color(Theme::shared().desktop_color());
    if true {
        if let Ok(mut file) = FileManager::open("wall.bmp") {
            let stat = file.stat().unwrap();
            let mut vec = Vec::with_capacity(stat.len() as usize);
            file.read_to_end(&mut vec).unwrap();
            if let Some(mut dib) = ImageLoader::from_msdib(vec.as_slice()) {
                WindowManager::set_desktop_bitmap(&dib.into_bitmap());
            }
        }
    }
    WindowManager::set_pointer_visible(true);

    let window = WindowBuilder::new()
        .style_add(WindowStyle::SUSPENDED)
        .style_sub(WindowStyle::CLOSE_BUTTON)
        .style_sub(WindowStyle::TITLE | WindowStyle::BORDER)
        .bg_color(Color::Transparent)
        .size(Size::new(width, height))
        .build("");

    window.draw(|bitmap| {
        AttributedString::new()
            .font(FontDescriptor::new(FontFamily::SansSerif, 24).unwrap())
            .color(Color::WHITE)
            .middle_center()
            .text("Starting up...")
            .draw_text(bitmap, bitmap.bounds(), 0);
    });
    window.show();

    window.create_timer(0, Duration::from_millis(2000));

    while let Some(message) = window.get_message().await {
        match message {
            WindowMessage::Timer(_) => window.close(),
            _ => window.handle_default_message(message),
        }
    }

    Scheduler::spawn_async(Task::new(status_bar_main()));
    Scheduler::spawn_async(Task::new(_notification_task()));
    Scheduler::spawn_async(Task::new(activity_monitor_main()));
    Scheduler::spawn_async(Task::new(shell_launcher(f)));

    // Scheduler::spawn_async(Task::new(test_window_main()));
}

#[allow(dead_code)]
async fn shell_launcher(f: fn()) {
    {
        // Main Terminal
        let main_screen = System::main_screen();
        let font = if main_screen.width() > 1024 && main_screen.height() > 600 {
            FontManager::system_font()
        } else {
            FontDescriptor::new(FontFamily::Terminal, 0).unwrap()
        };
        let terminal = Terminal::new(80, 24, font);
        System::set_stdout(Box::new(terminal));
    }
    SpawnOption::new().start_process(unsafe { core::mem::transmute(f) }, 0, "shell");
}

#[allow(dead_code)]
async fn status_bar_main() {
    const STATUS_BAR_HEIGHT: isize = 40;
    const STATUS_BAR_RADIUS: isize = 16;
    const STATUS_BAR_PADDING: EdgeInsets = EdgeInsets::new(8, 16, 0, 16);
    const INNER_PADDING: EdgeInsets = EdgeInsets::new(1, 16, 1, 16);

    let bg_color = Theme::shared()
        // .window_title_active_background_dark();
        .status_bar_background();
    let fg_color = Theme::shared()
        // .window_title_active_foreground_dark();
        .status_bar_foreground();
    let border_color = Theme::shared().window_default_border_dark();

    let screen_bounds = WindowManager::main_screen_bounds();
    let window = WindowBuilder::new()
        // .style(WindowStyle::FLOATING | WindowStyle::NO_SHADOW)
        .style(WindowStyle::FLOATING | WindowStyle::SUSPENDED)
        .frame(Rect::new(0, 0, screen_bounds.width(), STATUS_BAR_HEIGHT))
        .bg_color(Color::Transparent)
        .build("Status Bar");

    window
        .draw_in_rect(
            Rect::from(window.content_size()).insets_by(STATUS_BAR_PADDING),
            |bitmap| {
                bitmap.fill_round_rect(bitmap.bounds(), STATUS_BAR_RADIUS, bg_color);
                bitmap.draw_round_rect(bitmap.bounds(), STATUS_BAR_RADIUS, border_color);

                let font = FontManager::title_font();
                let ats = AttributedString::new()
                    .font(font)
                    .color(fg_color)
                    .middle_left()
                    .text(System::short_name());
                let rect = Rect::new(INNER_PADDING.left, 0, 320, bitmap.height() as isize);
                ats.draw_text(bitmap, rect, 1);
            },
        )
        .unwrap();
    WindowManager::add_screen_insets(EdgeInsets::new(STATUS_BAR_HEIGHT, 0, 0, 0));
    window.show();

    let font = FontManager::system_font();
    let mut sb = Sb255::new();

    let interval = Duration::from_millis(500);
    window.create_timer(0, interval);
    while let Some(message) = window.get_message().await {
        match message {
            WindowMessage::Timer(_) => {
                window.create_timer(0, interval);

                sb.clear();

                let time = System::system_time();
                let tod = time.secs % 86400;
                let min = tod / 60 % 60;
                let hour = tod / 3600;
                if true {
                    let sec = tod % 60;
                    if sec % 2 == 0 {
                        write!(sb, "{:2} {:02} {:02}", hour, min, sec).unwrap();
                    } else {
                        write!(sb, "{:2}:{:02}:{:02}", hour, min, sec).unwrap();
                    };
                } else {
                    write!(sb, "{:2}:{:02}", hour, min).unwrap();
                }
                let ats = AttributedString::new()
                    .font(font)
                    .color(fg_color)
                    .middle_right()
                    .text(sb.as_str());

                let bounds = Rect::from(window.content_size())
                    .insets_by(STATUS_BAR_PADDING)
                    .insets_by(INNER_PADDING);
                let width = ats
                    .bounding_size(Size::new(isize::MAX, isize::MAX), 1)
                    .width;
                let rect = Rect::new(
                    bounds.max_x() - width,
                    bounds.min_y(),
                    width,
                    bounds.height(),
                );
                window
                    .draw_in_rect(rect, |bitmap| {
                        bitmap.fill_rect(bitmap.bounds(), bg_color);
                        ats.draw_text(bitmap, bitmap.bounds(), 1);
                    })
                    .unwrap();

                window.set_needs_display();
            }
            // WindowMessage::MouseDown(_) => {
            //     if let Some(activity) = unsafe { ACTIVITY_WINDOW } {
            //         let _ = activity.post(WindowMessage::User(if activity.is_visible() {
            //             0
            //         } else {
            //             1
            //         }));
            //     }
            // }
            _ => window.handle_default_message(message),
        }
    }
}

static mut ACTIVITY_WINDOW: Option<WindowHandle> = None;

fn format_bytes(sb: &mut dyn Write, val: usize) -> core::fmt::Result {
    let kb = (val >> 10) & 0x3FF;
    let mb = (val >> 20) & 0x3FF;
    let gb = val >> 30;

    if gb >= 10 {
        // > 10G
        write!(sb, "{:4}G", gb)
    } else if gb >= 1 {
        // 1G~10G
        let mb0 = (mb * 100) >> 10;
        write!(sb, "{}.{:02}G", gb, mb0)
    } else if mb >= 100 {
        // 100M~1G
        write!(sb, "{:4}M", mb)
    } else if mb >= 10 {
        // 10M~100M
        let kb00 = (kb * 10) >> 10;
        write!(sb, "{:2}.{}M", mb, kb00)
    } else if mb >= 1 {
        // 1M~10M
        let kb0 = (kb * 100) >> 10;
        write!(sb, "{}.{:02}M", mb, kb0)
    } else if kb >= 100 {
        // 100K~1M
        write!(sb, "{:4}K", kb)
    } else if kb >= 10 {
        // 10K~100K
        let b00 = ((val & 0x3FF) * 10) >> 10;
        write!(sb, "{:2}.{}K", kb, b00)
    } else {
        // 0~10K
        write!(sb, "{:5}", val)
    }
}

#[allow(dead_code)]
async fn activity_monitor_main() {
    let bg_alpha = 0xE0;
    let bg_color32 = TrueColor::from(IndexedColor::BLACK);
    let bg_color = Color::Argb32(bg_color32.with_opacity(bg_alpha));
    let fg_color2 = Color::DARK_GRAY;
    let fg_color = Color::YELLOW;
    let graph_border_color = Color::LIGHT_GRAY;
    let graph_sub_color = Color::LIGHT_GREEN;
    let graph_main_color1 = Color::LIGHT_RED;
    let graph_main_color2 = Color::YELLOW;
    let graph_main_color3 = Color::LIGHT_GREEN;
    let margin = EdgeInsets::new(0, 0, 0, 0);

    let width = 260;
    let height = 180;
    let window = WindowBuilder::new()
        .style_sub(WindowStyle::CLOSE_BUTTON)
        .frame(Rect::new(-width - 16, -height - 16, width, height))
        .bg_color(bg_color)
        .build("Activity Monitor");

    unsafe {
        ACTIVITY_WINDOW = Some(window);
    }

    let font = FontDescriptor::new(FontFamily::SmallFixed, 8).unwrap_or(FontManager::system_font());

    let num_of_cpus = System::current_device().num_of_active_cpus();
    let n_items = 64;
    let mut usage_temp = Vec::with_capacity(num_of_cpus);
    let mut usage_cursor = 0;
    let mut usage_history = {
        let mut vec = Vec::with_capacity(n_items);
        vec.resize(n_items, u8::MAX);
        vec
    };

    let mut sb = StringBuffer::with_capacity(0x1000);
    let mut time0 = Timer::measure();
    let mut tsc0 = unsafe { Cpu::read_tsc() };

    let interval = Duration::from_secs(1);
    window.create_timer(0, interval);
    while let Some(message) = window.get_message().await {
        match message {
            WindowMessage::Timer(_) => {
                let time1 = Timer::measure();
                let tsc1 = unsafe { Cpu::read_tsc() };

                Scheduler::get_idle_statistics(&mut usage_temp);
                let max_value = num_of_cpus as u32 * 1000;
                usage_history[usage_cursor] = (254
                    * u32::min(max_value, usage_temp.iter().fold(0, |acc, v| acc + *v))
                    / max_value) as u8;
                usage_cursor = (usage_cursor + 1) % n_items;

                window
                    .draw_in_rect(
                        Rect::from(window.content_size()).insets_by(margin),
                        |bitmap| {
                            bitmap.fill_rect(bitmap.bounds(), bg_color);

                            let spacing = 4;
                            let mut cursor;

                            {
                                let spacing = 4;
                                let item_size = Size::new(n_items as isize, 32);
                                let rect =
                                    Rect::new(spacing, spacing, item_size.width, item_size.height);
                                cursor = rect.x() + rect.width() + spacing;

                                let h_lines = 4;
                                let v_lines = 4;
                                for i in 1..h_lines {
                                    let point = Point::new(
                                        rect.x(),
                                        rect.y() + i * item_size.height / h_lines,
                                    );
                                    bitmap.draw_hline(point, item_size.width, graph_sub_color);
                                }
                                for i in 1..v_lines {
                                    let point = Point::new(
                                        rect.x() + i * item_size.width / v_lines,
                                        rect.y(),
                                    );
                                    bitmap.draw_vline(point, item_size.height, graph_sub_color);
                                }

                                let limit = item_size.width as usize - 2;
                                for i in 0..limit {
                                    let scale = item_size.height - 2;
                                    let value1 = usage_history
                                        [((usage_cursor + i - limit) % n_items)]
                                        as isize
                                        * scale
                                        / 255;
                                    let value2 = usage_history
                                        [((usage_cursor + i - 1 - limit) % n_items)]
                                        as isize
                                        * scale
                                        / 255;
                                    let c0 = Point::new(
                                        rect.x() + i as isize + 1,
                                        rect.y() + 1 + value1,
                                    );
                                    let c1 =
                                        Point::new(rect.x() + i as isize, rect.y() + 1 + value2);
                                    bitmap.draw_line(c0, c1, graph_main_color2);
                                }
                                bitmap.draw_rect(rect, graph_border_color);
                            }

                            for cpu_index in 0..num_of_cpus {
                                let padding = 4;
                                let rect = Rect::new(cursor, padding, 8, 32);
                                cursor += rect.width() + padding;

                                let value = usage_temp[cpu_index];
                                let graph_color = if value < 250 {
                                    graph_main_color1
                                } else if value < 750 {
                                    graph_main_color2
                                } else {
                                    graph_main_color3
                                };

                                let mut coords = Coordinates::from_rect(rect).unwrap();
                                coords.top += (rect.height() - 1) * value as isize / 1000;

                                bitmap.fill_rect(coords.into(), graph_color);
                                bitmap.draw_rect(rect, graph_border_color);
                            }

                            sb.clear();

                            let device = System::current_device();

                            write!(sb, "Memory ").unwrap();
                            format_bytes(&mut sb, device.total_memory_size()).unwrap();
                            write!(sb, "B, ").unwrap();
                            format_bytes(&mut sb, MemoryManager::free_memory_size()).unwrap();
                            write!(sb, "B Free, ").unwrap();
                            format_bytes(
                                &mut sb,
                                device.total_memory_size()
                                    - MemoryManager::free_memory_size()
                                    - MemoryManager::reserved_memory_size(),
                            )
                            .unwrap();
                            writeln!(sb, "B Used").unwrap();

                            let hz = ((tsc1 - tsc0) as usize / (time1.0 - time0.0) + 5) / 10;
                            let hz0 = hz % 100;
                            let hz1 = hz / 100;
                            let usage = Scheduler::usage_per_cpu();
                            let usage0 = usage % 10;
                            let usage1 = usage / 10;
                            writeln!(
                                sb,
                                "CPU: {}.{:02} GHz {:3}.{}% {} Cores {} Threads",
                                hz1,
                                hz0,
                                usage1,
                                usage0,
                                device.num_of_performance_cpus(),
                                device.num_of_active_cpus(),
                            )
                            .unwrap();
                            Scheduler::print_statistics(&mut sb);

                            let mut rect = bitmap
                                .bounds()
                                .insets_by(EdgeInsets::new(38, spacing, 4, spacing));
                            rect.origin += Point::new(1, 1);
                            AttributedString::new()
                                .font(font)
                                .color(fg_color2)
                                .valign(VerticalAlignment::Top)
                                .text(sb.as_str())
                                .draw_text(bitmap, rect, 0);
                            rect.origin += Point::new(-1, -1);
                            AttributedString::new()
                                .font(font)
                                .color(fg_color)
                                .valign(VerticalAlignment::Top)
                                .text(sb.as_str())
                                .draw_text(bitmap, rect, 0);
                        },
                    )
                    .unwrap();

                tsc0 = tsc1;
                time0 = time1;
                window.set_needs_display();
                window.create_timer(0, interval);
            }
            WindowMessage::User(flag) => {
                let become_active = flag != 0;
                if become_active {
                    window.show();
                } else {
                    window.hide();
                }
            }
            _ => window.handle_default_message(message),
        }
    }
}

/// Simple Notification Task
async fn _notification_task() {
    let padding = 8;
    let radius = 8;
    let bg_color = Color::from_argb(0xE0FFF9C4);
    let fg_color = Color::BLACK;
    let border_color = Color::from_rgb(0xCBC693);
    let window_width = 280;
    let window_height = 90;
    let screen_bounds = WindowManager::user_screen_bounds();

    let window = WindowBuilder::new()
        .style(WindowStyle::FLOATING | WindowStyle::SUSPENDED)
        .level(WindowLevel::POPUP)
        .frame(Rect::new(
            screen_bounds.max_x() - window_width,
            screen_bounds.min_y(),
            window_width,
            window_height,
        ))
        .bg_color(Color::TRANSPARENT)
        .build("Notification Center");

    let message_buffer = Arc::new(ConcurrentFifo::with_capacity(100));
    Scheduler::spawn_async(Task::new(_notification_observer(
        window,
        message_buffer.clone(),
    )));

    let dismiss_time = Duration::from_millis(5000);
    let mut last_timer = Timer::new(dismiss_time);

    while let Some(message) = window.get_message().await {
        match message {
            WindowMessage::Timer(_) => {
                if last_timer.is_expired() {
                    window.hide();
                }
            }
            WindowMessage::User(_) => {
                if let Some(message) = message_buffer.dequeue() {
                    window
                        .draw_in_rect(Rect::from(window.content_size()), |bitmap| {
                            bitmap.clear();
                            let rect = bitmap.bounds().insets_by(EdgeInsets::padding_each(padding));
                            bitmap.fill_round_rect(rect, radius, bg_color);
                            bitmap.draw_round_rect(rect, radius, border_color);

                            let rect2 = rect.insets_by(EdgeInsets::padding_each(padding));
                            let ats = AttributedString::new()
                                .font(FontDescriptor::new(FontFamily::SansSerif, 14).unwrap())
                                .color(fg_color)
                                .center()
                                .text(message.as_str());
                            ats.draw_text(bitmap, rect2, 0);
                        })
                        .unwrap();
                    window.show();
                    last_timer = Timer::new(dismiss_time);
                    window.create_timer(0, dismiss_time);
                }
            }
            _ => window.handle_default_message(message),
        }
    }
}

async fn _notification_observer(window: WindowHandle, buffer: Arc<ConcurrentFifo<String>>) {
    // Timer::sleep_async(Duration::from_millis(1000)).await;
    while let Some(message) = EventManager::monitor_notification().await {
        buffer.enqueue(message).unwrap();
        window.post(WindowMessage::User(0)).unwrap();
        Timer::sleep_async(Duration::from_millis(3000)).await;
    }
}

#[allow(dead_code)]
async fn test_window_main() {
    let bg_color = Color::from_argb(0x80FFFFFF);
    // Timer::sleep_async(Duration::from_millis(500)).await;

    let width = 480;
    let height = 360;
    let window = WindowBuilder::new()
        .size(Size::new(width, height))
        .bg_color(bg_color)
        .inactive_title_color(bg_color)
        .active_title_color(Color::LIGHT_BLUE)
        .level(WindowLevel::POPUP)
        .build("Welcome");
    window.set_back_button_enabled(true);

    window.draw(|bitmap| {
        bitmap.fill_round_rect(bitmap.bounds(), 4, Color::WHITE);
        bitmap.draw_round_rect(bitmap.bounds(), 4, Color::LIGHT_GRAY);

        // let radius = 4;
        // bitmap.fill_round_rect(bitmap.bounds(), radius, Color::WHITE);
        // bitmap.draw_round_rect(bitmap.bounds(), radius, Color::LIGHT_GRAY);

        let font = FontManager::title_font();
        let title_height = 0;
        let button_width = 120;
        let button_height = 28;
        let button_radius = 8;
        let padding = 8;
        let padding_bottom = button_height;
        let button_center_top = Point::new(
            bitmap.bounds().mid_x(),
            bitmap.bounds().max_y() - padding_bottom - padding,
        );
        // {
        //     let mut rect = bitmap.bounds();
        //     rect.size.height = title_height;
        //     bitmap
        //         .view(rect, |mut bitmap| {
        //             let rect = bitmap.bounds();
        //             bitmap.fill_rect(rect, Color::LIGHT_BLUE);
        //             AttributedString::new()
        //                 .font(FontDescriptor::new(FontFamily::SansSerif, 32).unwrap())
        //                 .middle_center()
        //                 .color(Color::WHITE)
        //                 .text("Welcome to MYOS!")
        //                 .draw_text(&mut bitmap, rect, 1);
        //         })
        //         .unwrap();
        // }
        {
            let rect = bitmap.bounds().insets_by(EdgeInsets::new(
                title_height + padding,
                4,
                padding_bottom + padding + padding,
                4,
            ));
            bitmap
                .view(rect, |mut bitmap| {
                    let mut offset = 0;
                    for family in [
                        FontFamily::SansSerif,
                        FontFamily::SystemUI,
                        FontFamily::Serif,
                        // FontFamily::Cursive,
                        // FontFamily::Japanese,
                    ] {
                        for point in [32, 28, 24, 20, 16, 14, 12, 10, 8] {
                            offset +=
                                font_test(&mut bitmap, offset, Color::BLACK, family, point, 1);
                        }
                    }
                })
                .unwrap();
        }
        if true {
            let rect = Rect::new(
                button_center_top.x() - button_width - padding / 2,
                button_center_top.y(),
                button_width,
                button_height,
            );
            bitmap
                .view(rect, |mut bitmap| {
                    let rect = bitmap.bounds();
                    bitmap.fill_round_rect(
                        rect,
                        button_radius,
                        Theme::shared().button_default_background(),
                    );
                    // bitmap.draw_round_rect(
                    //     rect,
                    //     button_radius,
                    //     Theme::shared().button_default_border(),
                    // );
                    AttributedString::new()
                        .font(font)
                        .middle_center()
                        .color(Theme::shared().button_default_foreground())
                        .text("Ok")
                        .draw_text(&mut bitmap, rect, 1);
                })
                .unwrap();
        }
        if true {
            let rect = Rect::new(
                button_center_top.x() + padding / 2,
                button_center_top.y(),
                button_width,
                button_height,
            );
            bitmap
                .view(rect, |mut bitmap| {
                    let rect = bitmap.bounds();
                    bitmap.fill_round_rect(
                        rect,
                        button_radius,
                        Theme::shared().button_destructive_background(),
                    );
                    // bitmap.draw_round_rect(
                    //     rect,
                    //     button_radius,
                    //     Theme::shared().button_destructive_border(),
                    // );
                    AttributedString::new()
                        .font(font)
                        .middle_center()
                        .color(Theme::shared().button_destructive_foreground())
                        .text("Cancel")
                        .draw_text(&mut bitmap, rect, 1);
                })
                .unwrap();
        }
    });

    // WindowManager::set_barrier_opacity(0x80);

    while let Some(message) = window.get_message().await {
        match message {
            WindowMessage::Close => {
                WindowManager::set_barrier_opacity(0);
                window.close();
                return;
            }
            _ => window.handle_default_message(message),
        }
    }
}

fn font_test(
    bitmap: &mut Bitmap,
    offset: isize,
    color: Color,
    family: FontFamily,
    point: isize,
    max_lines: usize,
) -> isize {
    let font = FontDescriptor::new(family, point).unwrap();
    let rect = Rect::new(0, offset, bitmap.width() as isize, isize::MAX);

    let ats = AttributedString::new()
        .font(font)
        .top_left()
        .color(color)
        .text("The quick brown fox jumps over the lazy dog.");
    // .text("AVATAR Lorem ipsum dolor sit amet,");

    let bounds = ats.bounding_size(rect.size(), max_lines);
    ats.draw_text(bitmap, rect, max_lines);

    bounds.height()
}
