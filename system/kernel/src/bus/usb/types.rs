//! USB Types & Descriptors

use core::{fmt, num::NonZeroU8};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
// use num_traits::FromPrimitive;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbDeviceAddress(pub NonZeroU8);

/// 16-bit word type used in the USB descriptor.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct UsbWord(pub [u8; 2]);

impl UsbWord {
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

impl fmt::Debug for UsbVersion {
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

/// USB Product Id
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbProductId(pub u16);

/// Vendor Id and Product Id pair
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbVidPid(pub u32);

impl UsbVidPid {
    #[inline]
    pub const fn vid_pid(vid: UsbVendorId, pid: UsbProductId) -> Self {
        Self((vid.0 as u32) * 0x10000 | (pid.0 as u32))
    }

    #[inline]
    pub const fn vid(&self) -> UsbVendorId {
        UsbVendorId((self.0 >> 16) as u16)
    }

    #[inline]
    pub const fn pid(&self) -> UsbProductId {
        UsbProductId(self.0 as u16)
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
    pub const STORAGE_BULK: Self = Self(0x08_06_50);
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
}

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
    HidClass = 0x21,
    HidReport,
    Hub = 0x29,
    SsHub = 0x2A,
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
    bLength: u8,
    bDescriptorType: UsbDescriptorType,
    bcdUSB: UsbWord,
    bDeviceClass: UsbBaseClass,
    bDeviceSubClass: UsbSubClass,
    bDeviceProtocol: UsbProtocolCode,
    bMaxPacketSize0: u8,
    idVendor: UsbWord,
    idProduct: UsbWord,
    bcdDevice: UsbWord,
    iManufacturer: u8,
    iProduct: u8,
    iSerialNumber: u8,
    bNumConfigurations: u8,
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
    bLength: u8,
    bDescriptorType: UsbDescriptorType,
    wTotalLength: UsbWord,
    bNumInterface: u8,
    bConfigurationValue: UsbConfigurationValue,
    iConfiguration: u8,
    bmAttributes: u8,
    bMaxPower: u8,
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
    bLength: u8,
    bDescriptorType: UsbDescriptorType,
    bInterfaceNumber: UsbInterfaceNumber,
    bAlternateSetting: UsbAlternateSettingNumber,
    bNumEndpoints: u8,
    bInterfaceClass: UsbBaseClass,
    bInterfaceSubClass: UsbSubClass,
    bInterfaceProtocol: UsbProtocolCode,
    iInterface: u8,
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
    bLength: u8,
    bDescriptorType: UsbDescriptorType,
    bEndpointAddress: u8,
    bmAttributes: u8,
    wMaxPacketSize: UsbWord,
    bInterval: u8,
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
    bLength: u8,
    bDescriptorType: UsbDescriptorType,
    bcdUSB: UsbWord,
    bDeviceClass: UsbBaseClass,
    bDeviceSubClass: UsbSubClass,
    bDeviceProtocol: UsbProtocolCode,
    bMaxPacketSize0: u8,
    bNumConfigurations: u8,
    bReserved: u8,
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

// /// USB String Descriptor
// #[repr(C, packed)]
// #[allow(non_snake_case)]
// pub struct UsbStringDescriptor {
//     bLength: u8,
//     bDescriptorType: UsbDescriptorType,
//     data: [u16; 127],
// }

/// USB HID Report Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
pub struct UsbHidReportDescriptor {
    bDescriptorType: UsbDescriptorType,
    wDescriptorLength: UsbWord,
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
    bLength: u8,
    bDescriptorType: UsbDescriptorType,
    bcdHID: UsbWord,
    bCountryCode: u8,
    bNumDescriptors: u8,
    bDescriptorType_: u8,
    wDescriptorLength_: UsbWord,
    reports: [u8; 246],
}

impl UsbHidClassDescriptor {
    #[inline]
    pub const fn num_descriptors(&self) -> usize {
        self.bNumDescriptors as usize
    }

    #[inline]
    pub fn first_descriptor(&self) -> (u8, u16) {
        (self.bDescriptorType_, self.wDescriptorLength_.as_u16())
    }

    #[inline]
    pub fn descriptor(&self, nth: usize) -> Option<(u8, u16)> {
        todo!()
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

/// USB Hub Descriptor
#[repr(C, packed)]
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub struct UsbHubDescriptor {
    bLength: u8,
    bDescriptorType: UsbDescriptorType,
    bNbrPorts: u8,
    wHubCharacteristics: UsbWord,
    bPwrOn2PwrGood: u8,
    bHubContrCurrent: u8,
    DeviceRemovable: UsbWord,
}

impl UsbDescriptor for UsbHubDescriptor {
    #[inline]
    fn len(&self) -> usize {
        self.bLength as usize
    }

    #[inline]
    fn descriptor_type(&self) -> UsbDescriptorType {
        self.bDescriptorType
    }
}

/// USB3 Route String
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct RouteString(u32);

impl RouteString {
    /// Empty route (root)
    pub const EMPTY: Self = Self(0);
    /// Since a valid Route String is 20 bits, a valid mask is 0xFFFFF.
    pub const VALID_MASK: u32 = 0xF_FFFF;
    /// Max level is 5
    pub const MAX_LEVEL: usize = 5;

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
    pub const fn level(&self) -> usize {
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
    pub const fn appending(&self, component: RouteStringPathComponent) -> Result<Self, Self> {
        let raw = self.0;
        let level = self.level();
        if level < Self::MAX_LEVEL {
            Ok(Self(raw | ((component.0.get() as u32) << (level * 4))))
        } else {
            Err(*self)
        }
    }

    #[inline]
    pub const fn append(&mut self, component: RouteStringPathComponent) -> Result<(), ()> {
        match self.appending(component) {
            Ok(v) => {
                *self = v;
                Ok(())
            }
            Err(_) => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RouteStringPathComponent(NonZeroU8);

impl RouteStringPathComponent {
    #[inline]
    pub fn new(value: u8) -> Option<Self> {
        NonZeroU8::new(value).and_then(|v| if v.get() < 0x10 { Some(Self(v)) } else { None })
    }

    #[inline]
    pub const fn value(&self) -> NonZeroU8 {
        self.0
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

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsbControlRequestBitmap(pub u8);

impl UsbControlRequestBitmap {
    pub const GET_DEVICE: Self = Self(0x80);
    pub const SET_DEVICE: Self = Self(0x00);

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

/// Protocol Speed Identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum PSIV {
    FS = 1,
    LS = 2,
    HS = 3,
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
    ShortPacket,
    UsbTransactionError,
}
