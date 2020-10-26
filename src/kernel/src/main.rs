// My OS Entry
// (c) 2020 Nerry
// License: MIT

#![no_std]
#![no_main]
#![feature(asm)]

// use acpi;
use alloc::boxed::Box;
use alloc::vec::*;
use arch::cpu::*;
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
use uuid::*;
use window::view::*;
use window::*;

extern crate alloc;
extern crate rlibc;

// use expr::simple_executor::*;
// use expr::*;
// use futures_util::stream::StreamExt;

myos_entry!(main);

fn main() {
    if System::is_headless() {
    } else {
        if false {
            // Test Window 1
            let window = WindowBuilder::new("Welcome")
                .size(Size::new(512, 384))
                .center()
                .build();

            if let Some(view) = window.view() {
                let mut rect = view.bounds();
                rect.size.height = 56;
                let mut shape = View::with_frame(rect);
                // shape.set_background_color(Color::from_rgb(0x64B5F6));
                shape.set_background_color(Color::from_rgb(0x2196F3));
                // shape.set_background_color(Color::from_rgb(0xFF9800));
                view.add_subview(shape);

                let mut rect = view.bounds().insets_by(EdgeInsets::new(16, 16, 0, 16));
                rect.size.height = 44;
                let mut text_view = TextView::with_text("Welcome to My OS !");
                FontDescriptor::new(FontFamily::SansSerif, 32).map(|font| text_view.set_font(font));
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

    // if System::is_headless() {
    // } else {
    // }
    tasks.push(Box::pin(repl_main()));

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
async fn repl_main() {
    Timer::sleep_async(Duration::from_millis(500)).await;

    exec("ver");

    loop {
        print!("# ");
        if let Some(cmdline) = stdout().read_line_async(120).await {
            exec(&cmdline);
        }
    }
}

fn exec(cmdline: &str) {
    if cmdline.len() == 0 {
        return;
    }
    let mut sb = string::StringBuffer::with_capacity(cmdline.len());
    let mut args = Vec::new();
    let mut phase = CmdLinePhase::LeadingSpace;
    sb.clear();
    for c in cmdline.chars() {
        match phase {
            CmdLinePhase::LeadingSpace => match c {
                ' ' => (),
                _ => {
                    sb.write_char(c).unwrap();
                    phase = CmdLinePhase::Token;
                }
            },
            CmdLinePhase::Token => match c {
                ' ' => {
                    args.push(sb.as_str());
                    phase = CmdLinePhase::LeadingSpace;
                    sb.split();
                }
                _ => {
                    sb.write_char(c).unwrap();
                }
            },
        }
    }
    if sb.len() > 0 {
        args.push(sb.as_str());
    }

    if args.len() > 0 {
        let cmd = args[0];
        match command(cmd) {
            Some(exec) => {
                exec(args.as_slice());
            }
            None => println!("Command not found: {}", cmd),
        }
    }
}

enum CmdLinePhase {
    LeadingSpace,
    Token,
}

fn command(cmd: &str) -> Option<&'static fn(&[&str]) -> isize> {
    for command in &COMMAND_TABLE {
        if command.0 == cmd {
            return Some(&command.1);
        }
    }
    None
}

const COMMAND_TABLE: [(&str, fn(&[&str]) -> isize, &str); 9] = [
    ("help", cmd_help, "Show Help"),
    ("cls", cmd_cls, "Clear screen"),
    ("ver", cmd_ver, "Display version"),
    ("sysctl", cmd_sysctl, "System Control"),
    ("lspci", cmd_lspci, "Show List of PCI Devices"),
    ("uuidgen", cmd_uuidgen, ""),
    ("reboot", cmd_reboot, "Restart computer"),
    ("exit", cmd_reserved, ""),
    ("echo", cmd_echo, ""),
];

fn cmd_reserved(_: &[&str]) -> isize {
    println!("Feature not available");
    1
}

fn cmd_reboot(_: &[&str]) -> isize {
    unsafe {
        System::reset();
    }
}

fn cmd_help(_: &[&str]) -> isize {
    for cmd in &COMMAND_TABLE {
        if cmd.2.len() > 0 {
            println!("{}\t{}", cmd.0, cmd.2);
        }
    }
    0
}

fn cmd_cls(_: &[&str]) -> isize {
    match stdout().reset() {
        Ok(_) => 0,
        Err(_) => 1,
    }
}

fn cmd_ver(_: &[&str]) -> isize {
    println!("{} v{}", System::name(), System::version(),);
    0
}

fn cmd_echo(args: &[&str]) -> isize {
    println!("{}", args[1..].join(" "));
    0
}

fn cmd_uuidgen(_: &[&str]) -> isize {
    match Uuid::generate() {
        Some(v) => {
            println!("{}", v);
            return 0;
        }
        None => {
            println!("Feature not available");
            return 1;
        }
    }
}

fn cmd_sysctl(argv: &[&str]) -> isize {
    if argv.len() < 2 {
        println!("usage: sysctl command [options]");
        println!("memory:\tShow memory information");
        return 1;
    }
    let subcmd = argv[1];
    match subcmd {
        "memory" => {
            let mut sb = string::StringBuffer::with_capacity(256);
            MemoryManager::statistics(&mut sb);
            print!("{}", sb.as_str());
        }
        "random" => match Cpu::secure_rand() {
            Ok(rand) => println!("{:016x}", rand),
            Err(_) => println!("# No SecureRandom"),
        },
        "cpuid" => {
            let cpuid0 = Cpu::cpuid(0, 0);
            let cpuid1 = Cpu::cpuid(1, 0);
            let cpuid7 = Cpu::cpuid(7, 0);
            let cpuid81 = Cpu::cpuid(0x8000_0001, 0);
            println!("CPUID {:08x}", cpuid0.eax());
            println!(
                "Feature 0000_0001 EDX {:08x} ECX {:08x}",
                cpuid1.edx(),
                cpuid1.ecx(),
            );
            println!(
                "Feature 0000_0007 EBX {:08x} ECX {:08x} EDX {:08x}",
                cpuid7.ebx(),
                cpuid7.ecx(),
                cpuid7.edx(),
            );
            println!(
                "Feature 8000_0001 EDX {:08x} ECX {:08x}",
                cpuid81.edx(),
                cpuid81.ecx(),
            );
            if cpuid0.eax() >= 0x0B {
                let cpuid0b = Cpu::cpuid(0x0B, 0);
                println!(
                    "CPUID0B: {:08x} {:08x} {:08x} {:08x}",
                    cpuid0b.eax(),
                    cpuid0b.ebx(),
                    cpuid0b.ecx(),
                    cpuid0b.edx()
                );
            }
        }
        _ => {
            println!("Unknown command: {}", subcmd);
            return 1;
        }
    }
    0
}

fn cmd_lspci(argv: &[&str]) -> isize {
    let opt_all = argv.len() > 1;
    for device in bus::pci::Pci::devices() {
        let addr = device.address();
        println!(
            "{:02x}.{:02x}.{} {:04x}:{:04x} {:06x} {}",
            addr.0,
            addr.1,
            addr.2,
            device.vendor_id().0,
            device.device_id().0,
            device.class_code(),
            device.class_string(),
        );
        if opt_all {
            for function in device.functions() {
                let addr = function.address();
                println!(
                    "     .{} {:04x}:{:04x} {:06x} {}",
                    addr.2,
                    function.vendor_id().0,
                    function.device_id().0,
                    function.class_code(),
                    function.class_string(),
                );
            }
        }
    }
    0
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
