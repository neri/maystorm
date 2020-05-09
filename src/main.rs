// My UEFI-Rust Playground
#![feature(abi_efiapi)]
#![no_std]
#![no_main]
use core::fmt::Write;
use uefi::prelude::*;
use uefi::proto::console::gop::*;
use uefi_pg::*;

fn draw_test(gop: &mut GraphicsOutput) {
    let op = BltOp::VideoFill {
        color: BltPixel::new(255, 105, 97),
        dest: (100, 100),
        dims: (300, 300),
    };
    gop.blt(op).unwrap_success();
}

#[entry]
fn efi_main(_handle: Handle, st: SystemTable<Boot>) -> Status {
    init(&st);

    stdout().reset(false).unwrap_success();

    let bt = boot_services();

    if let Ok(gop) = bt.locate_protocol::<GraphicsOutput>() {
        let gop = gop.unwrap();
        let gop = unsafe { &mut *gop.get() };

        draw_test(gop);

        println!("GOP Info");
        let info = gop.current_mode_info();
        let (h_res, v_res) = info.resolution();
        println!("Mode: {} x {}, {}", h_res, v_res, info.stride());
        let mut fb = gop.frame_buffer();
        println!("FrameBuffer {:#?} {}", fb.as_mut_ptr(), fb.size())
    }

    loop {}
    // Status::SUCCESS
}
