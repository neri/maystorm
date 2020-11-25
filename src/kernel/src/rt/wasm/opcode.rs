// Wasm Opcode Table (AUTO GENERATED)
use core::convert::TryFrom;

#[derive(Debug, Copy, Clone)]
pub enum WasmMnemonicType {
    Implied,
    Block,
    Loop,
    If,
    Else,
    End,
    Br,
    BrTable,
    Call,
    CallIndirect,
    Local,
    Global,
    Memory,
    Memsize,
    I32,
    I64,
    F32,
    F64,
}

#[derive(Debug, Copy, Clone)]
pub enum WasmOpcode {
    /// 00 unreachable 
    Unreachable = 0x00,
    /// 01 nop 
    Nop = 0x01,
    /// 02 block block_type; expr; end
    Block = 0x02,
    /// 03 loop block_type; expr; end
    Loop = 0x03,
    /// 04 if block_type; expr; else; expr; end
    If = 0x04,
    /// 05 else expr; end
    Else = 0x05,
    /// 0B end 
    End = 0x0B,
    /// 0C br labelidx
    Br = 0x0C,
    /// 0D br_if labelidx
    BrIf = 0x0D,
    /// 0E br_table vec(labelidx) labelidx
    BrTable = 0x0E,
    /// 0F return 
    Return = 0x0F,
    /// 10 call funcidx
    Call = 0x10,
    /// 11 call_indirect typeidx 0x00
    CallIndirect = 0x11,
    /// 12 return_call funcidx
    ReturnCall = 0x12,
    /// 13 return_call_indirect typeidx 0x00
    ReturnCallIndirect = 0x13,
    /// 1A drop 
    Drop = 0x1A,
    /// 1B select 
    Select = 0x1B,
    /// 20 local.get localidx
    LocalGet = 0x20,
    /// 21 local.set localidx
    LocalSet = 0x21,
    /// 22 local.tee localidx
    LocalTee = 0x22,
    /// 23 global.get globalidx
    GlobalGet = 0x23,
    /// 24 global.set globalidx
    GlobalSet = 0x24,
    /// 28 i32.load align offset
    I32Load = 0x28,
    /// 29 i64.load align offset
    I64Load = 0x29,
    /// 2A f32.load align offset
    F32Load = 0x2A,
    /// 2B f64.load align offset
    F64Load = 0x2B,
    /// 2C i32.load8_s align offset
    I32Load8S = 0x2C,
    /// 2D i32.load8_u align offset
    I32Load8U = 0x2D,
    /// 2E i32.load16_s align offset
    I32Load16S = 0x2E,
    /// 2F i32.load16_u align offset
    I32Load16U = 0x2F,
    /// 30 i64.load8_s align offset
    I64Load8S = 0x30,
    /// 31 i64.load8_u align offset
    I64Load8U = 0x31,
    /// 32 i64.load16_s align offset
    I64Load16S = 0x32,
    /// 33 i64.load16_u align offset
    I64Load16U = 0x33,
    /// 34 i64.load32_s align offset
    I64Load32S = 0x34,
    /// 35 i64.load32_u align offset
    I64Load32U = 0x35,
    /// 36 i32.store align offset
    I32Store = 0x36,
    /// 37 i64.store align offset
    I64Store = 0x37,
    /// 38 f32.store align offset
    F32Store = 0x38,
    /// 39 f64.store align offset
    F64Store = 0x39,
    /// 3A i32.store8 align offset
    I32Store8 = 0x3A,
    /// 3B i32.store16 align offset
    I32Store16 = 0x3B,
    /// 3C i64.store8 align offset
    I64Store8 = 0x3C,
    /// 3D i64.store16 align offset
    I64Store16 = 0x3D,
    /// 3E i64.store32 align offset
    I64Store32 = 0x3E,
    /// 3F memory.size 0x00
    MemorySize = 0x3F,
    /// 40 memory.grow 0x00
    MemoryGrow = 0x40,
    /// 41 i32.const n
    I32Const = 0x41,
    /// 42 i64.const n
    I64Const = 0x42,
    /// 43 f32.const z
    F32Const = 0x43,
    /// 44 f64.const z
    F64Const = 0x44,
    /// 45 i32.eqz 
    I32Eqz = 0x45,
    /// 46 i32.eq 
    I32Eq = 0x46,
    /// 47 i32.ne 
    I32Ne = 0x47,
    /// 48 i32.lt_s 
    I32LtS = 0x48,
    /// 49 i32.lt_u 
    I32LtU = 0x49,
    /// 4A i32.gt_s 
    I32GtS = 0x4A,
    /// 4B i32.gt_u 
    I32GtU = 0x4B,
    /// 4C i32.le_s 
    I32LeS = 0x4C,
    /// 4D i32.le_u 
    I32LeU = 0x4D,
    /// 4E i32.ge_s 
    I32GeS = 0x4E,
    /// 4F i32.ge_u 
    I32GeU = 0x4F,
    /// 50 i64.eqz 
    I64Eqz = 0x50,
    /// 51 i64.eq 
    I64Eq = 0x51,
    /// 52 i64.ne 
    I64Ne = 0x52,
    /// 53 i64.lt_s 
    I64LtS = 0x53,
    /// 54 i64.lt_u 
    I64LtU = 0x54,
    /// 55 i64.gt_s 
    I64GtS = 0x55,
    /// 56 i64.gt_u 
    I64GtU = 0x56,
    /// 57 i64.le_s 
    I64LeS = 0x57,
    /// 58 i64.le_u 
    I64LeU = 0x58,
    /// 59 i64.ge_s 
    I64GeS = 0x59,
    /// 5A i64.ge_u 
    I64GeU = 0x5A,
    /// 5B f32.eq 
    F32Eq = 0x5B,
    /// 5C f32.ne 
    F32Ne = 0x5C,
    /// 5D f32.lt 
    F32Lt = 0x5D,
    /// 5E f32.gt 
    F32Gt = 0x5E,
    /// 5F f32.le 
    F32Le = 0x5F,
    /// 60 f32.ge 
    F32Ge = 0x60,
    /// 61 f64.eq 
    F64Eq = 0x61,
    /// 62 f64.ne 
    F64Ne = 0x62,
    /// 63 f64.lt 
    F64Lt = 0x63,
    /// 64 f64.gt 
    F64Gt = 0x64,
    /// 65 f64.le 
    F64Le = 0x65,
    /// 66 f64.ge 
    F64Ge = 0x66,
    /// 67 i32.clz 
    I32Clz = 0x67,
    /// 68 i32.ctz 
    I32Ctz = 0x68,
    /// 69 i32.popcnt 
    I32Popcnt = 0x69,
    /// 6A i32.add 
    I32Add = 0x6A,
    /// 6B i32.sub 
    I32Sub = 0x6B,
    /// 6C i32.mul 
    I32Mul = 0x6C,
    /// 6D i32.div_s 
    I32DivS = 0x6D,
    /// 6E i32.div_u 
    I32DivU = 0x6E,
    /// 6F i32.rem_s 
    I32RemS = 0x6F,
    /// 70 i32.rem_u 
    I32RemU = 0x70,
    /// 71 i32.and 
    I32And = 0x71,
    /// 72 i32.or 
    I32Or = 0x72,
    /// 73 i32.xor 
    I32Xor = 0x73,
    /// 74 i32.shl 
    I32Shl = 0x74,
    /// 75 i32.shr_s 
    I32ShrS = 0x75,
    /// 76 i32.shr_u 
    I32ShrU = 0x76,
    /// 77 i32.rotl 
    I32Rotl = 0x77,
    /// 78 i32.rotr 
    I32Rotr = 0x78,
    /// 79 i64.clz 
    I64Clz = 0x79,
    /// 7A i64.ctz 
    I64Ctz = 0x7A,
    /// 7B i64.popcnt 
    I64Popcnt = 0x7B,
    /// 7C i64.add 
    I64Add = 0x7C,
    /// 7D i64.sub 
    I64Sub = 0x7D,
    /// 7E i64.mul 
    I64Mul = 0x7E,
    /// 7F i64.div_s 
    I64DivS = 0x7F,
    /// 80 i64.div_u 
    I64DivU = 0x80,
    /// 81 i64.rem_s 
    I64RemS = 0x81,
    /// 82 i64.rem_u 
    I64RemU = 0x82,
    /// 83 i64.and 
    I64And = 0x83,
    /// 84 i64.or 
    I64Or = 0x84,
    /// 85 i64.xor 
    I64Xor = 0x85,
    /// 86 i64.shl 
    I64Shl = 0x86,
    /// 87 i64.shr_s 
    I64ShrS = 0x87,
    /// 88 i64.shr_u 
    I64ShrU = 0x88,
    /// 89 i64.rotl 
    I64Rotl = 0x89,
    /// 8A i64.rotr 
    I64Rotr = 0x8A,
    /// 8B f32.abs 
    F32Abs = 0x8B,
    /// 8C f32.neg 
    F32Neg = 0x8C,
    /// 8D f32.ceil 
    F32Ceil = 0x8D,
    /// 8E f32.floor 
    F32Floor = 0x8E,
    /// 8F f32.trunc 
    F32Trunc = 0x8F,
    /// 90 f32.nearest 
    F32Nearest = 0x90,
    /// 91 f32.sqrt 
    F32Sqrt = 0x91,
    /// 92 f32.add 
    F32Add = 0x92,
    /// 93 f32.sub 
    F32Sub = 0x93,
    /// 94 f32.mul 
    F32Mul = 0x94,
    /// 95 f32.div 
    F32Div = 0x95,
    /// 96 f32.min 
    F32Min = 0x96,
    /// 97 f32.max 
    F32Max = 0x97,
    /// 98 f32.copysign 
    F32Copysign = 0x98,
    /// 99 f64.abs 
    F64Abs = 0x99,
    /// 9A f64.neg 
    F64Neg = 0x9A,
    /// 9B f64.ceil 
    F64Ceil = 0x9B,
    /// 9C f64.floor 
    F64Floor = 0x9C,
    /// 9D f64.trunc 
    F64Trunc = 0x9D,
    /// 9E f64.nearest 
    F64Nearest = 0x9E,
    /// 9F f64.sqrt 
    F64Sqrt = 0x9F,
    /// A0 f64.add 
    F64Add = 0xA0,
    /// A1 f64.sub 
    F64Sub = 0xA1,
    /// A2 f64.mul 
    F64Mul = 0xA2,
    /// A3 f64.div 
    F64Div = 0xA3,
    /// A4 f64.min 
    F64Min = 0xA4,
    /// A5 f64.max 
    F64Max = 0xA5,
    /// A6 f64.copysign 
    F64Copysign = 0xA6,
    /// A7 i32.wrap_i64 
    I32WrapI64 = 0xA7,
    /// A8 i32.trunc_f32_s 
    I32TruncF32S = 0xA8,
    /// A9 i32.trunc_f32_u 
    I32TruncF32U = 0xA9,
    /// AA i32.trunc_f64_s 
    I32TruncF64S = 0xAA,
    /// AB i32.trunc_f64_u 
    I32TruncF64U = 0xAB,
    /// AC i64.extend_i32_s 
    I64ExtendI32S = 0xAC,
    /// AD i64.extend_i32_u 
    I64ExtendI32U = 0xAD,
    /// AE i64.trunc_f32_s 
    I64TruncF32S = 0xAE,
    /// AF i64.trunc_f32_u 
    I64TruncF32U = 0xAF,
    /// B0 i64.trunc_f64_s 
    I64TruncF64S = 0xB0,
    /// B1 i64.trunc_f64_u 
    I64TruncF64U = 0xB1,
    /// B2 f32.convert_i32_s 
    F32ConvertI32S = 0xB2,
    /// B3 f32.convert_i32_u 
    F32ConvertI32U = 0xB3,
    /// B4 f32.convert_i64_s 
    F32ConvertI64S = 0xB4,
    /// B5 f32.convert_i64_u 
    F32ConvertI64U = 0xB5,
    /// B6 f32.demote_f64 
    F32DemoteF64 = 0xB6,
    /// B7 f64.convert_i32_s 
    F64ConvertI32S = 0xB7,
    /// B8 f64.convert_i32_u 
    F64ConvertI32U = 0xB8,
    /// B9 f64.convert_i64_s 
    F64ConvertI64S = 0xB9,
    /// BA f64.convert_i64_u 
    F64ConvertI64U = 0xBA,
    /// BB f64.promote_f32 
    F64PromoteF32 = 0xBB,
    /// BC i32.reinterpret_f32 
    I32ReinterpretF32 = 0xBC,
    /// BD i64.reinterpret_f64 
    I64ReinterpretF64 = 0xBD,
    /// BE f32.reinterpret_i32 
    F32ReinterpretI32 = 0xBE,
    /// BF f64.reinterpret_i64 
    F64ReinterpretI64 = 0xBF,
    /// C0 i32.extend8_s 
    I32Extend8S = 0xC0,
    /// C1 i32.extend16_s 
    I32Extend16S = 0xC1,
    /// C2 i64.extend8_s 
    I64Extend8S = 0xC2,
    /// C3 i64.extend16_s 
    I64Extend16S = 0xC3,
    /// C4 i64.extend32_s 
    I64Extend32S = 0xC4,
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

    pub fn mnemonic_type(&self) -> WasmMnemonicType {
        match *self {
            Self::Block => WasmMnemonicType::Block,
            Self::Loop => WasmMnemonicType::Loop,
            Self::If => WasmMnemonicType::If,
            Self::Else => WasmMnemonicType::Else,
            Self::End => WasmMnemonicType::End,
            Self::Br => WasmMnemonicType::Br,
            Self::BrIf => WasmMnemonicType::Br,
            Self::BrTable => WasmMnemonicType::BrTable,
            Self::Call => WasmMnemonicType::Call,
            Self::CallIndirect => WasmMnemonicType::CallIndirect,
            Self::ReturnCall => WasmMnemonicType::Call,
            Self::ReturnCallIndirect => WasmMnemonicType::CallIndirect,
            Self::LocalGet => WasmMnemonicType::Local,
            Self::LocalSet => WasmMnemonicType::Local,
            Self::LocalTee => WasmMnemonicType::Local,
            Self::GlobalGet => WasmMnemonicType::Global,
            Self::GlobalSet => WasmMnemonicType::Global,
            Self::I32Load => WasmMnemonicType::Memory,
            Self::I64Load => WasmMnemonicType::Memory,
            Self::F32Load => WasmMnemonicType::Memory,
            Self::F64Load => WasmMnemonicType::Memory,
            Self::I32Load8S => WasmMnemonicType::Memory,
            Self::I32Load8U => WasmMnemonicType::Memory,
            Self::I32Load16S => WasmMnemonicType::Memory,
            Self::I32Load16U => WasmMnemonicType::Memory,
            Self::I64Load8S => WasmMnemonicType::Memory,
            Self::I64Load8U => WasmMnemonicType::Memory,
            Self::I64Load16S => WasmMnemonicType::Memory,
            Self::I64Load16U => WasmMnemonicType::Memory,
            Self::I64Load32S => WasmMnemonicType::Memory,
            Self::I64Load32U => WasmMnemonicType::Memory,
            Self::I32Store => WasmMnemonicType::Memory,
            Self::I64Store => WasmMnemonicType::Memory,
            Self::F32Store => WasmMnemonicType::Memory,
            Self::F64Store => WasmMnemonicType::Memory,
            Self::I32Store8 => WasmMnemonicType::Memory,
            Self::I32Store16 => WasmMnemonicType::Memory,
            Self::I64Store8 => WasmMnemonicType::Memory,
            Self::I64Store16 => WasmMnemonicType::Memory,
            Self::I64Store32 => WasmMnemonicType::Memory,
            Self::MemorySize => WasmMnemonicType::Memsize,
            Self::MemoryGrow => WasmMnemonicType::Memsize,
            Self::I32Const => WasmMnemonicType::I32,
            Self::I64Const => WasmMnemonicType::I64,
            Self::F32Const => WasmMnemonicType::F32,
            Self::F64Const => WasmMnemonicType::F64,
            _ => WasmMnemonicType::Implied,
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
