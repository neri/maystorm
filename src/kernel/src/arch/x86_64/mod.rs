pub mod apic;
pub mod cpu;
pub mod hpet;
pub mod page;
pub mod ps2;
pub mod serial;

use crate::bus::uart::*;
use crate::system::*;
use alloc::boxed::Box;
use alloc::vec::*;
use serial::*;

static mut UARTS: Vec<Box<dyn Uart>> = Vec::new();

pub struct Arch;

impl Arch {
    pub(crate) unsafe fn init() {
        cpu::Cpu::init();

        if let acpi::InterruptModel::Apic(apic) = System::acpi().interrupt_model.as_ref().unwrap() {
            apic::Apic::init(apic);
        } else {
            panic!("NO APIC");
        }

        UARTS = SerialPort::all_ports();
    }

    pub(crate) unsafe fn init_late() {
        let _ = ps2::Ps2::init();
    }

    #[inline]
    pub(crate) fn uarts<'a>() -> &'a [Box<dyn Uart>] {
        unsafe { UARTS.as_slice() }
    }
}
