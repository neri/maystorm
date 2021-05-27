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
        let mut stack = SharedStack::with_capacity(0x10000);

        let mut locals = {
            let output = stack.alloc(locals.len());
            output.copy_from_slice(locals);
            output
        };

        self.func_index = func_index;

        self.interpret(code_block, &mut locals, result_types, &mut stack)
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
        stack: &mut SharedStack,
    ) -> Result<Option<WasmValue>, WasmRuntimeError> {
        let mut codes = WasmIntermediateCodeStream::from_codes(code_block.intermediate_codes());

        let value_stack = stack.alloc(code_block.max_value_stack());
        for value in value_stack.iter_mut() {
            *value = WasmStackValue::zero();
        }

        let mut result_stack_level = 0;

        let mut last_code = WasmImc::from_mnemonic(WasmIntMnemonic::Unreachable);

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
                    last_code = *code;
                    result_stack_level = code.stack_level();
                    break;
                }

                WasmIntMnemonic::Call => {
                    let func = unsafe {
                        self.module
                            .functions()
                            .get_unchecked(code.param1() as usize)
                    };
                    self.call(func, code, value_stack, stack)?;
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
                    self.call(func, code, value_stack, stack)?;
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
                    let memory = unsafe { self.module.memory_unchecked(0) };
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = match memory.read_u32(offset).map(|v| WasmStackValue::from(v)) {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I32Load8S => {
                    let memory = unsafe { self.module.memory_unchecked(0) };
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
                    let memory = unsafe { self.module.memory_unchecked(0) };
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
                    let memory = unsafe { self.module.memory_unchecked(0) };
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
                    let memory = unsafe { self.module.memory_unchecked(0) };
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
                    let memory = unsafe { self.module.memory_unchecked(0) };
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = match memory.read_u64(offset).map(|v| WasmStackValue::from(v)) {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I64Load8S => {
                    let memory = unsafe { self.module.memory_unchecked(0) };
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
                    let memory = unsafe { self.module.memory_unchecked(0) };
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
                    let memory = unsafe { self.module.memory_unchecked(0) };
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
                    let memory = unsafe { self.module.memory_unchecked(0) };
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
                    let memory = unsafe { self.module.memory_unchecked(0) };
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
                    let memory = unsafe { self.module.memory_unchecked(0) };
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
                    let memory = unsafe { self.module.memory_unchecked(0) };
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
                    let memory = unsafe { self.module.memory_unchecked(0) };
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
                    let memory = unsafe { self.module.memory_unchecked(0) };
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
                    let memory = unsafe { self.module.memory_unchecked(0) };
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
                    let memory = unsafe { self.module.memory_unchecked(0) };
                    let ref_a = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *ref_a = WasmStackValue::from(memory.size() as u32);
                }
                WasmIntMnemonic::MemoryGrow => {
                    let memory = unsafe { self.module.memory_unchecked(0) };
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

                #[allow(unreachable_patterns)]
                _ => return Err(self.error(WasmRuntimeErrorType::InvalidBytecode, code)),
            }
        }
        if let Some(result_type) = result_types.first() {
            let val = unsafe { value_stack.get_unchecked(result_stack_level) };
            match result_type {
                WasmValType::I32 => Ok(Some(WasmValue::I32(val.get_i32()))),
                WasmValType::I64 => Ok(Some(WasmValue::I64(val.get_i64()))),
                // WasmValType::F32 => {}
                // WasmValType::F64 => {}
                _ => Err(self.error(WasmRuntimeErrorType::InvalidParameter, &last_code)),
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
        stack: &mut SharedStack,
    ) -> Result<(), WasmRuntimeError> {
        let stack_pointer = code.stack_level();
        let current_function = self.func_index;
        let module = self.module;
        let result_types = target.result_types();

        let param_len = target.param_types().len();
        // if stack_pointer < param_len {
        //     return Err(self.error(WasmRuntimeError::InternalInconsistency, code));
        // }

        if let Some(body) = target.body() {
            stack.snapshot(|stack| {
                let code_block = body.block_info();
                let mut locals = stack.alloc_stack(param_len + body.local_types().len());
                let stack_under = stack_pointer - param_len;

                locals.extend_from_slice(&value_stack[stack_under..stack_under + param_len]);
                for _ in body.local_types() {
                    let _ = locals.push(WasmStackValue::zero());
                }

                self.func_index = target.index();

                self.interpret(code_block, locals.as_mut_slice(), result_types, stack)
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
            stack.snapshot(|stack| {
                let mut locals = stack.alloc_stack(param_len);
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
        let body = function
            .body()
            .ok_or(WasmRuntimeError::from(WasmRuntimeErrorType::NoMethod))?;

        let mut locals =
            Vec::with_capacity(function.param_types().len() + body.local_types().len());
        for (index, param_type) in function.param_types().iter().enumerate() {
            let param = params.get(index).ok_or(WasmRuntimeError::from(
                WasmRuntimeErrorType::InvalidParameter,
            ))?;
            if !param.is_valid_type(*param_type) {
                return Err(WasmRuntimeErrorType::InvalidParameter.into());
            }
            locals.push(WasmStackValue::from(param.clone()));
        }
        for _ in body.local_types() {
            locals.push(WasmStackValue::zero());
        }

        let result_types = function.result_types();

        let mut interp = WasmInterpreter::new(self.module());
        interp.invoke(
            function.index(),
            body.block_info(),
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

#[cfg(test)]
mod tests {

    use super::{WasmInterpreter, WasmInvocation};
    use crate::{
        wasm::{
            Leb128Stream, WasmCodeBlock, WasmDecodeErrorType, WasmLoader, WasmModule, WasmValType,
        },
        WasmRuntimeErrorType,
    };

    #[test]
    fn add() {
        let slice = [0x20, 0, 0x20, 1, 0x6A, 0x0B];
        let local_types = [WasmValType::I32, WasmValType::I32];
        let result_types = [WasmValType::I32];
        let mut stream = Leb128Stream::from_slice(&slice);
        let module = WasmModule::new();
        let info =
            WasmCodeBlock::generate(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [1234.into(), 5678.into()];

        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 6912);

        let params = [0xDEADBEEFu32.into(), 0x55555555.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0x34031444);
    }

    #[test]
    fn fused_add() {
        let slice = [0x20, 0, 0x41, 1, 0x6A, 0x0B];
        let local_types = [WasmValType::I32, WasmValType::I32];
        let result_types = [WasmValType::I32];
        let mut stream = Leb128Stream::from_slice(&slice);
        let module = WasmModule::new();
        let info =
            WasmCodeBlock::generate(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [1234_5678.into()];

        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 12345679);

        let params = [0xFFFF_FFFFu32.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn sub() {
        let slice = [0x20, 0, 0x20, 1, 0x6B, 0x0B];
        let local_types = [WasmValType::I32, WasmValType::I32];
        let result_types = [WasmValType::I32];
        let mut stream = Leb128Stream::from_slice(&slice);
        let module = WasmModule::new();
        let info =
            WasmCodeBlock::generate(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [1234.into(), 5678.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, -4444);

        let params = [0x55555555.into(), 0xDEADBEEFu32.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0x76a79666);
    }

    #[test]
    fn mul() {
        let slice = [0x20, 0, 0x20, 1, 0x6C, 0x0B];
        let local_types = [WasmValType::I32, WasmValType::I32];
        let result_types = [WasmValType::I32];
        let mut stream = Leb128Stream::from_slice(&slice);
        let module = WasmModule::new();
        let info =
            WasmCodeBlock::generate(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [1234.into(), 5678.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 7006652);

        let params = [0x55555555.into(), 0xDEADBEEFu32.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0x6070c05b);
    }

    #[test]
    fn div_s() {
        let slice = [0x20, 0, 0x20, 1, 0x6D, 0x0B];
        let local_types = [WasmValType::I32, WasmValType::I32];
        let result_types = [WasmValType::I32];
        let mut stream = Leb128Stream::from_slice(&slice);
        let module = WasmModule::new();
        let info =
            WasmCodeBlock::generate(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [7006652.into(), 5678.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 1234);

        let params = [42.into(), (-6).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, -7);

        let params = [(-42).into(), (6).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, -7);

        let params = [(-42).into(), (-6).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 7);

        let params = [1234.into(), 0.into()];
        let result = interp.invoke(0, &info, &params, &result_types).unwrap_err();
        assert_eq!(WasmRuntimeErrorType::DivideByZero, result.kind());
    }

    #[test]
    fn div_u() {
        let slice = [0x20, 0, 0x20, 1, 0x6E, 0x0B];
        let local_types = [WasmValType::I32, WasmValType::I32];
        let result_types = [WasmValType::I32];
        let mut stream = Leb128Stream::from_slice(&slice);
        let module = WasmModule::new();
        let info =
            WasmCodeBlock::generate(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [7006652.into(), 5678.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 1234);

        let params = [42.into(), (-6).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0);

        let params = [(-42).into(), (6).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 715827875);

        let params = [1234.into(), 0.into()];
        let result = interp.invoke(0, &info, &params, &result_types).unwrap_err();
        assert_eq!(WasmRuntimeErrorType::DivideByZero, result.kind());
    }

    #[test]
    fn select() {
        let slice = [0x20, 0, 0x20, 1, 0x20, 2, 0x1B, 0x0B];
        let local_types = [WasmValType::I32, WasmValType::I32, WasmValType::I32];
        let result_types = [WasmValType::I32];
        let mut stream = Leb128Stream::from_slice(&slice);
        let module = WasmModule::new();
        let info =
            WasmCodeBlock::generate(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [123.into(), 456.into(), 789.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 123);

        let params = [123.into(), 456.into(), 0.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 456);
    }

    #[test]
    fn lts() {
        let slice = [0x20, 0, 0x20, 1, 0x48, 0x0B];
        let local_types = [WasmValType::I32, WasmValType::I32];
        let result_types = [WasmValType::I32];
        let mut stream = Leb128Stream::from_slice(&slice);
        let module = WasmModule::new();
        let info =
            WasmCodeBlock::generate(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [123.into(), 456.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 1);

        let params = [123.into(), 123.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0);

        let params = [456.into(), 123.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0);

        let params = [123.into(), (-456).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0);

        let params = [456.into(), (-123).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn ltu() {
        let slice = [0x20, 0, 0x20, 1, 0x49, 0x0B];
        let local_types = [WasmValType::I32, WasmValType::I32];
        let result_types = [WasmValType::I32];
        let mut stream = Leb128Stream::from_slice(&slice);
        let module = WasmModule::new();
        let info =
            WasmCodeBlock::generate(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [123.into(), 456.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 1);

        let params = [123.into(), 123.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0);

        let params = [456.into(), 123.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0);

        let params = [123.into(), (-456).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 1);

        let params = [456.into(), (-123).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 1);
    }

    #[test]
    fn les() {
        let slice = [0x20, 0, 0x20, 1, 0x4C, 0x0B];
        let local_types = [WasmValType::I32, WasmValType::I32];
        let result_types = [WasmValType::I32];
        let mut stream = Leb128Stream::from_slice(&slice);
        let module = WasmModule::new();
        let info =
            WasmCodeBlock::generate(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [123.into(), 456.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 1);

        let params = [123.into(), 123.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 1);

        let params = [456.into(), 123.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0);

        let params = [123.into(), (-456).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0);

        let params = [456.into(), (-123).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn br_table() {
        let slice = [
            0x02, 0x40, 0x02, 0x40, 0x0b, 0x0b, 0x02, 0x40, 0x02, 0x40, 0x02, 0x40, 0x20, 0x00,
            0x0e, 0x02, 0x00, 0x01, 0x02, 0x0b, 0x41, 0xfb, 0x00, 0x0f, 0x0b, 0x41, 0xc8, 0x03,
            0x0f, 0x0b, 0x41, 0x95, 0x06, 0x0b,
        ];
        let local_types = [WasmValType::I32];
        let result_types = [WasmValType::I32];
        let mut stream = Leb128Stream::from_slice(&slice);
        let module = WasmModule::new();
        let info =
            WasmCodeBlock::generate(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [0.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 123);

        let params = [1.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 456);

        let params = [2.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 789);

        let params = [3.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 789);

        let params = [4.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 789);

        let params = [5.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 789);

        let params = [(-1).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 789);
    }

    #[test]
    fn app_factorial() {
        let slice = [
            0x41, 0x01, 0x21, 0x01, 0x02, 0x40, 0x03, 0x40, 0x20, 0x00, 0x45, 0x0d, 0x01, 0x20,
            0x01, 0x20, 0x00, 0x6c, 0x21, 0x01, 0x20, 0x00, 0x41, 0x01, 0x6b, 0x21, 0x00, 0x0c,
            0x00, 0x0b, 0x0b, 0x20, 0x01, 0x0b,
        ];
        let local_types = [WasmValType::I32, WasmValType::I32];
        let result_types = [WasmValType::I32];
        let mut stream = Leb128Stream::from_slice(&slice);
        let module = WasmModule::new();
        let info =
            WasmCodeBlock::generate(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [7.into(), 0.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 5040);

        let params = [10.into(), 0.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 3628800);
    }

    #[test]
    fn app_fibonacci() {
        let slice = [
            0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00, 0x01, 0x06, 0x01, 0x60, 0x01, 0x7F,
            0x01, 0x7F, 0x03, 0x02, 0x01, 0x00, 0x0A, 0x31, 0x01, 0x2F, 0x01, 0x01, 0x7F, 0x41,
            0x00, 0x21, 0x01, 0x02, 0x40, 0x03, 0x40, 0x20, 0x00, 0x41, 0x02, 0x49, 0x0D, 0x01,
            0x20, 0x00, 0x41, 0x7F, 0x6A, 0x10, 0x00, 0x20, 0x01, 0x6A, 0x21, 0x01, 0x20, 0x00,
            0x41, 0x7E, 0x6A, 0x21, 0x00, 0x0C, 0x00, 0x0B, 0x0B, 0x20, 0x00, 0x20, 0x01, 0x6A,
            0x0B,
        ];

        let module =
            WasmLoader::instantiate(&slice, |_, _, _| Err(WasmDecodeErrorType::DynamicLinkError))
                .unwrap();
        let runnable = module.func_by_index(0).unwrap();

        let result = runnable
            .invoke(&[5.into()])
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 5);

        let result = runnable
            .invoke(&[10.into()])
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 55);

        let result = runnable
            .invoke(&[20.into()])
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 6765);
    }

    #[test]
    fn global() {
        let slice = [
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x06, 0x01, 0x60, 0x01, 0x7f,
            0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x05, 0x03, 0x01, 0x00, 0x01, 0x06, 0x07, 0x01,
            0x7f, 0x01, 0x41, 0xfb, 0x00, 0x0b, 0x0a, 0x0d, 0x01, 0x0b, 0x00, 0x23, 0x00, 0x20,
            0x00, 0x6a, 0x24, 0x00, 0x23, 0x00, 0x0b,
        ];

        let module =
            WasmLoader::instantiate(&slice, |_, _, _| Err(WasmDecodeErrorType::DynamicLinkError))
                .unwrap();
        let runnable = module.func_by_index(0).unwrap();

        assert_eq!(module.global(0).unwrap().value().get_i32().unwrap(), 123);

        let result = runnable
            .invoke(&[456.into()])
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 579);

        assert_eq!(module.global(0).unwrap().value().get_i32().unwrap(), 579);

        let result = runnable
            .invoke(&[789.into()])
            .unwrap()
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 1368);

        assert_eq!(module.global(0).unwrap().value().get_i32().unwrap(), 1368);
    }
}
