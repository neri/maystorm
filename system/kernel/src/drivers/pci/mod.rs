//! Peripheral Component Interconnect Bus

mod pci;
use alloc::{boxed::Box, vec::Vec};
pub use pci::*;

fn install_drivers(drivers: &mut Vec<Box<dyn PciDriverRegistrar>>) {
    // XHCI
    drivers.push(super::usb::xhci::Xhci::registrar());
}
