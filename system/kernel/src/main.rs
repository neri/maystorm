// MEG-OS Kernel
// (c) 2020 Nerry
// License: MIT

#![no_std]
#![no_main]
#![feature(asm)]

use alloc::string::*;
use alloc::vec::*;
use bootprot::*;
use core::fmt::Write;
use kernel::fs::*;
use kernel::mem::*;
use kernel::rt::*;
use kernel::system::*;
use kernel::task::scheduler::*;
use kernel::task::Task;
use kernel::*;
use kernel::{arch::cpu::*, bus::pci::PciClass};
use megstd::string::*;

extern crate alloc;

/// Kernel entry point
#[no_mangle]
unsafe fn _start(info: &BootInfo) -> ! {
    system::System::init(info, Shell::start);
}

static mut MAIN: Shell = Shell::new();

pub struct Shell {
    path_ext: Vec<String>,
}

enum ParsedCmdLine {
    Empty,
    InvalidQuote,
}

impl Shell {
    const fn new() -> Self {
        Self {
            path_ext: Vec::new(),
        }
    }

    fn shared<'a>() -> &'a mut Self {
        unsafe { &mut MAIN }
    }

    // Shell entry point
    fn start() {
        let shared = Self::shared();
        for ext in RuntimeEnvironment::supported_extensions() {
            shared.path_ext.push(ext.to_string());
        }

        Scheduler::spawn_async(Task::new(Self::repl_main()));
        Scheduler::perform_tasks();
    }

    async fn repl_main() {
        Self::exec_cmd("ver");

        let device = System::current_device();
        println!(
            "Processor {} Cores / {} Threads, Memory {} MB",
            device.num_of_performance_cpus(),
            device.num_of_active_cpus(),
            device.total_memory_size() >> 20,
        );

        let manufacturer_name = device.manufacturer_name();
        let model_name = device.model_name();
        if manufacturer_name.is_some() || model_name.is_some() {
            println!("Manufacturer: {}", manufacturer_name.unwrap_or("Unknown"),);
            println!("Model: {}", model_name.unwrap_or("Unknown"),);
        }

        loop {
            print!("# ");
            if let Ok(cmdline) = System::stdout().read_line_async(120).await {
                Self::exec_cmd(&cmdline);
            }
        }
    }

    fn exec_cmd(cmdline: &str) {
        match Self::parse_cmd(&cmdline, |name, args| match name {
            "clear" | "cls" => System::stdout().reset().unwrap(),
            "cd" | "exit" => println!("Feature not available"),
            // "dir" => Self::cmd_dir(args),
            // "type" => Self::cmd_type(stdout, args),
            "echo" => {
                let stdout = System::stdout();
                for (index, word) in args.iter().skip(1).enumerate() {
                    if index > 0 {
                        stdout.write_char(' ').unwrap();
                    }
                    stdout.write_str(word).unwrap();
                }
                stdout.write_str("\r\n").unwrap();
            }
            "ver" => {
                println!("{} v{}", System::name(), System::version(),)
            }
            "reboot" => unsafe {
                System::reset();
            },
            "memory" => {
                let mut sb = StringBuffer::with_capacity(0x1000);
                MemoryManager::statistics(&mut sb);
                print!("{}", sb.as_str());
            }
            "open" => {
                let args = &args[1..];
                let name = args[0];
                Self::spawn(name, args, false);
            }
            _ => match Self::command(name) {
                Some(exec) => {
                    exec(args);
                }
                None => {
                    Self::spawn(name, args, true);
                }
            },
        }) {
            Ok(_) => {}
            Err(ParsedCmdLine::Empty) => (),
            Err(ParsedCmdLine::InvalidQuote) => {
                println!("Error: Invalid quote");
            }
        }
    }

    fn parse_cmd<F, R>(cmdline: &str, f: F) -> Result<R, ParsedCmdLine>
    where
        F: FnOnce(&str, &[&str]) -> R,
    {
        enum CmdLinePhase {
            LeadingSpace,
            Token,
            SingleQuote,
            DoubleQuote,
        }

        if cmdline.len() == 0 {
            return Err(ParsedCmdLine::Empty);
        }
        let mut sb = StringBuffer::with_capacity(cmdline.len());
        let mut args = Vec::new();
        let mut phase = CmdLinePhase::LeadingSpace;
        sb.clear();
        for c in cmdline.chars() {
            match phase {
                CmdLinePhase::LeadingSpace => match c {
                    ' ' => (),
                    '\'' => {
                        phase = CmdLinePhase::SingleQuote;
                    }
                    '\"' => {
                        phase = CmdLinePhase::DoubleQuote;
                    }
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
                CmdLinePhase::SingleQuote => match c {
                    '\'' => {
                        args.push(sb.as_str());
                        phase = CmdLinePhase::LeadingSpace;
                        sb.split();
                    }
                    _ => {
                        sb.write_char(c).unwrap();
                    }
                },
                CmdLinePhase::DoubleQuote => match c {
                    '\"' => {
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
        match phase {
            CmdLinePhase::LeadingSpace | CmdLinePhase::Token => (),
            CmdLinePhase::SingleQuote | CmdLinePhase::DoubleQuote => {
                return Err(ParsedCmdLine::InvalidQuote)
            }
        }
        if sb.len() > 0 {
            args.push(sb.as_str());
        }
        if args.len() > 0 {
            Ok(f(args[0], args.as_slice()))
        } else {
            Err(ParsedCmdLine::Empty)
        }
    }

    fn spawn(name: &str, argv: &[&str], wait_until: bool) -> usize {
        Self::spawn_main(name, argv, wait_until).unwrap_or_else(|| {
            let mut sb = StringBuffer::new();
            let shared = Self::shared();
            for ext in &shared.path_ext {
                sb.clear();
                write!(sb, "{}.{}", name, ext).unwrap();
                match Self::spawn_main(sb.as_str(), argv, wait_until) {
                    Some(v) => return v,
                    None => (),
                }
            }
            println!("Command not found: {}", name);
            1
        })
    }

    fn spawn_main(name: &str, argv: &[&str], wait_until: bool) -> Option<usize> {
        FileManager::open(name)
            .map(|mut fcb| {
                let stat = fcb.stat().unwrap();
                let file_size = stat.len() as usize;
                if file_size > 0 {
                    let mut vec = Vec::with_capacity(file_size);
                    vec.resize(file_size, 0);
                    let act_size = fcb.read(vec.as_mut_slice()).unwrap();
                    let blob = &vec[..act_size];
                    if let Some(mut loader) = RuntimeEnvironment::recognize(blob) {
                        loader.option().name = name.to_string();
                        loader.option().argv = argv.iter().map(|v| v.to_string()).collect();
                        match loader.load(blob) {
                            Ok(_) => {
                                let child = loader.invoke_start();
                                if wait_until {
                                    child.map(|thread| thread.join());
                                }
                            }
                            Err(_) => {
                                println!("Load error");
                                return 1;
                            }
                        }
                    } else {
                        println!("Bad executable");
                        return 1;
                    }
                }
                0
            })
            .ok()
    }

    fn command(cmd: &str) -> Option<&'static fn(&[&str]) -> isize> {
        for command in &Self::COMMAND_TABLE {
            if command.0 == cmd {
                return Some(&command.1);
            }
        }
        None
    }

    const COMMAND_TABLE: [(&'static str, fn(&[&str]) -> isize, &'static str); 6] = [
        ("dir", Self::cmd_dir, "Show directory"),
        ("help", Self::cmd_help, "Show Help"),
        ("type", Self::cmd_type, "Show file"),
        //
        ("ps", Self::cmd_ps, ""),
        ("lspci", Self::cmd_lspci, "Show List of PCI Devices"),
        ("sysctl", Self::cmd_sysctl, "System Control"),
    ];

    fn cmd_help(_: &[&str]) -> isize {
        for cmd in &Self::COMMAND_TABLE {
            if cmd.2.len() > 0 {
                println!("{}\t{}", cmd.0, cmd.2);
            }
        }
        0
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
                let mut sb = StringBuffer::with_capacity(256);
                MemoryManager::statistics(&mut sb);
                print!("{}", sb.as_str());
            }
            "random" => match Cpu::secure_rand() {
                Ok(rand) => println!("{:016x}", rand),
                Err(_) => println!("# No SecureRandom"),
            },
            _ => {
                println!("Unknown command: {}", subcmd);
                return 1;
            }
        }
        0
    }

    fn cmd_dir(_args: &[&str]) -> isize {
        let dir = match FileManager::read_dir("/") {
            Ok(v) => v,
            Err(_) => return 1,
        };
        for dir_ent in dir {
            print!(" {:<14} ", dir_ent.name());
        }
        println!("");
        0
    }

    fn cmd_type(args: &[&str]) -> isize {
        let len = 1024;
        let mut sb = Vec::with_capacity(len);
        sb.resize(len, 0);
        for path in args.iter().skip(1) {
            let mut file = match FileManager::open(path) {
                Ok(v) => v,
                Err(err) => {
                    println!("{:?}", err.kind());
                    continue;
                }
            };
            loop {
                match file.read(sb.as_mut_slice()) {
                    Ok(0) => break,
                    Ok(size) => {
                        for b in &sb[..size] {
                            System::stdout().write_char(*b as char).unwrap();
                        }
                    }
                    Err(err) => {
                        println!("Error: {:?}", err.kind());
                        break;
                    }
                }
            }
            System::stdout().write_str("\r\n").unwrap();
        }
        0
    }

    fn cmd_ps(_argv: &[&str]) -> isize {
        let mut sb = StringBuffer::with_capacity(1024);
        Scheduler::print_statistics(&mut sb, false);
        print!("{}", sb.as_str());
        0
    }

    fn cmd_lspci(argv: &[&str]) -> isize {
        let opt_all = argv.len() > 1;
        for device in bus::pci::Pci::devices() {
            let addr = device.address();
            let class_string = Self::find_class_string(device.class_code());
            println!(
                "{:02x}:{:02x}.{} {:04x}:{:04x} {}",
                addr.get_bus(),
                addr.get_dev(),
                addr.get_fun(),
                device.vendor_id().0,
                device.device_id().0,
                class_string,
            );
            if opt_all {
                for function in device.functions() {
                    let addr = function.address();
                    let class_string = Self::find_class_string(function.class_code());
                    println!(
                        "     .{} {:04x}:{:04x} {}",
                        addr.get_fun(),
                        function.vendor_id().0,
                        function.device_id().0,
                        class_string,
                    );
                }
            }
        }
        0
    }

    fn find_class_string(cc: PciClass) -> &'static str {
        #[rustfmt::skip]
        let entries = [
            (PciClass::code(0x00).sub(0x00), "Non-VGA-Compatible devices"),
            (PciClass::code(0x00).sub(0x01), "VGA-Compatible Device"),
            (PciClass::code(0x01).sub(0x00), "SCSI Bus Controller"),
            (PciClass::code(0x01).sub(0x01), "IDE Controller"),
            (PciClass::code(0x01).sub(0x04), "Raid Controller"),
            (PciClass::code(0x01).sub(0x05), "ATA Controller"),
            (PciClass::code(0x01).sub(0x06).interface(0x01), "AHCI 1.0"),
            (PciClass::code(0x01).sub(0x06), "Serial ATA"),
            (PciClass::code(0x01).sub(0x07).interface(0x00), "SAS"),
            (PciClass::code(0x01).sub(0x07), "Serial Attached SCSI"),
            (PciClass::code(0x01).sub(0x08).interface(0x01), "NVMHCI"),
            (PciClass::code(0x01).sub(0x08).interface(0x02), "NVM Express"),
            (PciClass::code(0x01).sub(0x08), "Non-Volatile Memory Controller"),
            (PciClass::code(0x01), "Mass Storage Controller"),
            (PciClass::code(0x02).sub(0x00), "Ethernet Controller"),
            (PciClass::code(0x02), "Network Controller"),
            (PciClass::code(0x03).sub(0x00), "VGA Compatible Controller"),
            (PciClass::code(0x03), "Display Controller"),
            (PciClass::code(0x04).sub(0x00), "Multimedia Video Controller"),
            (PciClass::code(0x04).sub(0x01), "Multimedia Audio Controller"),
            (PciClass::code(0x04).sub(0x02), "Computer Telephony Device"),
            (PciClass::code(0x04).sub(0x03), "Audio Device"),
            (PciClass::code(0x04), "Multimedia Controller"),
            (PciClass::code(0x05).sub(0x00), "RAM Controller"),
            (PciClass::code(0x05).sub(0x01), "Flash Controller"),
            (PciClass::code(0x05), "Memory Controller"),
            (PciClass::code(0x06).sub(0x00), "Host Bridge"),
            (PciClass::code(0x06).sub(0x01), "ISA Bridge"),
            (PciClass::code(0x06).sub(0x02), "EISA Bridge"),
            (PciClass::code(0x06).sub(0x03), "MCA Bridge"),
            (PciClass::code(0x06).sub(0x04).interface(0x00), "PCI-to-PCI Bridge (Normal Decode)"),
            (PciClass::code(0x06).sub(0x04).interface(0x01), "PCI-to-PCI Bridge (Subtractive Decode)"),
            (PciClass::code(0x06).sub(0x05), "PCMCIA Bridge"),
            (PciClass::code(0x06).sub(0x06), "NuBus Bridge"),
            (PciClass::code(0x06).sub(0x07), "CardBus Bridge"),
            (PciClass::code(0x06).sub(0x08), "RACEway Bridge"),
            (PciClass::code(0x06).sub(0x09), "PCI-to-PCI Bridge"),
            (PciClass::code(0x06).sub(0x0A), "InfiniBand-to-PCI Host Bridge"),
            (PciClass::code(0x06), "Bridge Device"),
            (PciClass::code(0x07).sub(0x00), "Serial Controller"),
            (PciClass::code(0x07).sub(0x01), "Parallel Controller"),
            (PciClass::code(0x07).sub(0x03), "Modem"),
            (PciClass::code(0x07).sub(0x04), "IEEE 488.1/2 (GPIB) Controller"),
            (PciClass::code(0x07).sub(0x05), "Smart Card"),
            (PciClass::code(0x07), "Simple Communication Controller"),
            (PciClass::code(0x08).sub(0x05), "SD Host controller"),
            (PciClass::code(0x08).sub(0x06), "IOMMU"),
            (PciClass::code(0x08), "Base System Peripheral"),
            (PciClass::code(0x09), "Input Device Controller"),
            (PciClass::code(0x0A), "Docking Station"),
            (PciClass::code(0x0B), "Processor"),
            (PciClass::code(0x0C).sub(0x03).interface(0x00), "UHCI Controller"),
            (PciClass::code(0x0C).sub(0x03).interface(0x10), "OHCI Controller"),
            (PciClass::code(0x0C).sub(0x03).interface(0x20), "EHCI Controller"),
            (PciClass::code(0x0C).sub(0x03).interface(0x30), "XHCI Controller"),
            (PciClass::code(0x0C).sub(0x03).interface(0x40), "USB4 Host Controller"),
            (PciClass::code(0x0C).sub(0x03), "USB Controller"),
            (PciClass::code(0x0C).sub(0x04), "Fibre Channel"),
            (PciClass::code(0x0C).sub(0x05), "SMBus"),
            (PciClass::code(0x0C).sub(0x06), "InfiniBand"),
            (PciClass::code(0x0C).sub(0x03), "USB Controller"),
            (PciClass::code(0x0C), "Serial Bus Controller"),
            (PciClass::code(0x0D).sub(0x11), "Bluetooth Controller"),
            (PciClass::code(0x0D), "Wireless Controller"),
            (PciClass::code(0x0E), "Intelligent Controller"),
            (PciClass::code(0x0F), "Satellite Communication Controller"),
            (PciClass::code(0x10), "Encryption Controller"),
            (PciClass::code(0x11), "Signal Processing Controller"),
            (PciClass::code(0x12), "Processing Accelerator"),
            (PciClass::code(0x13), "Encryption Controller"),
            (PciClass::code(0x14), "Non-Essential Instrumentation"),
            (PciClass::code(0x40), "Co-Processor"),
            (PciClass::code(0xFF), "(Vendor specific)"),
        ];
        for entry in &entries {
            if cc.matches(entry.0) {
                return entry.1;
            }
        }
        "(Unknown Device)"
    }
}
