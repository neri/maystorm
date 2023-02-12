//! Peripheral Component Interconnect Bus

mod pci;
use megstd::{Box, Vec};
pub use pci::*;

fn install_drivers(drivers: &mut Vec<Box<dyn PciDriverRegistrar>>) {
    // XHCI
    drivers.push(super::usb::xhci::Xhci::registrar());

    // High Definition Audio
    drivers.push(super::hda::HdAudioController::registrar());

    // VIRTIO
    // drivers.push(super::virtio::Virtio::registrar());
}
