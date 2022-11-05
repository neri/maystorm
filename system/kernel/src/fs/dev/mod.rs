pub mod null;
pub mod zero;

use crate::{assert_call_once, System};

pub(super) fn install_drivers() {
    assert_call_once!();

    null::Null::init();
    zero::Zero::init();
}
