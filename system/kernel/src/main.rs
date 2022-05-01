// MEG-OS Kernel
// (c) 2020 Nerry
// License: MIT

#![no_std]
#![no_main]

use alloc::{format, string::*, vec::*};
use bootprot::*;
use core::{fmt, fmt::Write, num::NonZeroU8};
use kernel::{
    drivers::pci,
    drivers::usb,
    fs::*,
    mem::*,
    rt::*,
    system::*,
    task::{scheduler::*, Task},
    ui::window::WindowManager,
    *,
};
use megstd::{io::Read, string::*};

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
        Self::exec_cmd("sysctl device");

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
            "exit" => println!("Feature not available"),
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
                println!(
                    "{} v{} [codename {}]",
                    System::name(),
                    System::version(),
                    System::codename()
                )
            }
            "reboot" => {
                System::reset();
            }
            "uptime" => {
                let systime = System::system_time();
                let sec = systime.secs;
                // let time_s = sec % 60;
                let time_m = (sec / 60) % 60;
                let time_h = (sec / 3600) % 24;

                let uptime = Timer::monotonic();
                let sec = uptime.as_secs();
                let upt_s = sec % 60;
                let upt_m = (sec / 60) % 60;
                let upt_h = (sec / 3600) % 24;
                let upt_d = sec / 86400;

                if upt_d > 0 {
                    println!(
                        "{:02}:{:02} up {} days, {:02}:{:02}",
                        time_h, time_m, upt_d, upt_h, upt_m
                    );
                } else {
                    println!(
                        "{:02}:{:02} up {:02}:{:02}:{:02}",
                        time_h, time_m, upt_h, upt_m, upt_s
                    );
                }
            }
            "ts" => {
                let mut sb = StringBuffer::with_capacity(1024);
                Scheduler::get_thread_statistics(&mut sb);
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
                    if args.len() > 1 && args.last() == Some(&"&") {
                        let mut args = Vec::from(args);
                        args.remove(args.len() - 1);
                        Self::spawn(name, &args, false);
                    } else {
                        Self::spawn(name, args, true);
                    }
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
                let stat = fcb.fstat().unwrap();
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

    const COMMAND_TABLE: [(&'static str, fn(&[&str]) -> isize, &'static str); 9] = [
        ("cd", Self::cmd_cd, ""),
        ("pwd", Self::cmd_pwd, ""),
        ("dir", Self::cmd_dir, "Show directory"),
        ("help", Self::cmd_help, "Show Help"),
        ("type", Self::cmd_type, "Show file"),
        //
        ("ps", Self::cmd_ps, ""),
        ("lspci", Self::cmd_lspci, "Show List of PCI Devices"),
        ("lsusb", Self::cmd_lsusb, "Show List of USB Devices"),
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

    fn cmd_cd(argv: &[&str]) -> isize {
        match FileManager::chdir(argv.get(1).unwrap_or(&"/")) {
            Ok(_) => 0,
            Err(err) => {
                println!("{:?}", err.kind());
                1
            }
        }
    }

    fn cmd_pwd(_argv: &[&str]) -> isize {
        println!("{}", Scheduler::current_pid().cwd());
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
            "device" => {
                let device = System::current_device();
                let n_cores = device.num_of_performance_cpus();
                let n_threads = device.num_of_active_cpus();
                if n_threads > 1 {
                    if n_cores != n_threads {
                        print!(
                            "  {} Cores {} Threads {}",
                            n_cores,
                            n_threads,
                            device.processor_system_type().to_string(),
                        );
                    } else {
                        print!(
                            "  {} Processors {}",
                            n_cores,
                            device.processor_system_type().to_string(),
                        );
                    }
                } else {
                    print!("  Uniprocessor system");
                }

                let bytes = device.total_memory_size();
                let gb = bytes >> 30;
                let mb = (100 * (bytes & 0x3FFF_FFFF)) / 0x4000_0000;
                println!(", Memory {}.{:02} GB", gb, mb);

                if let Some(manufacturer_name) = device.manufacturer_name() {
                    println!("  Manufacturer: {}", manufacturer_name);
                }
                if let Some(model_name) = device.model_name() {
                    println!("  Model: {}", model_name);
                }
            }
            "cpu" => {
                let device = System::current_device();

                let n_cores = device.num_of_performance_cpus();
                let n_threads = device.num_of_active_cpus();
                if n_threads > 1 {
                    if n_cores != n_threads {
                        println!(
                            "{}: {} Cores {} Threads",
                            device.processor_system_type().to_string(),
                            n_cores,
                            n_threads,
                        );
                    } else {
                        println!(
                            "{}: {} Processors",
                            device.processor_system_type().to_string(),
                            n_cores,
                        );
                    }
                } else {
                    println!("Uniprocessor system");
                }

                for index in 0..device.num_of_active_cpus() {
                    let cpu = System::cpu(ProcessorIndex(index));
                    println!(
                        "CPU #{} {:08x} {:?}",
                        index,
                        cpu.physical_id(),
                        cpu.processor_type()
                    );
                }
            }
            "memory" => {
                let mut sb = StringBuffer::with_capacity(256);
                MemoryManager::statistics(&mut sb);
                print!("{}", sb.as_str());
            }
            "windows" => {
                let mut sb = StringBuffer::with_capacity(4096);
                WindowManager::get_statistics(&mut sb);
                print!("{}", sb.as_str());
            }
            "drivers" => {
                for driver in pci::Pci::drivers() {
                    println!(
                        "PCI {:?} {} {}",
                        driver.address(),
                        driver.name(),
                        driver.current_status()
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

    fn cmd_dir(args: &[&str]) -> isize {
        let path = args.get(1).unwrap_or(&"");
        let dir = match FileManager::read_dir(path) {
            Ok(v) => v,
            Err(err) => {
                println!("{:?}", err.kind());
                return 1;
            }
        };

        let stdout = System::stdout();
        let width = stdout.dims().0 as usize;
        let item_len = 16;
        let items_per_line = width / item_len;
        let needs_new_line = items_per_line > 0 && (width - (items_per_line * item_len)) > 0;

        let mut acc = 0;
        for dir_ent in dir {
            let metadata = dir_ent.metadata();
            let suffix = if metadata.file_type().is_dir() {
                "/"
            } else if metadata.file_type().is_symlink() {
                "@"
            } else {
                ""
            };
            let name = format!("{}{}", dir_ent.name(), suffix);
            print!(" {:<15}", name);

            acc += 1;
            if acc >= items_per_line {
                if needs_new_line {
                    println!("");
                }
                acc = 0;
            }
        }
        if acc < items_per_line {
            println!("");
        }
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
        Scheduler::print_statistics(&mut sb);
        print!("{}", sb.as_str());
        0
    }

    fn cmd_lsusb(argv: &[&str]) -> isize {
        if let Some(addr) = argv.get(1).and_then(|v| v.parse::<NonZeroU8>().ok()) {
            let addr = usb::UsbAddress(addr);
            let device = match usb::UsbManager::device_by_addr(addr) {
                Some(v) => v,
                None => {
                    println!("Error: Device not found");
                    return 1;
                }
            };

            let class_string = device
                .class()
                .class_string(false)
                .unwrap_or("Unknown Device")
                .to_string();
            println!(
                "{:02x} VID {} PID {} class {} USB {} {}",
                device.addr().0.get(),
                device.vid(),
                device.pid(),
                device.class(),
                device.descriptor().usb_version(),
                class_string,
            );
            println!(
                "manufacturer: {}\nproduct: {}",
                device.manufacturer_string().unwrap_or("Unknown"),
                device.product_string().unwrap_or("Unknown"),
            );

            for config in device.configurations() {
                println!(
                    "config #{} {}",
                    config.configuration_value().0,
                    config.name().unwrap_or(""),
                );
                for interface in config.interfaces() {
                    let if_string = interface
                        .class()
                        .class_string(true)
                        .unwrap_or("Unknown Interface");
                    println!(
                        " interface #{}.{} class {:06x} {}",
                        interface.if_no().0,
                        interface.alternate_setting().0,
                        interface.class().0,
                        interface.name().unwrap_or(if_string),
                    );
                    for endpoint in interface.endpoints() {
                        println!(
                            "  endpoint {:02x} {:?} size {} interval {}",
                            endpoint.address().0,
                            endpoint.ep_type(),
                            endpoint.descriptor().max_packet_size(),
                            endpoint.descriptor().interval(),
                        );
                    }
                }
            }
        } else {
            Self::print_usb_device(0, None);
        }
        0
    }

    fn print_usb_device(nest: usize, parent: Option<usb::UsbAddress>) {
        for device in usb::UsbManager::devices().filter(|v| v.parent() == parent) {
            for _ in 0..nest {
                print!("  ");
            }
            println!(
                "{:02x} VID {} PID {} class {} {}{}",
                device.addr().0.get(),
                device.vid(),
                device.pid(),
                device.class(),
                if device.is_configured() { "" } else { "? " },
                device.preferred_device_name().unwrap_or("Unknown Device"),
            );
            if device.children().len() > 0 {
                Self::print_usb_device(nest + 1, Some(device.addr()));
            }
        }
    }

    fn cmd_lspci(_argv: &[&str]) -> isize {
        // let _opt_all = argv.len() > 1;
        for device in drivers::pci::Pci::devices() {
            let addr = device.address();
            let class_string = Self::find_pci_class_string(device.class_code());
            println!(
                "{:02x}:{:02x}.{} {:04x}:{:04x} {:06x} {}",
                addr.get_bus(),
                addr.get_dev(),
                addr.get_fun(),
                device.vendor_id().0,
                device.device_id().0,
                device.class_code().data(),
                class_string,
            );
        }
        0
    }

    fn find_pci_class_string(cc: pci::PciClass) -> &'static str {
        use pci::PciClass;
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
            (PciClass::code(0x04).sub(0x03), "HD Audio Controller"),
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

    #[allow(dead_code)]
    fn format_si(sb: &mut dyn fmt::Write, val: usize) -> core::fmt::Result {
        let kb = (val / 1000) % 1000;
        let mb = (val / 1000_000) % 1000;
        let gb = val / 1000_000_000;

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
    fn format_bytes(sb: &mut dyn fmt::Write, val: usize) -> core::fmt::Result {
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
}
