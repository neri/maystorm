// MEG-OS Boot loader for UEFI

#![feature(abi_efiapi)]
#![no_std]
#![no_main]
#![feature(asm)]

use boot_efi::{config::*, invocation::*, loader::*, page::*, *};
use bootprot::*;
use core::{ffi::c_void, fmt::Write, mem::*};
use uefi::prelude::*;

#[entry]
fn efi_main(handle: Handle, st: SystemTable<Boot>) -> Status {
    let mut info = BootInfo::default();
    let bs = st.boot_services();
    info.platform = Platform::UEFI;
    info.color_mode = ColorMode::Argb32;

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
    if config.force_single() {
        info.flags |= BootFlags::FORCE_SINGLE;
    }
    if config.is_headless() {
        info.flags.insert(BootFlags::HEADLESS);
    }
    if config.is_debug_mode() {
        info.flags.insert(BootFlags::DEBUG_MODE);
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
    info.smbios = match st.find_config_table(::uefi::table::cfg::SMBIOS_GUID) {
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

        if width > stride {
            // GPD micro PC fake landscape mode
            swap(&mut width, &mut height);
        }

        info.vram_stride = stride as u16;
        info.screen_width = width as u16;
        info.screen_height = height as u16;

        debug::Console::init(info.vram_base as usize, width, height, stride);
    } else if !info.flags.contains(BootFlags::HEADLESS) {
        writeln!(st.stdout(), "Error: GOP Not Found").unwrap();
        return Status::UNSUPPORTED;
    }

    // Load KERNEL
    let mut kernel = ElfLoader::new(match get_file(handle, &bs, config.kernel_path()) {
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
    let bounds = kernel.image_bounds();
    info.kernel_base = bounds.0.as_u64();

    // Load initrd
    match get_file(handle, &bs, config.initrd_path()) {
        Ok(blob) => {
            info.initrd_base = &blob[0] as *const u8 as u32;
            info.initrd_size = blob.len() as u32;
        }
        Err(status) => {
            writeln!(st.stdout(), "Error: Load failed {}", config.initrd_path()).unwrap();
            return status;
        }
    };

    unsafe {
        match PageManager::init_first(&bs) {
            Ok(_) => (),
            Err(err) => {
                writeln!(st.stdout(), "Error: {:?}", err).unwrap();
                return err;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Exit Boot Services
    //

    // because some UEFI implementations require an additional buffer during exit_boot_services
    let buf_size = st.boot_services().memory_map_size() * 2;
    let buf_ptr = st
        .boot_services()
        .allocate_pool(::uefi::table::boot::MemoryType::LOADER_DATA, buf_size)
        .unwrap()
        .unwrap();
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr, buf_size) };
    let (_st, mm) = st.exit_boot_services(handle, buf).unwrap().unwrap();

    // ------------------------------------------------------------------------

    unsafe {
        PageManager::init_late(&mut info, mm);
    }

    let entry = kernel.locate(VirtualAddress(info.kernel_base));

    let stack_size: usize = 0x4000;
    let new_sp = VirtualAddress(info.kernel_base + 0x3FFFF000);
    PageManager::valloc(new_sp - stack_size, stack_size);

    // println!("Now starting MEG-OS...");
    unsafe {
        Invocation::invoke_kernel(&info, entry, new_sp);
    }
}

#[allow(dead_code)]
#[cfg(any(target_arch = "x86_64"))]
fn write_b0c2(addr: u16, data: u16) {
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
