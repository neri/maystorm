pub mod apic;
pub mod comport;
pub mod cpu;
pub mod hpet;
pub mod page;
pub mod ps2;

use crate::dev::uart::*;
use crate::system::*;
use alloc::boxed::Box;
// use alloc::vec::*;
use comport::*;

pub(crate) struct Arch;

impl Arch {
    pub unsafe fn init() {
        cpu::Cpu::init();

        if let acpi::InterruptModel::Apic(apic) = System::acpi().interrupt_model.as_ref().unwrap() {
            apic::Apic::init(apic);
        } else {
            panic!("NO APIC");
        }
    }

    pub unsafe fn init_late() {
        ComPort::init_late();
        let _ = ps2::Ps2::init();
    }

    pub unsafe fn master_uart() -> Option<&'static Box<dyn Uart>> {
        ComPort::init_first()
    }

    #[inline]
    pub fn uarts<'a>() -> &'a [Box<dyn Uart>] {
        ComPort::ports()
    }
}
