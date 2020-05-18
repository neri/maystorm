// A Computer System

use super::cpu::*;
use alloc::boxed::Box;
use alloc::vec::*;

pub struct System {
    total_memory_size: u64,
    number_of_cpus: usize,
    cpus: Vec<Box<Cpu>>,
}

static mut SYSTEM: System = System::new();

unsafe impl Sync for System {}

impl System {
    const fn new() -> Self {
        System {
            total_memory_size: 0,
            number_of_cpus: 0,
            cpus: Vec::new(),
        }
    }

    pub unsafe fn init(number_of_cpus: usize, total_memory_size: u64) {
        SYSTEM.total_memory_size = total_memory_size;
        SYSTEM.number_of_cpus = number_of_cpus;
        SYSTEM.cpus.push(Cpu::new());
        Cpu::init();
    }

    #[inline]
    pub fn shared() -> &'static System {
        unsafe { &SYSTEM }
    }

    #[inline]
    pub fn number_of_cpus(&self) -> usize {
        self.number_of_cpus
    }

    #[inline]
    pub fn number_of_active_cpus(&self) -> usize {
        self.cpus.len()
    }

    #[inline]
    pub fn total_memory_size(&self) -> u64 {
        self.total_memory_size
    }
}
