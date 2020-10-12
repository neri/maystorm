// MyOS Boot loader for UEFI
#![feature(abi_efiapi)]
#![no_std]
#![no_main]
#![feature(asm)]

use boot_efi::config::*;
use boot_efi::invocation::*;
use boot_efi::loader::*;
use boot_efi::page::*;
use boot_efi::*;
use bootprot::*;
use core::ffi::c_void;
use core::fmt::Write;
use core::mem::*;
use uefi::prelude::*;
extern crate rlibc;

#[entry]
fn efi_main(handle: Handle, st: SystemTable<Boot>) -> Status {
    let mut info = BootInfo::default();
    let bs = st.boot_services();

    // Load CONFIG
    let config = match get_file(handle, &bs, BootSettings::DEFAULT_CONFIG_PATH) {
        Ok(blob) => match BootSettings::load(unsafe { core::str::from_utf8_unchecked(blob) }) {
            Ok(result) => result,
            Err(err) => {
                writeln!(st.stdout(), "Error in config: {}", err).unwrap();
                return Status::LOAD_ERROR;
            }
        },
        Err(status) => match status {
            Status::NOT_FOUND => BootSettings::default(),
            _ => {
                writeln!(
                    st.stdout(),
                    "Error: Load failed {}",
                    BootSettings::DEFAULT_CONFIG_PATH
                )
                .unwrap();
                return status;
            }
        },
    };

    // Load values from CONFIG
    info.cmdline = config.cmdline().as_ptr() as usize as u64;
    info.kernel_base = config.base_address().as_u64();
    if config.is_headless() {
        info.flags.insert(BootFlags::HEADLESS);
    }
    if config.is_debug_mode() {
        info.flags.insert(BootFlags::DEBUG_MODE);
    }

    // Load KERNEL
    let mut kernel = ImageLoader::new(match get_file(handle, &bs, config.kernel_path()) {
        Ok(blob) => (blob),
        Err(status) => {
            writeln!(st.stdout(), "Error: Load failed {}", config.kernel_path()).unwrap();
            return status;
        }
    });
    if kernel.recognize().is_err() {
        writeln!(st.stdout(), "Error: BAD KERNEL SIGNATURE FOUND").unwrap();
        return Status::UNSUPPORTED;
    }

    // Find ACPI Table
    info.acpi_rsdptr = match st.find_config_table(::uefi::table::cfg::ACPI2_GUID) {
        Some(val) => val as u64,
        None => {
            writeln!(st.stdout(), "Error: ACPI Table Not Found").unwrap();
            return Status::UNSUPPORTED;
        }
    };

    // Find SMBIOS Table
    info.smbios = match st.find_config_table(::uefi::table::cfg::SMBIOS3_GUID) {
        Some(val) => val as u64,
        None => 0,
    };

    // Init graphics
    if let Ok(gop) = bs.locate_protocol::<::uefi::proto::console::gop::GraphicsOutput>() {
        let gop = gop.unwrap();
        let gop = unsafe { &mut *gop.get() };

        let gop_info = gop.current_mode_info();
        let mut fb = gop.frame_buffer();
        info.vram_base = fb.as_mut_ptr() as usize as u64;

        let stride = gop_info.stride();
        let (mut width, mut height) = gop_info.resolution();

        if height > width {
            info.flags.insert(BootFlags::PORTRAIT);
            swap(&mut width, &mut height);
        }
        if width > stride {
            // GPD micro PC fake landscape mode
            info.flags.insert(BootFlags::PORTRAIT);
        }

        // let width = 800;
        // let height = 600;
        // let stride = 600;
        // info.flags.insert(BootFlags::PORTRAIT);

        // write_boc2(0x0004, 0x0000);
        // write_boc2(0x0001, 600);
        // write_boc2(0x0002, 800);
        // write_boc2(0x0003, 32);
        // write_boc2(0x0005, 0x0000);
        // write_boc2(0x0004, 0x0001);

        info.vram_stride = stride as u16;
        info.screen_width = width as u16;
        info.screen_height = height as u16;
    } else if !info.flags.contains(BootFlags::HEADLESS) {
        writeln!(st.stdout(), "Error: GOP Not Found").unwrap();
        return Status::UNSUPPORTED;
    }

    // {
    //     let time = st.runtime_services().get_time().unwrap().unwrap();
    //     info.boot_time = unsafe { transmute(time) };
    // }

    // ----------------------------------------------------------------
    // Exit Boot Services

    // because some UEFI implementations require an additional buffer during exit_boot_services
    let buf_size = st.boot_services().memory_map_size() * 2;
    let buf_ptr = st
        .boot_services()
        .allocate_pool(::uefi::table::boot::MemoryType::LOADER_DATA, buf_size)
        .unwrap()
        .unwrap();
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr, buf_size) };
    let (_st, mm) = st.exit_boot_services(handle, buf).unwrap().unwrap();

    // ----------------------------------------------------------------

    // let mut mm: Vec<uefi::table::boot::MemoryDescriptor> = mm
    //     .copied()
    //     .map(|mut m| {
    //         m.virt_start = m.phys_start;
    //         m
    //     })
    //     .collect();
    // unsafe {
    //     st.runtime_services()
    //         .set_virtual_address_map(&mut mm)
    //         .unwrap()
    //         .unwrap();
    // }

    PageManager::init(&mut info, mm);

    let entry = kernel.locate(VirtualAddress(info.kernel_base));

    let stack_size: usize = 0x4000;
    let new_sp = VirtualAddress(info.kernel_base + 0x3FFFF000);
    PageManager::valloc(new_sp - stack_size, stack_size);

    println!("Now starting kernel...");
    unsafe {
        PageManager::finalize(&mut info);
        Invocation::invoke_kernel(info, entry, new_sp);
    }
}

#[allow(dead_code)]
#[cfg(any(target_arch = "x86_64"))]
fn write_boc2(addr: u16, data: u16) {
    unsafe {
        asm!("out dx, ax", in("dx") 0x1ce, in("ax") addr);
        asm!("out dx, ax", in("dx") 0x1cf, in("ax") data);
    }
}

pub trait MyUefiLib {
    fn find_config_table(&self, _: ::uefi::Guid) -> Option<*const c_void>;
}

impl MyUefiLib for SystemTable<::uefi::table::Boot> {
    fn find_config_table(&self, guid: ::uefi::Guid) -> Option<*const c_void> {
        for entry in self.config_table() {
            if entry.guid == guid {
                return Some(entry.address);
            }
        }
        None
    }
}
