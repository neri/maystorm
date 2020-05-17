pub mod cpu;
pub mod system;

#[cfg(any(target_arch = "x86_64"))]
pub mod x86_64;