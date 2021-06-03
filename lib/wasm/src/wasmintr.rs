//! WebAssembly Intermediate Code Interpreter

use super::{intcode::*, stack::*, wasm::*};
use crate::opcode::WasmOpcode;
use alloc::vec::Vec;
use core::fmt;

type StackType = usize;

/// Wasm Intermediate Code
#[derive(Debug, Clone, Copy)]
pub struct WasmImc {
    pub source: u32,
    pub mnemonic: WasmIntMnemonic,
    pub stack_level: StackType,
    pub param1: u64,
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
            param1: 0,
        }
    }

    #[inline]
    pub const fn new(
        source_position: usize,
        opcode: WasmOpcode,
        mnemonic: WasmIntMnemonic,
        stack_level: usize,
        param1: u64,
    ) -> Self {
        let source = ((source_position as u32) << 8) | (opcode as u32);
        Self {
            source,
            mnemonic,
            stack_level: stack_level as StackType,
            param1,
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
    pub const fn mnemonic(&self) -> WasmIntMnemonic {
        self.mnemonic
    }

    #[inline]
    pub const fn stack_level(&self) -> usize {
        self.stack_level as usize
    }

    #[inline]
    pub const fn param1(&self) -> u64 {
        self.param1
    }

    #[inline]
    pub fn set_param1(&mut self, val: u64) {
        self.param1 = val;
    }
}

impl From<WasmIntMnemonic> for WasmImc {
    #[inline]
    fn from(val: WasmIntMnemonic) -> Self {
        Self::from_mnemonic(val)
    }
}

/// Wasm Intermediate Code Interpreter
pub struct WasmInterpreter<'a> {
    module: &'a WasmModule,
    func_index: usize,
}

impl<'a> WasmInterpreter<'a> {
    #[inline]
    pub fn new(module: &'a WasmModule) -> Self {
        Self {
            module,
            func_index: 0,
        }
    }
}

impl WasmInterpreter<'_> {
    pub fn invoke(
        &mut self,
        func_index: usize,
        code_block: &WasmCodeBlock,
        locals: &[WasmStackValue],
        result_types: &[WasmValType],
    ) -> Result<Option<WasmValue>, WasmRuntimeError> {
        let mut heap = StackHeap::with_capacity(0x10000);

        let mut locals = {
            let output = heap.alloc(locals.len());
            output.copy_from_slice(locals);
            output
        };

        self.func_index = func_index;

        self.interpret(code_block, &mut locals, result_types, &mut heap)
    }

    #[inline]
    fn error(&self, kind: WasmRuntimeErrorType, code: &WasmImc) -> WasmRuntimeError {
        WasmRuntimeError {
            kind,
            function: self.func_index,
            position: code.source_position(),
            opcode: code.opcode().unwrap_or(WasmOpcode::Unreachable),
        }
    }

    fn interpret(
        &mut self,
        code_block: &WasmCodeBlock,
        locals: &mut [WasmStackValue],
        result_types: &[WasmValType],
        heap: &mut StackHeap,
    ) -> Result<Option<WasmValue>, WasmRuntimeError> {
        let mut codes = WasmIntermediateCodeStream::from_codes(code_block.intermediate_codes());

        let value_stack = heap.alloc(code_block.max_value_stack());
        for value in value_stack.iter_mut() {
            *value = WasmStackValue::zero();
        }

        let mut result_stack_level = 0;

        // let mut last_code = WasmImc::from_mnemonic(WasmIntMnemonic::Unreachable);

        let memory = unsafe { self.module.memory_unchecked(0) };

        while let Some(code) = codes.fetch() {
            match code.mnemonic() {
                WasmIntMnemonic::Unreachable => {
                    return Err(self.error(WasmRuntimeErrorType::Unreachable, code))
                }

                // Currently, NOP is unreachable
                WasmIntMnemonic::Nop => unreachable!(),

                WasmIntMnemonic::Br => {
                    let br = code.param1() as usize;
                    codes.set_position(br);
                }

                WasmIntMnemonic::BrIf => {
                    let cc = value_stack[code.stack_level()].get_bool();
                    if cc {
                        let br = code.param1() as usize;
                        codes.set_position(br);
                    }
                }
                WasmIntMnemonic::BrTable => {
                    let mut index = value_stack[code.stack_level()].get_u32() as usize;
                    let ext_params = code_block.ext_params();
                    let table_position = code.param1() as usize;
                    let table_len = ext_params[table_position] - 1;
                    if index >= table_len {
                        index = table_len;
                    }
                    let target = ext_params[table_position + index + 1];
                    codes.set_position(target);
                }

                WasmIntMnemonic::Return => {
                    // last_code = *code;
                    result_stack_level = code.stack_level();
                    break;
                }

                WasmIntMnemonic::Call => {
                    let func = unsafe {
                        self.module
                            .functions()
                            .get_unchecked(code.param1() as usize)
                    };
                    self.call(func, code, value_stack, heap)?;
                }
                WasmIntMnemonic::CallIndirect => {
                    let type_index = code.param1() as usize;
                    let index =
                        unsafe { value_stack.get_unchecked(code.stack_level()).get_i32() as usize };
                    let func = self
                        .module
                        .elem_by_index(index)
                        .ok_or(self.error(WasmRuntimeErrorType::NoMethod, code))?;
                    if func.type_index() != type_index {
                        return Err(self.error(WasmRuntimeErrorType::TypeMismatch, code));
                    }
                    self.call(func, code, value_stack, heap)?;
                }

                WasmIntMnemonic::Select => {
                    let stack_level = code.stack_level();
                    let cc = unsafe { value_stack.get_unchecked(stack_level + 2).get_bool() };
                    if !cc {
                        unsafe {
                            let b = *value_stack.get_unchecked(stack_level + 1);
                            let ref_a = value_stack.get_unchecked_mut(stack_level);
                            *ref_a = b;
                        }
                    }
                }

                WasmIntMnemonic::LocalGet => {
                    let local = unsafe { locals.get_unchecked(code.param1() as usize) };
                    let ref_a = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *ref_a = *local;
                }
                WasmIntMnemonic::LocalSet | WasmIntMnemonic::LocalTee => {
                    let local = unsafe { locals.get_unchecked_mut(code.param1() as usize) };
                    let ref_a = unsafe { value_stack.get_unchecked(code.stack_level()) };
                    *local = *ref_a;
                }

                WasmIntMnemonic::GlobalGet => {
                    let global =
                        unsafe { self.module.globals().get_unchecked(code.param1() as usize) };
                    let ref_a = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *ref_a = WasmValue::from(*global.value()).into();
                }
                WasmIntMnemonic::GlobalSet => {
                    let global =
                        unsafe { self.module.globals().get_unchecked(code.param1() as usize) };
                    let ref_a = unsafe { value_stack.get_unchecked(code.stack_level()) };
                    global.set(|v| *v = ref_a.get_by_type(global.val_type()));
                }

                WasmIntMnemonic::I32Load => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = match memory.read_u32(offset).map(|v| WasmStackValue::from(v)) {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I32Load8S => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = match memory
                        .read_u8(offset)
                        .map(|v| WasmStackValue::from(v as i8 as i32))
                    {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I32Load8U => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = match memory
                        .read_u8(offset)
                        .map(|v| WasmStackValue::from(v as u32))
                    {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I32Load16S => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = match memory
                        .read_u16(offset)
                        .map(|v| WasmStackValue::from(v as i16 as i32))
                    {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I32Load16U => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = match memory
                        .read_u16(offset)
                        .map(|v| WasmStackValue::from(v as u32))
                    {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }

                WasmIntMnemonic::I64Load => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = match memory.read_u64(offset).map(|v| WasmStackValue::from(v)) {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I64Load8S => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = match memory
                        .read_u8(offset)
                        .map(|v| WasmStackValue::from(v as i8 as i64))
                    {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I64Load8U => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = match memory
                        .read_u8(offset)
                        .map(|v| WasmStackValue::from(v as u64))
                    {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I64Load16S => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = match memory
                        .read_u16(offset)
                        .map(|v| WasmStackValue::from(v as i16 as i64))
                    {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I64Load16U => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = match memory
                        .read_u16(offset)
                        .map(|v| WasmStackValue::from(v as u64))
                    {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I64Load32S => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = match memory
                        .read_u32(offset)
                        .map(|v| WasmStackValue::from(v as i32 as i64))
                    {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I64Load32U => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = match memory
                        .read_u32(offset)
                        .map(|v| WasmStackValue::from(v as u64))
                    {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }

                WasmIntMnemonic::I64Store32 | WasmIntMnemonic::I32Store => {
                    let stack_level = code.stack_level();
                    let index =
                        unsafe { value_stack.get_unchecked(stack_level).get_u32() as usize };
                    let data = unsafe { value_stack.get_unchecked(stack_level + 1).get_u32() };
                    let offset = code.param1() as usize + index;
                    match memory.write_u32(offset, data) {
                        Ok(_) => {}
                        Err(e) => return Err(self.error(e, code)),
                    }
                }
                WasmIntMnemonic::I64Store8 | WasmIntMnemonic::I32Store8 => {
                    let stack_level = code.stack_level();
                    let index =
                        unsafe { value_stack.get_unchecked(stack_level).get_u32() as usize };
                    let data = unsafe { value_stack.get_unchecked(stack_level + 1).get_u8() };
                    let offset = code.param1() as usize + index;
                    match memory.write_u8(offset, data) {
                        Ok(_) => {}
                        Err(e) => return Err(self.error(e, code)),
                    }
                }
                WasmIntMnemonic::I64Store16 | WasmIntMnemonic::I32Store16 => {
                    let stack_level = code.stack_level();
                    let index =
                        unsafe { value_stack.get_unchecked(stack_level).get_u32() as usize };
                    let data = unsafe { value_stack.get_unchecked(stack_level + 1).get_u16() };
                    let offset = code.param1() as usize + index;
                    match memory.write_u16(offset, data) {
                        Ok(_) => {}
                        Err(e) => return Err(self.error(e, code)),
                    }
                }
                WasmIntMnemonic::I64Store => {
                    let stack_level = code.stack_level();
                    let index =
                        unsafe { value_stack.get_unchecked(stack_level).get_u32() as usize };
                    let data = unsafe { value_stack.get_unchecked(stack_level + 1).get_u64() };
                    let offset = code.param1() as usize + index;
                    match memory.write_u64(offset, data) {
                        Ok(_) => {}
                        Err(e) => return Err(self.error(e, code)),
                    }
                }

                WasmIntMnemonic::MemorySize => {
                    let ref_a = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *ref_a = WasmStackValue::from(memory.size() as u32);
                }
                WasmIntMnemonic::MemoryGrow => {
                    let ref_a = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *ref_a = WasmStackValue::from(memory.grow(ref_a.get_u32() as usize) as u32);
                }

                WasmIntMnemonic::I32Const => {
                    let ref_a = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *ref_a = WasmStackValue::from_u32(code.param1() as u32);
                }
                WasmIntMnemonic::I64Const => {
                    let ref_a = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *ref_a = WasmStackValue::from_u64(code.param1());
                }

                WasmIntMnemonic::I32Eqz => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = WasmStackValue::from_bool(var.get_i32() == 0);
                }
                WasmIntMnemonic::I32Eq => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmStackValue::from(lhs.get_u32() == rhs.get_u32());
                }
                WasmIntMnemonic::I32Ne => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmStackValue::from(lhs.get_u32() != rhs.get_u32());
                }
                WasmIntMnemonic::I32LtS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmStackValue::from(lhs.get_i32() < rhs.get_i32());
                }
                WasmIntMnemonic::I32LtU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmStackValue::from(lhs.get_u32() < rhs.get_u32());
                }
                WasmIntMnemonic::I32GtS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmStackValue::from(lhs.get_i32() > rhs.get_i32());
                }
                WasmIntMnemonic::I32GtU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmStackValue::from(lhs.get_u32() > rhs.get_u32());
                }
                WasmIntMnemonic::I32LeS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmStackValue::from(lhs.get_i32() <= rhs.get_i32());
                }
                WasmIntMnemonic::I32LeU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmStackValue::from(lhs.get_u32() <= rhs.get_u32());
                }
                WasmIntMnemonic::I32GeS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmStackValue::from(lhs.get_i32() >= rhs.get_i32());
                }
                WasmIntMnemonic::I32GeU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmStackValue::from(lhs.get_u32() >= rhs.get_u32());
                }

                WasmIntMnemonic::I32Clz => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    var.map_u32(|v| v.leading_zeros());
                }
                WasmIntMnemonic::I32Ctz => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    var.map_u32(|v| v.trailing_zeros());
                }
                WasmIntMnemonic::I32Popcnt => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    var.map_u32(|v| v.count_ones());
                }
                WasmIntMnemonic::I32Add => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_i32(|lhs| lhs.wrapping_add(rhs.get_i32()));
                }
                WasmIntMnemonic::I32Sub => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_i32(|lhs| lhs.wrapping_sub(rhs.get_i32()));
                }
                WasmIntMnemonic::I32Mul => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_i32(|lhs| lhs.wrapping_mul(rhs.get_i32()));
                }

                WasmIntMnemonic::I32DivS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { value_stack.get_unchecked(stack_level + 1).get_i32() };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    if rhs == 0 {
                        return Err(self.error(WasmRuntimeErrorType::DivideByZero, code));
                    }
                    lhs.map_i32(|lhs| lhs.wrapping_div(rhs));
                }
                WasmIntMnemonic::I32DivU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { value_stack.get_unchecked(stack_level + 1).get_u32() };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    if rhs == 0 {
                        return Err(self.error(WasmRuntimeErrorType::DivideByZero, code));
                    }
                    lhs.map_u32(|lhs| lhs.wrapping_div(rhs));
                }
                WasmIntMnemonic::I32RemS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { value_stack.get_unchecked(stack_level + 1).get_i32() };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    if rhs == 0 {
                        return Err(self.error(WasmRuntimeErrorType::DivideByZero, code));
                    }
                    lhs.map_i32(|lhs| lhs.wrapping_rem(rhs));
                }
                WasmIntMnemonic::I32RemU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { value_stack.get_unchecked(stack_level + 1).get_u32() };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    if rhs == 0 {
                        return Err(self.error(WasmRuntimeErrorType::DivideByZero, code));
                    }
                    lhs.map_u32(|lhs| lhs.wrapping_rem(rhs));
                }

                WasmIntMnemonic::I32And => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_u32(|lhs| lhs & rhs.get_u32());
                }
                WasmIntMnemonic::I32Or => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_u32(|lhs| lhs | rhs.get_u32());
                }
                WasmIntMnemonic::I32Xor => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_u32(|lhs| lhs ^ rhs.get_u32());
                }
                WasmIntMnemonic::I32Shl => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_u32(|lhs| lhs << rhs.get_u32());
                }
                WasmIntMnemonic::I32ShrS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_i32(|lhs| lhs >> rhs.get_i32());
                }
                WasmIntMnemonic::I32ShrU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_u32(|lhs| lhs >> rhs.get_u32());
                }
                WasmIntMnemonic::I32Rotl => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_u32(|lhs| lhs.rotate_left(rhs.get_u32()));
                }
                WasmIntMnemonic::I32Rotr => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_u32(|lhs| lhs.rotate_right(rhs.get_u32()));
                }

                WasmIntMnemonic::I64Eqz => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = WasmStackValue::from_bool(var.get_i64() == 0);
                }
                WasmIntMnemonic::I64Eq => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmStackValue::from(lhs.get_u64() == rhs.get_u64());
                }
                WasmIntMnemonic::I64Ne => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmStackValue::from(lhs.get_u64() != rhs.get_u64());
                }
                WasmIntMnemonic::I64LtS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmStackValue::from(lhs.get_i64() < rhs.get_i64());
                }
                WasmIntMnemonic::I64LtU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmStackValue::from(lhs.get_u64() < rhs.get_u64());
                }
                WasmIntMnemonic::I64GtS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmStackValue::from(lhs.get_i64() > rhs.get_i64());
                }
                WasmIntMnemonic::I64GtU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmStackValue::from(lhs.get_u64() > rhs.get_u64());
                }
                WasmIntMnemonic::I64LeS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmStackValue::from(lhs.get_i64() <= rhs.get_i64());
                }
                WasmIntMnemonic::I64LeU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmStackValue::from(lhs.get_u64() <= rhs.get_u64());
                }
                WasmIntMnemonic::I64GeS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmStackValue::from(lhs.get_i64() >= rhs.get_i64());
                }
                WasmIntMnemonic::I64GeU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmStackValue::from(lhs.get_u64() >= rhs.get_u64());
                }

                WasmIntMnemonic::I64Clz => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    var.map_u64(|v| v.leading_zeros() as u64);
                }
                WasmIntMnemonic::I64Ctz => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    var.map_u64(|v| v.trailing_zeros() as u64);
                }
                WasmIntMnemonic::I64Popcnt => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    var.map_u64(|v| v.count_ones() as u64);
                }
                WasmIntMnemonic::I64Add => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_i64(|lhs| lhs.wrapping_add(rhs.get_i64()));
                }
                WasmIntMnemonic::I64Sub => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_i64(|lhs| lhs.wrapping_sub(rhs.get_i64()));
                }
                WasmIntMnemonic::I64Mul => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_i64(|lhs| lhs.wrapping_mul(rhs.get_i64()));
                }

                WasmIntMnemonic::I64DivS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { value_stack.get_unchecked(stack_level + 1).get_i64() };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    if rhs == 0 {
                        return Err(self.error(WasmRuntimeErrorType::DivideByZero, code));
                    }
                    lhs.map_i64(|lhs| lhs.wrapping_div(rhs));
                }
                WasmIntMnemonic::I64DivU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { value_stack.get_unchecked(stack_level + 1).get_u64() };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    if rhs == 0 {
                        return Err(self.error(WasmRuntimeErrorType::DivideByZero, code));
                    }
                    lhs.map_u64(|lhs| lhs.wrapping_div(rhs));
                }
                WasmIntMnemonic::I64RemS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { value_stack.get_unchecked(stack_level + 1).get_i64() };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    if rhs == 0 {
                        return Err(self.error(WasmRuntimeErrorType::DivideByZero, code));
                    }
                    lhs.map_i64(|lhs| lhs.wrapping_rem(rhs));
                }
                WasmIntMnemonic::I64RemU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { value_stack.get_unchecked(stack_level + 1).get_u64() };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    if rhs == 0 {
                        return Err(self.error(WasmRuntimeErrorType::DivideByZero, code));
                    }
                    lhs.map_u64(|lhs| lhs.wrapping_rem(rhs));
                }

                WasmIntMnemonic::I64And => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_u64(|lhs| lhs & rhs.get_u64());
                }
                WasmIntMnemonic::I64Or => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_u64(|lhs| lhs | rhs.get_u64());
                }
                WasmIntMnemonic::I64Xor => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_u64(|lhs| lhs ^ rhs.get_u64());
                }
                WasmIntMnemonic::I64Shl => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_u64(|lhs| lhs << rhs.get_u64());
                }
                WasmIntMnemonic::I64ShrS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_i64(|lhs| lhs >> rhs.get_i64());
                }
                WasmIntMnemonic::I64ShrU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_u64(|lhs| lhs >> rhs.get_u64());
                }
                WasmIntMnemonic::I64Rotl => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_u64(|lhs| lhs.rotate_left(rhs.get_u32()));
                }
                WasmIntMnemonic::I64Rotr => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    lhs.map_u64(|lhs| lhs.rotate_right(rhs.get_u32()));
                }

                WasmIntMnemonic::I64Extend8S => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = WasmStackValue::from_i64(var.get_i8() as i64);
                }
                WasmIntMnemonic::I64Extend16S => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = WasmStackValue::from_i64(var.get_i16() as i64);
                }
                WasmIntMnemonic::I64Extend32S | WasmIntMnemonic::I64ExtendI32S => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = WasmStackValue::from_i64(var.get_i32() as i64);
                }
                WasmIntMnemonic::I64ExtendI32U => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = WasmStackValue::from_u64(var.get_u32() as u64);
                }
                WasmIntMnemonic::I32WrapI64 => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = WasmStackValue::from_i32(var.get_i64() as i32);
                }
                WasmIntMnemonic::I32Extend8S => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = WasmStackValue::from_i32(var.get_i8() as i32);
                }
                WasmIntMnemonic::I32Extend16S => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = WasmStackValue::from_i32(var.get_i16() as i32);
                }

                WasmIntMnemonic::FusedI32AddI => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    lhs.map_i32(|lhs| lhs.wrapping_add(code.param1() as i32));
                }
                WasmIntMnemonic::FusedI32SubI => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    lhs.map_i32(|lhs| lhs.wrapping_sub(code.param1() as i32));
                }
                WasmIntMnemonic::FusedI32AndI => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    lhs.map_i32(|lhs| lhs & code.param1() as i32);
                }
                WasmIntMnemonic::FusedI32OrI => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    lhs.map_i32(|lhs| lhs | code.param1() as i32);
                }
                WasmIntMnemonic::FusedI32XorI => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    lhs.map_i32(|lhs| lhs ^ code.param1() as i32);
                }
                WasmIntMnemonic::FusedI32ShlI => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    lhs.map_u32(|lhs| lhs << (code.param1() as u32));
                }
                WasmIntMnemonic::FusedI32ShrUI => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    lhs.map_u32(|lhs| lhs >> (code.param1() as u32));
                }
                WasmIntMnemonic::FusedI32ShrSI => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    lhs.map_i32(|lhs| lhs >> (code.param1() as i32));
                }

                WasmIntMnemonic::FusedI64AddI => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    lhs.map_i64(|lhs| lhs.wrapping_add(code.param1() as i64));
                }
                WasmIntMnemonic::FusedI64SubI => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    lhs.map_i64(|lhs| lhs.wrapping_sub(code.param1() as i64));
                }

                WasmIntMnemonic::FusedI32BrZ => {
                    let cc =
                        unsafe { value_stack.get_unchecked_mut(code.stack_level()).get_i32() == 0 };
                    if cc {
                        let br = code.param1() as usize;
                        codes.set_position(br);
                    }
                }
                WasmIntMnemonic::FusedI64BrZ => {
                    let cc =
                        unsafe { value_stack.get_unchecked_mut(code.stack_level()).get_i64() == 0 };
                    if cc {
                        let br = code.param1() as usize;
                        codes.set_position(br);
                    }
                }

                _ => return Err(self.error(WasmRuntimeErrorType::NotSupprted, code)),
            }
        }
        if let Some(result_type) = result_types.first() {
            let val = unsafe { value_stack.get_unchecked(result_stack_level) };
            match result_type {
                WasmValType::I32 => Ok(Some(WasmValue::I32(val.get_i32()))),
                WasmValType::I64 => Ok(Some(WasmValue::I64(val.get_i64()))),
                // WasmValType::F32 => {}
                // WasmValType::F64 => {}
                _ => unreachable!(),
            }
        } else {
            Ok(None)
        }
    }

    #[inline]
    fn call(
        &mut self,
        target: &WasmFunction,
        code: &WasmImc,
        value_stack: &mut [WasmStackValue],
        heap: &mut StackHeap,
    ) -> Result<(), WasmRuntimeError> {
        let stack_pointer = code.stack_level();
        let current_function = self.func_index;
        let module = self.module;
        let result_types = target.result_types();

        let param_len = target.param_types().len();
        // if stack_pointer < param_len {
        //     return Err(self.error(WasmRuntimeError::InternalInconsistency, code));
        // }

        if let Some(code_block) = target.code_block() {
            heap.snapshot(|heap| {
                let mut locals = heap.alloc_stack(param_len + code_block.local_types().len());
                let stack_under = stack_pointer - param_len;

                locals.extend_from_slice(&value_stack[stack_under..stack_under + param_len]);
                for _ in code_block.local_types() {
                    let _ = locals.push(WasmStackValue::zero());
                }

                self.func_index = target.index();

                self.interpret(code_block, locals.as_mut_slice(), result_types, heap)
                    .and_then(|v| {
                        if let Some(result) = v {
                            let var = unsafe { value_stack.get_unchecked_mut(stack_under) };
                            *var = WasmStackValue::from(result);
                        }
                        self.func_index = current_function;
                        Ok(())
                    })
            })
        } else if let Some(dlink) = target.dlink() {
            heap.snapshot(|heap| {
                let mut locals = heap.alloc_stack(param_len);
                let stack_under = stack_pointer - param_len;
                let params = &value_stack[stack_under..stack_under + param_len];
                for (index, val_type) in target.param_types().iter().enumerate() {
                    let _ = locals.push(params[index].get_by_type(*val_type));
                }

                let result = match dlink(module, locals.as_slice()) {
                    Ok(v) => v,
                    Err(e) => return Err(self.error(e, code)),
                };

                if let Some(t) = result_types.first() {
                    if result.is_valid_type(*t) {
                        let var = match value_stack.get_mut(stack_under) {
                            Some(v) => v,
                            None => {
                                return Err(self.error(WasmRuntimeErrorType::TypeMismatch, code))
                            }
                        };
                        *var = WasmStackValue::from(result);
                    } else {
                        return Err(self.error(WasmRuntimeErrorType::TypeMismatch, code));
                    }
                }
                Ok(())
            })
        } else {
            Err(self.error(WasmRuntimeErrorType::NoMethod, code))
        }
    }
}

struct WasmIntermediateCodeStream<'a> {
    codes: &'a [WasmImc],
    position: usize,
}

impl<'a> WasmIntermediateCodeStream<'a> {
    #[inline]
    fn from_codes(codes: &'a [WasmImc]) -> Self {
        Self { codes, position: 0 }
    }
}

impl WasmIntermediateCodeStream<'_> {
    #[inline]
    fn fetch(&mut self) -> Option<&WasmImc> {
        self.codes.get(self.position).map(|v| {
            self.position += 1;
            v
        })
    }

    #[allow(dead_code)]
    #[inline]
    const fn position(&self) -> usize {
        self.position
    }

    #[inline]
    fn set_position(&mut self, val: usize) {
        self.position = val;
    }
}

pub trait WasmInvocation {
    fn invoke(&self, params: &[WasmValue]) -> Result<Option<WasmValue>, WasmRuntimeError>;
}

impl WasmInvocation for WasmRunnable<'_> {
    fn invoke(&self, params: &[WasmValue]) -> Result<Option<WasmValue>, WasmRuntimeError> {
        let function = self.function();
        let code_block = function
            .code_block()
            .ok_or(WasmRuntimeError::from(WasmRuntimeErrorType::NoMethod))?;

        let mut locals =
            Vec::with_capacity(function.param_types().len() + code_block.local_types().len());
        for (index, param_type) in function.param_types().iter().enumerate() {
            let param = params.get(index).ok_or(WasmRuntimeError::from(
                WasmRuntimeErrorType::InvalidParameter,
            ))?;
            if !param.is_valid_type(*param_type) {
                return Err(WasmRuntimeErrorType::InvalidParameter.into());
            }
            locals.push(WasmStackValue::from(param.clone()));
        }
        for _ in code_block.local_types() {
            locals.push(WasmStackValue::zero());
        }

        let result_types = function.result_types();

        let mut interp = WasmInterpreter::new(self.module());
        interp.invoke(
            function.index(),
            code_block,
            locals.as_slice(),
            result_types,
        )
    }
}

pub struct WasmRuntimeError {
    kind: WasmRuntimeErrorType,
    function: usize,
    position: usize,
    opcode: WasmOpcode,
}

impl WasmRuntimeError {
    #[inline]
    pub const fn kind(&self) -> WasmRuntimeErrorType {
        self.kind
    }

    #[inline]
    pub const fn function(&self) -> usize {
        self.function
    }

    #[inline]
    pub const fn position(&self) -> usize {
        self.position
    }

    #[inline]
    pub const fn opcode(&self) -> WasmOpcode {
        self.opcode
    }
}

impl From<WasmRuntimeErrorType> for WasmRuntimeError {
    #[inline]
    fn from(kind: WasmRuntimeErrorType) -> Self {
        Self {
            kind,
            function: 0,
            position: 0,
            opcode: WasmOpcode::Unreachable,
        }
    }
}

impl fmt::Debug for WasmRuntimeError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let opcode = self.opcode();
        write!(
            f,
            "{:?} (function {} position {:x} bytecode {:02x} {})",
            self.kind(),
            self.function(),
            self.position(),
            opcode as usize,
            opcode.to_str(),
        )
    }
}

/// A shared data type for storing in the value stack in the WebAssembly interpreter.
///
/// The internal representation is `union`, so information about the type needs to be provided externally.
#[derive(Copy, Clone)]
pub union WasmStackValue {
    i32: i32,
    u32: u32,
    i64: i64,
    u64: u64,
    f32: f32,
    f64: f64,
}

impl WasmStackValue {
    #[inline]
    pub const fn zero() -> Self {
        Self { u64: 0 }
    }

    #[inline]
    pub const fn from_bool(v: bool) -> Self {
        if v {
            Self::from_i32(1)
        } else {
            Self::from_i32(0)
        }
    }

    #[inline]
    pub const fn from_i32(v: i32) -> Self {
        Self { i32: v }
    }

    #[inline]
    pub const fn from_u32(v: u32) -> Self {
        Self { u32: v }
    }

    #[inline]
    pub const fn from_i64(v: i64) -> Self {
        Self { i64: v }
    }

    #[inline]
    pub const fn from_u64(v: u64) -> Self {
        Self { u64: v }
    }

    #[inline]
    pub fn get_bool(&self) -> bool {
        unsafe { self.i32 != 0 }
    }

    #[inline]
    pub fn get_i32(&self) -> i32 {
        unsafe { self.i32 }
    }

    #[inline]
    pub fn get_u32(&self) -> u32 {
        unsafe { self.u32 }
    }

    #[inline]
    pub fn get_i64(&self) -> i64 {
        unsafe { self.i64 }
    }

    #[inline]
    pub fn get_u64(&self) -> u64 {
        unsafe { self.u64 }
    }

    #[inline]
    pub fn get_f32(&self) -> f32 {
        unsafe { self.f32 }
    }

    #[inline]
    pub fn get_f64(&self) -> f64 {
        unsafe { self.f64 }
    }

    #[inline]
    pub fn get_i8(&self) -> i8 {
        unsafe { self.u32 as i8 }
    }

    #[inline]
    pub fn get_u8(&self) -> u8 {
        unsafe { self.u32 as u8 }
    }

    #[inline]
    pub fn get_i16(&self) -> i16 {
        unsafe { self.u32 as i16 }
    }

    #[inline]
    pub fn get_u16(&self) -> u16 {
        unsafe { self.u32 as u16 }
    }

    /// Retrieves the value held by the instance as a value of type `i32` and re-stores the value processed by the closure.
    #[inline]
    pub fn map_i32<F>(&mut self, f: F)
    where
        F: FnOnce(i32) -> i32,
    {
        let val = unsafe { self.i32 };
        self.i32 = f(val);
    }

    /// Retrieves the value held by the instance as a value of type `u32` and re-stores the value processed by the closure.
    #[inline]
    pub fn map_u32<F>(&mut self, f: F)
    where
        F: FnOnce(u32) -> u32,
    {
        let val = unsafe { self.u32 };
        self.u32 = f(val);
    }

    /// Retrieves the value held by the instance as a value of type `i64` and re-stores the value processed by the closure.
    #[inline]
    pub fn map_i64<F>(&mut self, f: F)
    where
        F: FnOnce(i64) -> i64,
    {
        let val = unsafe { self.i64 };
        self.i64 = f(val);
    }

    /// Retrieves the value held by the instance as a value of type `u64` and re-stores the value processed by the closure.
    #[inline]
    pub fn map_u64<F>(&mut self, f: F)
    where
        F: FnOnce(u64) -> u64,
    {
        let val = unsafe { self.u64 };
        self.u64 = f(val);
    }

    /// Converts the value held by the instance to the `WasmValue` type as a value of the specified type.
    #[inline]
    pub fn get_by_type(&self, val_type: WasmValType) -> WasmValue {
        match val_type {
            WasmValType::I32 => WasmValue::I32(self.get_i32()),
            WasmValType::I64 => WasmValue::I64(self.get_i64()),
            // WasmValType::F32 => {}
            // WasmValType::F64 => {}
            _ => todo!(),
        }
    }
}

impl From<bool> for WasmStackValue {
    #[inline]
    fn from(v: bool) -> Self {
        Self::from_bool(v)
    }
}

impl From<u32> for WasmStackValue {
    #[inline]
    fn from(v: u32) -> Self {
        Self::from_u32(v)
    }
}

impl From<i32> for WasmStackValue {
    #[inline]
    fn from(v: i32) -> Self {
        Self::from_i32(v)
    }
}

impl From<u64> for WasmStackValue {
    #[inline]
    fn from(v: u64) -> Self {
        Self::from_u64(v)
    }
}

impl From<i64> for WasmStackValue {
    #[inline]
    fn from(v: i64) -> Self {
        Self::from_i64(v)
    }
}

impl From<WasmValue> for WasmStackValue {
    #[inline]
    fn from(v: WasmValue) -> Self {
        match v {
            WasmValue::I32(v) => Self::from_i64(v as i64),
            WasmValue::I64(v) => Self::from_i64(v),
            _ => todo!(),
        }
    }
}
