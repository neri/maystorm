pub mod megos;

cfg_if::cfg_if! {
    if #[cfg(feature = "wasm")] {
        #[macro_use]
        mod wasm;
        pub use wasm::*;
    } else {
        #[macro_use]
        mod kernel;
        pub use kernel::*;
    }
}
