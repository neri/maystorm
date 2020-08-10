// Boot for UEFI

use super::*;
use bootinfo::*;
use uefi::prelude::*;

#[macro_export]
macro_rules! entry {
    ($path:path) => {
        #[no_mangle]
        pub fn efi_main(
            handle: ::uefi::Handle,
            st: Option<::uefi::table::SystemTable<::uefi::table::Boot>>,
        ) -> ::uefi::Status {
            let f: fn(&BootInfo) = $path;
            startup(handle, st, f)
        }
    };
}

#[inline]
pub fn startup<F>(handle: Handle, st: Option<SystemTable<Boot>>, main: F) -> Status
where
    F: Fn(&BootInfo),
{
    let st = match st {
        None => unsafe {
            let info: &BootInfo = core::mem::transmute(handle);

            mem::alloc::init(info.static_start as usize, info.free_memory as usize);
            mem::alloc::CustomAlloc::init_real(info.real_bitmap);

            let screen = Bitmap::from(info);
            screen.reset();
            BOOT_SCREEN = Some(Box::new(screen));
            let stdout = Box::new(GraphicalConsole::from(boot_screen()));
            EMCONSOLE = Some(stdout);

            main(&info);

            Cpu::stop();
        },
        Some(st) => st,
    };

    let mut info = BootInfo::default();

    // Find ACPI Table
    info.acpi_rsdptr = match st.find_config_table(::uefi::table::cfg::ACPI2_GUID) {
        Some(val) => val as u64,
        None => {
            writeln!(st.stdout(), "Error: ACPI Table Not Found").unwrap();
            return Status::LOAD_ERROR;
        }
    };

    // Init graphics
    let bs = st.boot_services();
    if let Ok(gop) = bs.locate_protocol::<::uefi::proto::console::gop::GraphicsOutput>() {
        let gop = gop.unwrap();
        let gop = unsafe { &mut *gop.get() };
        {
            let gop_info = gop.current_mode_info();
            let mut fb = gop.frame_buffer();
            info.vram_base = fb.as_mut_ptr() as usize as u64;
            info.vram_delta = gop_info.stride() as u16;
            let (w, h) = gop_info.resolution();
            info.screen_width = w as u16;
            info.screen_height = h as u16;
        }
    } else {
        writeln!(st.stdout(), "Error: GOP Not Found").unwrap();
        return Status::UNSUPPORTED;
    }

    // ----------------------------------------------------------------

    // TODO: init custom allocator
    let buf_size = 0x1000000;
    let page_size = 0x1000;
    let buf_ptr = st
        .boot_services()
        .allocate_pages(
            ::uefi::table::boot::AllocateType::AnyPages,
            ::uefi::table::boot::MemoryType::LOADER_DATA,
            buf_size / page_size,
        )
        .unwrap()
        .unwrap();
    mem::alloc::init(buf_ptr as usize, buf_size);

    // ----------------------------------------------------------------

    {
        unsafe {
            let screen = Bitmap::from(&info);
            screen.reset();
            BOOT_SCREEN = Some(Box::new(screen));
            let stdout = Box::new(GraphicalConsole::from(boot_screen()));
            EMCONSOLE = Some(stdout);
        }
    }

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

    // TODO: manage memory map
    let mut total_memory_size: u64 = 0;
    for mem_desc in mm {
        if mem_desc.phys_start < 0x100_000 && mem_desc.ty.is_conventional_at_runtime() {
            let base = mem_desc.phys_start / 0x1000;
            let count = mem_desc.page_count;
            let limit = core::cmp::min(base + count, 256);
            for i in base..limit {
                let index = i as usize / 32;
                let bit = 1 << (i & 31);
                info.real_bitmap[index] |= bit;
            }
        }
        if mem_desc.ty.is_countable() {
            total_memory_size += mem_desc.page_count << 12;
        }
    }
    info.total_memory_size = total_memory_size;
    unsafe {
        mem::alloc::CustomAlloc::init_real(info.real_bitmap);
    }

    main(&info);

    Status::LOAD_ERROR
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

use uefi::table::boot::MemoryType;
pub trait MemoryTypeHelper {
    fn is_conventional_at_runtime(&self) -> bool;
    fn is_countable(&self) -> bool;
}
impl MemoryTypeHelper for MemoryType {
    fn is_conventional_at_runtime(&self) -> bool {
        match *self {
            MemoryType::CONVENTIONAL
            | MemoryType::BOOT_SERVICES_CODE
            | MemoryType::BOOT_SERVICES_DATA => true,
            _ => false,
        }
    }

    fn is_countable(&self) -> bool {
        match *self {
            MemoryType::CONVENTIONAL
            | MemoryType::LOADER_CODE
            | MemoryType::LOADER_DATA
            | MemoryType::BOOT_SERVICES_CODE
            | MemoryType::BOOT_SERVICES_DATA
            | MemoryType::RUNTIME_SERVICES_CODE
            | MemoryType::RUNTIME_SERVICES_DATA
            | MemoryType::ACPI_RECLAIM => true,
            _ => false,
        }
    }
}
