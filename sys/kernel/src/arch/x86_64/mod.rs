pub mod apic;
pub mod comport;
pub mod cpu;
pub mod hpet;
pub mod page;
pub mod ps2;
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
