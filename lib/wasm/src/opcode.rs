mod single;
pub use single::*;
mod opcode_fc;
pub use opcode_fc::*;
mod opcode_fd;
pub use opcode_fd::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WasmOpcode {
    /// Single Byte Opcode
    Single(WasmSingleOpcode),
    /// Multi Byte Opcode - FC proposals
    PrefixFC(WasmOpcodeFC),
    /// Multi Byte Opcode - FD SIMD
    PrefixFD(WasmOpcodeFD),
}

impl WasmOpcode {
    pub const NOP: Self = Self::Single(WasmSingleOpcode::Nop);
    pub const UNREACHABLE: Self = Self::Single(WasmSingleOpcode::Unreachable);
    pub const END: Self = Self::Single(WasmSingleOpcode::End);

    pub fn decode<E, F>(lead: u8, failure: E, trail: F) -> Result<Self, E>
    where
        F: FnOnce() -> Result<u32, E>,
    {
        match WasmSingleOpcode::new(lead) {
            Some(WasmSingleOpcode::PrefixFC) => trail().and_then(|v| {
                WasmOpcodeFC::new(v)
                    .map(|opcode| Self::PrefixFC(opcode))
                    .ok_or(failure)
            }),
            Some(WasmSingleOpcode::PrefixFD) => trail().and_then(|v| {
                WasmOpcodeFD::new(v)
                    .map(|opcode| Self::PrefixFD(opcode))
                    .ok_or(failure)
            }),
            Some(opcode) => Ok(Self::Single(opcode)),
            None => Err(failure),
        }
    }

    pub const fn proposal_type(&self) -> WasmProposalType {
        match self {
            WasmOpcode::Single(v) => v.proposal_type(),
            WasmOpcode::PrefixFC(v) => v.proposal_type(),
            WasmOpcode::PrefixFD(_) => WasmProposalType::Simd,
        }
    }

    pub const fn to_str(&self) -> &str {
        match self {
            WasmOpcode::Single(v) => v.to_str(),
            WasmOpcode::PrefixFC(v) => v.to_str(),
            WasmOpcode::PrefixFD(_) => "(todo)",
        }
    }

    pub const fn leading_byte(&self) -> u8 {
        match self {
            WasmOpcode::Single(v) => *v as u8,
            WasmOpcode::PrefixFC(_) => 0xFC,
            WasmOpcode::PrefixFD(_) => 0xFD,
        }
    }

    pub const fn trail_code(&self) -> Option<u32> {
        match self {
            WasmOpcode::Single(_) => None,
            WasmOpcode::PrefixFC(v) => Some(*v as u32),
            WasmOpcode::PrefixFD(_) => None,
        }
    }
}

impl core::fmt::Display for WasmOpcode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.to_str())
    }
}

impl core::fmt::Debug for WasmOpcode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let Some(trail_code) = self.trail_code() {
            write!(
                f,
                "{:02x} {:02x} {}",
                self.leading_byte(),
                trail_code,
                self.to_str(),
            )
        } else {
            write!(f, "{:02x} {}", self.leading_byte(), self.to_str())
        }
    }
}

impl const From<WasmSingleOpcode> for WasmOpcode {
    #[inline]
    fn from(value: WasmSingleOpcode) -> Self {
        Self::Single(value)
    }
}

impl const From<WasmOpcodeFC> for WasmOpcode {
    #[inline]
    fn from(value: WasmOpcodeFC) -> Self {
        Self::PrefixFC(value)
    }
}

impl const From<WasmOpcodeFD> for WasmOpcode {
    #[inline]
    fn from(value: WasmOpcodeFD) -> Self {
        Self::PrefixFD(value)
    }
}
