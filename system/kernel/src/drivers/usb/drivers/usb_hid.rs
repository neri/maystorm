//! USB HID Class Driver (03_xx_xx)

use super::super::*;
use crate::{
    io::hid_mgr::*,
    task::{scheduler::Timer, Task},
    *,
};
use alloc::{boxed::Box, sync::Arc, vec::Vec};
use core::{pin::Pin, time::Duration};
use futures_util::Future;
use megstd::io::hid::*;

pub struct UsbHidStarter;

impl UsbHidStarter {
    #[inline]
    pub fn new() -> Box<dyn UsbInterfaceDriverStarter> {
        Box::new(Self {})
    }
}

impl UsbInterfaceDriverStarter for UsbHidStarter {
    fn instantiate(
        &self,
        device: &Arc<UsbDeviceControl>,
        if_no: UsbInterfaceNumber,
        class: UsbClass,
    ) -> Option<Pin<Box<dyn Future<Output = Result<Task, UsbError>>>>> {
        if class.base() == UsbBaseClass::HID {
            Some(Box::pin(UsbHidDriver::_instantiate(
                device.clone(),
                if_no,
                class,
            )))
        } else {
            None
        }
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
        if !endpoint.is_dir_in() {
            return Err(UsbError::InvalidDescriptor);
        }
        let ep = endpoint.address();
        let ps = endpoint.descriptor().max_packet_size() as usize;
        if ps > Self::BUFFER_LEN {
            return Err(UsbError::InvalidDescriptor);
        }

        let report_desc = interface
            .hid_reports_by(UsbDescriptorType::HidReport)
            .unwrap_or(&[]);
        let report_desc = match HidParsedReport::parse(report_desc) {
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
        //     "HID {:03}:{} VEN {} DEV {} UP {:08x} {:?} {}",
        //     addr.as_u8(),
        //     if_no.0,
        //     device.device().vid(),
        //     device.device().pid(),
        //     report_desc.primary_app().unwrap().usage().0,
        //     report_desc.report_ids.iter().map(|v| v.as_u8()),
        //     device.device().preferred_device_name().unwrap_or_default(),
        // );
        // for app in report_desc.applications() {
        //     log!(
        //         " {} {} I {} O {} F {}",
        //         app.report_id().map(|v| v.as_u8()).unwrap_or_default(),
        //         app.usage(),
        //         app.bit_count_for_input(),
        //         app.bit_count_for_output(),
        //         app.bit_count_for_feature(),
        //     );
        //     log!(" {:?}", app.entries().collect::<Vec<_>>());
        // }

        for app in report_desc
            .primary_app()
            .into_iter()
            .chain(report_desc.applications())
        {
            let mut data = Vec::new();
            data.resize((app.bit_count_for_feature() + 7) / 8, 0);
            let empty_data = [0; Self::BUFFER_LEN];
            let mut bit_position = 0;
            match app.usage() {
                HidUsage::KEYBOARD => {
                    // Flashing LED on the keyboard
                    for item in app.output_items() {
                        if item.is_const() {
                            bit_position += item.bit_count();
                            continue;
                        }
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
                            app.report_id(),
                            len,
                            &data,
                        )
                        .await;
                        Timer::sleep_async(Duration::from_millis(100)).await;
                        let _ = Self::set_report(
                            &device,
                            if_no,
                            HidReportType::Output,
                            app.report_id(),
                            len,
                            &empty_data,
                        )
                        .await;
                        Timer::sleep_async(Duration::from_millis(50)).await;
                    }
                }
                // HidUsage::DEVICE_CONFIGURATION => {
                //     let mut bit_position = 0;
                //     for item in app.features() {
                //         if item.is_const() {
                //             bit_position += item.bit_count();
                //             continue;
                //         }
                //         match item.usage_min() {
                //             HidUsage::DEVICE_MODE => {
                //                 Self::write_bits(
                //                     &mut data,
                //                     bit_position,
                //                     item.bit_count(),
                //                     DeviceMode::Mouse as usize,
                //                 );
                //             }
                //             HidUsage::SURFACE_SWITCH | HidUsage::BUTTON_SWITCH => {
                //                 Self::write_bits(&mut data, bit_position, item.bit_count(), 1);
                //             }
                //             _ => (),
                //         }
                //         bit_position += item.bit_count();
                //     }
                //     let len = (bit_position + 7) / 8;
                //     let _ = Self::set_report(
                //         &device,
                //         if_no,
                //         HidReportType::Feature,
                //         app.report_id(),
                //         len,
                //         data.as_slice(),
                //     )
                //     .await;
                // }
                _ => (),
            }
        }

        let mut key_state = KeyboardState::new();
        let mut mouse_state = MouseState::empty();
        let mut buffer = Vec::new();
        loop {
            match device.read_vec(ep, &mut buffer, 1, ps).await {
                Ok(_) => {
                    let (app, _report_id) = if report_desc.has_report_id() {
                        let report_id = HidReportId::new(buffer[0]);
                        let app =
                            report_id.and_then(|report_id| report_desc.app_by_report_id(report_id));
                        (app, report_id)
                    } else {
                        (report_desc.primary_app(), None)
                    };
                    // if buffer.iter().fold(0, |a, b| a | *b) > 0 {
                    //     log!(
                    //         "APP {}> {:?}",
                    //         _report_id.map(|v| v.as_u8()).unwrap_or_default(),
                    //         HexDump(&buffer)
                    //     );
                    // }
                    let app = match app {
                        Some(v) => v,
                        None => {
                            // log!(
                            //     "HID {:03}.{} UNKNOWN APP {}",
                            //     addr.as_u8(),
                            //     if_no.0,
                            //     _report_id.map(|v| v.as_u8()).unwrap_or_default(),
                            // );
                            continue;
                        }
                    };
                    if buffer.len() * 8 < app.bit_count_for_input() {
                        // Some devices send smaller garbage data
                        continue;
                    }

                    let mut bit_position = report_desc.initial_bit_position();
                    match app.usage() {
                        HidUsage::KEYBOARD => {
                            let mut report = KeyReportRaw::default();
                            for item in app.input_items() {
                                if item.is_const() {
                                    bit_position += item.bit_count();
                                    continue;
                                };
                                if item.is_variable()
                                    && item.report_size() == 1
                                    && item.usage_min() == HidUsage::from(Usage::MOD_MIN)
                                    && item.usage_max() == HidUsage::from(Usage::MOD_MAX)
                                {
                                    // Modifier bit array
                                    report.modifier =
                                        Self::read_bits(&buffer, bit_position, item.report_count())
                                            .map(|v| Modifier::from_bits_retain(v as u8))
                                            .unwrap_or_default();
                                } else if item.report_size() == 8
                                    && item.is_array()
                                    && item.usage_min().usage_page() == UsagePage::KEYBOARD
                                {
                                    // Keyboard usage array
                                    let limit =
                                        usize::min(report.keydata.len(), item.report_count());
                                    for i in 0..limit {
                                        report.keydata[i] = Usage(buffer[bit_position / 8 + i]);
                                    }
                                }

                                bit_position += item.bit_count();
                            }
                            key_state.process_report(report);
                        }
                        HidUsage::MOUSE => {
                            let mut is_absolute = false;
                            let mut report = MouseReport::default();
                            for item in app.input_items() {
                                if item.is_const() {
                                    bit_position += item.bit_count();
                                    continue;
                                };
                                match item.usage_min() {
                                    HidUsage::BUTTON_1 => {
                                        if item.is_variable() && item.report_size() == 1 {
                                            report.buttons = Self::read_bits(
                                                &buffer,
                                                bit_position,
                                                item.report_count(),
                                            )
                                            .map(|v| MouseButton::from_bits_retain(v as u8))
                                            .unwrap();
                                        }
                                    }
                                    HidUsage::X => {
                                        if item.is_relative() {
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
                                            mouse_state.max_x = item.logical_max() as isize;
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
                                        if item.is_relative() {
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
                                            mouse_state.max_y = item.logical_max() as isize;
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
                                        if item.is_relative() {
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
                        HidUsage::CONSUMER_CONTROL => {
                            let mut bitmap = Vec::new();
                            for item in app.input_items() {
                                if item.is_const() {
                                    bit_position += item.bit_count();
                                    continue;
                                };

                                if item.is_variable() && item.report_size() == 1 {
                                    Self::read_bits(&buffer, bit_position, item.report_size()).map(
                                        |data| {
                                            if data != 0 {
                                                bitmap.push(item.usage_min());
                                            }
                                        },
                                    );
                                }

                                bit_position += item.bit_count();
                            }
                            if bitmap.len() > 0 {
                                log!("CONSUME {:?}", bitmap);
                            }
                        }
                        _ => {
                            // TODO: Other app
                        }
                    }
                }
                Err(UsbError::Aborted) => break,
                // Err(UsbError::InvalidParameter) => {
                //     log!(
                //         "USB HID error {}:{} {:?}",
                //         addr.as_u8(),
                //         if_no.0,
                //         UsbError::InvalidParameter
                //     );
                //     break;
                // }
                Err(err) => {
                    // TODO: error
                    log!("USB HID error {}:{} {:?}", addr.as_u8(), if_no.0, err);
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
        report_id: Option<HidReportId>,
        len: usize,
        vec: &mut Vec<u8>,
    ) -> Result<(), UsbError> {
        device
            .control_vec(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap(0xA1),
                    UsbControlRequest(0x01),
                )
                .value(
                    (report_type as u16) * 256
                        + (report_id.map(|v| v.as_u8()).unwrap_or_default() as u16),
                )
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
        report_id: Option<HidReportId>,
        max_len: usize,
        data: &[u8],
    ) -> Result<(), UsbError> {
        device
            .control_send(
                UsbControlSetupData::request(
                    UsbControlRequestBitmap(0x21),
                    UsbControlRequest(0x09),
                )
                .value(
                    (report_type as u16) * 256
                        + (report_id.map(|v| v.as_u8()).unwrap_or_default() as u16),
                )
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
            if size + position7 > 8 {
                todo!();
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
}
