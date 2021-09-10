//! USB HID Class Driver

use super::super::*;
use crate::{
    io::hid::*,
    task::{scheduler::Timer, Task},
    *,
};
use alloc::{boxed::Box, collections::BTreeMap, sync::Arc, vec::Vec};
use core::{num::NonZeroU8, time::Duration};
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
    fn instantiate(&self, device: &UsbDevice, interface: &UsbInterface) -> bool {
        let class = interface.class();
        if class.base() != UsbBaseClass::HID {
            return false;
        }
        let addr = device.addr();
        let if_no = interface.if_no();
        let endpoint = match interface.endpoints().first() {
            Some(v) => v,
            None => todo!(),
        };
        let ep = endpoint.address();
        let ps = endpoint.descriptor().max_packet_size();
        if ps > 64 {
            return false;
        }

        let mut report_desc = None;
        for report in interface.hid_reports() {
            if report.0 == UsbDescriptorType::HidReport {
                let mut vec = Vec::new();
                UsbHidDriver::get_report_desc(
                    &device,
                    if_no,
                    report.0,
                    0,
                    report.1 as usize,
                    &mut vec,
                )
                .unwrap();
                report_desc = Some(vec);
                break;
            }
        }
        let report_desc = match report_desc {
            Some(v) => v.into_boxed_slice(),
            None => Box::new([]),
        };

        device
            .host()
            .configure_endpoint(endpoint.descriptor())
            .unwrap();

        UsbManager::register_xfer_task(Task::new(UsbHidDriver::_usb_hid_task(
            addr,
            if_no,
            ep,
            class,
            ps,
            report_desc,
        )));

        true
    }
}

pub struct UsbHidDriver;

impl UsbHidDriver {
    async fn _usb_hid_task(
        addr: UsbDeviceAddress,
        if_no: UsbInterfaceNumber,
        ep: UsbEndpointAddress,
        _class: UsbClass,
        ps: u16,
        report_desc: Box<[u8]>,
    ) {
        let device = UsbManager::device_by_addr(addr).unwrap();

        let report_desc = match Self::parse_report(&report_desc) {
            Ok(v) => v,
            Err(err) => {
                log!("HID PARSE ERROR {}", err);
                return;
            }
        };
        // log!("REPORT {:?}", report_desc);

        match Self::set_boot_protocol(&device, if_no, false) {
            Ok(_) => (),
            Err(_) => (),
        }

        let mut key_state = KeyboardState::new();
        let mut mouse_state = MouseState::empty();
        let mut buffer = [0; 64];
        loop {
            match device.read_slice(ep, &mut buffer, 1, ps as usize).await {
                Ok(_size) => {
                    let app = if report_desc.has_report_id() {
                        report_desc.app_by_report_id(buffer[0])
                    } else {
                        report_desc.primary.as_ref()
                    };
                    if let Some(app) = app {
                        let mut bit_position = if report_desc.has_report_id() { 8 } else { 0 };
                        match app.usage {
                            HidUsage::KEYBOARD => {
                                let mut report = KeyReportRaw::default();
                                for entry in &app.inputs {
                                    if !entry.flag.contains(HidReportMainFlag::CONSTANT)
                                        && entry.usage_page == UsagePage::KEYBOARD
                                    {
                                        if entry.flag.contains(HidReportMainFlag::VARIABLE)
                                            && entry.report_size == 1
                                            && entry.usage_min == 0xE0
                                            && entry.usage_max == 0xE7
                                        {
                                            report.modifier = Self::read_bits(
                                                &buffer,
                                                bit_position,
                                                entry.report_count(),
                                            )
                                            .map(|v| Modifier::from_bits_truncate(v as u8))
                                            .unwrap();
                                        } else if entry.report_size == 8
                                            && !entry.flag.contains(HidReportMainFlag::VARIABLE)
                                        {
                                            let limit = usize::min(
                                                report.keydata.len(),
                                                entry.report_count(),
                                            );
                                            for i in 0..limit {
                                                report.keydata[i] =
                                                    Usage(buffer[bit_position / 8 + i]);
                                            }
                                        }
                                    }
                                    bit_position += entry.bit_count();
                                }
                                key_state.process_key_report(report);
                            }
                            HidUsage::MOUSE => {
                                let mut report = MouseReport {
                                    buttons: MouseButton::empty(),
                                    x: 0,
                                    y: 0,
                                };
                                for entry in &app.inputs {
                                    if !entry.flag.contains(HidReportMainFlag::CONSTANT) {
                                        if entry.usage_page == UsagePage::BUTTON
                                            && entry.usage_min == 1
                                            && entry.flag.contains(HidReportMainFlag::VARIABLE)
                                            && entry.report_size == 1
                                        {
                                            report.buttons = Self::read_bits(
                                                &buffer,
                                                bit_position,
                                                entry.report_count(),
                                            )
                                            .map(|v| MouseButton::from_bits_truncate(v as u8))
                                            .unwrap();
                                        } else if entry.usage_min() == HidUsage::X {
                                            report.x = match Self::read_bits_signed(
                                                &buffer,
                                                bit_position,
                                                entry.report_size(),
                                            ) {
                                                Some(v) => v as isize,
                                                None => todo!(),
                                            }
                                        } else if entry.usage_min() == HidUsage::Y {
                                            report.y = match Self::read_bits_signed(
                                                &buffer,
                                                bit_position,
                                                entry.report_size(),
                                            ) {
                                                Some(v) => v as isize,
                                                None => todo!(),
                                            }
                                        }
                                    }
                                    bit_position += entry.bit_count();
                                }
                                mouse_state.process_mouse_report(report);
                            }
                            _ => {
                                // other app
                            }
                        }
                    } else {
                        // unknown report_id - ignore
                    }
                }
                Err(UsbError::Aborted) => break,
                Err(_err) => {
                    // TODO: error
                }
            }
        }
    }

    pub fn set_boot_protocol(
        device: &UsbDevice,
        if_no: UsbInterfaceNumber,
        is_boot: bool,
    ) -> Result<(), UsbError> {
        device
            .host()
            .control(UsbControlSetupData {
                bmRequestType: UsbControlRequestBitmap(0x21),
                bRequest: UsbControlRequest::HID_SET_PROTOCOL,
                wValue: (!is_boot) as u16,
                wIndex: if_no.0 as u16,
                wLength: 0,
            })
            .map(|_| ())
    }

    pub fn get_report_desc(
        device: &UsbDevice,
        if_no: UsbInterfaceNumber,
        report_type: UsbDescriptorType,
        report_id: u8,
        len: usize,
        vec: &mut Vec<u8>,
    ) -> Result<(), UsbError> {
        match device.host().control(UsbControlSetupData {
            bmRequestType: UsbControlRequestBitmap(0x81),
            bRequest: UsbControlRequest::GET_DESCRIPTOR,
            wValue: (report_type as u16) * 256 + (report_id as u16),
            wIndex: if_no.0 as u16,
            wLength: len as u16,
        }) {
            Ok(result) => {
                vec.resize(result.len(), 0);
                vec.copy_from_slice(result);
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    pub fn get_report(
        device: &UsbDevice,
        if_no: UsbInterfaceNumber,
        report_type: HidReportType,
        report_id: u8,
        len: usize,
        vec: &mut Vec<u8>,
    ) -> Result<(), UsbError> {
        match device.host().control(UsbControlSetupData {
            bmRequestType: UsbControlRequestBitmap(0xA1),
            bRequest: UsbControlRequest::GET_DESCRIPTOR,
            wValue: (report_type as u16) * 256 + (report_id as u16),
            wIndex: if_no.0 as u16,
            wLength: len as u16,
        }) {
            Ok(result) => {
                vec.resize(result.len(), 0);
                vec.copy_from_slice(result);
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    pub fn set_report(
        device: &UsbDevice,
        if_no: UsbInterfaceNumber,
        report_type: HidReportType,
        report_id: u8,
        data: &[u8],
    ) -> Result<usize, UsbError> {
        device.host().control_send(
            UsbControlSetupData {
                bmRequestType: UsbControlRequestBitmap(0x21),
                bRequest: UsbControlRequest::HID_SET_REPORT,
                wValue: (report_type as u16) * 256 + (report_id as u16),
                wIndex: if_no.0 as u16,
                wLength: data.len() as u16,
            },
            data,
        )
    }

    pub fn read_bits(blob: &[u8], position: usize, size: usize) -> Option<u64> {
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
            (data >> position7) & mask
        })
    }

    pub fn read_bits_signed(blob: &[u8], position: usize, size: usize) -> Option<i64> {
        Self::read_bits(blob, position, size).map(|v| {
            let mask = 1 << (size - 1);
            if (v & mask) == 0 {
                v as i64
            } else {
                (!(mask - 1) | v) as i64
            }
        })
    }

    fn parse_report(report_desc: &[u8]) -> Result<HidParsedReport, usize> {
        let mut parsed_report = HidParsedReport::new();

        let mut current_app = HidParsedReportApplication::empty();
        let mut collection_ctx = Vec::new();

        let mut reader = HidReporteReader::new(report_desc);
        let mut stack = Vec::new();
        let mut global = HidReportGlobalState::new();
        let mut local = HidReportLocalState::new();
        while let Some(lead_byte) = reader.next() {
            let lead_byte = HidReportLeadByte(lead_byte);
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

            match tag {
                HidReportItemTag::Input => {
                    let flag = HidReportMainFlag::from_bits_truncate(param.usize());
                    HidParsedReportEntry::parse(&mut current_app.inputs, flag, &global, &local);
                    local.reset();
                }
                HidReportItemTag::Output => {
                    let flag = HidReportMainFlag::from_bits_truncate(param.usize());
                    HidParsedReportEntry::parse(&mut current_app.outputs, flag, &global, &local);
                    local.reset();
                }
                HidReportItemTag::Feature => {
                    let flag = HidReportMainFlag::from_bits_truncate(param.usize());
                    HidParsedReportEntry::parse(&mut current_app.features, flag, &global, &local);
                    local.reset();
                }

                HidReportItemTag::Collection => {
                    let collection_type: HidReportCollectionType =
                        match FromPrimitive::from_usize(param.usize()) {
                            Some(v) => v,
                            None => todo!(),
                        };
                    collection_ctx.push(collection_type);

                    match collection_type {
                        HidReportCollectionType::Application => {
                            current_app.clear();
                            current_app.usage = HidUsage::new(
                                global.usage_page,
                                local.usage.first().map(|v| *v).unwrap_or_default(),
                            );
                        }
                        _ => (),
                    }

                    local.reset();
                }

                HidReportItemTag::EndCollection => {
                    match collection_ctx.pop() {
                        Some(v) => match v {
                            HidReportCollectionType::Application => {
                                let report_id = current_app.report_id;
                                if report_id > 0 {
                                    parsed_report.tagged.insert(report_id, current_app.clone());
                                } else {
                                    parsed_report.primary = Some(current_app.clone());
                                }
                                current_app.clear();
                            }
                            _ => (),
                        },
                        None => return Err(reader.position()),
                    }
                    local.reset();
                }

                HidReportItemTag::UsagePage => global.usage_page = UsagePage(param.usize() as u16),
                HidReportItemTag::LogicalMinimum => global.logical_minimum = param,
                HidReportItemTag::LogicalMaximum => global.logical_maximum = param,
                HidReportItemTag::PhysicalMinimum => global.physical_minimum = param,
                HidReportItemTag::PhysicalMaximum => global.physical_maximum = param,
                HidReportItemTag::UnitExponent => global.unit_exponent = param.isize(),
                HidReportItemTag::Unit => global.unit = param.usize(),
                HidReportItemTag::ReportSize => global.report_size = param.usize(),
                HidReportItemTag::ReportId => {
                    let report_id = param.usize() as u8;
                    if !parsed_report.report_ids.contains(&report_id) {
                        parsed_report.report_ids.push(report_id);
                    }
                    if current_app.report_id > 0 {
                        let current_usage = current_app.usage;
                        parsed_report
                            .tagged
                            .insert(current_app.report_id, current_app.clone());
                        if let Some(app) = parsed_report.tagged.remove(&report_id) {
                            current_app = app;
                            current_app.usage = current_usage;
                        } else {
                            current_app.clear_stream();
                            current_app.report_id = report_id;
                        }
                    }
                    current_app.report_id = report_id;

                    global.report_id = NonZeroU8::new(report_id);
                }
                HidReportItemTag::ReportCount => global.report_count = param.usize(),
                HidReportItemTag::Push => stack.push(global),
                HidReportItemTag::Pop => {
                    global = match stack.pop() {
                        Some(v) => v,
                        None => return Err(reader.position()),
                    }
                }
                HidReportItemTag::Usage => local.usage.push(param.usize() as u16),
                HidReportItemTag::UsageMinimum => local.usage_minimum = param.usize() as u16,
                HidReportItemTag::UsageMaximum => local.usage_maximum = param.usize() as u16,

                _ => todo!(),
            }
        }

        Ok(parsed_report)
    }
}

#[derive(Debug)]
pub struct HidParsedReport {
    report_ids: Vec<u8>,
    primary: Option<HidParsedReportApplication>,
    tagged: BTreeMap<u8, HidParsedReportApplication>,
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
    pub fn app_by_report_id(&self, report_id: u8) -> Option<&HidParsedReportApplication> {
        self.tagged.get(&report_id)
    }
}

#[derive(Clone)]
pub struct HidParsedReportApplication {
    report_id: u8,
    usage: HidUsage,
    inputs: Vec<HidParsedReportEntry>,
    outputs: Vec<HidParsedReportEntry>,
    features: Vec<HidParsedReportEntry>,
}

impl HidParsedReportApplication {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            report_id: 0,
            usage: HidUsage::NONE,
            inputs: Vec::new(),
            outputs: Vec::new(),
            features: Vec::new(),
        }
    }

    pub fn clear_stream(&mut self) {
        self.inputs = Vec::new();
        self.outputs = Vec::new();
        self.features = Vec::new();
    }

    pub fn clear(&mut self) {
        self.report_id = 0;
        self.usage = HidUsage::NONE;
        self.clear_stream();
    }
}

#[derive(Clone, Copy)]
pub struct HidParsedReportEntry {
    flag: HidReportMainFlag,
    report_size: u8,
    report_count: u8,
    usage_page: UsagePage,
    usage_min: u16,
    usage_max: u16,
    logical_min: u32,
    logical_max: u32,
}

impl HidParsedReportEntry {
    pub fn parse(
        vec: &mut Vec<HidParsedReportEntry>,
        flag: HidReportMainFlag,
        global: &HidReportGlobalState,
        local: &HidReportLocalState,
    ) {
        if local.usage.len() > 0 {
            let report_count = global.report_count / local.usage.len();
            for usage in &local.usage {
                vec.push(Self::new(flag, global, local, Some(*usage), report_count));
            }
        } else {
            vec.push(Self::new(flag, global, local, None, 0));
        }
    }

    #[inline]
    pub fn new(
        flag: HidReportMainFlag,
        global: &HidReportGlobalState,
        local: &HidReportLocalState,
        usage: Option<u16>,
        report_count: usize,
    ) -> Self {
        let (usage_min, usage_max) = if let Some(usage) = usage {
            (usage, 0)
        } else {
            (local.usage_minimum, local.usage_maximum)
        };
        let report_count = if report_count > 0 {
            report_count
        } else {
            global.report_count
        } as u8;
        let logical_min = if flag.contains(HidReportMainFlag::RELATIVE) {
            global.logical_minimum.isize() as u32
        } else {
            global.logical_minimum.usize() as u32
        };
        let logical_max = if flag.contains(HidReportMainFlag::RELATIVE) {
            global.logical_maximum.isize() as u32
        } else {
            global.logical_maximum.usize() as u32
        };
        Self {
            flag,
            report_size: global.report_size as u8,
            report_count,
            usage_page: global.usage_page,
            usage_min,
            usage_max,
            logical_min,
            logical_max,
        }
    }

    #[inline]
    pub const fn usage_min(&self) -> HidUsage {
        HidUsage::new(self.usage_page, self.usage_min)
    }

    #[inline]
    pub const fn usage_max(&self) -> HidUsage {
        HidUsage::new(self.usage_page, self.usage_max)
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

impl core::fmt::Debug for HidParsedReportApplication {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let _ = writeln!(
            f,
            "APP_{:02x} UP{:04x}_{:04x}",
            self.report_id,
            self.usage.usage_page().0,
            self.usage.usage(),
        );

        for input in &self.inputs {
            let _ = writeln!(f, "INPUT {:?}", input);
        }
        for output in &self.outputs {
            let _ = writeln!(f, "OUTPUT {:?}", output);
        }
        for feature in &self.features {
            let _ = writeln!(f, "FEATURE {:?}", feature);
        }

        Ok(())
    }
}

impl core::fmt::Debug for HidParsedReportEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let _ = write!(
            f,
            "{:?} size {} {}",
            self.flag, self.report_size, self.report_count
        );
        if !self.flag.contains(HidReportMainFlag::CONSTANT) {
            if self.usage_max > self.usage_min {
                let _ = write!(
                    f,
                    " usage {:04x} ({:04x}..{:04x})",
                    self.usage_page.0, self.usage_min, self.usage_max
                );
            } else {
                let _ = write!(f, " usage {:04x} {:04x}", self.usage_page.0, self.usage_min);
            }
            if self.flag.contains(HidReportMainFlag::RELATIVE) {
                let _ = write!(
                    f,
                    " logical {}..{}",
                    self.logical_min as i32, self.logical_max as i32
                );
            } else {
                let _ = write!(f, " logical {}..{}", self.logical_min, self.logical_max);
            }
        }

        Ok(())
    }
}
