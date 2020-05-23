// Legacy PC's Low Pin Count Device

use super::apic::*;
use crate::stdout;
use crate::*;

pub struct LowPinCount {}

impl LowPinCount {
    pub unsafe fn init() {
        Ps2::init();
    }
}

struct Ps2 {}

impl Ps2 {
    pub unsafe fn init() {
        Apic::register(Irq(1), Self::irq_01).unwrap();
    }

    fn irq_01(_irq: Irq) {
        unsafe {
            let mut al: u8;
            llvm_asm!("inb $$0x60, %al": "={al}"(al));
            print!(" {:02x}", al);
        }
    }
}
