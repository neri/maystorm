//! Universal Serial Bus
//!
//! ```text
//!   ┏━○
//! ○┻┳━|＞
//! ┗■
//! ```

mod types;
mod usb;
pub use types::*;
pub use usb::*;
pub mod drivers;
pub mod xhci;
