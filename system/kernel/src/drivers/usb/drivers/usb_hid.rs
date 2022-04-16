//! USB HID Class Driver (03_xx_xx)

use super::super::*;
use crate::{
    io::hid_mgr::*,
    task::{scheduler::Timer, Task},
    *,
};
use alloc::{boxed::Box, collections::BTreeMap, sync::Arc, vec::Vec};
use core::{num::NonZeroU8, pin::Pin, time::Duration};
use futures_util::Future;
use megstd::io::hid::*;
use num_traits::FromPrimitive;

pub struct UsbHidStarter;

impl UsbHidStarter {
    #[inline]
    pub fn new() -> Arc<dyn UsbInterfaceDriverStarter> {
        Arc::new(Self {})
    }
}

impl UsbInterfaceDriverStarter for UsbHidStarter {
    fn instantiate(
        &self,
        device: &Arc<UsbDeviceControl>,
        if_no: UsbInterfaceNumber,
        class: UsbClass,
    ) -> Pin<Box<dyn Future<Output = Result<Task, UsbError>>>> {
        Box::pin(UsbHidDriver::_instantiate(device.clone(), if_no, class))
    }
}

pub struct UsbHidDriver;

impl UsbHidDriver {
    const BUFFER_LEN: usize = 64;

    async fn _instantiate(
        device: Arc<UsbDeviceControl>,
        if_no: UsbInterfaceNumber,
        class: UsbClass,
    ) -> Result<Task, UsbError> {
        if class.base() != UsbBaseClass::HID {
            return Err(UsbError::Unsupported);
        }
        let interface = match device
            .device()
            .current_configuration()
            .find_interface(if_no, None)
        {
            Some(v) => v,
            None => return Err(UsbError::InvalidParameter),
        };
        let endpoint = match interface.endpoints().first() {
            Some(v) => v,
            None => return Err(UsbError::InvalidDescriptor),
        };
        let ep = endpoint.address();
        let ps = endpoint.descriptor().max_packet_size() as usize;
        if ps > Self::BUFFER_LEN {
            return Err(UsbError::InvalidDescriptor);
        }

        let report_desc = match interface.hid_reports_by(UsbDescriptorType::HidReport) {
            Some(v) => {
                let mut vec = Vec::new();
                vec.extend_from_slice(v);
                vec.into_boxed_slice()
            }
            None => Box::new([]),
        };
        let report_desc = match Self::parse_report(&report_desc) {
            Ok(v) => v,
            Err(err) => {
                log!("HID PARSE ERROR {}", err);
                return Err(UsbError::InvalidDescriptor);
            }
        };

        device.configure_endpoint(endpoint.descriptor()).unwrap();

        // disable boot protocol
        if class.sub() == UsbSubClass(1) {
            let _result = Self::set_boot_protocol(&device, if_no, false).await.is_ok();
        }

        Ok(Task::new(Self::_usb_hid_task(
            device.clone(),
            if_no,
            ep,
            ps,
            report_desc,
        )))
    }

    async fn _usb_hid_task(
        device: Arc<UsbDeviceControl>,
        if_no: UsbInterfaceNumber,
        ep: UsbEndpointAddress,
        ps: usize,
        report_desc: HidParsedReport,
    ) {
        let addr = device.device().addr();

        // log!(
        //     "HID {}:{} VEN {} DEV {} CLASS {} UP {:08x} {:?} {}",
        //     addr.0.get(),
        //     if_no.0,
        //     device.device().vid(),
        //     device.device().pid(),
        //     device.device().class(),
        //     report_desc.primary_app().unwrap().usage.0,
        //     report_desc.report_ids,
        //     device.device().preferred_device_name().unwrap_or_default(),
        // );
        // for app in report_desc.tagged.values() {
        //     log!(" APP {} {:08x}", app.report_id, app.usage.0);
        // }

        if let Some(app) = report_desc.primary_app() {
            let mut data = [0; Self::BUFFER_LEN];
            let empty_data = [0; Self::BUFFER_LEN];
            let mut bit_position = 0;
            match app.usage {
                HidUsage::KEYBOARD => {
                    // Flashing LED on the keyboard
                    for entry in &app.entries {
                        let item = match entry {
                            ParsedReportEntry::Output(item) => {
                                if item.flag.is_const() {
                                    bit_position += item.bit_count();
                                    continue;
                                } else {
                                    item
                                }
                            }
                            _ => continue,
                        };
                        if item.report_size() == 1
                            && item.usage_min().usage_page() == UsagePage::LED
                        {
                            for i in 0..item.report_count() {
                                let _ = Self::write_bits(&mut data, bit_position + i, 1, 1);
                            }
                        }
                        bit_position += item.bit_count();
                    }
                    let len = (bit_position + 7) / 8;
                    if len > 0 {
                        let _ = Self::set_report(
                            &device,
                            if_no,
                            HidReportType::Output,
                            app.report_id,
                            len,
                            &data,
                        )
                        .await;
                        Timer::sleep_async(Duration::from_millis(100)).await;
                        let _ = Self::set_report(
                            &device,
                            if_no,
                            HidReportType::Output,
                            app.report_id,
                            len,
                            &empty_data,
                        )
                        .await;
                        Timer::sleep_async(Duration::from_millis(50)).await;
                    }
                }
                _ => (),
            }
        }

        let mut key_state = KeyboardState::new();
        let mut mouse_state = MouseState::empty();
        let mut buffer = [0; Self::BUFFER_LEN];
        loop {
            buffer.fill(0);
            match device.read_slice(ep, &mut buffer, 1, ps).await {
                Ok(size) => {
                    let app = match if report_desc.has_report_id() {
                        report_desc.app_by_report_id(buffer[0])
                    } else {
                        report_desc.primary_app()
                    } {
                        Some(v) => v,
                        None => continue,
                    };
                    if size * 8 < app.bit_count_input() {
                        log!("HID DATA SIZE {} < {}", size * 8, app.bit_count_input());
                        continue;
                    }

                    let mut bit_position = report_desc.initial_bit_position();
                    match app.usage {
                        HidUsage::KEYBOARD => {
                            let mut report = KeyReportRaw::default();
                            for entry in &app.entries {
                                let item = match entry {
                                    ParsedReportEntry::Input(item) => {
                                        if item.flag.is_const() {
                                            bit_position += item.bit_count();
                                            continue;
                                        } else {
                                            item
                                        }
                                    }
                                    _ => continue,
                                };
                                if item.flag.is_variable()
                                    && item.report_size == 1
                                    && item.usage_min() == Usage::MOD_MIN.full_qualified_usage()
                                    && item.usage_max() == Usage::MOD_MAX.full_qualified_usage()
                                {
                                    report.modifier =
                                        Self::read_bits(&buffer, bit_position, item.report_count())
                                            .map(|v| Modifier::from_bits_truncate(v as u8))
                                            .unwrap();
                                } else if item.report_size() == 8
                                    && item.flag.is_array()
                                    && item.usage_min().usage_page() == UsagePage::KEYBOARD
                                {
                                    let limit =
                                        usize::min(report.keydata.len(), item.report_count());
                                    for i in 0..limit {
                                        report.keydata[i] = Usage(buffer[bit_position / 8 + i]);
                                    }
                                }

                                bit_position += item.bit_count();
                            }
                            key_state.process_key_report(report);
                        }
                        HidUsage::MOUSE => {
                            let mut is_absolute = false;
                            let mut report = MouseReport::default();
                            for entry in &app.entries {
                                let item = match entry {
                                    ParsedReportEntry::Input(item) => {
                                        if item.flag.is_const() {
                                            bit_position += item.bit_count();
                                            continue;
                                        } else {
                                            item
                                        }
                                    }
                                    _ => continue,
                                };
                                match item.usage_min() {
                                    HidUsage::BUTTON_1 => {
                                        if item.flag.is_variable() && item.report_size() == 1 {
                                            report.buttons = Self::read_bits(
                                                &buffer,
                                                bit_position,
                                                item.report_count(),
                                            )
                                            .map(|v| MouseButton::from_bits_truncate(v as u8))
                                            .unwrap();
                                        }
                                    }
                                    HidUsage::X => {
                                        if item.flag.is_relative() {
                                            report.x = match Self::read_bits_signed(
                                                &buffer,
                                                bit_position,
                                                item.report_size(),
                                            ) {
                                                Some(v) => v,
                                                None => todo!(),
                                            }
                                        } else {
                                            is_absolute = true;
                                            mouse_state.max_x = item.logical_max as isize;
                                            report.x = match Self::read_bits(
                                                &buffer,
                                                bit_position,
                                                item.report_size(),
                                            ) {
                                                Some(v) => v as isize,
                                                None => todo!(),
                                            }
                                        }
                                    }
                                    HidUsage::Y => {
                                        if item.flag.is_relative() {
                                            report.y = match Self::read_bits_signed(
                                                &buffer,
                                                bit_position,
                                                item.report_size(),
                                            ) {
                                                Some(v) => v,
                                                None => todo!(),
                                            }
                                        } else {
                                            is_absolute = true;
                                            mouse_state.max_y = item.logical_max as isize;
                                            report.y = match Self::read_bits(
                                                &buffer,
                                                bit_position,
                                                item.report_size(),
                                            ) {
                                                Some(v) => v as isize,
                                                None => todo!(),
                                            }
                                        }
                                    }
                                    HidUsage::WHEEL => {
                                        if item.flag.is_relative() {
                                            report.wheel = match Self::read_bits_signed(
                                                &buffer,
                                                bit_position,
                                                item.report_size(),
                                            ) {
                                                Some(v) => v,
                                                None => todo!(),
                                            }
                                        }
                                    }
                                    _ => (),
                                }
                                bit_position += item.bit_count();
                            }
                            if is_absolute {
                                mouse_state.process_absolute_report(report);
                            } else {
                                mouse_state.process_relative_report(report);
                            }
                        }
                        _ => {
                            // TODO: Other app
                        }
                    }
                }
                Err(UsbError::Aborted) => break,
                Err(err) => {
                    // TODO: error
                    log!("USB HID error {}:{} {:?}", addr.0, if_no.0, err);
                }
            }
        }
    }

    #[inline]
    pub async fn set_boot_protocol(
        device: &UsbDeviceControl,
        if_no: UsbInterfaceNumber,
        is_boot: bool,
    ) -> Result<(), UsbError> {
        device
            .control_nodata(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap(0x21),
                    UsbControlRequest::HID_SET_PROTOCOL,
                )
                .value((!is_boot) as u16)
                .index_if(if_no),
            )
            .await
    }

    #[inline]
    pub async fn get_report(
        device: &UsbDeviceControl,
        if_no: UsbInterfaceNumber,
        report_type: HidReportType,
        report_id: u8,
        len: usize,
        vec: &mut Vec<u8>,
    ) -> Result<(), UsbError> {
        device
            .control_var(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap(0xA1),
                    UsbControlRequest(0x01),
                )
                .value((report_type as u16) * 256 + (report_id as u16))
                .index_if(if_no),
                vec,
                len,
                len,
            )
            .await
    }

    #[inline]
    pub async fn set_report(
        device: &UsbDeviceControl,
        if_no: UsbInterfaceNumber,
        report_type: HidReportType,
        report_id: u8,
        max_len: usize,
        data: &[u8],
    ) -> Result<(), UsbError> {
        device
            .control_send(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap(0x21),
                    UsbControlRequest(0x09),
                )
                .value((report_type as u16) * 256 + (report_id as u16))
                .index_if(if_no),
                max_len,
                data,
            )
            .await
    }

    pub fn write_bits(blob: &mut [u8], position: usize, size: usize, value: usize) -> Option<()> {
        let range = (position / 8)..((position + size + 7) / 8);
        blob.get_mut(range).map(|slice| {
            let mask = if size > 31 {
                0xFFFF_FFFF
            } else {
                (1 << size) - 1
            };
            let value = value & mask;
            let position7 = position & 7;
            slice[0] |= (value << position7) as u8;
            let mut rest_bits = (size - position7) as isize;
            if rest_bits > 0 {
                let mut value = value >> position7;
                let mut cursor = 0;
                while rest_bits > 0 {
                    slice[cursor] |= value as u8;
                    value >>= 8;
                    cursor += 1;
                    rest_bits -= 8;
                }
            }
        })
    }

    pub fn read_bits(blob: &[u8], position: usize, size: usize) -> Option<usize> {
        let range = (position / 8)..((position + size + 7) / 8);
        blob.get(range).map(|slice| {
            let mask = if size > 63 {
                0xFFFF_FFFF_FFFF_FFFF
            } else {
                (1 << size) - 1
            };
            let position7 = position & 7;
            let read_size = position7 + size;
            let data = unsafe {
                if read_size < 8 {
                    *slice.get_unchecked(0) as u64
                } else if read_size < 16 {
                    *slice.get_unchecked(0) as u64 + (*slice.get_unchecked(1) as u64) * 0x100
                } else if read_size < 24 {
                    *slice.get_unchecked(0) as u64
                        + (*slice.get_unchecked(1) as u64) * 0x100
                        + (*slice.get_unchecked(2) as u64) * 0x100_00
                } else if read_size < 32 {
                    *slice.get_unchecked(0) as u64
                        + (*slice.get_unchecked(1) as u64) * 0x100
                        + (*slice.get_unchecked(2) as u64) * 0x100_00
                        + (*slice.get_unchecked(3) as u64) * 0x100_00_00
                } else {
                    *slice.get_unchecked(0) as u64
                        + (*slice.get_unchecked(1) as u64) * 0x100
                        + (*slice.get_unchecked(2) as u64) * 0x100_00
                        + (*slice.get_unchecked(3) as u64) * 0x100_00_00
                        + (*slice.get_unchecked(4) as u64) * 0x100_00_00_00
                }
            };
            ((data >> position7) & mask) as usize
        })
    }

    pub fn read_bits_signed(blob: &[u8], position: usize, size: usize) -> Option<isize> {
        Self::read_bits(blob, position, size).map(|v| {
            let mask = 1 << (size - 1);
            if (v & mask) == 0 {
                v as isize
            } else {
                (!(mask - 1) | v) as isize
            }
        })
    }

    fn parse_report(report_desc: &[u8]) -> Result<HidParsedReport, usize> {
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
                    let flag = HidReportMainFlag::from_bits_truncate(param.into());
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
                    let flag = HidReportMainFlag::from_bits_truncate(param.into());
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
                    let flag = HidReportMainFlag::from_bits_truncate(param.into());
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
                                let report_id = current_app.report_id;
                                if report_id > 0 {
                                    parsed_report.tagged.insert(report_id, current_app.clone());
                                } else {
                                    parsed_report.primary = Some(current_app.clone());
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
                                let report_id = current_app.report_id;
                                if report_id > 0 {
                                    parsed_report.tagged.insert(report_id, current_app.clone());
                                } else {
                                    parsed_report.primary = Some(current_app.clone());
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
                    let report_id = param.into();
                    if !parsed_report.report_ids.contains(&report_id) {
                        parsed_report.report_ids.push(report_id);
                    }
                    // if current_app.report_id > 0 {
                    //     let current_usage = current_app.usage;
                    //     parsed_report
                    //         .tagged
                    //         .insert(current_app.report_id, current_app.clone());
                    //     if let Some(app) = parsed_report.tagged.remove(&report_id) {
                    //         current_app = app;
                    //         current_app.usage = current_usage;
                    //     } else {
                    //         current_app.clear_stream();
                    //         current_app.report_id = report_id;
                    //     }
                    // }
                    current_app.report_id = report_id;
                    global.report_id = NonZeroU8::new(report_id);
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

                _ => todo!(),
            }
        }

        Ok(parsed_report)
    }
}

#[derive(Debug)]
pub struct HidParsedReport {
    report_ids: Vec<u8>,
    primary: Option<ParsedReportApplication>,
    tagged: BTreeMap<u8, ParsedReportApplication>,
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
    pub fn initial_bit_position(&self) -> usize {
        if self.has_report_id() {
            8
        } else {
            0
        }
    }

    #[inline]
    pub fn primary_app(&self) -> Option<&ParsedReportApplication> {
        self.primary.as_ref()
    }

    #[inline]
    pub fn app_by_report_id(&self, report_id: u8) -> Option<&ParsedReportApplication> {
        self.tagged.get(&report_id)
    }
}

#[derive(Clone)]
pub struct ParsedReportApplication {
    report_id: u8,
    usage: HidUsage,
    entries: Vec<ParsedReportEntry>,
}

impl ParsedReportApplication {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            report_id: 0,
            usage: HidUsage::NONE,
            entries: Vec::new(),
        }
    }

    pub fn clear_stream(&mut self) {
        self.entries = Vec::new();
    }

    pub fn clear(&mut self) {
        self.report_id = 0;
        self.usage = HidUsage::NONE;
        self.clear_stream();
    }

    pub fn bit_count_input(&self) -> usize {
        self.bit_count(|v| match v {
            ParsedReportEntry::Input(_) => true,
            _ => false,
        })
    }

    pub fn bit_count_output(&self) -> usize {
        self.bit_count(|v| match v {
            ParsedReportEntry::Output(_) => true,
            _ => false,
        })
    }

    pub fn bit_count<F>(&self, predicate: F) -> usize
    where
        F: Fn(&ParsedReportEntry) -> bool,
    {
        let mut acc = if self.report_id > 0 { 8 } else { 0 };
        for entry in self.entries.iter() {
            if predicate(entry) {
                acc += entry.bit_count()
            }
        }
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
    flag: HidReportMainFlag,
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
            flag: HidReportMainFlag::empty(),
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
        let report_count = if report_count > 0 {
            report_count
        } else {
            global.report_count
        } as u8;
        let logical_min = if flag.contains(HidReportMainFlag::RELATIVE) {
            global.logical_minimum.as_isize() as u32
        } else {
            global.logical_minimum.as_usize() as u32
        };
        let logical_max = if flag.contains(HidReportMainFlag::RELATIVE) {
            global.logical_maximum.as_isize() as u32
        } else {
            global.logical_maximum.as_usize() as u32
        };
        let physical_min = if flag.contains(HidReportMainFlag::RELATIVE) {
            global.physical_minimum.as_isize() as u32
        } else {
            global.physical_minimum.as_usize() as u32
        };
        let physical_max = if flag.contains(HidReportMainFlag::RELATIVE) {
            global.physical_maximum.as_isize() as u32
        } else {
            global.physical_maximum.as_usize() as u32
        };
        Self {
            flag,
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
    pub const fn usage_min(&self) -> HidUsage {
        self.usage_min
    }

    #[inline]
    pub const fn usage_max(&self) -> HidUsage {
        self.usage_max
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
        let _ = writeln!(f, "application {:02x} usage {}", self.report_id, self.usage,);

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
            self.flag, self.report_size, self.report_count
        );
        if !self.flag.contains(HidReportMainFlag::CONSTANT) {
            if self.usage_max > self.usage_min {
                let _ = write!(f, " usage {}..{}", self.usage_min, self.usage_max);
            } else {
                let _ = write!(f, " usage {}", self.usage_min);
            }
            if self.flag.contains(HidReportMainFlag::RELATIVE) {
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
