//! xHC Data Structures

use crate::bus::usb::*;
use core::{mem::transmute, num::NonZeroU8, sync::atomic::*};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

/// xHCI Port Id
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PortId(pub NonZeroU8);

/// xHCI Slot Id
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SlotId(pub NonZeroU8);

/// xHCI Device Context Index (Endpoint Id)
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DCI(pub NonZeroU8);

impl DCI {
    pub const CONTROL: Self = Self(unsafe { NonZeroU8::new_unchecked(1) });

    #[inline]
    pub const fn ep_out(epno: u8) -> Option<Self> {
        if epno >= 1 && epno <= 15 {
            match NonZeroU8::new(epno * 2) {
                Some(v) => Some(Self(v)),
                None => None,
            }
        } else {
            None
        }
    }

    #[inline]
    pub const fn ep_in(epno: u8) -> Option<Self> {
        if epno >= 1 && epno <= 15 {
            match NonZeroU8::new(epno * 2 + 1) {
                Some(v) => Some(Self(v)),
                None => None,
            }
        } else {
            None
        }
    }

    #[inline]
    pub const fn is_control(&self) -> bool {
        self.0.get() == Self::CONTROL.0.get()
    }

    #[inline]
    pub const fn is_dir_in(&self) -> bool {
        (self.0.get() & 1) != 0
    }

    #[inline]
    pub const fn ep_no(&self) -> usize {
        (self.0.get() / 2) as usize
    }
}

impl From<UsbEndpointAddress> for DCI {
    #[inline]
    fn from(val: UsbEndpointAddress) -> Self {
        unsafe {
            Self(NonZeroU8::new_unchecked(
                (val.is_dir_in() as u8) | ((val.ep_no() as u8) << 1),
            ))
        }
    }
}

#[repr(transparent)]
pub struct CycleBit(AtomicBool);

impl CycleBit {
    #[inline]
    pub const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    #[inline]
    pub fn reset(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    #[inline]
    pub fn set(&self, val: bool) {
        self.0.store(val, Ordering::SeqCst);
    }

    #[inline]
    pub fn value(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    #[inline]
    pub fn tr_value(&self) -> u64 {
        self.value() as u64
    }

    #[inline]
    pub fn trb_value(&self) -> u32 {
        self.value() as u32
    }

    #[inline]
    pub fn toggle(&self) {
        self.0.fetch_xor(true, Ordering::SeqCst);
    }
}

impl From<bool> for CycleBit {
    #[inline]
    fn from(val: bool) -> Self {
        Self(AtomicBool::new(val))
    }
}

impl PartialEq for CycleBit {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.load(Ordering::SeqCst) == other.0.load(Ordering::SeqCst)
    }
}

pub type TrbRawData = [AtomicU32; 4];

/// xHCI Common Transfer Request Block
#[repr(transparent)]
pub struct Trb(TrbRawData);

impl Trb {
    #[inline]
    pub const fn new(trb_type: TrbType) -> Self {
        let mut slice = [0; 4];
        slice[3] = (trb_type as u32) << 10;
        unsafe { transmute(slice) }
    }

    #[inline]
    pub const fn empty() -> Self {
        Self::new(TrbType::RESERVED)
    }

    #[inline]
    pub fn set_trb_type(&mut self, trb_type: TrbType) {
        let val =
            (self.raw_data()[3].load(Ordering::SeqCst) & 0xFFFF_03FF) | ((trb_type as u32) << 10);
        self.raw_data()[3].store(val, Ordering::SeqCst)
    }
}

impl TrbCommon for Trb {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

pub trait TrbCommon {
    fn raw_data(&self) -> &TrbRawData;

    #[inline]
    fn is_known_type(&self) -> bool {
        self.trb_type()
            .map(|v| v != TrbType::RESERVED)
            .unwrap_or(false)
    }

    #[inline]
    fn cycle_bit(&self) -> bool {
        (self.raw_data()[3].load(Ordering::SeqCst) & 0x01) != 0
    }

    #[inline]
    fn trb_type(&self) -> Option<TrbType> {
        let val = (self.raw_data()[3].load(Ordering::SeqCst) >> 10) & 0x3F;
        FromPrimitive::from_u32(val)
    }

    fn copy_from<T>(&self, src: &T, cycle: &CycleBit)
    where
        T: TrbCommon,
    {
        let dest = self.raw_data();
        let src = src.raw_data();
        for index in 0..3 {
            dest[index].store(src[index].load(Ordering::SeqCst), Ordering::SeqCst);
        }
        dest[3].store(
            src[3].load(Ordering::SeqCst) & 0xFFFF_FFFE | cycle.trb_value(),
            Ordering::SeqCst,
        );
    }

    fn raw_copy_from<T>(&self, src: &T)
    where
        T: TrbCommon,
    {
        let dest = self.raw_data();
        let src = src.raw_data();
        for index in 0..4 {
            dest[index].store(src[index].load(Ordering::SeqCst), Ordering::SeqCst);
        }
    }

    fn as_common_trb(&self) -> &Trb {
        unsafe { transmute(self.raw_data()) }
    }

    fn as_event(&self) -> Option<TrbEvent> {
        match self.trb_type() {
            Some(TrbType::COMMAND_COMPLETION_EVENT) => Some(TrbEvent::CommandCompletion(unsafe {
                transmute(self.raw_data())
            })),
            Some(TrbType::PORT_STATUS_CHANGE_EVENT) => Some(TrbEvent::PortStatusChange(unsafe {
                transmute(self.raw_data())
            })),
            Some(TrbType::TRANSFER_EVENT) => Some(TrbEvent::TransferEvent(unsafe {
                transmute(self.raw_data())
            })),
            Some(TrbType::DEVICE_NOTIFICATION_EVENT) => {
                Some(TrbEvent::DeviceNotification(unsafe {
                    transmute(self.raw_data())
                }))
            }
            _ => None,
        }
    }
}

pub trait TrbPtr: TrbCommon {
    #[inline]
    fn ptr(&self) -> u64 {
        let low = self.raw_data()[0].load(Ordering::SeqCst) as u64;
        let high = self.raw_data()[1].load(Ordering::SeqCst) as u64;
        low | (high << 32)
    }

    #[inline]
    fn set_ptr(&self, val: u64) {
        let low = val as u32;
        let high = (val >> 32) as u32;
        self.raw_data()[0].store(low, Ordering::SeqCst);
        self.raw_data()[1].store(high, Ordering::SeqCst);
    }
}

pub trait TrbCC: TrbCommon {
    #[inline]
    fn completion_code(&self) -> Option<TrbCompletionCode> {
        let val = (self.raw_data()[2].load(Ordering::SeqCst) >> 24) & 0xFF;
        FromPrimitive::from_u32(val)
    }
}

pub trait TrbPortId: TrbCommon {
    #[inline]
    fn port_id(&self) -> Option<PortId> {
        NonZeroU8::new((self.raw_data()[0].load(Ordering::SeqCst) >> 24) as u8).map(|v| PortId(v))
    }
}

pub trait TrbSlotId: TrbCommon {
    #[inline]
    fn slot_id(&self) -> Option<SlotId> {
        NonZeroU8::new((self.raw_data()[3].load(Ordering::SeqCst) >> 24) as u8).map(|v| SlotId(v))
    }

    #[inline]
    fn set_slot_id(&self, slot_id: SlotId) {
        self.raw_data()[3].store(
            self.raw_data()[3].load(Ordering::SeqCst) & 0x00FF_FFFF
                | ((slot_id.0.get() as u32) << 24),
            Ordering::SeqCst,
        );
    }
}

pub trait TrbDci: TrbCommon {
    #[inline]
    fn dci(&self) -> Option<DCI> {
        NonZeroU8::new((self.raw_data()[3].load(Ordering::SeqCst) >> 16) as u8).map(|v| DCI(v))
    }

    #[inline]
    fn set_dci(&self, dci: DCI) {
        self.raw_data()[3].store(
            self.raw_data()[3].load(Ordering::SeqCst) & 0xFFE0_FFFF | ((dci.0.get() as u32) << 16),
            Ordering::SeqCst,
        );
    }
}

pub trait TrbXferLen: TrbCommon {
    #[inline]
    fn xfer_len(&self) -> usize {
        (self.raw_data()[2].load(Ordering::SeqCst) & 0x0001_FFFF) as usize
    }

    #[inline]
    fn set_xfer_len(&self, xfer_len: usize) {
        self.raw_data()[2].store(
            self.raw_data()[2].load(Ordering::SeqCst) & 0xFFFE_0000
                | ((xfer_len & 0x0001_FFFF) as u32),
            Ordering::SeqCst,
        );
    }
}

/// Interrupt on Short Packet
pub trait TrbIsp: TrbCommon {
    #[inline]
    fn isp(&self) -> bool {
        (self.raw_data()[3].load(Ordering::SeqCst) & 0x0000_0004) != 0
    }

    #[inline]
    fn set_isp(&self, value: bool) {
        let bit = 0x0000_0004;
        if value {
            self.raw_data()[3].fetch_or(bit, Ordering::SeqCst);
        } else {
            self.raw_data()[3].fetch_and(!bit, Ordering::SeqCst);
        }
    }
}

/// Interrupt on Completion
pub trait TrbIoC: TrbCommon {
    #[inline]
    fn ioc(&self) -> bool {
        (self.raw_data()[3].load(Ordering::SeqCst) & 0x0000_0020) != 0
    }

    #[inline]
    fn set_ioc(&self, value: bool) {
        let bit = 0x0000_0020;
        if value {
            self.raw_data()[3].fetch_or(bit, Ordering::SeqCst);
        } else {
            self.raw_data()[3].fetch_and(!bit, Ordering::SeqCst);
        }
    }
}

/// Immediate Data
pub trait TrbIDt: TrbCommon {
    #[inline]
    fn idt(&self) -> bool {
        (self.raw_data()[3].load(Ordering::SeqCst) & 0x0000_0040) != 0
    }

    #[inline]
    fn set_idt(&self, value: bool) {
        let bit = 0x0000_0040;
        if value {
            self.raw_data()[3].fetch_or(bit, Ordering::SeqCst);
        } else {
            self.raw_data()[3].fetch_and(!bit, Ordering::SeqCst);
        }
    }
}

/// Direction is device to host
pub trait TrbDir: TrbCommon {
    #[inline]
    fn dir(&self) -> bool {
        (self.raw_data()[3].load(Ordering::SeqCst) & 0x0001_0000) != 0
    }

    #[inline]
    fn set_dir(&self, value: bool) {
        let bit = 0x0001_0000;
        if value {
            self.raw_data()[3].fetch_or(bit, Ordering::SeqCst);
        } else {
            self.raw_data()[3].fetch_and(!bit, Ordering::SeqCst);
        }
    }
}

/// TRB Types
#[allow(non_camel_case_types)]
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum TrbType {
    RESERVED = 0,
    NORMAL,
    SETUP,
    DATA,
    STATUS,
    ISOCH,
    LINK,
    EVENT_DATA,
    NOP,

    ENABLE_SLOT_COMMAND = 9,
    DISABLE_SLOT_COMMAND,
    ADDRESS_DEVICE_COMMAND,
    CONFIGURE_ENDPOINT_COMMAND,
    EVALUATE_CONTEXT_COMMAND,
    RESET_ENDPOINT_COMMAND,

    NOP_COMMAND = 23,

    TRANSFER_EVENT = 32,
    COMMAND_COMPLETION_EVENT = 33,
    PORT_STATUS_CHANGE_EVENT = 34,
    BANDWIDTH_REQUEST_EVENT = 35,
    DOORBELL_EVENT = 36,
    HOST_CONTROLLER_EVENT = 37,
    DEVICE_NOTIFICATION_EVENT = 38,
    MFINDEX_WRAP_EVENT = 39,
}

impl TrbType {
    #[inline]
    pub fn is_event(&self) -> bool {
        *self >= Self::TRANSFER_EVENT && *self <= Self::MFINDEX_WRAP_EVENT
    }

    #[inline]
    pub fn is_command(&self) -> bool {
        *self == Self::LINK || (*self >= Self::ENABLE_SLOT_COMMAND && *self < Self::TRANSFER_EVENT)
    }

    #[inline]
    pub fn is_transfer(&self) -> bool {
        *self >= Self::NORMAL && *self <= Self::NOP
    }
}

/// TRB Completion Codes
#[allow(non_camel_case_types)]
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum TrbCompletionCode {
    // invalid = 0
    SUCCESS = 1,
    DATA_BUFFER_ERROR,
    BABBLE_DETECTED,
    USB_TRANSACTION_ERROR,
    TRB_ERROR,
    STALL,
    RESOURCE_ERROR,
    BANDWIDTH_ERROR,
    NO_SLOTS_AVAILABLE,
    INVALID_STREAM_TYPE,
    SLOT_NOT_ENABLED,
    ENDPOINT_NOT_ENABLED,
    SHORT_PACKET,
    RING_UNDERRUN,
    RING_OVERRUN,
    VF_EVENT_RING_FULL,
    PARAMETER_ERROR,
    BANDWIDTH_OVERRUN,
    CONTEXT_STATE_ERROR,
    NO_PING_RESPONSE,
    EVENT_RING_FULL,
    IMCOMPATIBLE_DEVICE,
    MISSED_SERVICE,
    COMMAND_RING_STOPPED,
    COMMAND_ABORTED,
    STOPPED,
    STOPPED_LENGTH_INVALID,
    STOPPED_SHORT_PACKET,
    MAX_EXIT_LATENCY_TOO_LARGE,
    // reserved = 30,
    ISOCH_BUFFER_OVERRUN = 31,
    EVENT_LOST,
    UNDEFINED_ERROR,
    INVALID_STREAM_ID,
    SECONDARY_BANDWIDTH,
    SPLIT_TRANSACTION,
}

/// TRB for LINK
pub struct TrbLink(TrbRawData);

impl TrbLink {
    #[inline]
    pub fn new(ptr: u64, toggle_cycle: bool) -> Self {
        let result: Self = unsafe { transmute(Trb::new(TrbType::LINK)) };
        result.set_ptr(ptr);
        if toggle_cycle {
            result.raw_data()[3].fetch_or(0x02, Ordering::Release);
        }
        result
    }
}

impl TrbCommon for TrbLink {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbPtr for TrbLink {}

/// TRB for NORMAL
pub struct TrbNormal(TrbRawData);

impl TrbNormal {
    pub fn new(ptr: u64, xfer_len: usize, ioc: bool, isp: bool) -> Self {
        let result: Self = unsafe { transmute(Trb::new(TrbType::NORMAL)) };
        result.set_ptr(ptr);
        result.set_xfer_len(xfer_len);
        result.set_ioc(ioc);
        result.set_isp(isp);
        result
    }
}

impl TrbCommon for TrbNormal {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbPtr for TrbNormal {}

impl TrbXferLen for TrbNormal {}

impl TrbIsp for TrbNormal {}

impl TrbIoC for TrbNormal {}

impl TrbIDt for TrbNormal {}

/// TRB for SETUP
pub struct TrbSetupStage(TrbRawData);

impl TrbSetupStage {
    pub fn new(trt: TrbTranfserType, setup: UsbControlSetupData) -> Self {
        let result: Self = unsafe { transmute(Trb::new(TrbType::SETUP)) };
        unsafe {
            (&result as *const Self as *mut u32).copy_from(&setup as *const _ as *const u32, 2);
        }
        result.set_xfer_len(8);
        result.set_idt(true);
        result.raw_data()[3].fetch_or((trt as u32) << 16, Ordering::SeqCst);
        result
    }
}

impl TrbCommon for TrbSetupStage {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbXferLen for TrbSetupStage {}

impl TrbIDt for TrbSetupStage {}

/// TRB for DATA
pub struct TrbDataStage(TrbRawData);

impl TrbDataStage {
    pub fn new(ptr: u64, xfer_len: usize, dir: bool, isp: bool) -> Self {
        let result: Self = unsafe { transmute(Trb::new(TrbType::DATA)) };
        result.set_ptr(ptr);
        result.set_xfer_len(xfer_len);
        result.set_dir(dir);
        result.set_isp(isp);
        result
    }
}

impl TrbCommon for TrbDataStage {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbPtr for TrbDataStage {}

impl TrbXferLen for TrbDataStage {}

impl TrbIsp for TrbDataStage {}

impl TrbIoC for TrbDataStage {}

impl TrbDir for TrbDataStage {}

/// TRB for STATUS
pub struct TrbStatusStage(TrbRawData);

impl TrbStatusStage {
    pub fn new(dir: bool) -> Self {
        let result: Self = unsafe { transmute(Trb::new(TrbType::STATUS)) };
        result.set_dir(dir);
        result.set_ioc(true);
        result
    }
}

impl TrbCommon for TrbStatusStage {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbIoC for TrbStatusStage {}

impl TrbDir for TrbStatusStage {}

pub enum TrbEvent<'a> {
    CommandCompletion(&'a TrbCce),
    PortStatusChange(&'a TrbPsc),
    TransferEvent(&'a TrbTxe),
    DeviceNotification(&'a TrbDne),
}

impl TrbEvent<'_> {
    #[inline]
    pub fn completion_code(&self) -> Option<TrbCompletionCode> {
        match self {
            TrbEvent::CommandCompletion(ref v) => v.completion_code(),
            TrbEvent::PortStatusChange(ref v) => v.completion_code(),
            TrbEvent::TransferEvent(ref v) => v.completion_code(),
            &TrbEvent::DeviceNotification(ref v) => v.completion_code(),
        }
    }
}

/// TRB for COMMAND_COMPLETION_EVENT
pub struct TrbCce(TrbRawData);

impl TrbCce {
    #[inline]
    pub const fn empty() -> Self {
        unsafe { transmute(Trb::empty()) }
    }

    #[inline]
    pub fn copied(&self) -> Self {
        let result = Self::empty();
        result.raw_copy_from(self);
        result
    }
}

impl TrbCommon for TrbCce {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbCC for TrbCce {}

impl TrbSlotId for TrbCce {}

impl TrbPtr for TrbCce {}

/// TRB for PORT_STATUS_CHANGE_EVENT
pub struct TrbPsc(TrbRawData);

impl TrbCommon for TrbPsc {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbCC for TrbPsc {}

impl TrbPortId for TrbPsc {}

/// TRB for TRANSFER_EVENT
pub struct TrbTxe(TrbRawData);

impl TrbTxe {
    #[inline]
    pub const fn empty() -> Self {
        unsafe { transmute(Trb::empty()) }
    }

    #[inline]
    pub fn copied(&self) -> Self {
        let result = Self::empty();
        result.raw_copy_from(self);
        result
    }

    #[inline]
    pub fn transfer_length(&self) -> usize {
        (self.raw_data()[2].load(Ordering::SeqCst) & 0x00FF_FFFF) as usize
    }

    #[inline]
    pub fn is_event_data(&self) -> bool {
        (self.raw_data()[3].load(Ordering::Relaxed) & 4) != 0
    }
}

impl TrbCommon for TrbTxe {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbCC for TrbTxe {}

impl TrbSlotId for TrbTxe {}

impl TrbPtr for TrbTxe {}

/// TRB for DEVICE_NOTIFICATION_EVENT
pub struct TrbDne(TrbRawData);

impl TrbDne {
    #[inline]
    pub const fn empty() -> Self {
        unsafe { transmute(Trb::empty()) }
    }

    #[inline]
    pub fn copied(&self) -> Self {
        let result = Self::empty();
        result.raw_copy_from(self);
        result
    }
}

impl TrbCommon for TrbDne {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbCC for TrbDne {}

impl TrbSlotId for TrbDne {}

impl TrbPtr for TrbDne {}

/// TRB for ADDRESS_DEVICE_COMMAND
pub struct TrbAddressDeviceCommand(TrbRawData);

impl TrbAddressDeviceCommand {
    #[inline]
    pub fn new(slot_id: SlotId, input_context_ptr: u64) -> Self {
        let result: Self = unsafe { transmute(Trb::new(TrbType::ADDRESS_DEVICE_COMMAND)) };
        result.set_slot_id(slot_id);
        result.set_ptr(input_context_ptr);
        result
    }
}

impl TrbCommon for TrbAddressDeviceCommand {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbSlotId for TrbAddressDeviceCommand {}

impl TrbPtr for TrbAddressDeviceCommand {}

/// TRB for CONFIGURE_ENDPOINT_COMMAND
pub struct TrbConfigureEndpointCommand(TrbRawData);

impl TrbConfigureEndpointCommand {
    #[inline]
    pub fn new(slot_id: SlotId, input_context_ptr: u64) -> Self {
        let result: Self = unsafe { transmute(Trb::new(TrbType::CONFIGURE_ENDPOINT_COMMAND)) };
        result.set_slot_id(slot_id);
        result.set_ptr(input_context_ptr);
        result
    }
}

impl TrbCommon for TrbConfigureEndpointCommand {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbSlotId for TrbConfigureEndpointCommand {}

impl TrbPtr for TrbConfigureEndpointCommand {}

/// TRB for EVALUATE_CONTEXT_COMMAND
pub struct TrbEvaluateContextCommand(TrbRawData);

impl TrbEvaluateContextCommand {
    #[inline]
    pub fn new(slot_id: SlotId, input_context_ptr: u64) -> Self {
        let result: Self = unsafe { transmute(Trb::new(TrbType::EVALUATE_CONTEXT_COMMAND)) };
        result.set_slot_id(slot_id);
        result.set_ptr(input_context_ptr);
        result
    }
}

impl TrbCommon for TrbEvaluateContextCommand {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbSlotId for TrbEvaluateContextCommand {}

impl TrbPtr for TrbEvaluateContextCommand {}

/// TRB for RESET_ENDPOINT_COMMAND
pub struct TrbResetEndpointCommand(TrbRawData);

impl TrbResetEndpointCommand {
    #[inline]
    pub fn new(slot_id: SlotId, dci: DCI) -> Self {
        let result: Self = unsafe { transmute(Trb::new(TrbType::RESET_ENDPOINT_COMMAND)) };
        result.set_slot_id(slot_id);
        result.set_dci(dci);
        result
    }
}

impl TrbCommon for TrbResetEndpointCommand {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbSlotId for TrbResetEndpointCommand {}

impl TrbDci for TrbResetEndpointCommand {}

/// xHC Event Ring Segment Table Entry
#[allow(dead_code)]
#[repr(C)]
pub struct EventRingSegmentTableEntry {
    base: u64,
    size: u16,
    _rsrv1: u16,
    _rsrv2: u32,
}

impl EventRingSegmentTableEntry {
    #[inline]
    pub const fn new(base: u64, size: u16) -> Self {
        Self {
            base,
            size,
            _rsrv1: 0,
            _rsrv2: 0,
        }
    }

    #[inline]
    pub const fn base(&self) -> u64 {
        self.base
    }

    #[inline]
    pub const fn size(&self) -> u16 {
        self.size
    }
}

#[repr(C)]
pub struct InputControlContext {
    drop: u32,
    add: u32,
    _reserved: [u32; 5],
    _word7: u32,
}

impl InputControlContext {
    #[inline]
    pub fn clear(&mut self) {
        unsafe {
            (self as *mut Self).write_bytes(0, 1);
        }
    }

    #[inline]
    pub fn set_add(&mut self, val: u32) {
        self.add = val;
    }

    #[inline]
    pub fn set_drop(&mut self, val: u32) {
        self.drop = val;
    }
}

#[repr(C)]
pub struct SlotContext {
    data: [u32; 8],
}

impl SlotContext {
    #[inline]
    pub const fn route_string(&self) -> UsbRouteString {
        UsbRouteString::from_raw(self.data[0])
    }

    #[inline]
    pub fn set_route_string(&mut self, route: UsbRouteString) {
        self.data[0] = self.data[0] & 0xFFF0_0000 | route.as_u32();
    }

    #[inline]
    pub fn set_speed(&mut self, speed: usize) {
        self.data[0] = self.data[0] & 0xFF0F_FFFF | ((speed as u32) << 20)
    }

    #[inline]
    pub fn speed_raw(&self) -> usize {
        (self.data[0] >> 20) as usize & 15
    }

    #[inline]
    pub const fn set_is_mtt(&mut self, value: bool) {
        let bit = 1 << 25;
        if value {
            self.data[0] |= bit;
        } else {
            self.data[0] &= !bit;
        }
    }

    #[inline]
    pub const fn set_is_hub(&mut self, value: bool) {
        let bit = 1 << 26;
        if value {
            self.data[0] |= bit;
        } else {
            self.data[0] &= !bit;
        }
    }

    #[inline]
    pub const fn context_entries(&self) -> usize {
        (self.data[0] as usize) >> 27
    }

    #[inline]
    pub fn set_context_entries(&mut self, val: usize) {
        self.data[0] = self.data[0] & 0x07FF_FFFF | ((val as u32) << 27);
    }

    #[inline]
    pub fn max_exit_latency(&mut self) -> usize {
        (self.data[1] & 0xFFFF) as usize
    }

    #[inline]
    pub fn set_max_exit_latency(&mut self, val: usize) {
        self.data[1] = self.data[1] & 0xFFFF_0000 | ((val & 0xFFFF) as u32);
    }

    #[inline]
    pub fn root_hub_port(&self) -> Option<PortId> {
        NonZeroU8::new((self.data[1] >> 16) as u8).map(|v| PortId(v))
    }

    #[inline]
    pub fn set_root_hub_port(&mut self, port_id: PortId) {
        self.data[1] = self.data[1] & 0xFF00_FFFF | ((port_id.0.get() as u32) << 16)
    }

    #[inline]
    pub fn num_ports(&mut self) -> usize {
        (self.data[1] >> 24) as usize
    }

    #[inline]
    pub fn set_num_ports(&mut self, num: usize) {
        self.data[1] = self.data[1] & 0x00FF_FFFF | ((num as u32) << 24);
    }

    #[inline]
    pub fn set_parent_hub_slot_id(&mut self, slot_id: SlotId) {
        self.data[2] = self.data[2] & 0xFFFF_FF00 | (slot_id.0.get() as u32);
    }

    #[inline]
    pub fn set_parent_port_id(&mut self, port_id: UsbHubPortNumber) {
        self.data[2] = self.data[2] & 0xFFFF_00FF | ((port_id.0.get() as u32) << 8);
    }

    #[inline]
    pub fn set_ttt(&mut self, ttt: usize) {
        self.data[2] = self.data[2] & 0xFFFC_FFFF | ((ttt as u32) << 16);
    }
}

pub struct EndpointContext {
    data: [u32; 8],
}

impl EndpointContext {
    #[inline]
    pub fn set_interval(&mut self, value: u8) {
        self.data[0] = self.data[0] & 0xFF00_FFFF | ((value as u32) << 16);
    }

    #[inline]
    pub fn set_ep_type(&mut self, ep_type: EpType) {
        self.data[1] = self.data[1] & 0xFFFF_FFC7 | ((ep_type as u32) << 3)
    }

    #[inline]
    pub fn set_max_packet_size(&mut self, max_packet_size: usize) {
        self.data[1] = self.data[1] & 0x0000_FFFF | ((max_packet_size as u32) << 16);
    }

    #[inline]
    pub fn set_max_burst_size(&mut self, max_burst_size: usize) {
        self.data[1] = self.data[1] & 0xFFFF_00FF | ((max_burst_size as u32) << 8);
    }

    #[inline]
    pub fn set_average_trb_len(&mut self, average_trb_len: usize) {
        self.data[4] = self.data[4] & 0xFFFF_0000 | (average_trb_len as u32);
    }

    #[inline]
    pub fn set_error_count(&mut self, cerr: usize) {
        self.data[1] = self.data[1] & 0xFFFF_FFF9 | (cerr as u32 * 2);
    }

    #[inline]
    pub fn set_trdp(&mut self, dp: u64) {
        self.data[2] = dp as u32;
        self.data[3] = (dp >> 32) as u32;
    }

    #[inline]
    pub const fn trdp(&self) -> u64 {
        (self.data[2] as u64) + (self.data[3] as u64) * 0x10000
    }
}

/// Endpoint Type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum EpType {
    Invalid = 0,
    IsochOut,
    BulkOut,
    InterruptOut,
    Control,
    IsochIn,
    BulkIn,
    InterruptIn,
}

impl EpType {
    #[inline]
    pub fn from_usb_ep_type(usb_ep_type: UsbEndpointType, dir: bool) -> Self {
        FromPrimitive::from_usize((dir as usize * 4) + usb_ep_type as usize)
            .unwrap_or(Self::Invalid)
    }

    #[inline]
    pub const fn is_control(&self) -> bool {
        match self {
            EpType::Control => true,
            _ => false,
        }
    }

    #[inline]
    pub const fn is_isochronous(&self) -> bool {
        match self {
            EpType::IsochOut | EpType::IsochIn => true,
            _ => false,
        }
    }

    #[inline]
    pub const fn is_bulk(&self) -> bool {
        match self {
            EpType::BulkOut | EpType::BulkIn => true,
            _ => false,
        }
    }

    #[inline]
    pub const fn is_interrupt(&self) -> bool {
        match self {
            EpType::InterruptOut | EpType::InterruptIn => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
pub enum TrbTranfserType {
    NoData = 0,
    ControlOut = 2,
    ControlIn = 3,
}
