// Central Processing Unit

use super::system::*;
#[cfg(any(target_arch = "x86_64"))]
use super::x86_64::*;
use alloc::boxed::Box;

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ApicId(pub u32);

// #[derive(Debug)]
pub struct Cpu {
    pub apic_id: ApicId,
    pub gdt: Box<GlobalDescriptorTable>,
    pub tss: Box<TaskStateSegment>,
}

//unsafe impl Sync for Cpu {}

impl Cpu {
    pub fn new(acpi_proc: acpi::Processor) -> Box<Self> {
        let tss = TaskStateSegment::new();
        let gdt = GlobalDescriptorTable::new(&tss);
        let cpu = Box::new(Cpu {
            apic_id: ApicId(acpi_proc.local_apic_id as u32),
            gdt: gdt,
            tss: tss,
        });
        cpu
    }

    pub fn current() -> &'static Box<Cpu> {
        System::shared().cpu(0)
    }

    pub fn init() {
        InterruptDescriptorTable::init();

        unsafe {
            if let acpi::InterruptModel::Apic(apic) =
                System::shared().acpi().interrupt_model.as_ref().unwrap()
            {
                super::apic::Apic::new(apic);
            } else {
                panic!("NO APIC");
            }
        }
    }

    pub fn relax() {
        unsafe {
            llvm_asm!("pause");
        }
    }

    pub unsafe fn halt() {
        llvm_asm!("hlt");
    }

    pub unsafe fn disable() {
        llvm_asm!("cli");
    }

    pub unsafe fn enable() {
        llvm_asm!("sti");
    }

    pub unsafe fn reset() -> ! {
        // io_out8(0x0CF9, 0x06);
        // moe_usleep(10000);
        Cpu::out8(0x0092, 0x01);
        loop {
            Cpu::halt()
        }
    }

    pub unsafe fn out8(port: u16, value: u8) {
        llvm_asm!("outb %al, %dx" :: "{dx}"(port), "{al}"(value));
    }

    pub unsafe fn debug_assert() {
        // llvm_asm!("int3");
        llvm_asm!("movabs %eax, (0x7ffffffffff0)");
    }
}
