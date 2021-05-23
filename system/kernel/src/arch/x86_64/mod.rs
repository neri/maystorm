#[macro_use]
pub mod cpu;

#[doc(hidden)]
pub mod apic;
#[doc(hidden)]
pub mod comport;
#[doc(hidden)]
pub mod hpet;
pub mod page;
#[doc(hidden)]
pub mod ps2;
#[doc(hidden)]
pub mod rtc;

use crate::dev::uart::*;
use crate::system::*;
use alloc::boxed::Box;
use comport::*;
use megstd::time::SystemTime;

pub(crate) struct Arch;

impl Arch {
    pub unsafe fn init() {
        cpu::Cpu::init();

        if let acpi::InterruptModel::Apic(apic) = System::acpi_platform().interrupt_model {
            apic::Apic::init(&apic);
        } else {
            panic!("NO APIC");
        }

        rtc::Rtc::init();
    }

    pub unsafe fn late_init() {
        ComPort::late_init();
        let _ = ps2::Ps2::init();

        let device = System::current_device();
        match device.manufacturer_name() {
            Some("GPD") => {
                match device.model_name() {
                    Some("MicroPC") => {
                        // WORKAROUND: Enable the GPD MicroPC's built-in keyboard
                        // SBRG.H_EC.KBCD = 0x11
                        Self::wr_ec(0x11, 0x00);
                    }
                    _ => (),
                }
            }
            _ => (),
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
    #[allow(dead_code)]
    pub unsafe fn master_uart() -> Option<&'static Box<dyn Uart>> {
        ComPort::init_first()
    }

    #[inline]
    #[allow(dead_code)]
    pub fn uarts<'a>() -> &'a [Box<dyn Uart>] {
        ComPort::ports()
    }

    #[inline]
    pub fn system_time() -> SystemTime {
        rtc::Rtc::system_time()
    }
}
