//! The modules in the hierarchy below arch are architecture-dependent, so many items are not public.

// #[cfg(any(target_arch = "x86_64"))]
#[macro_use]
mod x86_64;
// #[cfg(any(target_arch = "x86_64"))]
pub use x86_64::*;
