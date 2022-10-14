//! Universal Serial Bus
//!
//! ```text
//!   ┏━○
//! ○┻┳━|＞
//! ┗■
//! ```

mod usb;
mod usb_mgr;
pub use usb::*;
pub use usb_mgr::*;
pub mod drivers;
pub mod xhci;
