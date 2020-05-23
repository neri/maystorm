// My UEFI-Rust Playground
#![feature(abi_efiapi)]
#![feature(llvm_asm)]
#![no_std]
#![no_main]
use core::fmt::Write;
use uefi::prelude::*;
use uefi_pg::myos::arch::cpu::Cpu;
use uefi_pg::myos::bus::lpc;
use uefi_pg::myos::io::graphics::*;
use uefi_pg::*;

uefi_pg_entry!(main);

fn main(handle: Handle, st: SystemTable<Boot>) -> Status {
    let rsdptr = match st.find_config_table(uefi::table::cfg::ACPI2_GUID) {
        Some(val) => val,
        None => {
            writeln!(st.stdout(), "Error: ACPI Table Not Found").unwrap();
            return Status::LOAD_ERROR;
        }
    };

    // TODO: init custom allocator
    let buf_size = 0x1000000;
    let buf_ptr = st
        .boot_services()
        .allocate_pool(uefi::table::boot::MemoryType::LOADER_DATA, buf_size)
        .unwrap()
        .unwrap();
    myos::mem::alloc::init(buf_ptr as usize, buf_size);

    //////// GUARD //////// exit_boot_services //////// GUARD ////////
    let (_st, mm) = exit_boot_services(st, handle);

    let fb = stdout().fb();
    // fb.reset();
    fb.fill_rect(
        Rect::new(50, 50, 200, 200),
        IndexedColor::LightRed.as_color(),
    );
    fb.fill_rect(
        Rect::new(100, 100, 200, 200),
        IndexedColor::LightGreen.as_color(),
    );
    fb.fill_rect(
        Rect::new(150, 150, 200, 200),
        IndexedColor::LightBlue.as_color(),
    );

    let mut total_memory_size: u64 = 0;
    for mem_desc in mm {
        if mem_desc.ty.is_countable() {
            total_memory_size += mem_desc.page_count << 12;
        }
    }
    unsafe {
        myos::arch::system::System::init(rsdptr as usize, total_memory_size, first_child);
    }
}

fn first_child(system: &myos::arch::system::System) {
    println!(
        "My practice OS version {} Total {} Cores, {} MB Memory",
        myos::MyOs::version(),
        system.number_of_active_cpus(),
        system.total_memory_size() >> 20,
    );
    println!("Hello, {:#}!", "Rust");

    for i in 0..system.number_of_active_cpus() {
        let cpu = system.cpu(i);
        println!("CPU index:{} apic_id:{}", i, cpu.apic_id.0);
    }

    loop {
        match lpc::get_key() {
            Some(key) => {
                if key > 0 {
                    print!("{}", hid_usage_to_unicode(key, 0));
                }
            }
            None => unsafe { Cpu::halt() },
        }
    }
}

const INVALID_UNICHAR: char = '\u{fffe}';
const HID_USAGE_DELETE: u8 = 0x4C;

// Non Alphabet
static USAGE_TO_CHAR_1E: [char; 27] = [
    '1',
    '2',
    '3',
    '4',
    '5',
    '6',
    '7',
    '8',
    '9',
    '0',
    '\x0D',
    '\x1B',
    '\x08',
    '\x09',
    ' ',
    '-',
    '^',
    '@',
    '[',
    ']',
    INVALID_UNICHAR,
    ';',
    ':',
    '`',
    ',',
    '.',
    '/',
];

// Arrows & Numpads
static USAGE_TO_CHAR_4F: [char; 21] = [
    '\u{2191}',
    '\u{2190}',
    '\u{2193}',
    '\u{2192}',
    INVALID_UNICHAR,
    '/',
    '*',
    '-',
    '+',
    '\x0D',
    '1',
    '2',
    '3',
    '4',
    '5',
    '6',
    '7',
    '8',
    '9',
    '0',
    '.',
];

//  JP 109
fn hid_usage_to_unicode(usage: u8, modifier: u8) -> char {
    // let usage = usage as usize;
    let mut uni: char = INVALID_UNICHAR;

    if usage >= 4 && usage <= 0x1D {
        // Alphabet
        uni = (usage - 4 + 0x61) as char;
    } else if usage >= 0x1E && usage <= 0x38 {
        // Non Alphabet
        uni = USAGE_TO_CHAR_1E[usage as usize - 0x1E];
    // if (uni > 0x20
    //     && uni < 0x40
    //     && uni != 0x30
    //     && (modifier & (HID_MOD_LSHIFT | HID_MOD_LSHIFT)))
    // {
    //     uni ^= 0x10;
    // }
    } else if usage == HID_USAGE_DELETE {
        // Delete
        uni = '\x7F';
    } else if usage >= 0x4F && usage <= 0x64 {
        // Arrows & Numpads
        uni = USAGE_TO_CHAR_4F[usage as usize - 0x4F];
    } else if usage == 0x89 {
        // '\|'
        uni = '\\';
    }
    // if (uni >= 0x40 && uni < 0x7F) {
    //     if (modifier & (HID_MOD_LCTRL | HID_MOD_RCTRL)) {
    //         uni &= 0x1F;
    //     } else if (modifier & (HID_MOD_LSHIFT | HID_MOD_LSHIFT)) {
    //         uni ^= 0x20;
    //     }
    // }
    // if (usage == 0x87) { // '_'
    //     if (modifier & (HID_MOD_LSHIFT | HID_MOD_LSHIFT)) {
    //         uni = '_';
    //     } else {
    //         uni = '\\';
    //     }
    // }

    uni as char
}
