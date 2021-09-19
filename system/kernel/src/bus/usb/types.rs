//! USB Types & Descriptors

use core::{fmt, num::NonZeroU8, time::Duration};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

/// Valid USB bus addresses are 1 to 127.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbDeviceAddress(pub NonZeroU8);

/// 16-bit word type used in the USB descriptor.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct UsbWord(pub [u8; 2]);

impl UsbWord {
    #[inline]
    pub const fn from_u16(val: u16) -> Self {
        Self([val as u8, (val >> 8) as u8])
    }

    #[inline]
    pub const fn as_u16(&self) -> u16 {
        self.0[0] as u16 + (self.0[1] as u16) * 256
    }
}

impl From<UsbWord> for u16 {
    #[inline]
    fn from(v: UsbWord) -> Self {
        v.as_u16()
    }
}

impl fmt::Debug for UsbWord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04x}", self.as_u16())
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbVersion(pub u16);

impl UsbVersion {
    pub const USB1_0: Self = Self(0x0100);
    pub const USB1_1: Self = Self(0x0110);
    pub const USB2_0: Self = Self(0x0200);
    pub const USB3_0: Self = Self(0x0300);
    pub const USB3_1: Self = Self(0x0310);
    pub const USB3_2: Self = Self(0x0320);

    /// BOS descriptors are only supported for version number 0x0201 and above.
    pub const BOS_MIN: Self = Self(0x0201);
}

impl fmt::Display for UsbVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let part1 = (self.0 >> 12) & 0x0F;
        let part2 = (self.0 >> 8) & 0x0F;
        let part3 = (self.0 >> 4) & 0x0F;
        // let part4 = self.0 & 0x0F;
        write!(f, "{}.{}", part1 * 10 + part2, part3)
    }
}

/// USB Vendor Id
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbVendorId(pub u16);

impl fmt::Display for UsbVendorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04x}", self.0)
    }
}

/// USB Product Id
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbProductId(pub u16);

impl fmt::Display for UsbProductId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04x}", self.0)
    }
}

/// USB Class code (BaseClass - SubClass - Protocol)
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbClass(pub u32);

impl UsbClass {
    pub const COMPOSITE: Self = Self(0x00_00_00);
    pub const MIDI_STREAMING: Self = Self(0x01_03_00);
    pub const HID_GENERIC: Self = Self(0x03_00_00);
    pub const HID_BOOT_KEYBOARD: Self = Self(0x03_01_01);
    pub const HID_BOOT_MOUSE: Self = Self(0x03_01_02);
    pub const MSD_BULK_ONLY: Self = Self(0x08_06_50);
    pub const FLOPPY: Self = Self(0x08_04_00);
    pub const HUB_FS: Self = Self(0x09_00_00);
    pub const HUB_HS_STT: Self = Self(0x09_00_01);
    pub const HUB_HS_MTT: Self = Self(0x09_00_02);
    pub const HUB_SS: Self = Self(0x09_00_03);
    pub const BLUETOOTH: Self = Self(0xE0_01_01);
    pub const XINPUT: Self = Self(0xFF_5D_01);
    pub const XINPUT_HEADSET: Self = Self(0xFF_5D_02);
    pub const XINPUT_IF2: Self = Self(0xFF_5D_03);
    pub const XINPUT_IF3: Self = Self(0xFF_5D_04);

    #[inline]
    pub const fn new(base: UsbBaseClass, sub: UsbSubClass, protocol: UsbProtocolCode) -> Self {
        Self(((base.0 as u32) << 16) | ((sub.0 as u32) << 8) | (protocol.0 as u32))
    }

    #[inline]
    pub const fn base(&self) -> UsbBaseClass {
        UsbBaseClass((self.0 >> 16) as u8)
    }

    #[inline]
    pub const fn sub(&self) -> UsbSubClass {
        UsbSubClass((self.0 >> 8) as u8)
    }

    #[inline]
    pub const fn protocol(&self) -> UsbProtocolCode {
        UsbProtocolCode(self.0 as u8)
    }

    #[inline]
    pub fn class_string(&self, is_interface: bool) -> Option<&'static str> {
        #[rustfmt::skip]
        let base_class_entries = [
            ( UsbBaseClass::COMPOSITE, 0x01, "USB Composite Device" ),
            ( UsbBaseClass::AUDIO, 0x02, "Audio Device" ),
            ( UsbBaseClass::COMM, 0x03, "Communication Device" ),
            ( UsbBaseClass::HID, 0x02, "Human Interface Device" ),
            ( UsbBaseClass::PRINTER, 0x02, "Printer" ),
            ( UsbBaseClass::STORAGE, 0x02, "Storage Device" ),
            ( UsbBaseClass::HUB, 0x01, "USB Hub" ),
            ( UsbBaseClass::CDC_DATA, 0x02, "CDC Data"),
            ( UsbBaseClass::VIDEO, 0x02, "Video Device" ),
            ( UsbBaseClass::AUDIO_VIDEO, 0x02, "Audio/Video Device" ),
            ( UsbBaseClass::BILLBOARD, 0x01, "Billboard Device" ),
            ( UsbBaseClass::TYPE_C_BRIDGE, 0x02, "Type-C Bridge" ),
            ( UsbBaseClass::DIAGNOSTIC, 0x03, "Diagnostic Device" ),
            ( UsbBaseClass::WIRELESS, 0x02, "Wireless Device" ),
            ( UsbBaseClass::APPLICATION_SPECIFIC, 0x02, "Application Specific" ),
            ( UsbBaseClass::VENDOR_SPECIFIC, 0x03, "Vendor Specific" ),
        ];

        #[rustfmt::skip]
        let full_class_entries = [
            (UsbClass::MIDI_STREAMING, "USB MIDI Streaming" ),
            (UsbClass::HID_BOOT_KEYBOARD, "HID Boot Keyboard" ),
            (UsbClass::HID_BOOT_MOUSE, "HID Boot Mouse" ),
            (UsbClass::MSD_BULK_ONLY, "Mass Storage Device" ),
            (UsbClass::FLOPPY, "Floppy Drive"),
            (UsbClass::HUB_FS, "Full Speed Hub"),
            (UsbClass::HUB_HS_STT, "High Speed Hub"),
            (UsbClass::HUB_HS_MTT, "High Speed Hub with multi TTs"),
            (UsbClass::HUB_SS, "Super Speed Hub"),
            (UsbClass::BLUETOOTH, "Bluetooth Interface"),
            (UsbClass::XINPUT, "XInput Device"),
        ];

        let bitmap = 1u8 << (is_interface as usize);
        match full_class_entries.binary_search_by_key(self, |v| v.0) {
            Ok(index) => full_class_entries.get(index).map(|v| v.1),
            Err(_) => None,
        }
        .or_else(
            || match base_class_entries.binary_search_by_key(&self.base(), |v| v.0) {
                Ok(index) => base_class_entries.get(index).and_then(|v| {
                    if (v.1 & bitmap) != 0 {
                        Some(v.2)
                    } else {
                        None
                    }
                }),
                Err(_) => None,
            },
        )
    }
}

impl fmt::Display for UsbClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:06x}", self.0)
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbBaseClass(pub u8);

impl UsbBaseClass {
    pub const COMPOSITE: Self = Self(0x00);
    pub const AUDIO: Self = Self(0x01);
    pub const COMM: Self = Self(0x02);
    pub const HID: Self = Self(0x03);
    pub const PHYSICAL: Self = Self(0x05);
    pub const IMAGE: Self = Self(0x06);
    pub const PRINTER: Self = Self(0x07);
    pub const STORAGE: Self = Self(0x08);
    pub const HUB: Self = Self(0x09);
    pub const CDC_DATA: Self = Self(0x0A);
    pub const SMART_CARD: Self = Self(0x0B);
    pub const CONTENT_SECURITY: Self = Self(0x0C);
    pub const VIDEO: Self = Self(0x0E);
    pub const PERSONAL_HEALTHCARE: Self = Self(0x0F);
    pub const AUDIO_VIDEO: Self = Self(0x10);
    pub const BILLBOARD: Self = Self(0x11);
    pub const TYPE_C_BRIDGE: Self = Self(0x12);
    pub const DIAGNOSTIC: Self = Self(0xDC);
    pub const WIRELESS: Self = Self(0xE0);
    pub const MISCELLANEOUS: Self = Self(0xEF);
    pub const APPLICATION_SPECIFIC: Self = Self(0xFE);
    pub const VENDOR_SPECIFIC: Self = Self(0xFF);
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbSubClass(pub u8);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbProtocolCode(pub u8);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbConfigurationValue(pub u8);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbInterfaceNumber(pub u8);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbAlternateSettingNumber(pub u8);

/// USB Descriptor type
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum UsbDescriptorType {
    Device = 1,
    Configuration,
    String,
    Interface,
    Endpoint,
    DeviceQualifier,
    InterfaceAssociation = 11,
    Bos = 15,
    DeviceCapability,
    HidClass = 0x21,
    HidReport,
    HidPhysical,
    Hub = 0x29,
    Hub3 = 0x2A,
    SuperspeedUsbEndpointCompanion = 48,
    SuperspeedplusIsochronousEndpointCompanion = 49,
}

/// USB Device Capability type
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum UsbDeviceCapabilityType {
    WirelessUsb = 1,
    Usb2_0Extention,
    SuperspeedUsb,
    ContainerId,
    Platform,
    PowerDelivery,
    BatteryInfo,
    PdConsumerPort,
    PdProviderPort,
    SuperspeedPlus,
    PrecisionTimeMeasurement,
    WirelessUsbExt,
    Billboard,
    Authentication,
    BillboardEx,
    ConfigurationSummary,
}

/// A type compatible with standard USB descriptors
pub trait UsbDescriptor {
    /// bLength
    fn len(&self) -> usize;
    /// bDescriptorType
    fn descriptor_type(&self) -> UsbDescriptorType;
}

/// USB Device Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct UsbDeviceDescriptor {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub bcdUSB: UsbWord,
    pub bDeviceClass: UsbBaseClass,
    pub bDeviceSubClass: UsbSubClass,
    pub bDeviceProtocol: UsbProtocolCode,
    pub bMaxPacketSize0: u8,
    pub idVendor: UsbWord,
    pub idProduct: UsbWord,
    pub bcdDevice: UsbWord,
    pub iManufacturer: u8,
    pub iProduct: u8,
    pub iSerialNumber: u8,
    pub bNumConfigurations: u8,
}

impl UsbDeviceDescriptor {
    #[inline]
    pub const fn usb_version(&self) -> UsbVersion {
        UsbVersion(self.bcdUSB.as_u16())
    }

    #[inline]
    pub const fn vid(&self) -> UsbVendorId {
        UsbVendorId(self.idVendor.as_u16())
    }

    #[inline]
    pub const fn pid(&self) -> UsbProductId {
        UsbProductId(self.idProduct.as_u16())
    }

    #[inline]
    pub const fn class(&self) -> UsbClass {
        UsbClass::new(
            self.bDeviceClass,
            self.bDeviceSubClass,
            self.bDeviceProtocol,
        )
    }

    #[inline]
    pub const fn max_packet_size(&self) -> usize {
        self.bMaxPacketSize0 as usize
    }

    #[inline]
    pub const fn manufacturer_index(&self) -> Option<NonZeroU8> {
        NonZeroU8::new(self.iManufacturer)
    }

    #[inline]
    pub const fn product_index(&self) -> Option<NonZeroU8> {
        NonZeroU8::new(self.iProduct)
    }

    #[inline]
    pub const fn serial_number_index(&self) -> Option<NonZeroU8> {
        NonZeroU8::new(self.iSerialNumber)
    }
}

impl UsbDescriptor for UsbDeviceDescriptor {
    #[inline]
    fn len(&self) -> usize {
        self.bLength as usize
    }

    #[inline]
    fn descriptor_type(&self) -> UsbDescriptorType {
        self.bDescriptorType
    }
}

/// USB Configuration Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct UsbConfigurationDescriptor {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub wTotalLength: UsbWord,
    pub bNumInterface: u8,
    pub bConfigurationValue: UsbConfigurationValue,
    pub iConfiguration: u8,
    pub bmAttributes: u8,
    pub bMaxPower: u8,
}

impl UsbConfigurationDescriptor {
    #[inline]
    pub const fn total_length(&self) -> u16 {
        self.wTotalLength.as_u16()
    }

    #[inline]
    pub const fn num_interface(&self) -> u8 {
        self.bNumInterface
    }

    #[inline]
    pub const fn configuration_value(&self) -> UsbConfigurationValue {
        self.bConfigurationValue
    }

    #[inline]
    pub const fn configuration_index(&self) -> Option<NonZeroU8> {
        NonZeroU8::new(self.iConfiguration)
    }

    #[inline]
    pub const fn max_power(&self) -> u8 {
        self.bMaxPower
    }
}

impl UsbDescriptor for UsbConfigurationDescriptor {
    #[inline]
    fn len(&self) -> usize {
        self.bLength as usize
    }

    #[inline]
    fn descriptor_type(&self) -> UsbDescriptorType {
        self.bDescriptorType
    }
}

/// USB Interface Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct UsbInterfaceDescriptor {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub bInterfaceNumber: UsbInterfaceNumber,
    pub bAlternateSetting: UsbAlternateSettingNumber,
    pub bNumEndpoints: u8,
    pub bInterfaceClass: UsbBaseClass,
    pub bInterfaceSubClass: UsbSubClass,
    pub bInterfaceProtocol: UsbProtocolCode,
    pub iInterface: u8,
}

impl UsbInterfaceDescriptor {
    #[inline]
    pub const fn if_no(&self) -> UsbInterfaceNumber {
        self.bInterfaceNumber
    }

    #[inline]
    pub const fn alternate_setting(&self) -> UsbAlternateSettingNumber {
        self.bAlternateSetting
    }

    #[inline]
    pub const fn num_endpoints(&self) -> usize {
        self.bNumEndpoints as usize
    }

    #[inline]
    pub const fn interface_index(&self) -> Option<NonZeroU8> {
        NonZeroU8::new(self.iInterface)
    }

    #[inline]
    pub const fn class(&self) -> UsbClass {
        UsbClass::new(
            self.bInterfaceClass,
            self.bInterfaceSubClass,
            self.bInterfaceProtocol,
        )
    }
}

impl UsbDescriptor for UsbInterfaceDescriptor {
    #[inline]
    fn len(&self) -> usize {
        self.bLength as usize
    }

    #[inline]
    fn descriptor_type(&self) -> UsbDescriptorType {
        self.bDescriptorType
    }
}

/// USB Endpoint Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct UsbEndpointDescriptor {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub bEndpointAddress: u8,
    pub bmAttributes: u8,
    pub wMaxPacketSize: UsbWord,
    pub bInterval: u8,
}

impl UsbEndpointDescriptor {
    #[inline]
    pub fn endpoint_address(&self) -> Option<UsbEndpointAddress> {
        NonZeroU8::new(self.bEndpointAddress).map(|v| UsbEndpointAddress(v))
    }

    #[inline]
    pub fn ep_type(&self) -> Option<UsbEndpointType> {
        FromPrimitive::from_u8(self.bmAttributes as u8 & 3)
    }

    #[inline]
    pub const fn max_packet_size(&self) -> u16 {
        self.wMaxPacketSize.as_u16()
    }

    #[inline]
    pub const fn interval(&self) -> u8 {
        self.bInterval
    }
}

impl UsbDescriptor for UsbEndpointDescriptor {
    #[inline]
    fn len(&self) -> usize {
        self.bLength as usize
    }

    #[inline]
    fn descriptor_type(&self) -> UsbDescriptorType {
        self.bDescriptorType
    }
}

/// USB Decive Qualifier Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct UsbDeviceQualifierDescriptor {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub bcdUSB: UsbWord,
    pub bDeviceClass: UsbBaseClass,
    pub bDeviceSubClass: UsbSubClass,
    pub bDeviceProtocol: UsbProtocolCode,
    pub bMaxPacketSize0: u8,
    pub bNumConfigurations: u8,
    pub bReserved: u8,
}

impl UsbDeviceQualifierDescriptor {
    #[inline]
    pub const fn class(&self) -> UsbClass {
        UsbClass::new(
            self.bDeviceClass,
            self.bDeviceSubClass,
            self.bDeviceProtocol,
        )
    }
}

impl UsbDescriptor for UsbDeviceQualifierDescriptor {
    #[inline]
    fn len(&self) -> usize {
        self.bLength as usize
    }

    #[inline]
    fn descriptor_type(&self) -> UsbDescriptorType {
        self.bDescriptorType
    }
}

/// BOS USB Binary Device Object Store Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct UsbBinaryObjectStoreDescriptor {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub wTotalLength: UsbWord,
    pub bNumDeviceCaps: u8,
}

impl UsbBinaryObjectStoreDescriptor {
    #[inline]
    pub const fn total_length(&self) -> u16 {
        self.wTotalLength.as_u16()
    }

    #[inline]
    pub const fn num_children(&self) -> usize {
        self.bNumDeviceCaps as usize
    }
}

impl UsbDescriptor for UsbBinaryObjectStoreDescriptor {
    #[inline]
    fn len(&self) -> usize {
        self.bLength as usize
    }

    #[inline]
    fn descriptor_type(&self) -> UsbDescriptorType {
        self.bDescriptorType
    }
}

/// A type compatible with standard USB device capabilities
pub trait UsbDeviceCapabilityDescriptor: UsbDescriptor {
    fn capability_type(&self) -> UsbDeviceCapabilityType;
}

/// USB Superspeed USB Device Capability
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct UsbSsDeviceCapability {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub bDevCapabilityType: UsbDeviceCapabilityType,
    pub bmAttributes: u8,
    pub wSpeedSupported: UsbWord,
    pub bFunctionalitySupport: u8,
    pub bU1DevExitLat: u8,
    pub wU2DevExitLat: UsbWord,
}

impl UsbSsDeviceCapability {
    #[inline]
    pub const fn u1_dev_exit_lat(&self) -> usize {
        self.bU1DevExitLat as usize
    }

    #[inline]
    pub const fn u2_dev_exit_lat(&self) -> usize {
        self.wU2DevExitLat.as_u16() as usize
    }
}

impl UsbDescriptor for UsbSsDeviceCapability {
    #[inline]
    fn len(&self) -> usize {
        self.bLength as usize
    }

    #[inline]
    fn descriptor_type(&self) -> UsbDescriptorType {
        self.bDescriptorType
    }
}

impl UsbDeviceCapabilityDescriptor for UsbSsDeviceCapability {
    #[inline]
    fn capability_type(&self) -> UsbDeviceCapabilityType {
        self.bDevCapabilityType
    }
}

/// USB Container Id capability
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct UsbContainerIdCapability {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub bDevCapabilityType: UsbDeviceCapabilityType,
    pub bReserved: u8,
    pub ContainerID: [u8; 16],
}

impl UsbContainerIdCapability {
    #[inline]
    pub const fn uuid(&self) -> &[u8; 16] {
        &self.ContainerID
    }
}

impl UsbDescriptor for UsbContainerIdCapability {
    #[inline]
    fn len(&self) -> usize {
        self.bLength as usize
    }

    #[inline]
    fn descriptor_type(&self) -> UsbDescriptorType {
        self.bDescriptorType
    }
}

impl UsbDeviceCapabilityDescriptor for UsbContainerIdCapability {
    #[inline]
    fn capability_type(&self) -> UsbDeviceCapabilityType {
        self.bDevCapabilityType
    }
}

/// USB HID Report Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
pub struct UsbHidReportDescriptor {
    pub bDescriptorType: UsbDescriptorType,
    pub wDescriptorLength: UsbWord,
}

impl UsbDescriptor for UsbHidReportDescriptor {
    #[inline]
    fn len(&self) -> usize {
        0
    }

    #[inline]
    fn descriptor_type(&self) -> UsbDescriptorType {
        self.bDescriptorType
    }
}

/// USB HID Class Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug)]
pub struct UsbHidClassDescriptor {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub bcdHID: UsbWord,
    pub bCountryCode: u8,
    pub bNumDescriptors: u8,
    pub bDescriptorType_: UsbDescriptorType,
    pub wDescriptorLength_: UsbWord,
}

impl UsbHidClassDescriptor {
    #[inline]
    pub const fn num_descriptors(&self) -> usize {
        self.bNumDescriptors as usize
    }

    #[inline]
    pub fn first_child(&self) -> (UsbDescriptorType, u16) {
        (self.bDescriptorType_, self.wDescriptorLength_.as_u16())
    }

    #[inline]
    pub fn children<'a>(&'a self) -> impl Iterator<Item = (UsbDescriptorType, u16)> + 'a {
        UsbHidClassDescriptorIter {
            base: self,
            index: 0,
        }
    }
}

impl UsbDescriptor for UsbHidClassDescriptor {
    #[inline]
    fn len(&self) -> usize {
        self.bLength as usize
    }

    #[inline]
    fn descriptor_type(&self) -> UsbDescriptorType {
        self.bDescriptorType
    }
}

struct UsbHidClassDescriptorIter<'a> {
    base: &'a UsbHidClassDescriptor,
    index: usize,
}

impl Iterator for UsbHidClassDescriptorIter<'_> {
    type Item = (UsbDescriptorType, u16);

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            if self.index < self.base.num_descriptors() {
                let p = self.base as *const _ as *const u8;
                let offset = self.index * 3 + 6;
                let p = p.add(offset);
                let ty = match FromPrimitive::from_u8(p.read()) {
                    Some(v) => v,
                    None => return None,
                };
                let len = (p.add(1).read() as u16) + (p.add(2).read() as u16 * 256);
                self.index += 1;
                Some((ty, len))
            } else {
                None
            }
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbHubPortNumber(pub NonZeroU8);

/// USB2 Hub Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct UsbHub2Descriptor {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub bNbrPorts: u8,
    pub wHubCharacteristics: UsbWord,
    pub bPwrOn2PwrGood: u8,
    pub bHubContrCurrent: u8,
    pub DeviceRemovable: UsbWord,
}

impl UsbHub2Descriptor {
    #[inline]
    pub const fn num_ports(&self) -> usize {
        self.bNbrPorts as usize
    }

    #[inline]
    pub const fn characteristics(&self) -> UsbHub2Characterisrics {
        UsbHub2Characterisrics(self.wHubCharacteristics.as_u16())
    }

    /// Time (in 2 ms intervals) from the time the power-on sequence begins on a port until power is good on that port. The USB System Software uses this value to determine how long to wait before accessing a powered-on port.
    #[inline]
    pub const fn power_on_to_power_good(&self) -> Duration {
        Duration::from_millis(self.bPwrOn2PwrGood as u64 * 2)
    }

    #[inline]
    pub const fn device_removable(&self) -> u16 {
        self.DeviceRemovable.as_u16()
    }
}

impl UsbDescriptor for UsbHub2Descriptor {
    #[inline]
    fn len(&self) -> usize {
        self.bLength as usize
    }

    #[inline]
    fn descriptor_type(&self) -> UsbDescriptorType {
        self.bDescriptorType
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbHub2Characterisrics(pub u16);

impl UsbHub2Characterisrics {
    #[inline]
    pub const fn is_compound_device(&self) -> bool {
        (self.0 & 0x0004) != 0
    }

    #[inline]
    pub const fn ttt(&self) -> usize {
        ((self.0 >> 5) & 3) as usize
    }

    #[inline]
    pub const fn supports_port_indicators(&self) -> bool {
        (self.0 & 0x0080) != 0
    }
}

/// USB3 Hub Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct UsbHub3Descriptor {
    pub bLength: u8,
    pub bDescriptorType: UsbDescriptorType,
    pub bNbrPorts: u8,
    pub wHubCharacteristics: UsbWord,
    pub bPwrOn2PwrGood: u8,
    pub bHubContrCurrent: u8,
    pub bHubHdrDecLat: u8,
    pub wHubDelay: UsbWord,
    pub DeviceRemovable: UsbWord,
}

impl UsbHub3Descriptor {
    #[inline]
    pub const fn num_ports(&self) -> usize {
        self.bNbrPorts as usize
    }

    #[inline]
    pub const fn characteristics(&self) -> UsbHub3Characterisrics {
        UsbHub3Characterisrics(self.wHubCharacteristics.as_u16())
    }

    /// Time (in 2 ms intervals) from the time the power-on sequence begins on a port until power is good on that port. The USB System Software uses this value to determine how long to wait before accessing a powered-on port.
    #[inline]
    pub const fn power_on_to_power_good(&self) -> Duration {
        Duration::from_millis(self.bPwrOn2PwrGood as u64 * 2)
    }

    #[inline]
    pub const fn device_removable(&self) -> u16 {
        self.DeviceRemovable.as_u16()
    }
}

impl UsbDescriptor for UsbHub3Descriptor {
    #[inline]
    fn len(&self) -> usize {
        self.bLength as usize
    }

    #[inline]
    fn descriptor_type(&self) -> UsbDescriptorType {
        self.bDescriptorType
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbHub3Characterisrics(pub u16);

impl UsbHub3Characterisrics {
    #[inline]
    pub const fn is_compound_device(&self) -> bool {
        (self.0 & 0x0004) != 0
    }
}

/// USB3 Route String
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct UsbRouteString(u32);

impl UsbRouteString {
    /// Empty route (root)
    pub const EMPTY: Self = Self(0);
    /// Since a valid Route String is 20 bits, a valid mask is 0xFFFFF.
    pub const VALID_MASK: u32 = 0xF_FFFF;
    /// Max depth is 5
    pub const MAX_DEPTH: usize = 5;

    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw & Self::VALID_MASK)
    }

    #[inline]
    pub const fn as_u32(&self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn depth(&self) -> usize {
        match self.0 {
            0 => 0,
            0x0_0001..=0x0_000F => 1,
            0x0_0010..=0x0_00FF => 2,
            0x0_0100..=0x0_0FFF => 3,
            0x0_1000..=0x0_FFFF => 4,
            _ => 5,
        }
    }

    #[inline]
    pub const fn appending(&self, component: UsbHubPortNumber) -> Result<Self, Self> {
        let lpc = component.0.get() as u32;
        if lpc > 15 {
            return Err(*self);
        }
        let raw = self.0;
        let depth = self.depth();
        if depth < Self::MAX_DEPTH {
            Ok(Self(raw | (lpc << (depth * 4))))
        } else {
            Err(*self)
        }
    }

    #[inline]
    pub const fn append(&mut self, component: UsbHubPortNumber) -> Result<(), ()> {
        match self.appending(component) {
            Ok(v) => {
                *self = v;
                Ok(())
            }
            Err(_) => Err(()),
        }
    }
}

#[repr(C)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbControlSetupData {
    pub bmRequestType: UsbControlRequestBitmap,
    pub bRequest: UsbControlRequest,
    pub wValue: u16,
    pub wIndex: u16,
    pub wLength: u16,
}

impl UsbControlSetupData {
    #[inline]
    pub const fn request(
        request_type: UsbControlRequestBitmap,
        request: UsbControlRequest,
    ) -> Self {
        Self {
            bmRequestType: request_type,
            bRequest: request,
            wValue: 0,
            wIndex: 0,
            wLength: 0,
        }
    }

    #[inline]
    pub const fn value(mut self, value: u16) -> Self {
        self.wValue = value;
        self
    }

    #[inline]
    pub const fn index(mut self, index: u16) -> Self {
        self.wIndex = index;
        self
    }

    #[inline]
    pub const fn index_if(self, if_no: UsbInterfaceNumber) -> Self {
        self.index(if_no.0 as u16)
    }

    #[inline]
    pub const fn length(mut self, length: u16) -> Self {
        self.wLength = length;
        self
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbControlRequestBitmap(pub u8);

impl UsbControlRequestBitmap {
    /// Device to host standard request
    pub const GET_DEVICE: Self = Self(0x80);
    /// Host to device standard request
    pub const SET_DEVICE: Self = Self(0x00);

    /// Device to host interface specific request
    pub const GET_INTERFACE: Self = Self(0x81);
    /// Host to device interface specific request
    pub const SET_INTERFACE: Self = Self(0x01);

    /// Device to host class specific request
    pub const GET_CLASS: Self = Self(0xA0);
    /// Host to device class specific request
    pub const SET_CLASS: Self = Self(0x20);

    #[inline]
    pub const fn new(
        device_to_host: bool,
        request_type: UsbControlRequestType,
        target: UsbControlRequestTarget,
    ) -> Self {
        Self(((device_to_host as u8) << 7) | ((request_type as u8) << 5) | (target as u8))
    }

    #[inline]
    pub const fn is_device_to_host(&self) -> bool {
        (self.0 & 0x80) != 0
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UsbControlRequestType {
    Standard = 0,
    Class,
    Vendor,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UsbControlRequestTarget {
    Device = 0,
    Interface,
    Endpoint,
    Other,
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbControlRequest(pub u8);

impl UsbControlRequest {
    pub const GET_STATUS: Self = Self(0);
    pub const CLEAR_FEATURE: Self = Self(1);
    pub const SET_FEATURE: Self = Self(3);
    // pub const SET_ADDRESS:Self = Self(5);
    pub const GET_DESCRIPTOR: Self = Self(6);
    pub const SET_DESCRIPTOR: Self = Self(7);
    pub const GET_CONFIGURATION: Self = Self(8);
    pub const SET_CONFIGURATION: Self = Self(9);
    pub const GET_INTERFACE: Self = Self(0x0A);
    pub const SET_INTERFACE: Self = Self(0x0B);
    // pub const SYNC_FRAME:Self = Self(0x0C);
    pub const SET_SEL: Self = Self(0x30);
    pub const SET_ISOCH_DELAY: Self = Self(0x31);
    pub const HID_GET_REPORT: Self = Self(1);
    pub const HID_SET_REPORT: Self = Self(9);
    pub const HID_SET_PROTOCOL: Self = Self(11);
}

#[repr(u16)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UsbDeviceFeatureSel {
    DEVICE_REMOTE_WAKEUP = 1,
}

/// Protocol Speed Identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum PSIV {
    /// USB1 FullSpeed 12Mbps
    FS = 1,
    /// USB1 LowSpeed 1.5Mbps
    LS = 2,
    /// USB2 HighSpeed 480Mbps
    HS = 3,
    /// USB3 SuperSpeed 5Gbps
    SS = 4,
    PSIV5,
    PSIV6,
    PSIV7,
    PSIV8,
    PSIV9,
    PSIV10,
    PSIV11,
    PSIV12,
    PSIV13,
    PSIV14,
    PSIV15,
}

impl PSIV {
    #[inline]
    pub const fn max_packet_size(&self) -> usize {
        match self {
            PSIV::FS | PSIV::LS => 8,
            PSIV::HS => 64,
            _ => 512,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum Usb3LinkState {
    U0 = 0,
    U1,
    U2,
    U3,
    Disabled,
    RxDetect,
    Inactive,
    Polling,
    Recovery,
    HotReset,
    ComplianceMode,
    TestMode,
    Resume = 15,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Usb3ExitLatencyValues {
    pub u1sel: u8,
    pub u1pel: u8,
    pub u2sel: UsbWord,
    pub u2pel: UsbWord,
}

impl Usb3ExitLatencyValues {
    #[inline]
    pub const fn from_ss_dev_cap(ss_dev_cap: &UsbSsDeviceCapability) -> Self {
        Self {
            u1sel: ss_dev_cap.bU1DevExitLat,
            u1pel: ss_dev_cap.bU1DevExitLat,
            u2sel: ss_dev_cap.wU2DevExitLat,
            u2pel: ss_dev_cap.wU2DevExitLat,
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbEndpointAddress(pub NonZeroU8);

impl UsbEndpointAddress {
    #[inline]
    pub const fn new(ep_no: u8, is_dir_in: bool) -> Option<Self> {
        match NonZeroU8::new((ep_no & 0x0F) | ((is_dir_in as u8) << 7)) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    #[inline]
    pub const fn is_dir_in(&self) -> bool {
        (self.0.get() & 0x80) != 0
    }

    #[inline]
    pub const fn ep_no(&self) -> usize {
        (self.0.get() & 0x0F) as usize
    }

    #[inline]
    pub const fn compact(&self) -> u8 {
        let c = self.0.get();
        (c & 0x0F) | ((c & 0x80) >> 3)
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum UsbEndpointType {
    Control,
    Isochronous,
    Bulk,
    Interrupt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum UsbError {
    General,
    HostUnavailable,
    InvalidParameter,
    InvalidDescriptor,
    UnexpectedToken,
    Aborted,
    ShortPacket,
    UsbTransactionError,
}
