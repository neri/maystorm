// Hello OS kernel

#![feature(abi_efiapi)]
#![no_std]
#![no_main]
#![feature(asm)]

use core::panic::PanicInfo;
use uefi::prelude::*;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[entry]
fn efi_main(_handle: Handle, _st: SystemTable<Boot>) -> Status {
    unsafe {
        for c in b"Hello, world!\n" {
            asm!("out dx, al", in("edx") 0x3F8, in("al") *c);
        }

        loop {
            asm!("cli\nhlt");
        }
    }
}
