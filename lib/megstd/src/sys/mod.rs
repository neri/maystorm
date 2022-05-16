pub mod megos;

#[cfg(feature = "wasm")]
#[macro_use]
mod wasm;
#[cfg(feature = "wasm")]
pub use wasm::*;

#[cfg(not(feature = "wasm"))]
#[macro_use]
mod kernel;
#[cfg(not(feature = "wasm"))]
pub use kernel::*;
