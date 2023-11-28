mod elfldr;
pub use elfldr::*;

pub trait ImageLoader {
    fn image_bounds(&self) -> (crate::page::VirtualAddress, usize);

    unsafe fn locate(&self, base: crate::page::VirtualAddress) -> crate::page::VirtualAddress;
}
