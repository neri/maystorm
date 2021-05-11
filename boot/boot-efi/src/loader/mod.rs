mod elfldr;
pub use elfldr::*;

pub mod elf;

pub trait ImageLoader {
    fn recognize(&mut self) -> Result<(), ()>;
    fn locate(&self, base: crate::page::VirtualAddress) -> crate::page::VirtualAddress;
}
