// Boot Settings

use crate::page::*;
use serde::Deserialize;
use serde_json_core::*;

#[serde(deny_unknown_fields)]
#[derive(Debug, Deserialize)]
pub struct BootSettings {
    #[serde(default = "default_kernel")]
    kernel: &'static str,

    #[serde(default = "default_cmdline")]
    cmdline: &'static str,

    #[serde(default = "default_base_address")]
    base_address: u64,

    #[serde(default)]
    aslr: bool,

    #[serde(default)]
    headless: bool,
}

fn default_kernel() -> &'static str {
    "/EFI/BOOT/kernel.bin"
}

fn default_cmdline() -> &'static str {
    ""
}

fn default_base_address() -> u64 {
    0xFFFF_FFFF_8000_0000
}

impl Default for BootSettings {
    fn default() -> Self {
        serde_json_core::from_str(Self::DEFAULT_JSON).unwrap()
    }
}

impl BootSettings {
    pub const DEFAULT_CONFIG_PATH: &'static str = "/EFI/BOOT/config.json";

    const DEFAULT_JSON: &'static str = r#"{}"#;

    pub fn load(json: &'static str) -> Result<Self, de::Error> {
        serde_json_core::from_str(json)
    }

    pub const fn kernel_path<'a>(&self) -> &'a str {
        self.kernel
    }

    pub const fn cmdline<'a>(&self) -> &'a str {
        self.cmdline
    }

    pub const fn base_address(&self) -> VirtualAddress {
        VirtualAddress(self.base_address)
    }

    pub const fn is_headless(&self) -> bool {
        self.headless
    }
}
