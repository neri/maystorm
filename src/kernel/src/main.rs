// My OS Entry
// (c) 2020 Nerry
// License: MIT

#![no_std]
#![no_main]
#![feature(asm)]

// use acpi;
use crate::alloc::string::ToString;
use alloc::vec::*;
use arch::cpu::*;
use bootprot::*;
use core::fmt::Write;
use fs::filesys::*;
use io::graphics::*;
use kernel::*;
use mem::memory::*;
use mem::string;
use rt::*;
use system::*;
use task::scheduler::*;
use task::Task;
use uuid::*;
use window::*;
// use core::time::Duration;
// use alloc::boxed::Box;
// use mem::string::*;
// use io::fonts::*;
// use core::sync::atomic::*;

extern crate alloc;
extern crate rlibc;

entry!(main);

fn main() {
    MyScheduler::spawn_async(Task::new(repl_main()));
    // MyScheduler::spawn_async(Task::new(test_task()));
    MyScheduler::perform_tasks();
}

async fn repl_main() {
    exec_cmd("ver");

    loop {
        print!("# ");
        if let Some(cmdline) = stdout().read_line_async(120).await {
            exec_cmd(&cmdline);
        }
    }
}

#[allow(dead_code)]
async fn test_task() {
    let window_size = Size::new(640, 480);
    let window = WindowBuilder::new("MyOS Paint")
        .size(window_size)
        .origin(Point::new(50, 50))
        .default_message_queue()
        .build();

    window.show();

    let canvas = Bitmap::new(
        window_size.width as usize,
        window_size.height as usize,
        false,
    );
    canvas.fill_rect(canvas.size().into(), Color::from_rgb(0xFFFFFF));
    // canvas.draw_rect(canvas.size().into(), Color::from_rgb(0xFF0000));

    let current_pen_radius = 1;
    let current_pen = Color::from(IndexedColor::Black);
    let mut is_drawing = false;
    let mut last_pen = Point::new(0, 0);

    while let Some(message) = window.get_message().await {
        match message {
            WindowMessage::Draw => {
                window
                    .draw(|bitmap| {
                        bitmap.blt(&canvas, Point::new(0, 0), bitmap.bounds(), BltOption::COPY);
                    })
                    .unwrap();
            }
            WindowMessage::Char(c) => match c {
                'c' => {
                    canvas.fill_rect(canvas.bounds(), Color::WHITE);
                    window.set_needs_display();
                }
                _ => (),
            },
            WindowMessage::MouseMove(e) => {
                if is_drawing {
                    let e_point = e.point();
                    last_pen.line_to(e_point, |point| {
                        canvas.fill_circle(point, current_pen_radius, current_pen);
                    });
                    last_pen = e_point;
                    window.set_needs_display();
                }
            }
            WindowMessage::MouseDown(e) => {
                let e_point = e.point();
                canvas.fill_circle(e_point, current_pen_radius, current_pen);
                last_pen = e_point;
                is_drawing = true;
                window.set_needs_display();
            }
            WindowMessage::MouseUp(_e) => {
                is_drawing = false;
            }
            WindowMessage::MouseLeave => {
                is_drawing = false;
            }
            _ => window.handle_default_message(message),
        }
    }
}

#[allow(dead_code)]
fn draw_cursor(bitmap: &Bitmap, point: Point<isize>, color: Color) {
    let size = 7;
    let size2 = size / 2;
    bitmap.draw_vline(Point::new(point.x, point.y - size2), size, color);
    bitmap.draw_hline(Point::new(point.x - size2, point.y), size, color);
}

fn exec_cmd(cmdline: &str) {
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
            None => {
                spawn(cmd, args.as_slice(), true);
            }
        }
    }
}

enum CmdLinePhase {
    LeadingSpace,
    Token,
}

fn spawn(name: &str, argv: &[&str], wait_until: bool) -> usize {
    let _ = argv;

    match Fs::find_file(name) {
        Some((fs, inode)) => {
            let stat = fs.stat(inode).unwrap();
            if stat.file_size > 0 {
                let mut blob = Vec::new();
                blob.resize(stat.block_size, 0);
                fs.x_read(inode, 0, 1, &mut blob);
                if let Some(mut loader) = RuntimeEnvironment::recognize(blob.as_slice()) {
                    if stat.blocks > 1 {
                        blob.resize(stat.blocks * stat.block_size, 0);
                        fs.x_read(inode, 0, stat.blocks, &mut blob);
                    }
                    loader.option().argv = argv.iter().map(|v| v.to_string()).collect();
                    loader.load(blob.as_slice());
                    let child = loader.invoke_start(name);
                    if wait_until {
                        child.map(|thread| thread.join());
                    }
                } else {
                    println!("Bad executable");
                    return 1;
                }
            }
            0
        }
        _ => {
            println!("Command not found: {}", name);
            1
        }
    }
}

fn command(cmd: &str) -> Option<&'static fn(&[&str]) -> isize> {
    for command in &COMMAND_TABLE {
        if command.0 == cmd {
            return Some(&command.1);
        }
    }
    None
}

const COMMAND_TABLE: [(&str, fn(&[&str]) -> isize, &str); 13] = [
    ("help", cmd_help, "Show Help"),
    ("cls", cmd_cls, "Clear screen"),
    ("ver", cmd_ver, "Display version"),
    ("sysctl", cmd_sysctl, "System Control"),
    ("lspci", cmd_lspci, "Show List of PCI Devices"),
    ("uuidgen", cmd_uuidgen, ""),
    ("reboot", cmd_reboot, "Restart computer"),
    ("exit", cmd_reserved, ""),
    ("echo", cmd_echo, ""),
    ("dir", cmd_dir, "Show directory"),
    ("stat", cmd_stat, "Show stat"),
    ("type", cmd_type, "Show file"),
    ("open", cmd_open, "Open program separated"),
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

fn cmd_dir(_argv: &[&str]) -> isize {
    let fs = match Fs::list_of_volumes().first() {
        Some(fs) => fs,
        None => return 1,
    };
    let inode = fs.root_dir();
    for file in fs.read_dir_iter(inode) {
        print!(" {:<14} ", file.name(),);
    }
    let info = fs.info();
    println!(
        "\n {} kb / {} kb",
        (info.free_records as usize * info.bytes_per_record) >> 10,
        (info.total_records as usize * info.bytes_per_record) >> 10,
    );
    0
}

fn cmd_stat(argv: &[&str]) -> isize {
    if argv.len() < 2 {
        println!("usage: stat FILENAME");
        return 1;
    }
    let name = argv[1];

    match Fs::find_file(name) {
        Some((fs, inode)) => {
            let stat = fs.stat(inode).unwrap();
            println!(
                "{} inode {} size {} blk {} {}",
                name, stat.inode, stat.file_size, stat.block_size, stat.blocks
            );
            0
        }
        _ => {
            println!("No such file: {}", name);
            1
        }
    }
}

fn cmd_type(argv: &[&str]) -> isize {
    if argv.len() < 2 {
        println!("usage: type FILENAME");
        return 1;
    }
    let name = argv[1];

    match Fs::find_file(name) {
        Some((fs, inode)) => {
            let stat = fs.stat(inode).unwrap();
            if stat.file_size > 0 {
                let mut buffer = Vec::new();
                buffer.resize(stat.block_size, 0);
                let last_bytes = stat.file_size % stat.block_size;
                for i in 0..(stat.blocks - 1) {
                    fs.x_read(inode, i, 1, &mut buffer);
                    for c in buffer.iter() {
                        stdout().write_char(*c as char).unwrap();
                    }
                }
                fs.x_read(inode, stat.blocks - 1, 1, &mut buffer);
                let buffer = &buffer[..last_bytes];
                for c in buffer.iter() {
                    stdout().write_char(*c as char).unwrap();
                }
            }
            0
        }
        _ => {
            println!("No such file: {}", name);
            1
        }
    }
}

fn cmd_open(argv: &[&str]) -> isize {
    if argv.len() < 2 {
        println!("usage: open PROGRAM [ARGUMENTS ...]");
        return 1;
    }

    let argv = &argv[1..];
    let name = argv[0];
    spawn(name, argv, false);

    0
}

fn cmd_lspci(argv: &[&str]) -> isize {
    let opt_all = argv.len() > 1;
    for device in bus::pci::Pci::devices() {
        let addr = device.address();
        let class_string = find_class_string(device.class_code());
        println!(
            "{:02x}.{:02x}.{} {:04x}:{:04x} {}",
            addr.bus,
            addr.dev,
            addr.fun,
            device.vendor_id().0,
            device.device_id().0,
            class_string,
        );
        if opt_all {
            for function in device.functions() {
                let addr = function.address();
                let class_string = find_class_string(function.class_code());
                println!(
                    "     .{} {:04x}:{:04x} {}",
                    addr.fun,
                    function.vendor_id().0,
                    function.device_id().0,
                    class_string,
                );
            }
        }
    }
    0
}

fn find_class_string(class_code: u32) -> &'static str {
    const CLASS: u32 = 0xFF_00_00;
    const SUB_CLASS: u32 = 0xFF_FF_00;
    const INTERFACE: u32 = u32::MAX;
    let entries = [
        (0x00_00_00, SUB_CLASS, "Non-VGA-Compatible devices"),
        (0x00_01_00, SUB_CLASS, "VGA-Compatible Device"),
        (0x01_00_00, SUB_CLASS, "SCSI Bus Controller"),
        (0x01_01_00, SUB_CLASS, "IDE Controller"),
        (0x01_05_00, SUB_CLASS, "ATA Controller"),
        (0x01_06_01, INTERFACE, "AHCI 1.0"),
        (0x01_06_00, SUB_CLASS, "Serial ATA"),
        (0x01_07_00, INTERFACE, "SAS"),
        (0x01_07_00, SUB_CLASS, "Serial Attached SCSI"),
        (0x01_08_01, INTERFACE, "NVMHCI"),
        (0x01_08_02, INTERFACE, "NVM Express"),
        (0x01_08_00, SUB_CLASS, "Non-Volatile Memory Controller"),
        (0x01_00_00, CLASS, "Mass Storage Controller"),
        (0x02_00_00, SUB_CLASS, "Ethernet Controller"),
        (0x02_00_00, CLASS, "Network Controller"),
        (0x03_00_00, CLASS, "Display Controller"),
        (0x04_00_00, SUB_CLASS, "Multimedia Video Controller"),
        (0x04_01_00, SUB_CLASS, "Multimedia Audio Controller"),
        (0x04_03_00, SUB_CLASS, "Audio Device"),
        (0x04_00_00, CLASS, "Multimedia Controller"),
        (0x05_00_00, CLASS, "Memory Controller"),
        (0x06_00_00, SUB_CLASS, "Host Bridge"),
        (0x06_01_00, SUB_CLASS, "ISA Bridge"),
        (0x06_04_00, SUB_CLASS, "PCI-to-PCI Bridge"),
        (0x06_09_00, SUB_CLASS, "PCI-to-PCI Bridge"),
        (0x06_00_00, CLASS, "Bridge Device"),
        (0x07_00_00, SUB_CLASS, "Serial Controller"),
        (0x07_01_00, SUB_CLASS, "Parallel Controller"),
        (0x07_00_00, CLASS, "Simple Communication Controller"),
        (0x08_00_00, CLASS, "Base System Peripheral"),
        (0x09_00_00, CLASS, "Input Device Controller"),
        (0x0A_00_00, CLASS, "Docking Station"),
        (0x0B_00_00, CLASS, "Processor"),
        (0x0C_03_30, INTERFACE, "XHCI Controller"),
        (0x0C_03_00, SUB_CLASS, "USB Controller"),
        (0x0C_05_00, SUB_CLASS, "SMBus"),
        (0x0C_00_00, CLASS, "Serial Bus Controller"),
        (0x0D_00_00, CLASS, "Wireless Controller"),
        (0x0E_00_00, CLASS, "Intelligent Controller"),
        (0x0F_00_00, CLASS, "Satellite Communication Controller"),
        (0x10_00_00, CLASS, "Encryption Controller"),
        (0x11_00_00, CLASS, "Signal Processing Controller"),
        (0x12_00_00, CLASS, "Processing Accelerator"),
        (0x13_00_00, CLASS, "Non-Essential Instrumentation"),
        (0x40_00_00, CLASS, "Co-Processor"),
        (0xFF_00_00, CLASS, "(Vendor specific)"),
    ];
    for entry in &entries {
        if (class_code & entry.1) == entry.0 {
            return entry.2;
        }
    }
    "(Unknown Device)"
}
