// My UEFI-Rust Playground
#![feature(abi_efiapi)]
#![no_std]
#![no_main]
use core::fmt::Write;
use uefi::prelude::*;
use uefi_pg::graphics::*;
use uefi_pg::*;
extern crate alloc;

uefi_pg_entry!(main);

fn main(_handle: Handle, _st: SystemTable<Boot>) -> Status {
    println!("Hello, {:#}!", "Rust");

    let fb = stdout().fb();
    // fb.reset();
    fb.fill_rect(Rect::new((50, 50, 200, 200)), Color::from(0x2196F3));
    fb.fill_rect(Rect::new((100, 100, 200, 200)), Color::from(0xf44336));

    panic!("test test test");

    loop {}
    // Status::SUCCESS
}
