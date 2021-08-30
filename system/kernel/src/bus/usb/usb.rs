//! Universal Serial Bus

use alloc::boxed::Box;

use super::desc::*;
use crate::system::System;
use core::{
    fmt::Write,
    mem::{size_of, MaybeUninit},
};

// for debug
macro_rules! print {
    ($($arg:tt)*) => {
        write!(System::em_console(), $($arg)*).unwrap()
    };
}

macro_rules! println {
    ($fmt:expr) => {
        print!(concat!($fmt, "\r\n"))
    };
    ($fmt:expr, $($arg:tt)*) => {
        print!(concat!($fmt, "\r\n"), $($arg)*)
    };
}

/// USB Device to host interface
pub trait UsbHostInterface {
    // fn configure_endpoint(&self, desc: &UsbEndpointDescriptor);
    // fn reset_endpoint(&self);
    // fn set_max_packet_size(&self, value: usize);
    // fn get_max_packet_size(&self) -> usize;

    fn control<'a>(
        &self,
        trt: UrbTranfserType,
        setup: &UsbControlSetupData,
    ) -> Result<&'a [u8], ()>;

    // fn transfer(&self, dci: usize) -> Result<*const c_void, ()>;

    // fn enter_configuration(&self);
    // fn leave_configuration(&self);
}

pub struct UsbDevice {
    ctx: Box<dyn UsbHostInterface>,
}

impl UsbDevice {
    #[inline]
    pub const fn new(ctx: Box<dyn UsbHostInterface>) -> Self {
        Self { ctx }
    }

    pub fn initialize(&self) {
        let setup = UsbControlSetupData {
            bmRequestType: 0x80,
            bRequest: UsbControlRequestKind::GET_DESCRIPTOR,
            wValue: (UsbDescriptorType::Device as u16) << 8,
            wIndex: 0,
            wLength: size_of::<UsbDeviceDescriptor>() as u16,
        };
        let mut device_desc = MaybeUninit::<UsbDeviceDescriptor>::zeroed();
        let device_desc = match self.ctx.control(UrbTranfserType::ControlIn, &setup) {
            Ok(v) => unsafe {
                let p = device_desc.as_mut_ptr();
                let src = &v[0] as *const _ as *const UsbDeviceDescriptor;
                p.copy_from(src, 1);
                device_desc.assume_init()
            },
            Err(_) => {
                println!("DEVICE DESCRIPTOR FAILED");
                return;
            }
        };

        println!("DEVICE DESCRIPTOR OK: {:?}", device_desc);
    }
}
