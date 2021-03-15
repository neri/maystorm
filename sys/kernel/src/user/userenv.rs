// User Environment Manager

use crate::arch::cpu::*;
use crate::drawing::*;
use crate::fonts::*;
use crate::mem::string;
use crate::system::*;
use crate::task::scheduler::*;
use crate::task::*;
use crate::util::rng::*;
use crate::window::*;
use crate::*;
use alloc::vec::*;
use core::fmt::Write;
use core::time::Duration;
use util::text::{AttributedString, VerticalAlignment};

const DESKTOP_COLOR: AmbiguousColor = AmbiguousColor::from_argb(0x802196F3);

pub struct UserEnv {
    _phantom: (),
}

impl UserEnv {
    pub(crate) fn start(f: fn()) {
        {
            let screen_bounds = WindowManager::main_screen_bounds();
            let mut bitmap = BoxedBitmap32::new(screen_bounds.size(), TrueColor::TRANSPARENT);

            {
                let slice = bitmap.slice_mut();
                let rng = XorShift64::default();
                for color in slice.iter_mut() {
                    *color = if (rng.next() & 1) > 0 {
                        TrueColor::WHITE
                    } else {
                        TrueColor::TRANSPARENT
                    }
                }
            }
            // bitmap.blur(&bitmap, 4);
            // bitmap.blend_rect(bitmap.bounds(), DESKTOP_COLOR);

            // WindowManager::set_desktop_bitmap(Some(Box::new(bitmap)));
            WindowManager::set_desktop_color(DESKTOP_COLOR);
            WindowManager::set_pointer_visible(true);
            Timer::sleep(Duration::from_millis(1000));
        }

        // {
        //     // Main Terminal
        //     let (console, window) =
        //         GraphicalConsole::new("Terminal", (80, 24), FontManager::fixed_system_font(), 0, 0);
        //     window.move_to(Point::new(16, 40));
        //     window.make_active();
        //     System::set_stdout(console);
        // }

        SpawnOption::new().spawn(unsafe { core::mem::transmute(f) }, 0, "shell");

        Scheduler::spawn_async(Task::new(status_bar_main()));
        Scheduler::spawn_async(Task::new(activity_monitor_main()));
        Scheduler::perform_tasks();
    }
}

#[allow(dead_code)]
async fn status_bar_main() {
    const STATUS_BAR_HEIGHT: isize = 24;
    let bg_color = AmbiguousColor::from_argb(0xC0EEEEEE);
    let fg_color = AmbiguousColor::BLACK;

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
            let ats = AttributedString::props()
                .font(font)
                .color(fg_color)
                .text(System::short_name());
            let rect = Rect::new(16, 0, isize::MAX, STATUS_BAR_HEIGHT);
            ats.draw_text(bitmap, rect, 1);
        })
        .unwrap();
    window.show();
    WindowManager::add_screen_insets(EdgeInsets::new(STATUS_BAR_HEIGHT, 0, 0, 0));

    let font = FontManager::system_font();
    let mut sb = string::Sb255::new();

    let interval = Duration::from_millis(500);
    window.create_timer(0, interval);
    while let Some(message) = window.get_message().await {
        match message {
            WindowMessage::Timer(_) => {
                window.set_needs_display();
                window.create_timer(0, interval);
            }
            WindowMessage::Draw => {
                sb.clear();

                // let usage = MyScheduler::usage_per_cpu();
                // let usage0 = usage / 10;
                // let usage1 = usage % 10;
                // write!(sb, "{:3}.{:1}%  ", usage0, usage1).unwrap();

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
                let ats = AttributedString::props()
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
                    .draw(|bitmap| {
                        bitmap.fill_rect(rect, bg_color);
                        ats.draw_text(bitmap, rect, 1);
                    })
                    .unwrap();
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

async fn activity_monitor_main() {
    let bg_color = AmbiguousColor::Argb32(TrueColor::from(IndexedColor::BLACK).set_opacity(0xC0));
    let fg_color = AmbiguousColor::from(IndexedColor::YELLOW);
    let graph_sub_color = AmbiguousColor::from(IndexedColor::LIGHT_GREEN);
    let graph_main_color = AmbiguousColor::from(IndexedColor::YELLOW);
    let graph_border_color = AmbiguousColor::from(IndexedColor::LIGHT_GRAY);

    let screen_bounds = WindowManager::user_screen_bounds();
    let width = 280;
    let window = WindowBuilder::new("Activity Monitor")
        .style(WindowStyle::NAKED)
        .level(WindowLevel::DESKTOP_ITEMS)
        .frame(Rect::new(
            screen_bounds.x() - width,
            screen_bounds.y(),
            width,
            screen_bounds.height(),
        ))
        .bg_color(bg_color)
        .build();

    unsafe {
        ACTIVITY_WINDOW = Some(window);
    }

    // window.show();

    let font = FontDescriptor::new(FontFamily::SmallFixed, 8).unwrap_or(FontManager::system_font());

    let num_of_cpus = System::num_of_cpus();
    let n_items = 64;
    let mut usage_temp = Vec::with_capacity(num_of_cpus);
    let mut usage_cursor = 0;
    let mut usage_history = {
        let mut vec = Vec::with_capacity(n_items);
        vec.resize(n_items, u8::MAX);
        vec
    };

    let mut sb = string::StringBuffer::with_capacity(0x1000);
    let mut time0 = Timer::measure();
    let mut tsc0 = unsafe { Cpu::read_tsc() };

    let interval = Duration::from_secs(1);
    window.create_timer(0, interval);
    while let Some(message) = window.get_message().await {
        match message {
            WindowMessage::Timer(_) => {
                window.set_needs_display();
                window.create_timer(0, interval);
            }
            WindowMessage::Draw => {
                let time1 = Timer::measure();
                let tsc1 = unsafe { Cpu::read_tsc() };

                Scheduler::get_idle_statistics(&mut usage_temp);
                let max_value = num_of_cpus as u32 * 1000;
                usage_history[usage_cursor] = (254
                    * u32::min(max_value, usage_temp.iter().fold(0, |acc, v| acc + *v))
                    / max_value) as u8;
                usage_cursor = (usage_cursor + 1) % n_items;

                window
                    .draw(|bitmap| {
                        bitmap.fill_rect(bitmap.bounds(), bg_color);

                        let mut cursor;

                        {
                            let padding = 4;
                            let item_size = Size::new(n_items as isize, 32);
                            let rect =
                                Rect::new(padding, padding, item_size.width, item_size.height);
                            cursor = rect.x() + rect.width() + padding;

                            let h_lines = 4;
                            let v_lines = 4;
                            for i in 1..h_lines {
                                let point =
                                    Point::new(rect.x(), rect.y() + i * item_size.height / h_lines);
                                bitmap.draw_hline(point, item_size.width, graph_sub_color);
                            }
                            for i in 1..v_lines {
                                let point =
                                    Point::new(rect.x() + i * item_size.width / v_lines, rect.y());
                                bitmap.draw_vline(point, item_size.height, graph_sub_color);
                            }

                            let limit = item_size.width as usize - 2;
                            for i in 0..limit {
                                let scale = item_size.height - 2;
                                let value1 = usage_history[((usage_cursor + i - limit) % n_items)]
                                    as isize
                                    * scale
                                    / 255;
                                let value2 = usage_history
                                    [((usage_cursor + i - 1 - limit) % n_items)]
                                    as isize
                                    * scale
                                    / 255;
                                let c0 =
                                    Point::new(rect.x() + i as isize + 1, rect.y() + 1 + value1);
                                let c1 = Point::new(rect.x() + i as isize, rect.y() + 1 + value2);
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
                            System::num_of_performance_cpus(),
                            System::num_of_cpus(),
                        )
                        .unwrap();
                        Scheduler::print_statistics(&mut sb, true);

                        let rect = bitmap.bounds().insets_by(EdgeInsets::new(38, 4, 4, 4));
                        AttributedString::props()
                            .font(font)
                            .color(fg_color)
                            .valign(VerticalAlignment::Top)
                            .text(sb.as_str())
                            .draw_text(bitmap, rect, 0);
                    })
                    .unwrap();

                tsc0 = tsc1;
                time0 = time1;
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
