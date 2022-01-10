//! Icon Resource Manager

use crate::r;
use megstd::drawing::{img::ImageLoader, *};

pub struct IconManager {}

impl IconManager {
    pub fn bitmap<'a>(icon: r::Icons) -> Option<BoxedBitmap<'a>> {
        match icon {
            r::Icons::Apps => ImageLoader::from_qoi(include_bytes!(
                "../../../../ext/material-design-icons/ic_apps_black_24dp.qoi"
            )),
            r::Icons::Cancel => ImageLoader::from_qoi(include_bytes!(
                "../../../../ext/material-design-icons/ic_cancel_black_24dp.qoi"
            )),
            r::Icons::Check => ImageLoader::from_qoi(include_bytes!(
                "../../../../ext/material-design-icons/ic_check_black_24dp.qoi"
            )),
            r::Icons::ChevronLeft => ImageLoader::from_qoi(include_bytes!(
                "../../../../ext/material-design-icons/ic_chevron_left_black_24dp.qoi"
            )),
            r::Icons::ChevronRight => ImageLoader::from_qoi(include_bytes!(
                "../../../../ext/material-design-icons/ic_chevron_right_black_24dp.qoi"
            )),
            r::Icons::Close => ImageLoader::from_qoi(include_bytes!(
                "../../../../ext/material-design-icons/ic_close_black_24dp.qoi"
            )),
            r::Icons::Delete => ImageLoader::from_qoi(include_bytes!(
                "../../../../ext/material-design-icons/ic_delete_black_24dp.qoi"
            )),
            r::Icons::Error => ImageLoader::from_qoi(include_bytes!(
                "../../../../ext/material-design-icons/ic_error_black_24dp.qoi"
            )),
            r::Icons::Menu => ImageLoader::from_qoi(include_bytes!(
                "../../../../ext/material-design-icons/ic_menu_black_24dp.qoi"
            )),
            r::Icons::Info => ImageLoader::from_qoi(include_bytes!(
                "../../../../ext/material-design-icons/ic_info_outline_black_24dp.qoi"
            )),
            r::Icons::Usb => ImageLoader::from_qoi(include_bytes!(
                "../../../../ext/material-design-icons/ic_usb_black_24dp.qoi"
            )),
            r::Icons::Warning => ImageLoader::from_qoi(include_bytes!(
                "../../../../ext/material-design-icons/ic_warning_black_24dp.qoi"
            )),
        }
    }

    pub fn mask(icon: r::Icons) -> Option<OperationalBitmap> {
        match icon {
            r::Icons::Close => ImageLoader::from_qoi_mask(include_bytes!(
                "../../../../ext/material-design-icons/ic_close_black_24dp.qoi"
            )),
            r::Icons::ChevronLeft => ImageLoader::from_qoi_mask(include_bytes!(
                "../../../../ext/material-design-icons/ic_chevron_left_black_24dp.qoi"
            )),
            _ => None,
        }
    }
}
