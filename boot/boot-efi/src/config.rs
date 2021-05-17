// Boot Settings

use serde::Deserialize;
use serde_json_core::*;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootSettings {
    #[serde(default = "config_default_kernel")]
    kernel: &'static str,

    #[serde(default = "config_default_initrd")]
    initrd: &'static str,

    #[serde(default = "config_default_cmdline")]
    cmdline: &'static str,

    #[serde(default)]
    force_single: bool,

    #[serde(default)]
    aslr: bool,

    #[serde(default)]
    headless: bool,

    #[serde(default)]
    debug_mode: bool,
}

fn config_default_kernel() -> &'static str {
    "/EFI/MEGOS/kernel.bin"
}

fn config_default_initrd() -> &'static str {
    "/EFI/MEGOS/initrd.img"
}

fn config_default_cmdline() -> &'static str {
    ""
}

impl Default for BootSettings {
    fn default() -> Self {
        serde_json_core::from_str(Self::DEFAULT_JSON).unwrap().0
    }
}

impl BootSettings {
    pub const DEFAULT_CONFIG_PATH: &'static str = "/EFI/MEGOS/config.json";

    const DEFAULT_JSON: &'static str = r#"{}"#;

    #[inline]
    pub fn load(json: &'static str) -> de::Result<Self> {
        serde_json_core::from_str(json).map(|v| v.0)
    }

    #[inline]
    pub const fn kernel_path<'a>(&self) -> &'a str {
        self.kernel
    }

    #[inline]
    pub const fn initrd_path<'a>(&self) -> &'a str {
        self.initrd
    }

    #[inline]
    pub const fn cmdline<'a>(&self) -> &'a str {
        self.cmdline
    }

    #[inline]
    pub const fn force_single(&self) -> bool {
        self.force_single
    }

    #[inline]
    pub const fn is_headless(&self) -> bool {
        self.headless
    }

    #[inline]
    pub const fn is_debug_mode(&self) -> bool {
        self.debug_mode
    }
}
