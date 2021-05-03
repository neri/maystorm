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
use kernel::arch::cpu::*;
use kernel::fs::*;
use kernel::mem::*;
use kernel::rt::*;
use kernel::system::*;
use kernel::task::scheduler::*;
use kernel::task::Task;
use kernel::*;
use megstd::string::*;

extern crate alloc;

entry!(Shell::start);

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

    // Shell's Entry point
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
            "System Manufacturer: {}",
            device.manufacturer_name().unwrap_or("Unknown"),
        );
        println!("System Model: {}", device.model_name().unwrap_or("Unknown"),);
        println!(
            "Processor Cores: {} / {}",
            device.num_of_performance_cpus(),
            device.num_of_active_cpus(),
        );

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
                    let class_string = Self::find_class_string(function.class_code());
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
            (0x0C_03_00, INTERFACE, "UHCI Controller"),
            (0x0C_03_10, INTERFACE, "OHCI Controller"),
            (0x0C_03_20, INTERFACE, "EHCI Controller"),
            (0x0C_03_30, INTERFACE, "XHCI Controller"),
            (0x0C_03_40, INTERFACE, "USB4 Host Controller"),
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
}
