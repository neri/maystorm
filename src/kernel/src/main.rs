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
use mem::string::*;
use system::*;
use task::scheduler::*;
use uuid::*;
use window::*;

extern crate alloc;
extern crate rlibc;

entry!(main);

fn main() {
    let mut tasks: Vec<Pin<Box<dyn Future<Output = ()>>>> = Vec::new();

    tasks.push(Box::pin(repl_main()));
    tasks.push(Box::pin(test_task()));
    tasks.push(Box::pin(test_task()));

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
    Timer::sleep_async(Duration::from_millis(500)).await;

    exec("ver");

    loop {
        print!("# ");
        if let Some(cmdline) = stdout().read_line_async(120).await {
            exec(&cmdline);
        }
    }
}

async fn test_task() {
    let window = WindowBuilder::new("Test")
        .size(Size::new(160, 72))
        .default_message_queue()
        .build();

    let mut sb = Sb255::new();
    // sb.write_str("Hello, Rust!").unwrap();

    window.show();
    // window.set_active();

    while let Some(message) = window.get_message_async().await {
        match message {
            WindowMessage::Key(e) => {
                if let Some(c) = e.key_data().map(|v| v.into_char()) {
                    match c {
                        '\x08' => {
                            sb.backspace();
                            window.invalidate();
                        }
                        _ => {
                            sb.write_char(c).unwrap();
                            window.invalidate();
                        }
                    }
                }
            }
            WindowMessage::Draw => {
                window
                    .draw(|bitmap| {
                        bitmap.fill_rect(bitmap.bounds(), Color::WHITE);
                        AttributedString::with(
                            sb.as_str(),
                            FontDescriptor::new(FontFamily::SansSerif, 24).unwrap(),
                            IndexedColor::Black.into(),
                        )
                        .draw(
                            bitmap,
                            bitmap.bounds().insets_by(EdgeInsets::padding_each(4)),
                        );
                    })
                    .unwrap();
            }
            _ => window.handle_default_message(message),
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
            let cpuid0 = Cpu::cpuid(0x000_0000, 0);
            let cpuid1 = Cpu::cpuid(0x000_0001, 0);
            let cpuid7 = Cpu::cpuid(0x000_0007, 0);
            let cpuid81 = Cpu::cpuid(0x8000_0001, 0);
            println!("CPUID {:08x}", cpuid0.eax());
            println!(
                "Feature 0~1 EDX {:08x} ECX {:08x}",
                cpuid1.edx(),
                cpuid1.ecx(),
            );
            println!(
                "Feature 0~7 EBX {:08x} ECX {:08x} EDX {:08x}",
                cpuid7.ebx(),
                cpuid7.ecx(),
                cpuid7.edx(),
            );
            println!(
                "Feature 8~1 EDX {:08x} ECX {:08x}",
                cpuid81.edx(),
                cpuid81.ecx(),
            );
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
