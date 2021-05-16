mod elfldr;
pub use elfldr::*;

pub mod elf;

pub trait ImageLoader {
    fn recognize(&mut self) -> Result<(), ()>;

    fn image_bounds(&self) -> (crate::page::VirtualAddress, usize);

    fn locate(&self, base: crate::page::VirtualAddress) -> crate::page::VirtualAddress;
}
