// Wasm Bytecode Table (AUTO GENERATED)
use core::convert::TryFrom;

#[non_exhaustive]
#[derive(Debug, Copy, Clone)]
pub enum WasmOpcode {
    /// 00 (mvp) unreachable
    Unreachable = 0x00,
    /// 01 (mvp) nop
    Nop = 0x01,
    /// 02 (mvp) block block_type; expr; end
    Block = 0x02,
    /// 03 (mvp) loop block_type; expr; end
    Loop = 0x03,
    /// 04 (mvp) if block_type; expr; else; expr; end
    If = 0x04,
    /// 05 (mvp) else expr; end
    Else = 0x05,
    /// 0B (mvp) end
    End = 0x0B,
    /// 0C (mvp) br labelidx
    Br = 0x0C,
    /// 0D (mvp) br_if labelidx
    BrIf = 0x0D,
    /// 0E (mvp) br_table vec(labelidx) labelidx
    BrTable = 0x0E,
    /// 0F (mvp) return
    Return = 0x0F,
    /// 10 (mvp) call funcidx
    Call = 0x10,
    /// 11 (mvp) call_indirect typeidx 0x00
    CallIndirect = 0x11,
    /// 12 (tail_call) return_call funcidx
    ReturnCall = 0x12,
    /// 13 (tail_call) return_call_indirect typeidx 0x00
    ReturnCallIndirect = 0x13,
    /// 1A (mvp) drop
    Drop = 0x1A,
    /// 1B (mvp) select
    Select = 0x1B,
    /// 20 (mvp) local.get localidx
    LocalGet = 0x20,
    /// 21 (mvp) local.set localidx
    LocalSet = 0x21,
    /// 22 (mvp) local.tee localidx
    LocalTee = 0x22,
    /// 23 (mvp) global.get globalidx
    GlobalGet = 0x23,
    /// 24 (mvp) global.set globalidx
    GlobalSet = 0x24,
    /// 28 (mvp) i32.load align offset
    I32Load = 0x28,
    /// 29 (mvp_i64) i64.load align offset
    I64Load = 0x29,
    /// 2A (mvp_f32) f32.load align offset
    F32Load = 0x2A,
    /// 2B (mvp_f64) f64.load align offset
    F64Load = 0x2B,
    /// 2C (mvp) i32.load8_s align offset
    I32Load8S = 0x2C,
    /// 2D (mvp) i32.load8_u align offset
    I32Load8U = 0x2D,
    /// 2E (mvp) i32.load16_s align offset
    I32Load16S = 0x2E,
    /// 2F (mvp) i32.load16_u align offset
    I32Load16U = 0x2F,
    /// 30 (mvp_i64) i64.load8_s align offset
    I64Load8S = 0x30,
    /// 31 (mvp_i64) i64.load8_u align offset
    I64Load8U = 0x31,
    /// 32 (mvp_i64) i64.load16_s align offset
    I64Load16S = 0x32,
    /// 33 (mvp_i64) i64.load16_u align offset
    I64Load16U = 0x33,
    /// 34 (mvp_i64) i64.load32_s align offset
    I64Load32S = 0x34,
    /// 35 (mvp_i64) i64.load32_u align offset
    I64Load32U = 0x35,
    /// 36 (mvp) i32.store align offset
    I32Store = 0x36,
    /// 37 (mvp_i64) i64.store align offset
    I64Store = 0x37,
    /// 38 (mvp_f32) f32.store align offset
    F32Store = 0x38,
    /// 39 (mvp_f64) f64.store align offset
    F64Store = 0x39,
    /// 3A (mvp) i32.store8 align offset
    I32Store8 = 0x3A,
    /// 3B (mvp) i32.store16 align offset
    I32Store16 = 0x3B,
    /// 3C (mvp_i64) i64.store8 align offset
    I64Store8 = 0x3C,
    /// 3D (mvp_i64) i64.store16 align offset
    I64Store16 = 0x3D,
    /// 3E (mvp_i64) i64.store32 align offset
    I64Store32 = 0x3E,
    /// 3F (mvp) memory.size 0x00
    MemorySize = 0x3F,
    /// 40 (mvp) memory.grow 0x00
    MemoryGrow = 0x40,
    /// 41 (mvp) i32.const n
    I32Const = 0x41,
    /// 42 (mvp_i64) i64.const n
    I64Const = 0x42,
    /// 43 (mvp_f32) f32.const z
    F32Const = 0x43,
    /// 44 (mvp_f64) f64.const z
    F64Const = 0x44,
    /// 45 (mvp) i32.eqz
    I32Eqz = 0x45,
    /// 46 (mvp) i32.eq
    I32Eq = 0x46,
    /// 47 (mvp) i32.ne
    I32Ne = 0x47,
    /// 48 (mvp) i32.lt_s
    I32LtS = 0x48,
    /// 49 (mvp) i32.lt_u
    I32LtU = 0x49,
    /// 4A (mvp) i32.gt_s
    I32GtS = 0x4A,
    /// 4B (mvp) i32.gt_u
    I32GtU = 0x4B,
    /// 4C (mvp) i32.le_s
    I32LeS = 0x4C,
    /// 4D (mvp) i32.le_u
    I32LeU = 0x4D,
    /// 4E (mvp) i32.ge_s
    I32GeS = 0x4E,
    /// 4F (mvp) i32.ge_u
    I32GeU = 0x4F,
    /// 50 (mvp_i64) i64.eqz
    I64Eqz = 0x50,
    /// 51 (mvp_i64) i64.eq
    I64Eq = 0x51,
    /// 52 (mvp_i64) i64.ne
    I64Ne = 0x52,
    /// 53 (mvp_i64) i64.lt_s
    I64LtS = 0x53,
    /// 54 (mvp_i64) i64.lt_u
    I64LtU = 0x54,
    /// 55 (mvp_i64) i64.gt_s
    I64GtS = 0x55,
    /// 56 (mvp_i64) i64.gt_u
    I64GtU = 0x56,
    /// 57 (mvp_i64) i64.le_s
    I64LeS = 0x57,
    /// 58 (mvp_i64) i64.le_u
    I64LeU = 0x58,
    /// 59 (mvp_i64) i64.ge_s
    I64GeS = 0x59,
    /// 5A (mvp_i64) i64.ge_u
    I64GeU = 0x5A,
    /// 5B (mvp_f32) f32.eq
    F32Eq = 0x5B,
    /// 5C (mvp_f32) f32.ne
    F32Ne = 0x5C,
    /// 5D (mvp_f32) f32.lt
    F32Lt = 0x5D,
    /// 5E (mvp_f32) f32.gt
    F32Gt = 0x5E,
    /// 5F (mvp_f32) f32.le
    F32Le = 0x5F,
    /// 60 (mvp_f32) f32.ge
    F32Ge = 0x60,
    /// 61 (mvp_f64) f64.eq
    F64Eq = 0x61,
    /// 62 (mvp_f64) f64.ne
    F64Ne = 0x62,
    /// 63 (mvp_f64) f64.lt
    F64Lt = 0x63,
    /// 64 (mvp_f64) f64.gt
    F64Gt = 0x64,
    /// 65 (mvp_f64) f64.le
    F64Le = 0x65,
    /// 66 (mvp_f64) f64.ge
    F64Ge = 0x66,
    /// 67 (mvp) i32.clz
    I32Clz = 0x67,
    /// 68 (mvp) i32.ctz
    I32Ctz = 0x68,
    /// 69 (mvp) i32.popcnt
    I32Popcnt = 0x69,
    /// 6A (mvp) i32.add
    I32Add = 0x6A,
    /// 6B (mvp) i32.sub
    I32Sub = 0x6B,
    /// 6C (mvp) i32.mul
    I32Mul = 0x6C,
    /// 6D (mvp) i32.div_s
    I32DivS = 0x6D,
    /// 6E (mvp) i32.div_u
    I32DivU = 0x6E,
    /// 6F (mvp) i32.rem_s
    I32RemS = 0x6F,
    /// 70 (mvp) i32.rem_u
    I32RemU = 0x70,
    /// 71 (mvp) i32.and
    I32And = 0x71,
    /// 72 (mvp) i32.or
    I32Or = 0x72,
    /// 73 (mvp) i32.xor
    I32Xor = 0x73,
    /// 74 (mvp) i32.shl
    I32Shl = 0x74,
    /// 75 (mvp) i32.shr_s
    I32ShrS = 0x75,
    /// 76 (mvp) i32.shr_u
    I32ShrU = 0x76,
    /// 77 (mvp) i32.rotl
    I32Rotl = 0x77,
    /// 78 (mvp) i32.rotr
    I32Rotr = 0x78,
    /// 79 (mvp_i64) i64.clz
    I64Clz = 0x79,
    /// 7A (mvp_i64) i64.ctz
    I64Ctz = 0x7A,
    /// 7B (mvp_i64) i64.popcnt
    I64Popcnt = 0x7B,
    /// 7C (mvp_i64) i64.add
    I64Add = 0x7C,
    /// 7D (mvp_i64) i64.sub
    I64Sub = 0x7D,
    /// 7E (mvp_i64) i64.mul
    I64Mul = 0x7E,
    /// 7F (mvp_i64) i64.div_s
    I64DivS = 0x7F,
    /// 80 (mvp_i64) i64.div_u
    I64DivU = 0x80,
    /// 81 (mvp_i64) i64.rem_s
    I64RemS = 0x81,
    /// 82 (mvp_i64) i64.rem_u
    I64RemU = 0x82,
    /// 83 (mvp_i64) i64.and
    I64And = 0x83,
    /// 84 (mvp_i64) i64.or
    I64Or = 0x84,
    /// 85 (mvp_i64) i64.xor
    I64Xor = 0x85,
    /// 86 (mvp_i64) i64.shl
    I64Shl = 0x86,
    /// 87 (mvp_i64) i64.shr_s
    I64ShrS = 0x87,
    /// 88 (mvp_i64) i64.shr_u
    I64ShrU = 0x88,
    /// 89 (mvp_i64) i64.rotl
    I64Rotl = 0x89,
    /// 8A (mvp_i64) i64.rotr
    I64Rotr = 0x8A,
    /// 8B (mvp_f32) f32.abs
    F32Abs = 0x8B,
    /// 8C (mvp_f32) f32.neg
    F32Neg = 0x8C,
    /// 8D (mvp_f32) f32.ceil
    F32Ceil = 0x8D,
    /// 8E (mvp_f32) f32.floor
    F32Floor = 0x8E,
    /// 8F (mvp_f32) f32.trunc
    F32Trunc = 0x8F,
    /// 90 (mvp_f32) f32.nearest
    F32Nearest = 0x90,
    /// 91 (mvp_f32) f32.sqrt
    F32Sqrt = 0x91,
    /// 92 (mvp_f32) f32.add
    F32Add = 0x92,
    /// 93 (mvp_f32) f32.sub
    F32Sub = 0x93,
    /// 94 (mvp_f32) f32.mul
    F32Mul = 0x94,
    /// 95 (mvp_f32) f32.div
    F32Div = 0x95,
    /// 96 (mvp_f32) f32.min
    F32Min = 0x96,
    /// 97 (mvp_f32) f32.max
    F32Max = 0x97,
    /// 98 (mvp_f32) f32.copysign
    F32Copysign = 0x98,
    /// 99 (mvp_f64) f64.abs
    F64Abs = 0x99,
    /// 9A (mvp_f64) f64.neg
    F64Neg = 0x9A,
    /// 9B (mvp_f64) f64.ceil
    F64Ceil = 0x9B,
    /// 9C (mvp_f64) f64.floor
    F64Floor = 0x9C,
    /// 9D (mvp_f64) f64.trunc
    F64Trunc = 0x9D,
    /// 9E (mvp_f64) f64.nearest
    F64Nearest = 0x9E,
    /// 9F (mvp_f64) f64.sqrt
    F64Sqrt = 0x9F,
    /// A0 (mvp_f64) f64.add
    F64Add = 0xA0,
    /// A1 (mvp_f64) f64.sub
    F64Sub = 0xA1,
    /// A2 (mvp_f64) f64.mul
    F64Mul = 0xA2,
    /// A3 (mvp_f64) f64.div
    F64Div = 0xA3,
    /// A4 (mvp_f64) f64.min
    F64Min = 0xA4,
    /// A5 (mvp_f64) f64.max
    F64Max = 0xA5,
    /// A6 (mvp_f64) f64.copysign
    F64Copysign = 0xA6,
    /// A7 (mvp_i64) i32.wrap_i64
    I32WrapI64 = 0xA7,
    /// A8 (mvp_f32) i32.trunc_f32_s
    I32TruncF32S = 0xA8,
    /// A9 (mvp_f32) i32.trunc_f32_u
    I32TruncF32U = 0xA9,
    /// AA (mvp_f32) i32.trunc_f64_s
    I32TruncF64S = 0xAA,
    /// AB (mvp_f32) i32.trunc_f64_u
    I32TruncF64U = 0xAB,
    /// AC (mvp_i64) i64.extend_i32_s
    I64ExtendI32S = 0xAC,
    /// AD (mvp_i64) i64.extend_i32_u
    I64ExtendI32U = 0xAD,
    /// AE (mvp_f32) i64.trunc_f32_s
    I64TruncF32S = 0xAE,
    /// AF (mvp_f32) i64.trunc_f32_u
    I64TruncF32U = 0xAF,
    /// B0 (mvp_f64) i64.trunc_f64_s
    I64TruncF64S = 0xB0,
    /// B1 (mvp_f64) i64.trunc_f64_u
    I64TruncF64U = 0xB1,
    /// B2 (mvp_f32) f32.convert_i32_s
    F32ConvertI32S = 0xB2,
    /// B3 (mvp_f32) f32.convert_i32_u
    F32ConvertI32U = 0xB3,
    /// B4 (mvp_f32) f32.convert_i64_s
    F32ConvertI64S = 0xB4,
    /// B5 (mvp_f32) f32.convert_i64_u
    F32ConvertI64U = 0xB5,
    /// B6 (mvp_f64) f32.demote_f64
    F32DemoteF64 = 0xB6,
    /// B7 (mvp_f64) f64.convert_i32_s
    F64ConvertI32S = 0xB7,
    /// B8 (mvp_f64) f64.convert_i32_u
    F64ConvertI32U = 0xB8,
    /// B9 (mvp_f64) f64.convert_i64_s
    F64ConvertI64S = 0xB9,
    /// BA (mvp_f64) f64.convert_i64_u
    F64ConvertI64U = 0xBA,
    /// BB (mvp_f64) f64.promote_f32
    F64PromoteF32 = 0xBB,
    /// BC (mvp_f32) i32.reinterpret_f32
    I32ReinterpretF32 = 0xBC,
    /// BD (mvp_f64) i64.reinterpret_f64
    I64ReinterpretF64 = 0xBD,
    /// BE (mvp_f32) f32.reinterpret_i32
    F32ReinterpretI32 = 0xBE,
    /// BF (mvp_f64) f64.reinterpret_i64
    F64ReinterpretI64 = 0xBF,
    /// C0 (sign_extend) i32.extend8_s
    I32Extend8S = 0xC0,
    /// C1 (sign_extend) i32.extend16_s
    I32Extend16S = 0xC1,
    /// C2 (sign_extend) i64.extend8_s
    I64Extend8S = 0xC2,
    /// C3 (sign_extend) i64.extend16_s
    I64Extend16S = 0xC3,
    /// C4 (sign_extend) i64.extend32_s
    I64Extend32S = 0xC4,
}

#[non_exhaustive]
#[derive(Debug, Copy, Clone)]
pub enum WasmOperandType {
    Implied,
    Block,
    Else,
    End,
    Br,
    BrTable,
    Call,
    CallIndirect,
    Local,
    Global,
    Memory,
    MemSize,
    I32,
    I64,
    F32,
    F64,
}

#[non_exhaustive]
#[derive(Debug, Copy, Clone)]
pub enum WasmProposalType {
    Mvp,
    TailCall,
    MvpI64,
    MvpF32,
    MvpF64,
    SignExtend,
}

impl WasmOpcode {
    pub fn from_u8(value: u8) -> Self {
        Self::try_from(value).unwrap_or(Self::Unreachable)
    }

    pub fn to_str(&self) -> &str {
        match *self {
            Self::Unreachable => "unreachable",
            Self::Nop => "nop",
            Self::Block => "block",
            Self::Loop => "loop",
            Self::If => "if",
            Self::Else => "else",
            Self::End => "end",
            Self::Br => "br",
            Self::BrIf => "br_if",
            Self::BrTable => "br_table",
            Self::Return => "return",
            Self::Call => "call",
            Self::CallIndirect => "call_indirect",
            Self::ReturnCall => "return_call",
            Self::ReturnCallIndirect => "return_call_indirect",
            Self::Drop => "drop",
            Self::Select => "select",
            Self::LocalGet => "local.get",
            Self::LocalSet => "local.set",
            Self::LocalTee => "local.tee",
            Self::GlobalGet => "global.get",
            Self::GlobalSet => "global.set",
            Self::I32Load => "i32.load",
            Self::I64Load => "i64.load",
            Self::F32Load => "f32.load",
            Self::F64Load => "f64.load",
            Self::I32Load8S => "i32.load8_s",
            Self::I32Load8U => "i32.load8_u",
            Self::I32Load16S => "i32.load16_s",
            Self::I32Load16U => "i32.load16_u",
            Self::I64Load8S => "i64.load8_s",
            Self::I64Load8U => "i64.load8_u",
            Self::I64Load16S => "i64.load16_s",
            Self::I64Load16U => "i64.load16_u",
            Self::I64Load32S => "i64.load32_s",
            Self::I64Load32U => "i64.load32_u",
            Self::I32Store => "i32.store",
            Self::I64Store => "i64.store",
            Self::F32Store => "f32.store",
            Self::F64Store => "f64.store",
            Self::I32Store8 => "i32.store8",
            Self::I32Store16 => "i32.store16",
            Self::I64Store8 => "i64.store8",
            Self::I64Store16 => "i64.store16",
            Self::I64Store32 => "i64.store32",
            Self::MemorySize => "memory.size",
            Self::MemoryGrow => "memory.grow",
            Self::I32Const => "i32.const",
            Self::I64Const => "i64.const",
            Self::F32Const => "f32.const",
            Self::F64Const => "f64.const",
            Self::I32Eqz => "i32.eqz",
            Self::I32Eq => "i32.eq",
            Self::I32Ne => "i32.ne",
            Self::I32LtS => "i32.lt_s",
            Self::I32LtU => "i32.lt_u",
            Self::I32GtS => "i32.gt_s",
            Self::I32GtU => "i32.gt_u",
            Self::I32LeS => "i32.le_s",
            Self::I32LeU => "i32.le_u",
            Self::I32GeS => "i32.ge_s",
            Self::I32GeU => "i32.ge_u",
            Self::I64Eqz => "i64.eqz",
            Self::I64Eq => "i64.eq",
            Self::I64Ne => "i64.ne",
            Self::I64LtS => "i64.lt_s",
            Self::I64LtU => "i64.lt_u",
            Self::I64GtS => "i64.gt_s",
            Self::I64GtU => "i64.gt_u",
            Self::I64LeS => "i64.le_s",
            Self::I64LeU => "i64.le_u",
            Self::I64GeS => "i64.ge_s",
            Self::I64GeU => "i64.ge_u",
            Self::F32Eq => "f32.eq",
            Self::F32Ne => "f32.ne",
            Self::F32Lt => "f32.lt",
            Self::F32Gt => "f32.gt",
            Self::F32Le => "f32.le",
            Self::F32Ge => "f32.ge",
            Self::F64Eq => "f64.eq",
            Self::F64Ne => "f64.ne",
            Self::F64Lt => "f64.lt",
            Self::F64Gt => "f64.gt",
            Self::F64Le => "f64.le",
            Self::F64Ge => "f64.ge",
            Self::I32Clz => "i32.clz",
            Self::I32Ctz => "i32.ctz",
            Self::I32Popcnt => "i32.popcnt",
            Self::I32Add => "i32.add",
            Self::I32Sub => "i32.sub",
            Self::I32Mul => "i32.mul",
            Self::I32DivS => "i32.div_s",
            Self::I32DivU => "i32.div_u",
            Self::I32RemS => "i32.rem_s",
            Self::I32RemU => "i32.rem_u",
            Self::I32And => "i32.and",
            Self::I32Or => "i32.or",
            Self::I32Xor => "i32.xor",
            Self::I32Shl => "i32.shl",
            Self::I32ShrS => "i32.shr_s",
            Self::I32ShrU => "i32.shr_u",
            Self::I32Rotl => "i32.rotl",
            Self::I32Rotr => "i32.rotr",
            Self::I64Clz => "i64.clz",
            Self::I64Ctz => "i64.ctz",
            Self::I64Popcnt => "i64.popcnt",
            Self::I64Add => "i64.add",
            Self::I64Sub => "i64.sub",
            Self::I64Mul => "i64.mul",
            Self::I64DivS => "i64.div_s",
            Self::I64DivU => "i64.div_u",
            Self::I64RemS => "i64.rem_s",
            Self::I64RemU => "i64.rem_u",
            Self::I64And => "i64.and",
            Self::I64Or => "i64.or",
            Self::I64Xor => "i64.xor",
            Self::I64Shl => "i64.shl",
            Self::I64ShrS => "i64.shr_s",
            Self::I64ShrU => "i64.shr_u",
            Self::I64Rotl => "i64.rotl",
            Self::I64Rotr => "i64.rotr",
            Self::F32Abs => "f32.abs",
            Self::F32Neg => "f32.neg",
            Self::F32Ceil => "f32.ceil",
            Self::F32Floor => "f32.floor",
            Self::F32Trunc => "f32.trunc",
            Self::F32Nearest => "f32.nearest",
            Self::F32Sqrt => "f32.sqrt",
            Self::F32Add => "f32.add",
            Self::F32Sub => "f32.sub",
            Self::F32Mul => "f32.mul",
            Self::F32Div => "f32.div",
            Self::F32Min => "f32.min",
            Self::F32Max => "f32.max",
            Self::F32Copysign => "f32.copysign",
            Self::F64Abs => "f64.abs",
            Self::F64Neg => "f64.neg",
            Self::F64Ceil => "f64.ceil",
            Self::F64Floor => "f64.floor",
            Self::F64Trunc => "f64.trunc",
            Self::F64Nearest => "f64.nearest",
            Self::F64Sqrt => "f64.sqrt",
            Self::F64Add => "f64.add",
            Self::F64Sub => "f64.sub",
            Self::F64Mul => "f64.mul",
            Self::F64Div => "f64.div",
            Self::F64Min => "f64.min",
            Self::F64Max => "f64.max",
            Self::F64Copysign => "f64.copysign",
            Self::I32WrapI64 => "i32.wrap_i64",
            Self::I32TruncF32S => "i32.trunc_f32_s",
            Self::I32TruncF32U => "i32.trunc_f32_u",
            Self::I32TruncF64S => "i32.trunc_f64_s",
            Self::I32TruncF64U => "i32.trunc_f64_u",
            Self::I64ExtendI32S => "i64.extend_i32_s",
            Self::I64ExtendI32U => "i64.extend_i32_u",
            Self::I64TruncF32S => "i64.trunc_f32_s",
            Self::I64TruncF32U => "i64.trunc_f32_u",
            Self::I64TruncF64S => "i64.trunc_f64_s",
            Self::I64TruncF64U => "i64.trunc_f64_u",
            Self::F32ConvertI32S => "f32.convert_i32_s",
            Self::F32ConvertI32U => "f32.convert_i32_u",
            Self::F32ConvertI64S => "f32.convert_i64_s",
            Self::F32ConvertI64U => "f32.convert_i64_u",
            Self::F32DemoteF64 => "f32.demote_f64",
            Self::F64ConvertI32S => "f64.convert_i32_s",
            Self::F64ConvertI32U => "f64.convert_i32_u",
            Self::F64ConvertI64S => "f64.convert_i64_s",
            Self::F64ConvertI64U => "f64.convert_i64_u",
            Self::F64PromoteF32 => "f64.promote_f32",
            Self::I32ReinterpretF32 => "i32.reinterpret_f32",
            Self::I64ReinterpretF64 => "i64.reinterpret_f64",
            Self::F32ReinterpretI32 => "f32.reinterpret_i32",
            Self::F64ReinterpretI64 => "f64.reinterpret_i64",
            Self::I32Extend8S => "i32.extend8_s",
            Self::I32Extend16S => "i32.extend16_s",
            Self::I64Extend8S => "i64.extend8_s",
            Self::I64Extend16S => "i64.extend16_s",
            Self::I64Extend32S => "i64.extend32_s",
        }
    }

    pub fn operand_type(&self) -> WasmOperandType {
        match *self {
            Self::Block => WasmOperandType::Block,
            Self::Loop => WasmOperandType::Block,
            Self::If => WasmOperandType::Block,
            Self::Else => WasmOperandType::Else,
            Self::End => WasmOperandType::End,
            Self::Br => WasmOperandType::Br,
            Self::BrIf => WasmOperandType::Br,
            Self::BrTable => WasmOperandType::BrTable,
            Self::Call => WasmOperandType::Call,
            Self::CallIndirect => WasmOperandType::CallIndirect,
            Self::ReturnCall => WasmOperandType::Call,
            Self::ReturnCallIndirect => WasmOperandType::CallIndirect,
            Self::LocalGet => WasmOperandType::Local,
            Self::LocalSet => WasmOperandType::Local,
            Self::LocalTee => WasmOperandType::Local,
            Self::GlobalGet => WasmOperandType::Global,
            Self::GlobalSet => WasmOperandType::Global,
            Self::I32Load => WasmOperandType::Memory,
            Self::I64Load => WasmOperandType::Memory,
            Self::F32Load => WasmOperandType::Memory,
            Self::F64Load => WasmOperandType::Memory,
            Self::I32Load8S => WasmOperandType::Memory,
            Self::I32Load8U => WasmOperandType::Memory,
            Self::I32Load16S => WasmOperandType::Memory,
            Self::I32Load16U => WasmOperandType::Memory,
            Self::I64Load8S => WasmOperandType::Memory,
            Self::I64Load8U => WasmOperandType::Memory,
            Self::I64Load16S => WasmOperandType::Memory,
            Self::I64Load16U => WasmOperandType::Memory,
            Self::I64Load32S => WasmOperandType::Memory,
            Self::I64Load32U => WasmOperandType::Memory,
            Self::I32Store => WasmOperandType::Memory,
            Self::I64Store => WasmOperandType::Memory,
            Self::F32Store => WasmOperandType::Memory,
            Self::F64Store => WasmOperandType::Memory,
            Self::I32Store8 => WasmOperandType::Memory,
            Self::I32Store16 => WasmOperandType::Memory,
            Self::I64Store8 => WasmOperandType::Memory,
            Self::I64Store16 => WasmOperandType::Memory,
            Self::I64Store32 => WasmOperandType::Memory,
            Self::MemorySize => WasmOperandType::MemSize,
            Self::MemoryGrow => WasmOperandType::MemSize,
            Self::I32Const => WasmOperandType::I32,
            Self::I64Const => WasmOperandType::I64,
            Self::F32Const => WasmOperandType::F32,
            Self::F64Const => WasmOperandType::F64,
            _ => WasmOperandType::Implied,
        }
    }

    pub fn proposal_type(&self) -> WasmProposalType {
        match *self {
            Self::ReturnCall => WasmProposalType::TailCall,
            Self::ReturnCallIndirect => WasmProposalType::TailCall,
            Self::I64Load => WasmProposalType::MvpI64,
            Self::F32Load => WasmProposalType::MvpF32,
            Self::F64Load => WasmProposalType::MvpF64,
            Self::I64Load8S => WasmProposalType::MvpI64,
            Self::I64Load8U => WasmProposalType::MvpI64,
            Self::I64Load16S => WasmProposalType::MvpI64,
            Self::I64Load16U => WasmProposalType::MvpI64,
            Self::I64Load32S => WasmProposalType::MvpI64,
            Self::I64Load32U => WasmProposalType::MvpI64,
            Self::I64Store => WasmProposalType::MvpI64,
            Self::F32Store => WasmProposalType::MvpF32,
            Self::F64Store => WasmProposalType::MvpF64,
            Self::I64Store8 => WasmProposalType::MvpI64,
            Self::I64Store16 => WasmProposalType::MvpI64,
            Self::I64Store32 => WasmProposalType::MvpI64,
            Self::I64Const => WasmProposalType::MvpI64,
            Self::F32Const => WasmProposalType::MvpF32,
            Self::F64Const => WasmProposalType::MvpF64,
            Self::I64Eqz => WasmProposalType::MvpI64,
            Self::I64Eq => WasmProposalType::MvpI64,
            Self::I64Ne => WasmProposalType::MvpI64,
            Self::I64LtS => WasmProposalType::MvpI64,
            Self::I64LtU => WasmProposalType::MvpI64,
            Self::I64GtS => WasmProposalType::MvpI64,
            Self::I64GtU => WasmProposalType::MvpI64,
            Self::I64LeS => WasmProposalType::MvpI64,
            Self::I64LeU => WasmProposalType::MvpI64,
            Self::I64GeS => WasmProposalType::MvpI64,
            Self::I64GeU => WasmProposalType::MvpI64,
            Self::F32Eq => WasmProposalType::MvpF32,
            Self::F32Ne => WasmProposalType::MvpF32,
            Self::F32Lt => WasmProposalType::MvpF32,
            Self::F32Gt => WasmProposalType::MvpF32,
            Self::F32Le => WasmProposalType::MvpF32,
            Self::F32Ge => WasmProposalType::MvpF32,
            Self::F64Eq => WasmProposalType::MvpF64,
            Self::F64Ne => WasmProposalType::MvpF64,
            Self::F64Lt => WasmProposalType::MvpF64,
            Self::F64Gt => WasmProposalType::MvpF64,
            Self::F64Le => WasmProposalType::MvpF64,
            Self::F64Ge => WasmProposalType::MvpF64,
            Self::I64Clz => WasmProposalType::MvpI64,
            Self::I64Ctz => WasmProposalType::MvpI64,
            Self::I64Popcnt => WasmProposalType::MvpI64,
            Self::I64Add => WasmProposalType::MvpI64,
            Self::I64Sub => WasmProposalType::MvpI64,
            Self::I64Mul => WasmProposalType::MvpI64,
            Self::I64DivS => WasmProposalType::MvpI64,
            Self::I64DivU => WasmProposalType::MvpI64,
            Self::I64RemS => WasmProposalType::MvpI64,
            Self::I64RemU => WasmProposalType::MvpI64,
            Self::I64And => WasmProposalType::MvpI64,
            Self::I64Or => WasmProposalType::MvpI64,
            Self::I64Xor => WasmProposalType::MvpI64,
            Self::I64Shl => WasmProposalType::MvpI64,
            Self::I64ShrS => WasmProposalType::MvpI64,
            Self::I64ShrU => WasmProposalType::MvpI64,
            Self::I64Rotl => WasmProposalType::MvpI64,
            Self::I64Rotr => WasmProposalType::MvpI64,
            Self::F32Abs => WasmProposalType::MvpF32,
            Self::F32Neg => WasmProposalType::MvpF32,
            Self::F32Ceil => WasmProposalType::MvpF32,
            Self::F32Floor => WasmProposalType::MvpF32,
            Self::F32Trunc => WasmProposalType::MvpF32,
            Self::F32Nearest => WasmProposalType::MvpF32,
            Self::F32Sqrt => WasmProposalType::MvpF32,
            Self::F32Add => WasmProposalType::MvpF32,
            Self::F32Sub => WasmProposalType::MvpF32,
            Self::F32Mul => WasmProposalType::MvpF32,
            Self::F32Div => WasmProposalType::MvpF32,
            Self::F32Min => WasmProposalType::MvpF32,
            Self::F32Max => WasmProposalType::MvpF32,
            Self::F32Copysign => WasmProposalType::MvpF32,
            Self::F64Abs => WasmProposalType::MvpF64,
            Self::F64Neg => WasmProposalType::MvpF64,
            Self::F64Ceil => WasmProposalType::MvpF64,
            Self::F64Floor => WasmProposalType::MvpF64,
            Self::F64Trunc => WasmProposalType::MvpF64,
            Self::F64Nearest => WasmProposalType::MvpF64,
            Self::F64Sqrt => WasmProposalType::MvpF64,
            Self::F64Add => WasmProposalType::MvpF64,
            Self::F64Sub => WasmProposalType::MvpF64,
            Self::F64Mul => WasmProposalType::MvpF64,
            Self::F64Div => WasmProposalType::MvpF64,
            Self::F64Min => WasmProposalType::MvpF64,
            Self::F64Max => WasmProposalType::MvpF64,
            Self::F64Copysign => WasmProposalType::MvpF64,
            Self::I32WrapI64 => WasmProposalType::MvpI64,
            Self::I32TruncF32S => WasmProposalType::MvpF32,
            Self::I32TruncF32U => WasmProposalType::MvpF32,
            Self::I32TruncF64S => WasmProposalType::MvpF32,
            Self::I32TruncF64U => WasmProposalType::MvpF32,
            Self::I64ExtendI32S => WasmProposalType::MvpI64,
            Self::I64ExtendI32U => WasmProposalType::MvpI64,
            Self::I64TruncF32S => WasmProposalType::MvpF32,
            Self::I64TruncF32U => WasmProposalType::MvpF32,
            Self::I64TruncF64S => WasmProposalType::MvpF64,
            Self::I64TruncF64U => WasmProposalType::MvpF64,
            Self::F32ConvertI32S => WasmProposalType::MvpF32,
            Self::F32ConvertI32U => WasmProposalType::MvpF32,
            Self::F32ConvertI64S => WasmProposalType::MvpF32,
            Self::F32ConvertI64U => WasmProposalType::MvpF32,
            Self::F32DemoteF64 => WasmProposalType::MvpF64,
            Self::F64ConvertI32S => WasmProposalType::MvpF64,
            Self::F64ConvertI32U => WasmProposalType::MvpF64,
            Self::F64ConvertI64S => WasmProposalType::MvpF64,
            Self::F64ConvertI64U => WasmProposalType::MvpF64,
            Self::F64PromoteF32 => WasmProposalType::MvpF64,
            Self::I32ReinterpretF32 => WasmProposalType::MvpF32,
            Self::I64ReinterpretF64 => WasmProposalType::MvpF64,
            Self::F32ReinterpretI32 => WasmProposalType::MvpF32,
            Self::F64ReinterpretI64 => WasmProposalType::MvpF64,
            Self::I32Extend8S => WasmProposalType::SignExtend,
            Self::I32Extend16S => WasmProposalType::SignExtend,
            Self::I64Extend8S => WasmProposalType::SignExtend,
            Self::I64Extend16S => WasmProposalType::SignExtend,
            Self::I64Extend32S => WasmProposalType::SignExtend,
            _ => WasmProposalType::Mvp,
        }
    }
}

impl TryFrom<u8> for WasmOpcode {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::Unreachable),
            0x01 => Ok(Self::Nop),
            0x02 => Ok(Self::Block),
            0x03 => Ok(Self::Loop),
            0x04 => Ok(Self::If),
            0x05 => Ok(Self::Else),
            0x0B => Ok(Self::End),
            0x0C => Ok(Self::Br),
            0x0D => Ok(Self::BrIf),
            0x0E => Ok(Self::BrTable),
            0x0F => Ok(Self::Return),
            0x10 => Ok(Self::Call),
            0x11 => Ok(Self::CallIndirect),
            0x12 => Ok(Self::ReturnCall),
            0x13 => Ok(Self::ReturnCallIndirect),
            0x1A => Ok(Self::Drop),
            0x1B => Ok(Self::Select),
            0x20 => Ok(Self::LocalGet),
            0x21 => Ok(Self::LocalSet),
            0x22 => Ok(Self::LocalTee),
            0x23 => Ok(Self::GlobalGet),
            0x24 => Ok(Self::GlobalSet),
            0x28 => Ok(Self::I32Load),
            0x29 => Ok(Self::I64Load),
            0x2A => Ok(Self::F32Load),
            0x2B => Ok(Self::F64Load),
            0x2C => Ok(Self::I32Load8S),
            0x2D => Ok(Self::I32Load8U),
            0x2E => Ok(Self::I32Load16S),
            0x2F => Ok(Self::I32Load16U),
            0x30 => Ok(Self::I64Load8S),
            0x31 => Ok(Self::I64Load8U),
            0x32 => Ok(Self::I64Load16S),
            0x33 => Ok(Self::I64Load16U),
            0x34 => Ok(Self::I64Load32S),
            0x35 => Ok(Self::I64Load32U),
            0x36 => Ok(Self::I32Store),
            0x37 => Ok(Self::I64Store),
            0x38 => Ok(Self::F32Store),
            0x39 => Ok(Self::F64Store),
            0x3A => Ok(Self::I32Store8),
            0x3B => Ok(Self::I32Store16),
            0x3C => Ok(Self::I64Store8),
            0x3D => Ok(Self::I64Store16),
            0x3E => Ok(Self::I64Store32),
            0x3F => Ok(Self::MemorySize),
            0x40 => Ok(Self::MemoryGrow),
            0x41 => Ok(Self::I32Const),
            0x42 => Ok(Self::I64Const),
            0x43 => Ok(Self::F32Const),
            0x44 => Ok(Self::F64Const),
            0x45 => Ok(Self::I32Eqz),
            0x46 => Ok(Self::I32Eq),
            0x47 => Ok(Self::I32Ne),
            0x48 => Ok(Self::I32LtS),
            0x49 => Ok(Self::I32LtU),
            0x4A => Ok(Self::I32GtS),
            0x4B => Ok(Self::I32GtU),
            0x4C => Ok(Self::I32LeS),
            0x4D => Ok(Self::I32LeU),
            0x4E => Ok(Self::I32GeS),
            0x4F => Ok(Self::I32GeU),
            0x50 => Ok(Self::I64Eqz),
            0x51 => Ok(Self::I64Eq),
            0x52 => Ok(Self::I64Ne),
            0x53 => Ok(Self::I64LtS),
            0x54 => Ok(Self::I64LtU),
            0x55 => Ok(Self::I64GtS),
            0x56 => Ok(Self::I64GtU),
            0x57 => Ok(Self::I64LeS),
            0x58 => Ok(Self::I64LeU),
            0x59 => Ok(Self::I64GeS),
            0x5A => Ok(Self::I64GeU),
            0x5B => Ok(Self::F32Eq),
            0x5C => Ok(Self::F32Ne),
            0x5D => Ok(Self::F32Lt),
            0x5E => Ok(Self::F32Gt),
            0x5F => Ok(Self::F32Le),
            0x60 => Ok(Self::F32Ge),
            0x61 => Ok(Self::F64Eq),
            0x62 => Ok(Self::F64Ne),
            0x63 => Ok(Self::F64Lt),
            0x64 => Ok(Self::F64Gt),
            0x65 => Ok(Self::F64Le),
            0x66 => Ok(Self::F64Ge),
            0x67 => Ok(Self::I32Clz),
            0x68 => Ok(Self::I32Ctz),
            0x69 => Ok(Self::I32Popcnt),
            0x6A => Ok(Self::I32Add),
            0x6B => Ok(Self::I32Sub),
            0x6C => Ok(Self::I32Mul),
            0x6D => Ok(Self::I32DivS),
            0x6E => Ok(Self::I32DivU),
            0x6F => Ok(Self::I32RemS),
            0x70 => Ok(Self::I32RemU),
            0x71 => Ok(Self::I32And),
            0x72 => Ok(Self::I32Or),
            0x73 => Ok(Self::I32Xor),
            0x74 => Ok(Self::I32Shl),
            0x75 => Ok(Self::I32ShrS),
            0x76 => Ok(Self::I32ShrU),
            0x77 => Ok(Self::I32Rotl),
            0x78 => Ok(Self::I32Rotr),
            0x79 => Ok(Self::I64Clz),
            0x7A => Ok(Self::I64Ctz),
            0x7B => Ok(Self::I64Popcnt),
            0x7C => Ok(Self::I64Add),
            0x7D => Ok(Self::I64Sub),
            0x7E => Ok(Self::I64Mul),
            0x7F => Ok(Self::I64DivS),
            0x80 => Ok(Self::I64DivU),
            0x81 => Ok(Self::I64RemS),
            0x82 => Ok(Self::I64RemU),
            0x83 => Ok(Self::I64And),
            0x84 => Ok(Self::I64Or),
            0x85 => Ok(Self::I64Xor),
            0x86 => Ok(Self::I64Shl),
            0x87 => Ok(Self::I64ShrS),
            0x88 => Ok(Self::I64ShrU),
            0x89 => Ok(Self::I64Rotl),
            0x8A => Ok(Self::I64Rotr),
            0x8B => Ok(Self::F32Abs),
            0x8C => Ok(Self::F32Neg),
            0x8D => Ok(Self::F32Ceil),
            0x8E => Ok(Self::F32Floor),
            0x8F => Ok(Self::F32Trunc),
            0x90 => Ok(Self::F32Nearest),
            0x91 => Ok(Self::F32Sqrt),
            0x92 => Ok(Self::F32Add),
            0x93 => Ok(Self::F32Sub),
            0x94 => Ok(Self::F32Mul),
            0x95 => Ok(Self::F32Div),
            0x96 => Ok(Self::F32Min),
            0x97 => Ok(Self::F32Max),
            0x98 => Ok(Self::F32Copysign),
            0x99 => Ok(Self::F64Abs),
            0x9A => Ok(Self::F64Neg),
            0x9B => Ok(Self::F64Ceil),
            0x9C => Ok(Self::F64Floor),
            0x9D => Ok(Self::F64Trunc),
            0x9E => Ok(Self::F64Nearest),
            0x9F => Ok(Self::F64Sqrt),
            0xA0 => Ok(Self::F64Add),
            0xA1 => Ok(Self::F64Sub),
            0xA2 => Ok(Self::F64Mul),
            0xA3 => Ok(Self::F64Div),
            0xA4 => Ok(Self::F64Min),
            0xA5 => Ok(Self::F64Max),
            0xA6 => Ok(Self::F64Copysign),
            0xA7 => Ok(Self::I32WrapI64),
            0xA8 => Ok(Self::I32TruncF32S),
            0xA9 => Ok(Self::I32TruncF32U),
            0xAA => Ok(Self::I32TruncF64S),
            0xAB => Ok(Self::I32TruncF64U),
            0xAC => Ok(Self::I64ExtendI32S),
            0xAD => Ok(Self::I64ExtendI32U),
            0xAE => Ok(Self::I64TruncF32S),
            0xAF => Ok(Self::I64TruncF32U),
            0xB0 => Ok(Self::I64TruncF64S),
            0xB1 => Ok(Self::I64TruncF64U),
            0xB2 => Ok(Self::F32ConvertI32S),
            0xB3 => Ok(Self::F32ConvertI32U),
            0xB4 => Ok(Self::F32ConvertI64S),
            0xB5 => Ok(Self::F32ConvertI64U),
            0xB6 => Ok(Self::F32DemoteF64),
            0xB7 => Ok(Self::F64ConvertI32S),
            0xB8 => Ok(Self::F64ConvertI32U),
            0xB9 => Ok(Self::F64ConvertI64S),
            0xBA => Ok(Self::F64ConvertI64U),
            0xBB => Ok(Self::F64PromoteF32),
            0xBC => Ok(Self::I32ReinterpretF32),
            0xBD => Ok(Self::I64ReinterpretF64),
            0xBE => Ok(Self::F32ReinterpretI32),
            0xBF => Ok(Self::F64ReinterpretI64),
            0xC0 => Ok(Self::I32Extend8S),
            0xC1 => Ok(Self::I32Extend16S),
            0xC2 => Ok(Self::I64Extend8S),
            0xC3 => Ok(Self::I64Extend16S),
            0xC4 => Ok(Self::I64Extend32S),
            _ => Err(()),
        }
    }
}
