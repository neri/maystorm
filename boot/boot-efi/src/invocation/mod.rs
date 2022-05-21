use crate::page::VirtualAddress;
use bootprot::BootInfo;

cfg_if::cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        mod amd64;
        pub use amd64::*;
    } else if #[cfg(target_arch = "x86")] {
        mod x86;
        pub use x86::*;
    }
}

pub trait Invoke {
    fn is_compatible(&self) -> bool;

    unsafe fn invoke_kernel(
        &self,
        info: &BootInfo,
        entry: VirtualAddress,
        new_sp: VirtualAddress,
    ) -> !;
}
