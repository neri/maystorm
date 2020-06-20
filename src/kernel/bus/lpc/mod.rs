// Legacy PC's Low Pin Count Bus Device

pub mod ps2;

pub struct LowPinCount {}

impl LowPinCount {
    pub unsafe fn init() {
        let _ = ps2::Ps2::init();
    }
}
