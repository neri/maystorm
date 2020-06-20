pub mod cpu;

#[cfg(any(target_arch = "x86_64"))]
pub mod apic;
