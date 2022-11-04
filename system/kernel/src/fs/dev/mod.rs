pub mod null;
pub mod zero;

use crate::{assert_call_once, fs::devfs::DevFs, System};

pub(super) fn install_drivers() {
    assert_call_once!();

    DevFs::install_minor_device(null::Null::new());
    DevFs::install_minor_device(zero::Zero::new());
}
