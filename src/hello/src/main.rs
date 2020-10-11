// Hello OS kernel

#![feature(abi_efiapi)]
#![no_std]
#![no_main]
#![feature(asm)]

extern crate rlibc;
use bootprot::*;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub fn efi_main(info: &BootInfo, mbz: usize) -> usize {
    if mbz != 0 {
        return !(isize::MAX as usize) + 1;
    }
    unsafe {
        let mut vram = info.vram_base as usize as *mut u32;
        let width = info.vram_stride as usize;
        let height = info.screen_height as usize;

        for y in 0..height {
            for x in 0..width {
                if ((x ^ y) & 1) == 0 {
                    vram.write_volatile(0xFF00FF);
                } else {
                    vram.write_volatile(0xFFFFFF);
                }
                vram = vram.add(1);
            }
        }

        loop {
            asm!("cli\nhlt");
        }
    }
}
