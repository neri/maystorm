//! Human Interface Device Manager

use crate::{sync::atomic::AtomicBitflags, sync::RwLock, ui::window::*, *};
use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};
use core::{
    num::*,
    sync::atomic::{AtomicUsize, Ordering},
};
use megstd::{drawing::*, io::hid::*};
use num_traits::FromPrimitive;

const INVALID_UNICHAR: char = '\u{FEFF}';

#[derive(Debug, Clone, Copy)]
pub struct KeyEventFlags(u8);

impl KeyEventFlags {
    pub const BREAK: Self = Self(0b1000_0000);

    #[inline]
    pub const fn bits(&self) -> u8 {
        self.0
    }

    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[inline]
    pub const fn from_bits_retain(val: u8) -> Self {
        Self(val)
    }

    #[inline]
    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

/// USB HID BOOT Keyboard Raw Report
#[repr(C, packed)]
#[derive(Debug, Copy, Clone, Default)]
pub struct KeyReportRaw {
    pub modifier: Modifier,
    _reserved_1: u8,
    pub keydata: [Usage; 6],
}

impl KeyReportRaw {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            modifier: Modifier::empty(),
            _reserved_1: 0,
            keydata: [Usage::NONE; 6],
        }
    }
}

#[derive(Debug)]
pub struct KeyboardState {
    pub current: KeyReportRaw,
    pub prev: KeyReportRaw,
}

impl KeyboardState {
    #[inline]
    pub const fn new() -> Self {
        Self {
            current: KeyReportRaw::empty(),
            prev: KeyReportRaw::empty(),
        }
    }

    pub fn process_report(&mut self, report: KeyReportRaw) {
        self.prev = self.current;
        self.current = report;
        for modifier in Usage::MOD_MIN.0..Usage::MOD_MAX.0 {
            let bit = 1u8 << (modifier - Usage::MOD_MIN.0);
            if (self.current.modifier.bits() & bit) == 0 && (self.prev.modifier.bits() & bit) != 0 {
                KeyEvent::new(Usage(modifier), Modifier::empty(), KeyEventFlags::BREAK).post();
            }
        }
        for modifier in Usage::MOD_MIN.0..Usage::MOD_MAX.0 {
            let bit = 1u8 << (modifier - Usage::MOD_MIN.0);
            if (self.current.modifier.bits() & bit) != 0 && (self.prev.modifier.bits() & bit) == 0 {
                KeyEvent::new(Usage(modifier), Modifier::empty(), KeyEventFlags::empty()).post();
            }
        }
        for usage in &self.prev.keydata {
            let usage = *usage;
            if usage != Usage::NONE
                && usage != Usage::ERR_ROLL_OVER
                && !self.current.keydata.contains(&usage)
            {
                KeyEvent::new(usage, Modifier::empty(), KeyEventFlags::BREAK).post();
            }
        }
        for usage in &self.current.keydata {
            let usage = *usage;
            if usage != Usage::NONE
                && usage != Usage::ERR_ROLL_OVER
                && !self.prev.keydata.contains(&usage)
            {
                KeyEvent::new(usage, Modifier::empty(), KeyEventFlags::empty()).post();
            }
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct KeyEvent(pub NonZeroU32);

impl KeyEvent {
    #[inline]
    pub const fn new(usage: Usage, modifier: Modifier, flags: KeyEventFlags) -> Self {
        unsafe {
            Self(NonZeroU32::new_unchecked(
                usage.0 as u32 | ((modifier.bits() as u32) << 16) | ((flags.bits() as u32) << 24),
            ))
        }
    }

    #[inline]
    pub fn into_char(self) -> char {
        HidManager::key_event_to_char(self)
    }

    #[inline]
    pub const fn usage(self) -> Usage {
        Usage(self.0.get() as u8)
    }

    #[inline]
    pub const fn modifier(self) -> Modifier {
        Modifier::from_bits_retain(((self.0.get() >> 16) & 0xFF) as u8)
    }

    #[inline]
    pub const fn flags(self) -> KeyEventFlags {
        KeyEventFlags::from_bits_retain(((self.0.get() >> 24) & 0xFF) as u8)
    }

    #[inline]
    pub fn is_make(&self) -> bool {
        !self.is_break()
    }

    #[inline]
    pub fn is_break(&self) -> bool {
        self.flags().contains(KeyEventFlags::BREAK)
    }

    /// Returns the data for which a valid key was pressed. Otherwise, it is None.
    #[inline]
    pub fn key_data(self) -> Option<Self> {
        if self.usage() != Usage::NONE
            && !(self.usage() >= Usage::MOD_MIN && self.usage() <= Usage::MOD_MAX)
            && !self.is_break()
        {
            Some(self)
        } else {
            None
        }
    }

    #[inline]
    pub fn post(self) {
        HidManager::post_key_event(self);
    }
}

impl Into<char> for KeyEvent {
    #[inline]
    fn into(self) -> char {
        self.into_char()
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct MouseState {
    pub current_buttons: MouseButton,
    pub prev_buttons: MouseButton,
    pub x: isize,
    pub y: isize,
    pub wheel: isize,
    pub max_x: isize,
    pub max_y: isize,
}

impl MouseState {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            current_buttons: MouseButton::empty(),
            prev_buttons: MouseButton::empty(),
            x: 0,
            y: 0,
            wheel: 0,
            max_x: 0,
            max_y: 0,
        }
    }

    #[inline]
    pub fn process_relative_report<T>(&mut self, report: MouseReport<T>)
    where
        T: Into<isize> + Copy,
    {
        self.prev_buttons = self.current_buttons;
        self.current_buttons = report.buttons;
        self.x += report.x.into();
        self.y += report.y.into();
        WindowManager::post_relative_pointer(self);
    }

    #[inline]
    pub fn process_absolute_report<T>(&mut self, report: MouseReport<T>)
    where
        T: Into<isize> + Copy,
    {
        self.prev_buttons = self.current_buttons;
        self.current_buttons = report.buttons;
        self.x = report.x.into();
        self.y = report.y.into();
        WindowManager::post_absolute_pointer(self);
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct MouseEvent {
    pub x: i16,
    pub y: i16,
    pub buttons: MouseButton,
    pub event_buttons: MouseButton,
}

impl MouseEvent {
    #[inline]
    pub const fn new(point: Point, buttons: MouseButton, event_buttons: MouseButton) -> Self {
        Self {
            x: point.x as i16,
            y: point.y as i16,
            buttons,
            event_buttons,
        }
    }

    #[inline]
    pub const fn point(&self) -> Point {
        Point {
            x: self.x as isize,
            y: self.y as isize,
        }
    }

    #[inline]
    pub const fn buttons(&self) -> MouseButton {
        self.buttons
    }

    #[inline]
    pub const fn event_buttons(&self) -> MouseButton {
        self.event_buttons
    }
}

#[derive(Debug)]
pub struct HidParsedReport {
    pub report_ids: Vec<HidReportId>,
    primary: Option<ParsedReportApplication>,
    tagged: BTreeMap<HidReportId, ParsedReportApplication>,
}

impl HidParsedReport {
    #[inline]
    pub const fn new() -> Self {
        Self {
            report_ids: Vec::new(),
            primary: None,
            tagged: BTreeMap::new(),
        }
    }

    #[inline]
    pub fn has_report_id(&self) -> bool {
        self.report_ids.len() > 0
    }

    #[inline]
    pub fn primary_app(&self) -> Option<&ParsedReportApplication> {
        self.primary.as_ref()
    }

    #[inline]
    pub fn app_by_report_id(&self, report_id: HidReportId) -> Option<&ParsedReportApplication> {
        self.tagged.get(&report_id)
    }

    #[inline]
    pub fn applications(&self) -> impl Iterator<Item = &ParsedReportApplication> {
        self.tagged.values()
    }

    pub fn parse(report_desc: &[u8]) -> Result<HidParsedReport, usize> {
        let mut parsed_report = HidParsedReport::new();

        let mut current_app = ParsedReportApplication::empty();
        let mut collection_ctx = Vec::new();

        let mut reader = HidReporteReader::new(report_desc);
        let mut stack = Vec::new();
        let mut global = HidReportGlobalState::new();
        let mut local = HidReportLocalState::new();

        while let Some(lead_byte) = reader.next() {
            let lead_byte = HidReportLeadByte(lead_byte);
            let (tag, param) = if lead_byte.is_long_item() {
                let len = match reader.next() {
                    Some(v) => v as usize,
                    None => return Err(reader.position()),
                };
                let lead_byte = match reader.next() {
                    Some(v) => v,
                    None => return Err(reader.position()),
                };
                if reader.advance_by(len).is_err() {
                    return Err(reader.position());
                }
                let lead_byte = HidReportLeadByte(lead_byte);
                let tag = match lead_byte.item_tag() {
                    Some(v) => v,
                    None => {
                        log!("UNKNOWN TAG {} {:02x}", reader.position(), lead_byte.0);
                        return Err(reader.position());
                    }
                };
                let param = HidReportAmbiguousSignedValue::Zero;
                (tag, param)
            } else {
                let tag = match lead_byte.item_tag() {
                    Some(v) => v,
                    None => {
                        log!("UNKNOWN TAG {} {:02x}", reader.position(), lead_byte.0);
                        return Err(reader.position());
                    }
                };
                let param = match reader.read_param(lead_byte) {
                    Some(v) => v,
                    None => return Err(reader.position()),
                };
                (tag, param)
            };

            match tag {
                HidReportItemTag::Input => {
                    let flag = HidReportMainFlag::from_bits_retain(param.into());
                    ParsedReportMainItem::parse(
                        &mut current_app.entries,
                        tag,
                        flag,
                        &global,
                        &local,
                    );
                    local.reset();
                }
                HidReportItemTag::Output => {
                    let flag = HidReportMainFlag::from_bits_retain(param.into());
                    ParsedReportMainItem::parse(
                        &mut current_app.entries,
                        tag,
                        flag,
                        &global,
                        &local,
                    );
                    local.reset();
                }
                HidReportItemTag::Feature => {
                    let flag = HidReportMainFlag::from_bits_retain(param.into());
                    ParsedReportMainItem::parse(
                        &mut current_app.entries,
                        tag,
                        flag,
                        &global,
                        &local,
                    );
                    local.reset();
                }

                HidReportItemTag::Collection => {
                    let collection_type: HidReportCollectionType =
                        match FromPrimitive::from_usize(param.into()) {
                            Some(v) => v,
                            None => todo!(),
                        };
                    collection_ctx.push(collection_type);

                    match collection_type {
                        HidReportCollectionType::Application => {
                            if collection_ctx.contains(&HidReportCollectionType::Application) {
                                match current_app.report_id {
                                    Some(report_id) => {
                                        parsed_report.tagged.insert(report_id, current_app.clone());
                                    }
                                    None => {
                                        parsed_report.primary = Some(current_app.clone());
                                    }
                                }
                            }
                            current_app.clear_stream();
                            let usage = local.usage.first().map(|v| *v).unwrap_or_default();
                            current_app.usage = if usage < 0x10000 {
                                HidUsage::new(global.usage_page, usage as u16)
                            } else {
                                HidUsage(usage)
                            };
                        }
                        _ => {
                            current_app
                                .entries
                                .push(ParsedReportEntry::Collection(collection_type));
                        }
                    }

                    local.reset();
                }

                HidReportItemTag::EndCollection => {
                    match collection_ctx.pop() {
                        Some(collection_type) => match collection_type {
                            HidReportCollectionType::Application => {
                                match current_app.report_id {
                                    Some(report_id) => {
                                        parsed_report.tagged.insert(report_id, current_app.clone());
                                    }
                                    None => {
                                        parsed_report.primary = Some(current_app.clone());
                                    }
                                }
                                current_app.clear();
                            }
                            _ => {
                                current_app
                                    .entries
                                    .push(ParsedReportEntry::EndCollection(collection_type));
                            }
                        },
                        None => return Err(reader.position()),
                    }
                    local.reset();
                }

                HidReportItemTag::UsagePage => global.usage_page = UsagePage(param.into()),
                HidReportItemTag::LogicalMinimum => global.logical_minimum = param,
                HidReportItemTag::LogicalMaximum => global.logical_maximum = param,
                HidReportItemTag::PhysicalMinimum => global.physical_minimum = param,
                HidReportItemTag::PhysicalMaximum => global.physical_maximum = param,
                HidReportItemTag::UnitExponent => global.unit_exponent = param.into(),
                HidReportItemTag::Unit => global.unit = param.into(),
                HidReportItemTag::ReportSize => global.report_size = param.into(),

                HidReportItemTag::ReportId => {
                    if let Some(report_id) = HidReportId::new(param.into()) {
                        if !parsed_report.report_ids.contains(&report_id) {
                            parsed_report.report_ids.push(report_id);
                        }
                        current_app.report_id = Some(report_id);
                        global.report_id = Some(report_id);
                    }
                }

                HidReportItemTag::ReportCount => global.report_count = param.into(),
                HidReportItemTag::Push => stack.push(global),
                HidReportItemTag::Pop => {
                    global = match stack.pop() {
                        Some(v) => v,
                        None => return Err(reader.position()),
                    }
                }
                HidReportItemTag::Usage => local.usage.push(param.into()),
                HidReportItemTag::UsageMinimum => local.usage_minimum = param.into(),
                HidReportItemTag::UsageMaximum => local.usage_maximum = param.into(),

                // HidReportItemTag::DesignatorIndex => todo!(),
                // HidReportItemTag::DesignatorMinimum => todo!(),
                // HidReportItemTag::DesignatorMaximum => todo!(),
                // HidReportItemTag::StringIndex => todo!(),
                // HidReportItemTag::StringMinimum => todo!(),
                // HidReportItemTag::StringMaximum => todo!(),
                // HidReportItemTag::Delimiter => todo!(),
                _ => todo!(),
            }
        }

        Ok(parsed_report)
    }

    #[inline]
    pub fn initial_bit_position(&self) -> usize {
        if self.has_report_id() {
            8
        } else {
            0
        }
    }
}

#[derive(Clone)]
pub struct ParsedReportApplication {
    report_id: Option<HidReportId>,
    usage: HidUsage,
    entries: Vec<ParsedReportEntry>,
}

impl ParsedReportApplication {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            report_id: None,
            usage: HidUsage::NONE,
            entries: Vec::new(),
        }
    }

    #[inline]
    pub const fn report_id(&self) -> Option<HidReportId> {
        self.report_id
    }

    #[inline]
    pub const fn usage(&self) -> HidUsage {
        self.usage
    }

    #[inline]
    pub fn entries(&self) -> impl Iterator<Item = &ParsedReportEntry> {
        self.entries.iter()
    }

    #[inline]
    pub fn input_items(&self) -> impl Iterator<Item = &ParsedReportMainItem> {
        self.entries().flat_map(|v| match v {
            ParsedReportEntry::Input(v) => Some(v),
            _ => None,
        })
    }

    #[inline]
    pub fn output_items(&self) -> impl Iterator<Item = &ParsedReportMainItem> {
        self.entries().flat_map(|v| match v {
            ParsedReportEntry::Output(v) => Some(v),
            _ => None,
        })
    }

    #[inline]
    pub fn features(&self) -> impl Iterator<Item = &ParsedReportMainItem> {
        self.entries().flat_map(|v| match v {
            ParsedReportEntry::Feature(v) => Some(v),
            _ => None,
        })
    }

    pub fn clear_stream(&mut self) {
        self.entries = Vec::new();
    }

    pub fn clear(&mut self) {
        self.report_id = None;
        self.usage = HidUsage::NONE;
        self.clear_stream();
    }

    pub fn bit_count_for_input(&self) -> usize {
        let acc = self.bit_count(|v| matches!(v, ParsedReportEntry::Input(_)));
        if acc > 0 && self.report_id.is_some() {
            acc + 8
        } else {
            acc
        }
    }

    pub fn bit_count_for_output(&self) -> usize {
        self.bit_count(|v| matches!(v, ParsedReportEntry::Output(_)))
    }

    pub fn bit_count_for_feature(&self) -> usize {
        self.bit_count(|v| matches!(v, ParsedReportEntry::Feature(_)))
    }

    pub fn bit_count<F>(&self, predicate: F) -> usize
    where
        F: FnMut(&&ParsedReportEntry) -> bool,
    {
        let acc = self
            .entries
            .iter()
            .filter(predicate)
            .fold(0, |acc, v| acc + v.bit_count());
        acc
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ParsedReportEntry {
    Input(ParsedReportMainItem),
    Output(ParsedReportMainItem),
    Feature(ParsedReportMainItem),
    Collection(HidReportCollectionType),
    EndCollection(HidReportCollectionType),
}

impl ParsedReportEntry {
    pub fn from_item(item: ParsedReportMainItem, tag: HidReportItemTag) -> Option<Self> {
        match tag {
            HidReportItemTag::Input => Some(Self::Input(item)),
            HidReportItemTag::Output => Some(Self::Output(item)),
            HidReportItemTag::Feature => Some(Self::Feature(item)),
            _ => None,
        }
    }

    pub fn bit_count(&self) -> usize {
        match self {
            ParsedReportEntry::Input(ref v) => v.bit_count(),
            ParsedReportEntry::Output(ref v) => v.bit_count(),
            ParsedReportEntry::Feature(ref v) => v.bit_count(),
            ParsedReportEntry::Collection(_) => 0,
            ParsedReportEntry::EndCollection(_) => 0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ParsedReportMainItem {
    flags: HidReportMainFlag,
    report_size: u8,
    report_count: u8,
    usage_min: HidUsage,
    usage_max: HidUsage,
    logical_min: u32,
    logical_max: u32,
    physical_min: u32,
    physical_max: u32,
}

impl ParsedReportMainItem {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            flags: HidReportMainFlag::empty(),
            report_size: 0,
            report_count: 0,
            usage_min: HidUsage::NONE,
            usage_max: HidUsage::NONE,
            logical_min: 0,
            logical_max: 0,
            physical_min: 0,
            physical_max: 0,
        }
    }

    pub fn parse(
        vec: &mut Vec<ParsedReportEntry>,
        tag: HidReportItemTag,
        flag: HidReportMainFlag,
        global: &HidReportGlobalState,
        local: &HidReportLocalState,
    ) {
        if local.usage.len() > 0 {
            let report_count = global.report_count / local.usage.len();
            for usage in &local.usage {
                ParsedReportEntry::from_item(
                    Self::new(flag, global, local, Some(*usage), report_count),
                    tag,
                )
                .map(|v| vec.push(v));
            }
        } else {
            ParsedReportEntry::from_item(Self::new(flag, global, local, None, 0), tag)
                .map(|v| vec.push(v));
        }
    }

    #[inline]
    pub fn new(
        flag: HidReportMainFlag,
        global: &HidReportGlobalState,
        local: &HidReportLocalState,
        usage: Option<u32>,
        report_count: usize,
    ) -> Self {
        let (usage_min, usage_max) = if flag.is_const() {
            (HidUsage(0), HidUsage(0))
        } else {
            let (usage_min, usage_max) = if let Some(usage) = usage {
                (usage, 0)
            } else {
                (local.usage_minimum, local.usage_maximum)
            };
            let usage_max = if usage_min < 0x10000 {
                HidUsage::new(global.usage_page, usage_max as u16)
            } else {
                HidUsage(usage_max)
            };
            let usage_min = if usage_min < 0x10000 {
                HidUsage::new(global.usage_page, usage_min as u16)
            } else {
                HidUsage(usage_min)
            };
            (usage_min, usage_max)
        };

        let report_count = if report_count > 0 {
            report_count
        } else {
            global.report_count
        } as u8;
        let logical_min = if flag.contains(HidReportMainFlag::RELATIVE) {
            global.logical_minimum.as_isize() as u32
        } else {
            global.logical_minimum.as_u32()
        };
        let logical_max = if flag.contains(HidReportMainFlag::RELATIVE) {
            global.logical_maximum.as_isize() as u32
        } else {
            global.logical_maximum.as_u32()
        };
        let physical_min = if flag.contains(HidReportMainFlag::RELATIVE) {
            global.physical_minimum.as_isize() as u32
        } else {
            global.physical_minimum.as_u32()
        };
        let physical_max = if flag.contains(HidReportMainFlag::RELATIVE) {
            global.physical_maximum.as_isize() as u32
        } else {
            global.physical_maximum.as_u32()
        };
        Self {
            flags: flag,
            report_size: global.report_size as u8,
            report_count,
            usage_min,
            usage_max,
            logical_min,
            logical_max,
            physical_min,
            physical_max,
        }
    }

    #[inline]
    pub const fn flags(&self) -> HidReportMainFlag {
        self.flags
    }

    #[inline]
    pub fn is_const(&self) -> bool {
        self.flags.is_const()
    }

    #[inline]
    pub fn is_array(&self) -> bool {
        self.flags.is_array()
    }

    #[inline]
    pub fn is_variable(&self) -> bool {
        self.flags.is_variable()
    }

    #[inline]
    pub fn is_relative(&self) -> bool {
        self.flags.is_relative()
    }

    #[inline]
    pub const fn usage_min(&self) -> HidUsage {
        self.usage_min
    }

    #[inline]
    pub const fn usage_max(&self) -> HidUsage {
        self.usage_max
    }

    #[inline]
    pub const fn logical_min(&self) -> u32 {
        self.logical_min
    }

    #[inline]
    pub const fn logical_max(&self) -> u32 {
        self.logical_max
    }

    #[inline]
    pub const fn report_size(&self) -> usize {
        self.report_size as usize
    }

    #[inline]
    pub const fn report_count(&self) -> usize {
        self.report_count as usize
    }

    #[inline]
    pub const fn bit_count(&self) -> usize {
        self.report_size() * self.report_count()
    }
}

impl core::fmt::Debug for ParsedReportApplication {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let _ = writeln!(
            f,
            "application {:02x} usage {}",
            self.report_id.map(|v| v.as_u8()).unwrap_or_default(),
            self.usage,
        );

        for entry in &self.entries {
            let _ = writeln!(f, "{:?}", entry);
        }

        Ok(())
    }
}

impl core::fmt::Debug for ParsedReportMainItem {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let _ = write!(
            f,
            "{:?} size {} {}",
            self.flags, self.report_size, self.report_count
        );
        if !self.flags.contains(HidReportMainFlag::CONSTANT) {
            if self.usage_max > self.usage_min {
                let _ = write!(f, " usage {}..{}", self.usage_min, self.usage_max);
            } else {
                let _ = write!(f, " usage {}", self.usage_min);
            }
            if self.flags.contains(HidReportMainFlag::RELATIVE) {
                let _ = write!(
                    f,
                    " log {}..{} phy {}..{}",
                    self.logical_min as i32,
                    self.logical_max as i32,
                    self.physical_min as i32,
                    self.physical_max as i32,
                );
            } else {
                let _ = write!(
                    f,
                    " log {}..{} phy {}..{}",
                    self.logical_min, self.logical_max, self.physical_min, self.physical_max
                );
            }
        }

        Ok(())
    }
}

/// HidManager relays between human interface devices and the window event subsystem.
///
/// Keyboard scancodes will be converted to the Usage specified by the USB-HID specification on all platforms.
pub struct HidManager {
    key_modifier: AtomicBitflags<Modifier>,
    simulated_game_input: RwLock<GameInput>,
    game_inputs: RwLock<BTreeMap<GameInputHandle, Arc<RwLock<GameInput>>>>,
    current_game_inputs: RwLock<Option<GameInputHandle>>,
}

static HID_MANAGER: HidManager = HidManager::new();

impl HidManager {
    #[inline]
    const fn new() -> Self {
        HidManager {
            key_modifier: AtomicBitflags::empty(),
            simulated_game_input: RwLock::new(GameInput::empty()),
            game_inputs: RwLock::new(BTreeMap::new()),
            current_game_inputs: RwLock::new(None),
        }
    }

    #[inline]
    pub unsafe fn init() {
        assert_call_once!();
    }

    #[inline]
    fn shared<'a>() -> &'a HidManager {
        &HID_MANAGER
    }

    fn post_key_event(event: KeyEvent) {
        let shared = Self::shared();
        let usage = event.usage();
        if usage >= Usage::MOD_MIN && usage <= Usage::MOD_MAX {
            let bit_position = Modifier::from_bits_retain(1 << (usage.0 - Usage::MOD_MIN.0));
            shared.key_modifier.set(bit_position, !event.is_break());
        }
        let event = KeyEvent::new(usage, shared.key_modifier.value(), event.flags());
        WindowManager::post_key_event(event);
    }

    #[inline]
    fn key_event_to_char(event: KeyEvent) -> char {
        if event.flags().contains(KeyEventFlags::BREAK) || event.usage() == Usage::NONE {
            '\0'
        } else {
            Self::usage_to_char_109(event.usage(), event.modifier())
        }
    }

    fn usage_to_char_109(usage: Usage, modifier: Modifier) -> char {
        let mut uni: char = INVALID_UNICHAR;

        if usage >= Usage::ALPHABET_MIN && usage <= Usage::ALPHABET_MAX {
            uni = (usage.0 - Usage::KEY_A.0 + 0x61) as char;
        } else if usage >= Usage::NUMBER_MIN && usage <= Usage::NON_ALPHABET_MAX {
            uni = USAGE_TO_CHAR_NON_ALPLABET_109[(usage.0 - Usage::NUMBER_MIN.0) as usize];
            if uni > ' ' && uni < '\x40' && uni != '0' && modifier.has_shift() {
                uni = (uni as u8 ^ 0x10) as char;
            }
        } else if usage == Usage::DELETE {
            uni = '\x7F';
        } else if usage >= Usage::NUMPAD_MIN && usage <= Usage::NUMPAD_MAX {
            uni = USAGE_TO_CHAR_NUMPAD[(usage.0 - Usage::NUMPAD_MIN.0) as usize];
        } else if usage == Usage::INTERNATIONAL_3 {
            // '\|'
            uni = '\\';
        }

        if uni >= '\x40' && uni < '\x7F' {
            if modifier.has_ctrl() {
                uni = (uni as u8 & 0x1F) as char;
            } else if modifier.has_shift() {
                uni = (uni as u8 ^ 0x20) as char;
            }
        }

        if usage == Usage::INTERNATIONAL_1 {
            if modifier.has_shift() {
                uni = '_';
            } else {
                uni = '\\';
            }
        }

        uni
    }
}

// Non Alphabet
static USAGE_TO_CHAR_NON_ALPLABET_109: [char; 27] = [
    '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '\x0D', '\x1B', '\x08', '\x09', ' ', '-',
    '^', '@', '[', ']', ']', ';', ':', '`', ',', '.', '/',
];

// Numpads
static USAGE_TO_CHAR_NUMPAD: [char; 16] = [
    '/', '*', '-', '+', '\x0D', '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '.',
];

pub struct GameInputManager;

impl GameInputManager {
    pub fn current_input() -> GameInput {
        let shared = HidManager::shared();

        let game_input = shared.game_inputs.read().unwrap();
        shared
            .current_game_inputs
            .read()
            .unwrap()
            .and_then(|key| game_input.get(&key))
            .map(|v| v.read().unwrap().clone())
            .unwrap_or(shared.simulated_game_input.read().unwrap().clone())
    }

    #[inline]
    fn next_game_input_handle() -> Option<GameInputHandle> {
        static NEXT_HANDLE: AtomicUsize = AtomicUsize::new(1);
        NonZeroUsize::new(NEXT_HANDLE.fetch_add(1, Ordering::AcqRel)).map(|v| GameInputHandle(v))
    }

    pub fn connect_new_input(input: Arc<RwLock<GameInput>>) -> Option<GameInputHandle> {
        Self::next_game_input_handle().map(|handle| {
            let shared = HidManager::shared();
            shared
                .game_inputs
                .write()
                .unwrap()
                .insert(handle, input.clone());
            *shared.current_game_inputs.write().unwrap() = Some(handle);
            handle
        })
    }

    pub fn send_key(event: KeyEvent) {
        let position = match event.usage() {
            Usage::NUMPAD_2 => Some(GameInputButtonType::DpadDown),
            Usage::NUMPAD_4 => Some(GameInputButtonType::DpadLeft),
            Usage::NUMPAD_6 => Some(GameInputButtonType::DpadRight),
            Usage::NUMPAD_8 => Some(GameInputButtonType::DpadUp),

            Usage::KEY_UP_ARROW => Some(GameInputButtonType::DpadUp),
            Usage::KEY_DOWN_ARROW => Some(GameInputButtonType::DpadDown),
            Usage::KEY_RIGHT_ARROW => Some(GameInputButtonType::DpadRight),
            Usage::KEY_LEFT_ARROW => Some(GameInputButtonType::DpadLeft),

            Usage::KEY_W => Some(GameInputButtonType::DpadUp),
            Usage::KEY_A => Some(GameInputButtonType::DpadLeft),
            Usage::KEY_S => Some(GameInputButtonType::DpadDown),
            Usage::KEY_D => Some(GameInputButtonType::DpadRight),

            Usage::KEY_ESCAPE => Some(GameInputButtonType::Menu),
            Usage::KEY_ENTER => Some(GameInputButtonType::Start),
            Usage::KEY_SPACE => Some(GameInputButtonType::Select),

            Usage::KEY_Z => Some(GameInputButtonType::A),
            Usage::KEY_X => Some(GameInputButtonType::B),
            Usage::KEY_C => Some(GameInputButtonType::Start),

            Usage::KEY_LEFT_CONTROL => Some(GameInputButtonType::B),
            Usage::KEY_RIGHT_CONTROL => Some(GameInputButtonType::B),

            Usage::KEY_LEFT_SHIFT => Some(GameInputButtonType::A),
            Usage::KEY_RIGHT_SHIFT => Some(GameInputButtonType::A),

            _ => None,
        };
        if let Some(position) = position {
            let position = 1u16 << (position as usize);
            let mut buttons = HidManager::shared().simulated_game_input.write().unwrap();
            if event.is_break() {
                buttons.bitmap &= !position;
            } else {
                buttons.bitmap |= position;
            }
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GameInputHandle(pub NonZeroUsize);

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GameInput {
    bitmap: u16,
    lt: u8,
    rt: u8,
    x1: u16,
    y1: u16,
    x2: u16,
    y2: u16,
}

impl GameInput {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            bitmap: 0,
            lt: 0,
            rt: 0,
            x1: 0,
            y1: 0,
            x2: 0,
            y2: 0,
        }
    }

    #[inline]
    pub const fn buttons(&self) -> u16 {
        self.bitmap
    }

    #[inline]
    pub fn copy_from(&mut self, other: &Self) {
        unsafe {
            (self as *mut Self).copy_from(other as *const Self, 1);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GameInputButtonType {
    DpadUp = 0,
    DpadDown,
    DpadLeft,
    DpadRight,
    Start,
    Select,
    ThumbL,
    ThumbR,
    LButton,
    RButton,
    Menu,
    _Reserved,
    A,
    B,
    X,
    Y,
}
