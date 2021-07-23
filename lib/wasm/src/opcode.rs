// Wasm Bytecode Table (AUTO GENERATED)
use core::convert::TryFrom;

#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum WasmOpcode {
    /// `00 unreachable` (mvp)
    Unreachable = 0x00,
    /// `01 nop` (mvp)
    Nop = 0x01,
    /// `02 block block_type; expr; end` (mvp)
    Block = 0x02,
    /// `03 loop block_type; expr; end` (mvp)
    Loop = 0x03,
    /// `04 if block_type; expr; else; expr; end` (mvp)
    If = 0x04,
    /// `05 else expr; end` (mvp)
    Else = 0x05,
    /// `0B end` (mvp)
    End = 0x0B,
    /// `0C br labelidx` (mvp)
    Br = 0x0C,
    /// `0D br_if labelidx` (mvp)
    BrIf = 0x0D,
    /// `0E br_table vec(labelidx) labelidx` (mvp)
    BrTable = 0x0E,
    /// `0F return` (mvp)
    Return = 0x0F,
    /// `10 call funcidx` (mvp)
    Call = 0x10,
    /// `11 call_indirect typeidx 0x00` (mvp)
    CallIndirect = 0x11,
    /// `12 return_call funcidx` (tail_call)
    ReturnCall = 0x12,
    /// `13 return_call_indirect typeidx 0x00` (tail_call)
    ReturnCallIndirect = 0x13,
    /// `1A drop` (mvp)
    Drop = 0x1A,
    /// `1B select` (mvp)
    Select = 0x1B,
    /// `20 local.get localidx` (mvp)
    LocalGet = 0x20,
    /// `21 local.set localidx` (mvp)
    LocalSet = 0x21,
    /// `22 local.tee localidx` (mvp)
    LocalTee = 0x22,
    /// `23 global.get globalidx` (mvp)
    GlobalGet = 0x23,
    /// `24 global.set globalidx` (mvp)
    GlobalSet = 0x24,
    /// `28 i32.load align offset` (mvp)
    I32Load = 0x28,
    /// `29 i64.load align offset` (mvp_i64)
    I64Load = 0x29,
    /// `2A f32.load align offset` (mvp_f32)
    F32Load = 0x2A,
    /// `2B f64.load align offset` (mvp_f64)
    F64Load = 0x2B,
    /// `2C i32.load8_s align offset` (mvp)
    I32Load8S = 0x2C,
    /// `2D i32.load8_u align offset` (mvp)
    I32Load8U = 0x2D,
    /// `2E i32.load16_s align offset` (mvp)
    I32Load16S = 0x2E,
    /// `2F i32.load16_u align offset` (mvp)
    I32Load16U = 0x2F,
    /// `30 i64.load8_s align offset` (mvp_i64)
    I64Load8S = 0x30,
    /// `31 i64.load8_u align offset` (mvp_i64)
    I64Load8U = 0x31,
    /// `32 i64.load16_s align offset` (mvp_i64)
    I64Load16S = 0x32,
    /// `33 i64.load16_u align offset` (mvp_i64)
    I64Load16U = 0x33,
    /// `34 i64.load32_s align offset` (mvp_i64)
    I64Load32S = 0x34,
    /// `35 i64.load32_u align offset` (mvp_i64)
    I64Load32U = 0x35,
    /// `36 i32.store align offset` (mvp)
    I32Store = 0x36,
    /// `37 i64.store align offset` (mvp_i64)
    I64Store = 0x37,
    /// `38 f32.store align offset` (mvp_f32)
    F32Store = 0x38,
    /// `39 f64.store align offset` (mvp_f64)
    F64Store = 0x39,
    /// `3A i32.store8 align offset` (mvp)
    I32Store8 = 0x3A,
    /// `3B i32.store16 align offset` (mvp)
    I32Store16 = 0x3B,
    /// `3C i64.store8 align offset` (mvp_i64)
    I64Store8 = 0x3C,
    /// `3D i64.store16 align offset` (mvp_i64)
    I64Store16 = 0x3D,
    /// `3E i64.store32 align offset` (mvp_i64)
    I64Store32 = 0x3E,
    /// `3F memory.size 0x00` (mvp)
    MemorySize = 0x3F,
    /// `40 memory.grow 0x00` (mvp)
    MemoryGrow = 0x40,
    /// `41 i32.const n` (mvp)
    I32Const = 0x41,
    /// `42 i64.const n` (mvp_i64)
    I64Const = 0x42,
    /// `43 f32.const z` (mvp_f32)
    F32Const = 0x43,
    /// `44 f64.const z` (mvp_f64)
    F64Const = 0x44,
    /// `45 i32.eqz` (mvp)
    I32Eqz = 0x45,
    /// `46 i32.eq` (mvp)
    I32Eq = 0x46,
    /// `47 i32.ne` (mvp)
    I32Ne = 0x47,
    /// `48 i32.lt_s` (mvp)
    I32LtS = 0x48,
    /// `49 i32.lt_u` (mvp)
    I32LtU = 0x49,
    /// `4A i32.gt_s` (mvp)
    I32GtS = 0x4A,
    /// `4B i32.gt_u` (mvp)
    I32GtU = 0x4B,
    /// `4C i32.le_s` (mvp)
    I32LeS = 0x4C,
    /// `4D i32.le_u` (mvp)
    I32LeU = 0x4D,
    /// `4E i32.ge_s` (mvp)
    I32GeS = 0x4E,
    /// `4F i32.ge_u` (mvp)
    I32GeU = 0x4F,
    /// `50 i64.eqz` (mvp_i64)
    I64Eqz = 0x50,
    /// `51 i64.eq` (mvp_i64)
    I64Eq = 0x51,
    /// `52 i64.ne` (mvp_i64)
    I64Ne = 0x52,
    /// `53 i64.lt_s` (mvp_i64)
    I64LtS = 0x53,
    /// `54 i64.lt_u` (mvp_i64)
    I64LtU = 0x54,
    /// `55 i64.gt_s` (mvp_i64)
    I64GtS = 0x55,
    /// `56 i64.gt_u` (mvp_i64)
    I64GtU = 0x56,
    /// `57 i64.le_s` (mvp_i64)
    I64LeS = 0x57,
    /// `58 i64.le_u` (mvp_i64)
    I64LeU = 0x58,
    /// `59 i64.ge_s` (mvp_i64)
    I64GeS = 0x59,
    /// `5A i64.ge_u` (mvp_i64)
    I64GeU = 0x5A,
    /// `5B f32.eq` (mvp_f32)
    F32Eq = 0x5B,
    /// `5C f32.ne` (mvp_f32)
    F32Ne = 0x5C,
    /// `5D f32.lt` (mvp_f32)
    F32Lt = 0x5D,
    /// `5E f32.gt` (mvp_f32)
    F32Gt = 0x5E,
    /// `5F f32.le` (mvp_f32)
    F32Le = 0x5F,
    /// `60 f32.ge` (mvp_f32)
    F32Ge = 0x60,
    /// `61 f64.eq` (mvp_f64)
    F64Eq = 0x61,
    /// `62 f64.ne` (mvp_f64)
    F64Ne = 0x62,
    /// `63 f64.lt` (mvp_f64)
    F64Lt = 0x63,
    /// `64 f64.gt` (mvp_f64)
    F64Gt = 0x64,
    /// `65 f64.le` (mvp_f64)
    F64Le = 0x65,
    /// `66 f64.ge` (mvp_f64)
    F64Ge = 0x66,
    /// `67 i32.clz` (mvp)
    I32Clz = 0x67,
    /// `68 i32.ctz` (mvp)
    I32Ctz = 0x68,
    /// `69 i32.popcnt` (mvp)
    I32Popcnt = 0x69,
    /// `6A i32.add` (mvp)
    I32Add = 0x6A,
    /// `6B i32.sub` (mvp)
    I32Sub = 0x6B,
    /// `6C i32.mul` (mvp)
    I32Mul = 0x6C,
    /// `6D i32.div_s` (mvp)
    I32DivS = 0x6D,
    /// `6E i32.div_u` (mvp)
    I32DivU = 0x6E,
    /// `6F i32.rem_s` (mvp)
    I32RemS = 0x6F,
    /// `70 i32.rem_u` (mvp)
    I32RemU = 0x70,
    /// `71 i32.and` (mvp)
    I32And = 0x71,
    /// `72 i32.or` (mvp)
    I32Or = 0x72,
    /// `73 i32.xor` (mvp)
    I32Xor = 0x73,
    /// `74 i32.shl` (mvp)
    I32Shl = 0x74,
    /// `75 i32.shr_s` (mvp)
    I32ShrS = 0x75,
    /// `76 i32.shr_u` (mvp)
    I32ShrU = 0x76,
    /// `77 i32.rotl` (mvp)
    I32Rotl = 0x77,
    /// `78 i32.rotr` (mvp)
    I32Rotr = 0x78,
    /// `79 i64.clz` (mvp_i64)
    I64Clz = 0x79,
    /// `7A i64.ctz` (mvp_i64)
    I64Ctz = 0x7A,
    /// `7B i64.popcnt` (mvp_i64)
    I64Popcnt = 0x7B,
    /// `7C i64.add` (mvp_i64)
    I64Add = 0x7C,
    /// `7D i64.sub` (mvp_i64)
    I64Sub = 0x7D,
    /// `7E i64.mul` (mvp_i64)
    I64Mul = 0x7E,
    /// `7F i64.div_s` (mvp_i64)
    I64DivS = 0x7F,
    /// `80 i64.div_u` (mvp_i64)
    I64DivU = 0x80,
    /// `81 i64.rem_s` (mvp_i64)
    I64RemS = 0x81,
    /// `82 i64.rem_u` (mvp_i64)
    I64RemU = 0x82,
    /// `83 i64.and` (mvp_i64)
    I64And = 0x83,
    /// `84 i64.or` (mvp_i64)
    I64Or = 0x84,
    /// `85 i64.xor` (mvp_i64)
    I64Xor = 0x85,
    /// `86 i64.shl` (mvp_i64)
    I64Shl = 0x86,
    /// `87 i64.shr_s` (mvp_i64)
    I64ShrS = 0x87,
    /// `88 i64.shr_u` (mvp_i64)
    I64ShrU = 0x88,
    /// `89 i64.rotl` (mvp_i64)
    I64Rotl = 0x89,
    /// `8A i64.rotr` (mvp_i64)
    I64Rotr = 0x8A,
    /// `8B f32.abs` (mvp_f32)
    F32Abs = 0x8B,
    /// `8C f32.neg` (mvp_f32)
    F32Neg = 0x8C,
    /// `8D f32.ceil` (mvp_f32)
    F32Ceil = 0x8D,
    /// `8E f32.floor` (mvp_f32)
    F32Floor = 0x8E,
    /// `8F f32.trunc` (mvp_f32)
    F32Trunc = 0x8F,
    /// `90 f32.nearest` (mvp_f32)
    F32Nearest = 0x90,
    /// `91 f32.sqrt` (mvp_f32)
    F32Sqrt = 0x91,
    /// `92 f32.add` (mvp_f32)
    F32Add = 0x92,
    /// `93 f32.sub` (mvp_f32)
    F32Sub = 0x93,
    /// `94 f32.mul` (mvp_f32)
    F32Mul = 0x94,
    /// `95 f32.div` (mvp_f32)
    F32Div = 0x95,
    /// `96 f32.min` (mvp_f32)
    F32Min = 0x96,
    /// `97 f32.max` (mvp_f32)
    F32Max = 0x97,
    /// `98 f32.copysign` (mvp_f32)
    F32Copysign = 0x98,
    /// `99 f64.abs` (mvp_f64)
    F64Abs = 0x99,
    /// `9A f64.neg` (mvp_f64)
    F64Neg = 0x9A,
    /// `9B f64.ceil` (mvp_f64)
    F64Ceil = 0x9B,
    /// `9C f64.floor` (mvp_f64)
    F64Floor = 0x9C,
    /// `9D f64.trunc` (mvp_f64)
    F64Trunc = 0x9D,
    /// `9E f64.nearest` (mvp_f64)
    F64Nearest = 0x9E,
    /// `9F f64.sqrt` (mvp_f64)
    F64Sqrt = 0x9F,
    /// `A0 f64.add` (mvp_f64)
    F64Add = 0xA0,
    /// `A1 f64.sub` (mvp_f64)
    F64Sub = 0xA1,
    /// `A2 f64.mul` (mvp_f64)
    F64Mul = 0xA2,
    /// `A3 f64.div` (mvp_f64)
    F64Div = 0xA3,
    /// `A4 f64.min` (mvp_f64)
    F64Min = 0xA4,
    /// `A5 f64.max` (mvp_f64)
    F64Max = 0xA5,
    /// `A6 f64.copysign` (mvp_f64)
    F64Copysign = 0xA6,
    /// `A7 i32.wrap_i64` (mvp_i64)
    I32WrapI64 = 0xA7,
    /// `A8 i32.trunc_f32_s` (mvp_f32)
    I32TruncF32S = 0xA8,
    /// `A9 i32.trunc_f32_u` (mvp_f32)
    I32TruncF32U = 0xA9,
    /// `AA i32.trunc_f64_s` (mvp_f32)
    I32TruncF64S = 0xAA,
    /// `AB i32.trunc_f64_u` (mvp_f32)
    I32TruncF64U = 0xAB,
    /// `AC i64.extend_i32_s` (mvp_i64)
    I64ExtendI32S = 0xAC,
    /// `AD i64.extend_i32_u` (mvp_i64)
    I64ExtendI32U = 0xAD,
    /// `AE i64.trunc_f32_s` (mvp_f32)
    I64TruncF32S = 0xAE,
    /// `AF i64.trunc_f32_u` (mvp_f32)
    I64TruncF32U = 0xAF,
    /// `B0 i64.trunc_f64_s` (mvp_f64)
    I64TruncF64S = 0xB0,
    /// `B1 i64.trunc_f64_u` (mvp_f64)
    I64TruncF64U = 0xB1,
    /// `B2 f32.convert_i32_s` (mvp_f32)
    F32ConvertI32S = 0xB2,
    /// `B3 f32.convert_i32_u` (mvp_f32)
    F32ConvertI32U = 0xB3,
    /// `B4 f32.convert_i64_s` (mvp_f32)
    F32ConvertI64S = 0xB4,
    /// `B5 f32.convert_i64_u` (mvp_f32)
    F32ConvertI64U = 0xB5,
    /// `B6 f32.demote_f64` (mvp_f64)
    F32DemoteF64 = 0xB6,
    /// `B7 f64.convert_i32_s` (mvp_f64)
    F64ConvertI32S = 0xB7,
    /// `B8 f64.convert_i32_u` (mvp_f64)
    F64ConvertI32U = 0xB8,
    /// `B9 f64.convert_i64_s` (mvp_f64)
    F64ConvertI64S = 0xB9,
    /// `BA f64.convert_i64_u` (mvp_f64)
    F64ConvertI64U = 0xBA,
    /// `BB f64.promote_f32` (mvp_f64)
    F64PromoteF32 = 0xBB,
    /// `BC i32.reinterpret_f32` (mvp_f32)
    I32ReinterpretF32 = 0xBC,
    /// `BD i64.reinterpret_f64` (mvp_f64)
    I64ReinterpretF64 = 0xBD,
    /// `BE f32.reinterpret_i32` (mvp_f32)
    F32ReinterpretI32 = 0xBE,
    /// `BF f64.reinterpret_i64` (mvp_f64)
    F64ReinterpretI64 = 0xBF,
    /// `C0 i32.extend8_s` (sign_extend)
    I32Extend8S = 0xC0,
    /// `C1 i32.extend16_s` (sign_extend)
    I32Extend16S = 0xC1,
    /// `C2 i64.extend8_s` (sign_extend)
    I64Extend8S = 0xC2,
    /// `C3 i64.extend16_s` (sign_extend)
    I64Extend16S = 0xC3,
    /// `C4 i64.extend32_s` (sign_extend)
    I64Extend32S = 0xC4,
}

#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
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
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum WasmProposalType {
    Mvp,
    TailCall,
    MvpI64,
    MvpF32,
    MvpF64,
    SignExtend,
}

impl WasmOpcode {
    pub const fn new(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(Self::Unreachable),
            0x01 => Some(Self::Nop),
            0x02 => Some(Self::Block),
            0x03 => Some(Self::Loop),
            0x04 => Some(Self::If),
            0x05 => Some(Self::Else),
            0x0B => Some(Self::End),
            0x0C => Some(Self::Br),
            0x0D => Some(Self::BrIf),
            0x0E => Some(Self::BrTable),
            0x0F => Some(Self::Return),
            0x10 => Some(Self::Call),
            0x11 => Some(Self::CallIndirect),
            0x12 => Some(Self::ReturnCall),
            0x13 => Some(Self::ReturnCallIndirect),
            0x1A => Some(Self::Drop),
            0x1B => Some(Self::Select),
            0x20 => Some(Self::LocalGet),
            0x21 => Some(Self::LocalSet),
            0x22 => Some(Self::LocalTee),
            0x23 => Some(Self::GlobalGet),
            0x24 => Some(Self::GlobalSet),
            0x28 => Some(Self::I32Load),
            0x29 => Some(Self::I64Load),
            0x2A => Some(Self::F32Load),
            0x2B => Some(Self::F64Load),
            0x2C => Some(Self::I32Load8S),
            0x2D => Some(Self::I32Load8U),
            0x2E => Some(Self::I32Load16S),
            0x2F => Some(Self::I32Load16U),
            0x30 => Some(Self::I64Load8S),
            0x31 => Some(Self::I64Load8U),
            0x32 => Some(Self::I64Load16S),
            0x33 => Some(Self::I64Load16U),
            0x34 => Some(Self::I64Load32S),
            0x35 => Some(Self::I64Load32U),
            0x36 => Some(Self::I32Store),
            0x37 => Some(Self::I64Store),
            0x38 => Some(Self::F32Store),
            0x39 => Some(Self::F64Store),
            0x3A => Some(Self::I32Store8),
            0x3B => Some(Self::I32Store16),
            0x3C => Some(Self::I64Store8),
            0x3D => Some(Self::I64Store16),
            0x3E => Some(Self::I64Store32),
            0x3F => Some(Self::MemorySize),
            0x40 => Some(Self::MemoryGrow),
            0x41 => Some(Self::I32Const),
            0x42 => Some(Self::I64Const),
            0x43 => Some(Self::F32Const),
            0x44 => Some(Self::F64Const),
            0x45 => Some(Self::I32Eqz),
            0x46 => Some(Self::I32Eq),
            0x47 => Some(Self::I32Ne),
            0x48 => Some(Self::I32LtS),
            0x49 => Some(Self::I32LtU),
            0x4A => Some(Self::I32GtS),
            0x4B => Some(Self::I32GtU),
            0x4C => Some(Self::I32LeS),
            0x4D => Some(Self::I32LeU),
            0x4E => Some(Self::I32GeS),
            0x4F => Some(Self::I32GeU),
            0x50 => Some(Self::I64Eqz),
            0x51 => Some(Self::I64Eq),
            0x52 => Some(Self::I64Ne),
            0x53 => Some(Self::I64LtS),
            0x54 => Some(Self::I64LtU),
            0x55 => Some(Self::I64GtS),
            0x56 => Some(Self::I64GtU),
            0x57 => Some(Self::I64LeS),
            0x58 => Some(Self::I64LeU),
            0x59 => Some(Self::I64GeS),
            0x5A => Some(Self::I64GeU),
            0x5B => Some(Self::F32Eq),
            0x5C => Some(Self::F32Ne),
            0x5D => Some(Self::F32Lt),
            0x5E => Some(Self::F32Gt),
            0x5F => Some(Self::F32Le),
            0x60 => Some(Self::F32Ge),
            0x61 => Some(Self::F64Eq),
            0x62 => Some(Self::F64Ne),
            0x63 => Some(Self::F64Lt),
            0x64 => Some(Self::F64Gt),
            0x65 => Some(Self::F64Le),
            0x66 => Some(Self::F64Ge),
            0x67 => Some(Self::I32Clz),
            0x68 => Some(Self::I32Ctz),
            0x69 => Some(Self::I32Popcnt),
            0x6A => Some(Self::I32Add),
            0x6B => Some(Self::I32Sub),
            0x6C => Some(Self::I32Mul),
            0x6D => Some(Self::I32DivS),
            0x6E => Some(Self::I32DivU),
            0x6F => Some(Self::I32RemS),
            0x70 => Some(Self::I32RemU),
            0x71 => Some(Self::I32And),
            0x72 => Some(Self::I32Or),
            0x73 => Some(Self::I32Xor),
            0x74 => Some(Self::I32Shl),
            0x75 => Some(Self::I32ShrS),
            0x76 => Some(Self::I32ShrU),
            0x77 => Some(Self::I32Rotl),
            0x78 => Some(Self::I32Rotr),
            0x79 => Some(Self::I64Clz),
            0x7A => Some(Self::I64Ctz),
            0x7B => Some(Self::I64Popcnt),
            0x7C => Some(Self::I64Add),
            0x7D => Some(Self::I64Sub),
            0x7E => Some(Self::I64Mul),
            0x7F => Some(Self::I64DivS),
            0x80 => Some(Self::I64DivU),
            0x81 => Some(Self::I64RemS),
            0x82 => Some(Self::I64RemU),
            0x83 => Some(Self::I64And),
            0x84 => Some(Self::I64Or),
            0x85 => Some(Self::I64Xor),
            0x86 => Some(Self::I64Shl),
            0x87 => Some(Self::I64ShrS),
            0x88 => Some(Self::I64ShrU),
            0x89 => Some(Self::I64Rotl),
            0x8A => Some(Self::I64Rotr),
            0x8B => Some(Self::F32Abs),
            0x8C => Some(Self::F32Neg),
            0x8D => Some(Self::F32Ceil),
            0x8E => Some(Self::F32Floor),
            0x8F => Some(Self::F32Trunc),
            0x90 => Some(Self::F32Nearest),
            0x91 => Some(Self::F32Sqrt),
            0x92 => Some(Self::F32Add),
            0x93 => Some(Self::F32Sub),
            0x94 => Some(Self::F32Mul),
            0x95 => Some(Self::F32Div),
            0x96 => Some(Self::F32Min),
            0x97 => Some(Self::F32Max),
            0x98 => Some(Self::F32Copysign),
            0x99 => Some(Self::F64Abs),
            0x9A => Some(Self::F64Neg),
            0x9B => Some(Self::F64Ceil),
            0x9C => Some(Self::F64Floor),
            0x9D => Some(Self::F64Trunc),
            0x9E => Some(Self::F64Nearest),
            0x9F => Some(Self::F64Sqrt),
            0xA0 => Some(Self::F64Add),
            0xA1 => Some(Self::F64Sub),
            0xA2 => Some(Self::F64Mul),
            0xA3 => Some(Self::F64Div),
            0xA4 => Some(Self::F64Min),
            0xA5 => Some(Self::F64Max),
            0xA6 => Some(Self::F64Copysign),
            0xA7 => Some(Self::I32WrapI64),
            0xA8 => Some(Self::I32TruncF32S),
            0xA9 => Some(Self::I32TruncF32U),
            0xAA => Some(Self::I32TruncF64S),
            0xAB => Some(Self::I32TruncF64U),
            0xAC => Some(Self::I64ExtendI32S),
            0xAD => Some(Self::I64ExtendI32U),
            0xAE => Some(Self::I64TruncF32S),
            0xAF => Some(Self::I64TruncF32U),
            0xB0 => Some(Self::I64TruncF64S),
            0xB1 => Some(Self::I64TruncF64U),
            0xB2 => Some(Self::F32ConvertI32S),
            0xB3 => Some(Self::F32ConvertI32U),
            0xB4 => Some(Self::F32ConvertI64S),
            0xB5 => Some(Self::F32ConvertI64U),
            0xB6 => Some(Self::F32DemoteF64),
            0xB7 => Some(Self::F64ConvertI32S),
            0xB8 => Some(Self::F64ConvertI32U),
            0xB9 => Some(Self::F64ConvertI64S),
            0xBA => Some(Self::F64ConvertI64U),
            0xBB => Some(Self::F64PromoteF32),
            0xBC => Some(Self::I32ReinterpretF32),
            0xBD => Some(Self::I64ReinterpretF64),
            0xBE => Some(Self::F32ReinterpretI32),
            0xBF => Some(Self::F64ReinterpretI64),
            0xC0 => Some(Self::I32Extend8S),
            0xC1 => Some(Self::I32Extend16S),
            0xC2 => Some(Self::I64Extend8S),
            0xC3 => Some(Self::I64Extend16S),
            0xC4 => Some(Self::I64Extend32S),
            _ => None,
        }
    }

    pub const fn to_str(&self) -> &str {
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

    pub const fn operand_type(&self) -> WasmOperandType {
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

    pub const fn proposal_type(&self) -> WasmProposalType {
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
    #[inline]
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value).ok_or(())
    }
}
