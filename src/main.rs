// My UEFI-Rust Playground
#![feature(abi_efiapi)]
#![no_std]
#![no_main]
use core::fmt::Write;
use uefi::prelude::*;
use uefi_pg::*;
use uefi_pg::graphics::*;

#[entry]
fn efi_main(_handle: Handle, st: SystemTable<Boot>) -> Status {
    init(&st);

    let bt = boot_services();

    if let Ok(gop) = bt.locate_protocol::<uefi::proto::console::gop::GraphicsOutput>() {
        let gop = gop.unwrap();
        let gop = unsafe { &mut *gop.get() };

        {
            let fb = FrameBuffer::from(gop);
            let mut conout = console::GraphicalConsole::new(&fb);

            fb.reset();
            fb.fill_rect(
                Rect::new((50, 50, 200, 200)),
                Color::from(0x2196F3),
            );
            fb.fill_rect(
                Rect::new((100, 100, 200, 200)),
                Color::from(0xf44336),
            );

            conout.print("Hello, Rust!");
        }
    }

    loop {}
    // Status::SUCCESS
}
