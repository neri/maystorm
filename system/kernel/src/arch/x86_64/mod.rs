#[macro_use]
pub mod cpu;

#[doc(hidden)]
pub mod apic;
#[doc(hidden)]
pub mod hpet;
pub mod page;
#[doc(hidden)]
pub mod ps2;
#[doc(hidden)]
pub mod rtc;

#[doc(hidden)]
mod hal_x64;
pub use hal_x64::*;

use crate::{check_once_call, system::*};
use core::arch::asm;
use megstd::time::SystemTime;

pub struct Arch;

impl Arch {
    pub unsafe fn init() {
        check_once_call!();

        cpu::Cpu::init();

        let acpi = System::acpi().unwrap();
        // let fadt = acpi.find_first::<myacpi::fadt::Fadt>().unwrap();
        // let (smi_cmd, enable, _disable) = fadt.acpi_enable();
        // asm!("out dx, al", in("edx") smi_cmd, in("al") enable);

        if let Some(madt) = acpi.find_first::<myacpi::madt::Madt>() {
            apic::Apic::init(madt);
        } else {
            panic!("NO APIC");
        }

        // apic::Apic::register(Irq(fadt.sci_int() as u8), Self::_sci_int, 0).unwrap();

        rtc::Rtc::init();
    }

    // fn _sci_int(_: usize) {
    //     let acpi = System::acpi().unwrap();
    //     let fadt = acpi.find_first::<myacpi::fadt::Fadt>().unwrap();

    //     if let Some(gpe_blk) = fadt.gpe0_blk() {
    //         let len = fadt.gpe0_blk_len() / 2;
    //         let mut vec = Vec::with_capacity(len);
    //         for i in 0..len {
    //             let al: u8;
    //             unsafe {
    //                 asm!("in al, dx", out("al")al, in("edx")
    //                     gpe_blk.address as u32 + i as u32
    //                 );
    //             }
    //             vec.push(al);
    //         }
    //         log!(" SCI {} {:?}", vec.len(), HexDump(&vec));
    //     }
    // }

    pub unsafe fn late_init() {
        check_once_call!();

        let _ = ps2::Ps2::init();

        let device = System::current_device();

        if let Some((manufacturer, model)) = device.manufacturer_name().zip(device.model_name()) {
            match (manufacturer, model) {
                ("GPD", "MicroPC") => {
                    // WORKAROUND: Enable the GPD MicroPC's built-in keyboard
                    // SBRG.H_EC.KBCD = 0x11
                    Self::wr_ec(0x11, 0x00);
                }
                _ => (),
            }
        }
    }

    /// Issue WR_EC command to embedded controller (expr)
    unsafe fn wr_ec(addr: u8, data: u8) {
        Self::ec_wait_for_ibf();
        asm!("out 0x66, al", in("al") 0x81u8);
        Self::ec_wait_for_ibf();
        asm!("out 0x62, al", in("al") addr);
        Self::ec_wait_for_ibf();
        asm!("out 0x62, al", in("al") data);
    }

    /// Wait for embedded controller (expr)
    unsafe fn ec_wait_for_ibf() {
        loop {
            let al: u8;
            asm!("in al, 0x66", out("al") al);
            if (al & 0x02) == 0 {
                break;
            }
            asm!("pause");
        }
    }

    #[inline]
    pub fn system_time() -> SystemTime {
        rtc::Rtc::system_time()
    }
}
