// Boot Settings

extern crate serde_derive;
use serde::Deserialize;
use serde_json_core::*;

#[derive(Debug, Deserialize)]
pub struct BootSettings {
    kernel: Option<&'static str>,
    cmdline: Option<&'static str>,
}

impl Default for BootSettings {
    fn default() -> Self {
        serde_json_core::from_str(Self::DEFAULT_JSON).unwrap()
    }
}

impl BootSettings {
    pub const DEFAULT_CONFIG_PATH: &'static str = "\\EFI\\BOOT\\config.json";

    const DEFAULT_JSON: &'static str = r#"{}"#;

    pub fn load(json: &'static str) -> Result<Self, de::Error> {
        serde_json_core::from_str(json)
    }

    const DEFAULT_KERNEL_PATH: &'static str = "\\EFI\\BOOT\\kernel.bin";
    pub fn kernel_path(&self) -> &'static str {
        self.kernel.unwrap_or(Self::DEFAULT_KERNEL_PATH)
    }

    const DEFAULT_CMDLINE: &'static str = "";
    pub fn cmdline(&self) -> &'static str {
        self.cmdline.unwrap_or(Self::DEFAULT_CMDLINE)
    }
}
