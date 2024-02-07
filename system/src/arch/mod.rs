//! The modules in the hierarchy below arch are architecture-dependent, so many items are not public.

// cfg_if::cfg_if! {
//     if #[cfg(target_arch = "aarch64")] {
//         mod aa64;
//         pub use aa64::*;
//     } else if #[cfg(target_arch = "x86_64")] {
mod x64;
pub use x64::*;
//     }
// }
