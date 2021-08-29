//! xHC Data Structures

use core::mem::transmute;
use core::num::NonZeroU8;
use core::sync::atomic::*;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PortId(pub NonZeroU8);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SlotId(pub NonZeroU8);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EpNo(pub NonZeroU8);

impl EpNo {
    pub const CONTROL: Self = Self(unsafe { NonZeroU8::new_unchecked(1) });

    #[inline]
    pub const fn ep_out(ep: u8) -> Option<Self> {
        if ep >= 1 && ep <= 15 {
            match NonZeroU8::new(ep * 2) {
                Some(v) => Some(Self(v)),
                None => None,
            }
        } else {
            None
        }
    }

    #[inline]
    pub const fn ep_in(ep: u8) -> Option<Self> {
        if ep >= 1 && ep <= 15 {
            match NonZeroU8::new(ep * 2 + 1) {
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
    pub const fn dir(&self) -> bool {
        (self.0.get() & 1) != 0
    }

    #[inline]
    pub const fn end_point(&self) -> usize {
        (self.0.get() / 2) as usize
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

/// Common Transfer Request Block
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
    fn is_valid(&self) -> bool {
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

pub enum TrbEvent<'a> {
    CommandCompletion(&'a TrbCce),
    PortStatusChange(&'a TrbPsc),
    TransferEvent(&'a TrbTxe),
}

impl TrbEvent<'_> {
    #[inline]
    pub fn completion_code(&self) -> Option<TrbCompletionCode> {
        match self {
            TrbEvent::CommandCompletion(ref v) => v.completion_code(),
            TrbEvent::PortStatusChange(ref v) => v.completion_code(),
            TrbEvent::TransferEvent(ref v) => v.completion_code(),
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

impl TrbCommon for TrbTxe {
    #[inline]
    fn raw_data(&self) -> &TrbRawData {
        &self.0
    }
}

impl TrbCC for TrbTxe {}

impl TrbSlotId for TrbTxe {}

impl TrbPtr for TrbTxe {}

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
    pub const fn context_entries(&self) -> usize {
        (self.data[0] as usize) >> 27
    }

    #[inline]
    pub fn set_context_entries(&mut self, val: usize) {
        self.data[0] = self.data[0] & 0x07FF_FFFF | ((val as u32) << 27);
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
    pub fn set_speed(&mut self, speed: usize) {
        self.data[0] = self.data[0] & 0xFF0F_FFFF | ((speed as u32) << 20)
    }

    #[inline]
    pub fn speed_raw(&self) -> usize {
        (self.data[0] >> 20) as usize & 15
    }
}

pub struct EndpointContext {
    data: [u32; 8],
}

impl EndpointContext {
    #[inline]
    pub fn set_ep_type(&mut self, ep_type: EpType) {
        self.data[1] = self.data[1] & 0xFFFF_FFC7 | ((ep_type as u32) << 3)
    }

    #[inline]
    pub fn set_max_packet_size(&mut self, max_packet_size: usize) {
        self.data[1] = self.data[1] & 0x0000_FFFF | ((max_packet_size as u32) << 16);
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
