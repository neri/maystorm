//! Intermediate code for Webassembly runtime

use crate::opcode::WasmOpcode;
use alloc::{boxed::Box, vec::Vec};

/// Intermediate code for Webassembly runtime
#[non_exhaustive]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WasmIntMnemonic {
    /// Undefined
    Undefined,
    /// Unreachable
    Unreachable,
    /// No operation, this mnemonic will be removed during the compaction phase.
    Nop,
    /// Block Marker, this mnemonic will be removed during the compaction phase.
    Block(usize),
    /// End of block marker, this mnemonic will be removed during the compaction phase.
    End(usize),

    /// branch
    Br(usize),
    /// branch if true
    BrIf(usize),
    /// branch table
    BrTable(Box<[usize]>),

    /// return from function
    Return,
    /// call function
    Call(usize),
    /// call indirect
    CallIndirect(usize),
    /// select value
    Select,

    /// Gets a value from a local variable
    LocalGet(usize),
    /// Sets a value to a local variable
    LocalSet(usize),
    LocalTee(usize),

    /// Gets a 32-bit value from a local variable
    LocalGet32(usize),
    /// Sets a 32-bit value to a local variable
    LocalSet32(usize),
    LocalTee32(usize),

    /// Gets a value from a global variable
    GlobalGet(usize),
    /// Sets a value to a global variable
    GlobalSet(usize),

    I32Load(u32),
    I32Load8S(u32),
    I32Load8U(u32),
    I32Load16S(u32),
    I32Load16U(u32),
    I32Store(u32),
    I32Store8(u32),
    I32Store16(u32),
    I64Load(u32),
    I64Load8S(u32),
    I64Load8U(u32),
    I64Load16S(u32),
    I64Load16U(u32),
    I64Load32S(u32),
    I64Load32U(u32),
    I64Store(u32),
    I64Store8(u32),
    I64Store16(u32),
    I64Store32(u32),
    MemorySize,
    MemoryGrow,

    I32Const(i32),
    I64Const(i64),

    I32Eqz,
    I32Eq,
    I32Ne,
    I32LtS,
    I32LtU,
    I32GtS,
    I32GtU,
    I32LeS,
    I32LeU,
    I32GeS,
    I32GeU,
    I32Clz,
    I32Ctz,
    I32Popcnt,
    I32Add,
    I32Sub,
    I32Mul,
    I32DivS,
    I32DivU,
    I32RemS,
    I32RemU,
    I32And,
    I32Or,
    I32Xor,
    I32Shl,
    I32ShrS,
    I32ShrU,
    I32Rotl,
    I32Rotr,

    I64Eqz,
    I64Eq,
    I64Ne,
    I64LtS,
    I64LtU,
    I64GtS,
    I64GtU,
    I64LeS,
    I64LeU,
    I64GeS,
    I64GeU,
    I64Clz,
    I64Ctz,
    I64Popcnt,
    I64Add,
    I64Sub,
    I64Mul,
    I64DivS,
    I64DivU,
    I64RemS,
    I64RemU,
    I64And,
    I64Or,
    I64Xor,
    I64Shl,
    I64ShrS,
    I64ShrU,
    I64Rotl,
    I64Rotr,

    I64Extend8S,
    I64Extend16S,
    I64Extend32S,
    I64ExtendI32S,
    I64ExtendI32U,
    I32WrapI64,
    I32Extend8S,
    I32Extend16S,

    // Fused Instructions
    FusedI32SetConst(usize, i32),
    FusedI32AddI(i32),
    FusedI32SubI(i32),
    FusedI32AndI(i32),
    FusedI32OrI(i32),
    FusedI32XorI(i32),
    FusedI32ShlI(i32),
    FusedI32ShrSI(i32),
    FusedI32ShrUI(i32),

    FusedI64SetConst(usize, i64),
    FusedI64AddI(i64),
    FusedI64SubI(i64),

    FusedI32BrZ(usize),
    FusedI32BrEq(usize),
    FusedI32BrNe(usize),
    FusedI32BrLtS(usize),
    FusedI32BrLtU(usize),
    FusedI32BrGtS(usize),
    FusedI32BrGtU(usize),
    FusedI32BrLeS(usize),
    FusedI32BrLeU(usize),
    FusedI32BrGeS(usize),
    FusedI32BrGeU(usize),

    FusedI64BrZ(usize),
    FusedI64BrEq(usize),
    FusedI64BrNe(usize),
}

type StackType = usize;

/// Wasm Intermediate Code
#[derive(Debug)]
pub struct WasmImc {
    pub source: u32,
    pub mnemonic: WasmIntMnemonic,
    pub stack_level: StackType,
}

impl WasmImc {
    /// Maximum size of a byte code
    pub const MAX_SOURCE_SIZE: usize = 0xFF_FF_FF;

    #[inline]
    pub fn from_mnemonic(mnemonic: WasmIntMnemonic) -> Self {
        Self {
            source: 0,
            mnemonic,
            stack_level: StackType::default(),
        }
    }

    #[inline]
    pub const fn new(
        source_position: usize,
        opcode: WasmOpcode,
        mnemonic: WasmIntMnemonic,
        stack_level: usize,
    ) -> Self {
        let source = ((source_position as u32) << 8) | (opcode as u32);
        Self {
            source,
            mnemonic,
            stack_level: stack_level as StackType,
        }
    }

    #[inline]
    pub const fn source_position(&self) -> usize {
        (self.source >> 8) as usize
    }

    #[inline]
    pub const fn opcode(&self) -> Option<WasmOpcode> {
        WasmOpcode::new(self.source as u8)
    }

    #[inline]
    pub const fn mnemonic(&self) -> &WasmIntMnemonic {
        &self.mnemonic
    }

    #[inline]
    pub const fn mnemonic_mut(&mut self) -> &mut WasmIntMnemonic {
        &mut self.mnemonic
    }

    #[inline]
    pub const fn stack_level(&self) -> usize {
        self.stack_level as usize
    }

    pub fn adjust_branch_target<F, E>(&mut self, mut f: F) -> Result<(), E>
    where
        F: FnMut(WasmOpcode, usize) -> Result<usize, E>,
    {
        use WasmIntMnemonic::*;
        match self.mnemonic_mut() {
            Br(target) => {
                *target = f(WasmOpcode::Br, *target)?;
            }
            BrIf(target) => {
                *target = f(WasmOpcode::BrIf, *target)?;
            }

            FusedI32BrZ(target) => {
                *target = f(WasmOpcode::BrIf, *target)?;
            }
            FusedI32BrEq(target) => {
                *target = f(WasmOpcode::BrIf, *target)?;
            }
            FusedI32BrNe(target) => {
                *target = f(WasmOpcode::BrIf, *target)?;
            }
            FusedI32BrLtS(target) => {
                *target = f(WasmOpcode::BrIf, *target)?;
            }
            FusedI32BrLtU(target) => {
                *target = f(WasmOpcode::BrIf, *target)?;
            }
            FusedI32BrGtS(target) => {
                *target = f(WasmOpcode::BrIf, *target)?;
            }
            FusedI32BrGtU(target) => {
                *target = f(WasmOpcode::BrIf, *target)?;
            }
            FusedI32BrLeS(target) => {
                *target = f(WasmOpcode::BrIf, *target)?;
            }
            FusedI32BrLeU(target) => {
                *target = f(WasmOpcode::BrIf, *target)?;
            }
            FusedI32BrGeS(target) => {
                *target = f(WasmOpcode::BrIf, *target)?;
            }
            FusedI32BrGeU(target) => {
                *target = f(WasmOpcode::BrIf, *target)?;
            }

            FusedI64BrZ(target) => {
                *target = f(WasmOpcode::BrIf, *target)?;
            }
            FusedI64BrEq(target) => {
                *target = f(WasmOpcode::BrIf, *target)?;
            }
            FusedI64BrNe(target) => {
                *target = f(WasmOpcode::BrIf, *target)?;
            }

            BrTable(table) => {
                let mut vec = Vec::with_capacity(table.len());
                for target in table.iter() {
                    vec.push(f(WasmOpcode::BrTable, *target)?);
                }
                *table = vec.into_boxed_slice();
            }
            _ => (),
        }
        Ok(())
    }
}

impl From<WasmIntMnemonic> for WasmImc {
    #[inline]
    fn from(val: WasmIntMnemonic) -> Self {
        Self::from_mnemonic(val)
    }
}
