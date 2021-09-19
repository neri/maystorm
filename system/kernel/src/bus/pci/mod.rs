//! Peripheral Component Interconnect bus
mod pci;
use alloc::{boxed::Box, vec::Vec};
pub use pci::*;

pub(super) fn install_drivers(drivers: &mut Vec<Box<dyn PciDriverRegistrar>>) {
    // XHCI
    drivers.push(xhci::XhciRegistrar::init());
}

pub mod xhci;
