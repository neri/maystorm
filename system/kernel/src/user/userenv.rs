// User Environment

use crate::fs::*;
use crate::mem::*;
use crate::system::*;
use crate::task::scheduler::*;
use crate::task::*;
use crate::ui::font::*;
use crate::ui::window::*;
use crate::*;
use crate::{arch::cpu::*, ui::terminal::Terminal};
use ::alloc::vec::*;
use core::fmt::Write;
use core::time::Duration;
use megstd::drawing::*;
use megstd::string::*;
use util::text::{AttributedString, VerticalAlignment};

const DESKTOP_COLOR: SomeColor = SomeColor::from_argb(0xFF2196F3);

pub struct UserEnv {
    _phantom: (),
}

impl UserEnv {
    pub(crate) fn start(f: fn()) {
        {
            WindowManager::set_desktop_color(DESKTOP_COLOR);
            if let Ok(mut file) = FileManager::open("wall.bmp") {
                let stat = file.stat().unwrap();
                let mut vec = Vec::with_capacity(stat.len() as usize);
                file.read_to_end(&mut vec).unwrap();
                if let Some(dib) = ImageLoader::from_msdib(vec.as_slice()) {
                    WindowManager::set_desktop_bitmap(&dib.as_const());
                }
            }
            WindowManager::set_pointer_visible(true);
            Timer::sleep(Duration::from_millis(1000));
        }

        Scheduler::spawn_async(Task::new(status_bar_main()));
        Scheduler::spawn_async(Task::new(activity_monitor_main()));
        Scheduler::spawn_async(Task::new(shell_launcher(f)));
        Scheduler::perform_tasks();
    }
}

async fn shell_launcher(f: fn()) {
    {
        // Main Terminal
        let terminal = Terminal::new(80, 24);
        System::set_stdout(Box::new(terminal));
    }
    SpawnOption::new().start_process(unsafe { core::mem::transmute(f) }, 0, "shell");
}

#[allow(dead_code)]
async fn status_bar_main() {
    const STATUS_BAR_HEIGHT: isize = 24;
    let bg_color = SomeColor::from_argb(0xC0EEEEEE);
    let fg_color = SomeColor::BLACK;

    let screen_bounds = WindowManager::main_screen_bounds();
    let window = WindowBuilder::new("Status Bar")
        .style(WindowStyle::NAKED | WindowStyle::FLOATING)
        .style_add(WindowStyle::BORDER)
        .frame(Rect::new(0, 0, screen_bounds.width(), STATUS_BAR_HEIGHT))
        .bg_color(bg_color)
        .build();

    window
        .draw(|bitmap| {
            let font = FontManager::title_font();
            let ats = AttributedString::new()
                .font(font)
                .color(fg_color)
                .text(System::short_name());
            let rect = Rect::new(16, 0, 320, STATUS_BAR_HEIGHT);
            ats.draw_text(bitmap, rect, 1);
        })
        .unwrap();
    window.show();
    WindowManager::add_screen_insets(EdgeInsets::new(STATUS_BAR_HEIGHT, 0, 0, 0));

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
                    .text(sb.as_str());

                let bounds = window.frame();
                let width = ats
                    .bounding_size(Size::new(isize::MAX, isize::MAX), 1)
                    .width;
                let rect = Rect::new(
                    bounds.width() - width - 16,
                    (bounds.height() - font.line_height()) / 2,
                    width,
                    font.line_height(),
                );
                window
                    .draw_in_rect(rect, |bitmap| {
                        bitmap.fill_rect(bitmap.bounds(), bg_color);
                        ats.draw_text(bitmap, bitmap.bounds(), 1);
                    })
                    .unwrap();

                window.set_needs_display();
            }
            WindowMessage::MouseDown(_) => {
                if let Some(activity) = unsafe { ACTIVITY_WINDOW } {
                    let _ = activity.post(WindowMessage::User(if activity.is_visible() {
                        0
                    } else {
                        1
                    }));
                }
            }
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

async fn activity_monitor_main() {
    let bg_alpha = 0xC0;
    let bg_color32 = TrueColor::from(IndexedColor::BLACK);
    let bg_color = SomeColor::Argb32(bg_color32.set_opacity(bg_alpha));
    let fg_color2 = SomeColor::DARK_GRAY;
    let fg_color = SomeColor::YELLOW;
    let graph_sub_color = SomeColor::from(IndexedColor::LIGHT_GREEN);
    let graph_main_color = SomeColor::from(IndexedColor::YELLOW);
    let graph_border_color = SomeColor::from(IndexedColor::LIGHT_GRAY);
    let margin = EdgeInsets::new(4, 4, 4, 4);

    // let screen_bounds = WindowManager::user_screen_bounds();
    let width = 260;
    let height = 200;
    let window = WindowBuilder::new("Activity Monitor")
        // .style_add(WindowStyle::NAKED)
        .frame(Rect::new(-width - 16, -height - 16, width, height))
        .bg_color(bg_color)
        .build();

    unsafe {
        ACTIVITY_WINDOW = Some(window);
    }

    // window
    //     .draw(|bitmap| {
    //         for i in 0..16 {
    //             let r = i as usize;
    //             let r = r * r;
    //             let origin = Point::new(i, 0);
    //             let color =
    //                 SomeColor::from(bg_color32.set_opacity((bg_alpha as usize * r / 256) as u8));
    //             bitmap.draw_vline(origin, bitmap.height() as isize, color);
    //         }
    //     })
    //     .unwrap();

    window.show();

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
                                    bitmap.draw_line(c0, c1, graph_main_color);
                                }
                                bitmap.draw_rect(rect, graph_border_color);
                            }

                            for cpu_index in 0..num_of_cpus {
                                let padding = 4;
                                let rect = Rect::new(cursor, padding, 8, 32);
                                cursor += rect.width() + padding;

                                let mut coords = Coordinates::from_rect(rect).unwrap();
                                coords.top +=
                                    (rect.height() - 1) * usage_temp[cpu_index] as isize / 1000;

                                bitmap.fill_rect(coords.into(), graph_main_color);
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
                            Scheduler::print_statistics(&mut sb, true);

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
