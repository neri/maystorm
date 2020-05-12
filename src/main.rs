// My UEFI-Rust Playground
#![feature(abi_efiapi)]
#![no_std]
#![no_main]
use core::fmt::Write;
use uefi::prelude::*;
use uefi_pg::*;
use uefi_pg::graphics::*;
extern crate alloc;

#[entry]
fn efi_main(handle: Handle, st: SystemTable<Boot>) -> Status {
    uefi_pg::init(handle, st, move |_handle, _st| {
        let stdout = stdout();
        println!("Hello, {}!", "Rust");

        let fb = stdout.fb();
//        fb.reset();
        fb.fill_rect(
            Rect::new((50, 50, 200, 200)),
            Color::from(0x2196F3),
        );
        fb.fill_rect(
            Rect::new((100, 100, 200, 200)),
            Color::from(0xf44336),
        );

        loop {}
        // Status::SUCCESS
    })
}
