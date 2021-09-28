//! WebAssembly Intermediate Code Interpreter

use super::{intcode::*, stack::*, wasm::*};
use crate::opcode::WasmOpcode;
use alloc::{borrow::ToOwned, string::String, vec::Vec};
use core::fmt;

type StackType = usize;

const INITIAL_VALUE_STACK_SIZE: usize = 512;

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
    #[inline]
    fn error(&self, kind: WasmRuntimeErrorKind, code: &WasmImc) -> WasmRuntimeError {
        let function_name = self
            .module
            .names()
            .and_then(|v| v.func_by_index(self.func_index))
            .map(|v| v.to_owned());
        let file_position = self
            .module
            .codeblock(self.func_index)
            .map(|v| v.file_position())
            .unwrap_or(0)
            + code.source_position();
        WasmRuntimeError {
            kind,
            file_position,
            function: self.func_index,
            function_name,
            position: code.source_position(),
            opcode: code.opcode().unwrap_or(WasmOpcode::Unreachable),
        }
    }

    #[inline]
    pub fn invoke(
        &mut self,
        func_index: usize,
        code_block: &WasmCodeBlock,
        locals: &mut [WasmUnsafeValue],
        result_types: &[WasmValType],
    ) -> Result<Option<WasmValue>, WasmRuntimeError> {
        let mut heap = StackHeap::with_capacity(0x10000);
        self.interpret(func_index, code_block, locals, result_types, &mut heap)
    }

    fn interpret(
        &mut self,
        func_index: usize,
        code_block: &WasmCodeBlock,
        locals: &mut [WasmUnsafeValue],
        result_types: &[WasmValType],
        heap: &mut StackHeap,
    ) -> Result<Option<WasmValue>, WasmRuntimeError> {
        self.func_index = func_index;
        let mut codes = WasmIntermediateCodeStream::from_codes(code_block.intermediate_codes());

        let value_stack = heap.alloc(code_block.max_value_stack());
        for value in value_stack.iter_mut() {
            *value = WasmUnsafeValue::zero();
        }

        let mut result_stack_level = 0;

        let memory = unsafe { self.module.memory_unchecked(0) };

        while let Some(code) = codes.fetch() {
            match code.mnemonic() {
                WasmIntMnemonic::Unreachable | WasmIntMnemonic::Nop => {
                    // Currently, NOP is unreachable
                    return Err(self.error(WasmRuntimeErrorKind::Unreachable, code));
                }

                WasmIntMnemonic::Br => {
                    let br = code.param1() as usize;
                    codes.set_position(br);
                }

                WasmIntMnemonic::BrIf => {
                    let cc = unsafe { value_stack.get_unchecked(code.stack_level()).get_bool() };
                    if cc {
                        let br = code.param1() as usize;
                        codes.set_position(br);
                    }
                }
                WasmIntMnemonic::BrTable => {
                    let mut index =
                        unsafe { value_stack.get_unchecked(code.stack_level()).get_u32() as usize };
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
                        .elem_get(index)
                        .ok_or(self.error(WasmRuntimeErrorKind::NoMethod, code))?;
                    if func.type_index() != type_index {
                        return Err(self.error(WasmRuntimeErrorKind::TypeMismatch, code));
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
                WasmIntMnemonic::LocalSet => {
                    let local = unsafe { locals.get_unchecked_mut(code.param1() as usize) };
                    let ref_a = unsafe { value_stack.get_unchecked(code.stack_level()) };
                    *local = *ref_a;
                }

                WasmIntMnemonic::GlobalGet => {
                    let global = unsafe {
                        &*self
                            .module
                            .globals()
                            .get_raw_unchecked(code.param1() as usize)
                            .get()
                    };
                    let ref_a = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *ref_a = *global;
                }
                WasmIntMnemonic::GlobalSet => {
                    let global = unsafe {
                        &mut *self
                            .module
                            .globals()
                            .get_raw_unchecked(code.param1() as usize)
                            .get()
                    };
                    let ref_a = unsafe { value_stack.get_unchecked(code.stack_level()) };
                    *global = *ref_a;
                }

                WasmIntMnemonic::I32Load => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + unsafe { var.get_u32() as usize };
                    *var = match memory.read_u32(offset).map(|v| WasmUnsafeValue::from(v)) {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I32Load8S => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + unsafe { var.get_u32() as usize };
                    *var = match memory
                        .read_u8(offset)
                        .map(|v| WasmUnsafeValue::from(v as i8 as i32))
                    {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I32Load8U => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + unsafe { var.get_u32() as usize };
                    *var = match memory
                        .read_u8(offset)
                        .map(|v| WasmUnsafeValue::from(v as u32))
                    {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I32Load16S => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + unsafe { var.get_u32() as usize };
                    *var = match memory
                        .read_u16(offset)
                        .map(|v| WasmUnsafeValue::from(v as i16 as i32))
                    {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I32Load16U => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + unsafe { var.get_u32() as usize };
                    *var = match memory
                        .read_u16(offset)
                        .map(|v| WasmUnsafeValue::from(v as u32))
                    {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }

                WasmIntMnemonic::I64Load => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + unsafe { var.get_u32() as usize };
                    *var = match memory.read_u64(offset).map(|v| WasmUnsafeValue::from(v)) {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I64Load8S => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + unsafe { var.get_u32() as usize };
                    *var = match memory
                        .read_u8(offset)
                        .map(|v| WasmUnsafeValue::from(v as i8 as i64))
                    {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I64Load8U => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + unsafe { var.get_u32() as usize };
                    *var = match memory
                        .read_u8(offset)
                        .map(|v| WasmUnsafeValue::from(v as u64))
                    {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I64Load16S => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + unsafe { var.get_u32() as usize };
                    *var = match memory
                        .read_u16(offset)
                        .map(|v| WasmUnsafeValue::from(v as i16 as i64))
                    {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I64Load16U => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + unsafe { var.get_u32() as usize };
                    *var = match memory
                        .read_u16(offset)
                        .map(|v| WasmUnsafeValue::from(v as u64))
                    {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I64Load32S => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + unsafe { var.get_u32() as usize };
                    *var = match memory
                        .read_u32(offset)
                        .map(|v| WasmUnsafeValue::from(v as i32 as i64))
                    {
                        Ok(v) => v,
                        Err(e) => return Err(self.error(e, code)),
                    };
                }
                WasmIntMnemonic::I64Load32U => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    let offset = code.param1() as usize + unsafe { var.get_u32() as usize };
                    *var = match memory
                        .read_u32(offset)
                        .map(|v| WasmUnsafeValue::from(v as u64))
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
                    *ref_a = WasmUnsafeValue::from(memory.size());
                }
                WasmIntMnemonic::MemoryGrow => {
                    let ref_a = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *ref_a = WasmUnsafeValue::from(memory.grow(unsafe { ref_a.get_i32() }));
                }

                WasmIntMnemonic::I32Const => {
                    let ref_a = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *ref_a = WasmUnsafeValue::from_u32(code.param1() as u32);
                }
                WasmIntMnemonic::I64Const => {
                    let ref_a = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *ref_a = WasmUnsafeValue::from_u64(code.param1());
                }

                WasmIntMnemonic::I32Eqz => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = WasmUnsafeValue::from_bool(unsafe { var.get_i32() == 0 });
                }
                WasmIntMnemonic::I32Eq => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmUnsafeValue::from(unsafe { lhs.get_u32() == rhs.get_u32() });
                }
                WasmIntMnemonic::I32Ne => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmUnsafeValue::from(unsafe { lhs.get_u32() != rhs.get_u32() });
                }
                WasmIntMnemonic::I32LtS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmUnsafeValue::from(unsafe { lhs.get_i32() < rhs.get_i32() });
                }
                WasmIntMnemonic::I32LtU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmUnsafeValue::from(unsafe { lhs.get_u32() < rhs.get_u32() });
                }
                WasmIntMnemonic::I32GtS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmUnsafeValue::from(unsafe { lhs.get_i32() > rhs.get_i32() });
                }
                WasmIntMnemonic::I32GtU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmUnsafeValue::from(unsafe { lhs.get_u32() > rhs.get_u32() });
                }
                WasmIntMnemonic::I32LeS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmUnsafeValue::from(unsafe { lhs.get_i32() <= rhs.get_i32() });
                }
                WasmIntMnemonic::I32LeU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmUnsafeValue::from(unsafe { lhs.get_u32() <= rhs.get_u32() });
                }
                WasmIntMnemonic::I32GeS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmUnsafeValue::from(unsafe { lhs.get_i32() >= rhs.get_i32() });
                }
                WasmIntMnemonic::I32GeU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmUnsafeValue::from(unsafe { lhs.get_u32() >= rhs.get_u32() });
                }

                WasmIntMnemonic::I32Clz => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        var.map_u32(|v| v.leading_zeros());
                    }
                }
                WasmIntMnemonic::I32Ctz => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        var.map_u32(|v| v.trailing_zeros());
                    }
                }
                WasmIntMnemonic::I32Popcnt => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        var.map_u32(|v| v.count_ones());
                    }
                }
                WasmIntMnemonic::I32Add => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_i32(|lhs| lhs.wrapping_add(rhs.get_i32()));
                    }
                }
                WasmIntMnemonic::I32Sub => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_i32(|lhs| lhs.wrapping_sub(rhs.get_i32()));
                    }
                }
                WasmIntMnemonic::I32Mul => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_i32(|lhs| lhs.wrapping_mul(rhs.get_i32()));
                    }
                }

                WasmIntMnemonic::I32DivS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { value_stack.get_unchecked(stack_level + 1).get_i32() };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    if rhs == 0 {
                        return Err(self.error(WasmRuntimeErrorKind::DivideByZero, code));
                    }
                    unsafe {
                        lhs.map_i32(|lhs| lhs.wrapping_div(rhs));
                    }
                }
                WasmIntMnemonic::I32DivU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { value_stack.get_unchecked(stack_level + 1).get_u32() };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    if rhs == 0 {
                        return Err(self.error(WasmRuntimeErrorKind::DivideByZero, code));
                    }
                    unsafe {
                        lhs.map_u32(|lhs| lhs.wrapping_div(rhs));
                    }
                }
                WasmIntMnemonic::I32RemS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { value_stack.get_unchecked(stack_level + 1).get_i32() };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    if rhs == 0 {
                        return Err(self.error(WasmRuntimeErrorKind::DivideByZero, code));
                    }
                    unsafe {
                        lhs.map_i32(|lhs| lhs.wrapping_rem(rhs));
                    }
                }
                WasmIntMnemonic::I32RemU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { value_stack.get_unchecked(stack_level + 1).get_u32() };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    if rhs == 0 {
                        return Err(self.error(WasmRuntimeErrorKind::DivideByZero, code));
                    }
                    unsafe {
                        lhs.map_u32(|lhs| lhs.wrapping_rem(rhs));
                    }
                }

                WasmIntMnemonic::I32And => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_u32(|lhs| lhs & rhs.get_u32());
                    }
                }
                WasmIntMnemonic::I32Or => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_u32(|lhs| lhs | rhs.get_u32());
                    }
                }
                WasmIntMnemonic::I32Xor => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_u32(|lhs| lhs ^ rhs.get_u32());
                    }
                }
                WasmIntMnemonic::I32Shl => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_u32(|lhs| lhs << rhs.get_u32());
                    }
                }
                WasmIntMnemonic::I32ShrS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_i32(|lhs| lhs >> rhs.get_i32());
                    }
                }
                WasmIntMnemonic::I32ShrU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_u32(|lhs| lhs >> rhs.get_u32());
                    }
                }
                WasmIntMnemonic::I32Rotl => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_u32(|lhs| lhs.rotate_left(rhs.get_u32()));
                    }
                }
                WasmIntMnemonic::I32Rotr => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_u32(|lhs| lhs.rotate_right(rhs.get_u32()));
                    }
                }

                WasmIntMnemonic::I64Eqz => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = WasmUnsafeValue::from_bool(unsafe { var.get_i64() == 0 });
                }
                WasmIntMnemonic::I64Eq => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmUnsafeValue::from(unsafe { lhs.get_u64() == rhs.get_u64() });
                }
                WasmIntMnemonic::I64Ne => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmUnsafeValue::from(unsafe { lhs.get_u64() != rhs.get_u64() });
                }
                WasmIntMnemonic::I64LtS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmUnsafeValue::from(unsafe { lhs.get_i64() < rhs.get_i64() });
                }
                WasmIntMnemonic::I64LtU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmUnsafeValue::from(unsafe { lhs.get_u64() < rhs.get_u64() });
                }
                WasmIntMnemonic::I64GtS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmUnsafeValue::from(unsafe { lhs.get_i64() > rhs.get_i64() });
                }
                WasmIntMnemonic::I64GtU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmUnsafeValue::from(unsafe { lhs.get_u64() > rhs.get_u64() });
                }
                WasmIntMnemonic::I64LeS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmUnsafeValue::from(unsafe { lhs.get_i64() <= rhs.get_i64() });
                }
                WasmIntMnemonic::I64LeU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmUnsafeValue::from(unsafe { lhs.get_u64() <= rhs.get_u64() });
                }
                WasmIntMnemonic::I64GeS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmUnsafeValue::from(unsafe { lhs.get_i64() >= rhs.get_i64() });
                }
                WasmIntMnemonic::I64GeU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    *lhs = WasmUnsafeValue::from(unsafe { lhs.get_u64() >= rhs.get_u64() });
                }

                WasmIntMnemonic::I64Clz => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        var.map_u64(|v| v.leading_zeros() as u64);
                    }
                }
                WasmIntMnemonic::I64Ctz => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        var.map_u64(|v| v.trailing_zeros() as u64);
                    }
                }
                WasmIntMnemonic::I64Popcnt => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        var.map_u64(|v| v.count_ones() as u64);
                    }
                }
                WasmIntMnemonic::I64Add => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_i64(|lhs| lhs.wrapping_add(rhs.get_i64()));
                    }
                }
                WasmIntMnemonic::I64Sub => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_i64(|lhs| lhs.wrapping_sub(rhs.get_i64()));
                    }
                }
                WasmIntMnemonic::I64Mul => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_i64(|lhs| lhs.wrapping_mul(rhs.get_i64()));
                    }
                }

                WasmIntMnemonic::I64DivS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { value_stack.get_unchecked(stack_level + 1).get_i64() };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    if rhs == 0 {
                        return Err(self.error(WasmRuntimeErrorKind::DivideByZero, code));
                    }
                    unsafe {
                        lhs.map_i64(|lhs| lhs.wrapping_div(rhs));
                    }
                }
                WasmIntMnemonic::I64DivU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { value_stack.get_unchecked(stack_level + 1).get_u64() };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    if rhs == 0 {
                        return Err(self.error(WasmRuntimeErrorKind::DivideByZero, code));
                    }
                    unsafe {
                        lhs.map_u64(|lhs| lhs.wrapping_div(rhs));
                    }
                }
                WasmIntMnemonic::I64RemS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { value_stack.get_unchecked(stack_level + 1).get_i64() };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    if rhs == 0 {
                        return Err(self.error(WasmRuntimeErrorKind::DivideByZero, code));
                    }
                    unsafe {
                        lhs.map_i64(|lhs| lhs.wrapping_rem(rhs));
                    }
                }
                WasmIntMnemonic::I64RemU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { value_stack.get_unchecked(stack_level + 1).get_u64() };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    if rhs == 0 {
                        return Err(self.error(WasmRuntimeErrorKind::DivideByZero, code));
                    }
                    unsafe {
                        lhs.map_u64(|lhs| lhs.wrapping_rem(rhs));
                    }
                }

                WasmIntMnemonic::I64And => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_u64(|lhs| lhs & rhs.get_u64());
                    }
                }
                WasmIntMnemonic::I64Or => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_u64(|lhs| lhs | rhs.get_u64());
                    }
                }
                WasmIntMnemonic::I64Xor => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_u64(|lhs| lhs ^ rhs.get_u64());
                    }
                }
                WasmIntMnemonic::I64Shl => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_u64(|lhs| lhs << rhs.get_u64());
                    }
                }
                WasmIntMnemonic::I64ShrS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_i64(|lhs| lhs >> rhs.get_i64());
                    }
                }
                WasmIntMnemonic::I64ShrU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_u64(|lhs| lhs >> rhs.get_u64());
                    }
                }
                WasmIntMnemonic::I64Rotl => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_u64(|lhs| lhs.rotate_left(rhs.get_u32()));
                    }
                }
                WasmIntMnemonic::I64Rotr => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe {
                        lhs.map_u64(|lhs| lhs.rotate_right(rhs.get_u32()));
                    }
                }

                WasmIntMnemonic::I64Extend8S => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = WasmUnsafeValue::from_i64(unsafe { var.get_i8() as i64 });
                }
                WasmIntMnemonic::I64Extend16S => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = WasmUnsafeValue::from_i64(unsafe { var.get_i16() as i64 });
                }
                WasmIntMnemonic::I64Extend32S | WasmIntMnemonic::I64ExtendI32S => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = WasmUnsafeValue::from_i64(unsafe { var.get_i32() as i64 });
                }
                WasmIntMnemonic::I64ExtendI32U => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = WasmUnsafeValue::from_u64(unsafe { var.get_u32() as u64 });
                }
                WasmIntMnemonic::I32WrapI64 => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = WasmUnsafeValue::from_i32(unsafe { var.get_i64() as i32 });
                }
                WasmIntMnemonic::I32Extend8S => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = WasmUnsafeValue::from_i32(unsafe { var.get_i8() as i32 });
                }
                WasmIntMnemonic::I32Extend16S => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = WasmUnsafeValue::from_i32(unsafe { var.get_i16() as i32 });
                }

                WasmIntMnemonic::FusedI32AddI => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        lhs.map_i32(|lhs| lhs.wrapping_add(code.param1() as i32));
                    }
                }
                WasmIntMnemonic::FusedI32SubI => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        lhs.map_i32(|lhs| lhs.wrapping_sub(code.param1() as i32));
                    }
                }
                WasmIntMnemonic::FusedI32AndI => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        lhs.map_i32(|lhs| lhs & code.param1() as i32);
                    }
                }
                WasmIntMnemonic::FusedI32OrI => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        lhs.map_i32(|lhs| lhs | code.param1() as i32);
                    }
                }
                WasmIntMnemonic::FusedI32XorI => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        lhs.map_i32(|lhs| lhs ^ code.param1() as i32);
                    }
                }
                WasmIntMnemonic::FusedI32ShlI => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        lhs.map_u32(|lhs| lhs << (code.param1() as u32));
                    }
                }
                WasmIntMnemonic::FusedI32ShrUI => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        lhs.map_u32(|lhs| lhs >> (code.param1() as u32));
                    }
                }
                WasmIntMnemonic::FusedI32ShrSI => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        lhs.map_i32(|lhs| lhs >> (code.param1() as i32));
                    }
                }

                WasmIntMnemonic::FusedI64AddI => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        lhs.map_i64(|lhs| lhs.wrapping_add(code.param1() as i64));
                    }
                }
                WasmIntMnemonic::FusedI64SubI => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        lhs.map_i64(|lhs| lhs.wrapping_sub(code.param1() as i64));
                    }
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

                _ => return Err(self.error(WasmRuntimeErrorKind::NotSupprted, code)),
            }
        }
        if let Some(result_type) = result_types.first() {
            let val = unsafe { value_stack.get_unchecked(result_stack_level) };
            Ok(Some(unsafe { val.get_by_type(*result_type) }))
        } else {
            Ok(None)
        }
    }

    #[inline]
    fn call(
        &mut self,
        target: &WasmFunction,
        code: &WasmImc,
        value_stack: &mut [WasmUnsafeValue],
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
                let stack_under = stack_pointer - param_len;
                let local_len = param_len + code_block.local_types().len();

                let locals = if value_stack.len() >= stack_under + local_len {
                    let (_, locals) = value_stack.split_at_mut(stack_under);
                    locals
                } else {
                    let locals = heap.alloc(usize::max(INITIAL_VALUE_STACK_SIZE, local_len));
                    for (index, value) in unsafe {
                        value_stack
                            .get_unchecked(stack_under..stack_under + param_len)
                            .iter()
                            .enumerate()
                    } {
                        unsafe {
                            *locals.get_unchecked_mut(index) = *value;
                        }
                    }
                    locals
                };
                for index in 0..code_block.local_types().len() {
                    unsafe {
                        *locals.get_unchecked_mut(param_len + index) = WasmUnsafeValue::zero();
                    }
                }

                self.interpret(target.index(), code_block, locals, result_types, heap)
                    .and_then(|v| {
                        if let Some(result) = v {
                            let var = unsafe { value_stack.get_unchecked_mut(stack_under) };
                            *var = WasmUnsafeValue::from(result);
                        }
                        self.func_index = current_function;
                        Ok(())
                    })
            })
        } else if let Some(dlink) = target.dlink() {
            heap.snapshot(|heap| {
                let mut locals = heap.alloc_stack(param_len);
                let stack_under = stack_pointer - param_len;
                let params =
                    unsafe { value_stack.get_unchecked(stack_under..stack_under + param_len) };
                for (index, val_type) in target.param_types().iter().enumerate() {
                    let _ =
                        locals.push(unsafe { params.get_unchecked(index).get_by_type(*val_type) });
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
                                return Err(self.error(WasmRuntimeErrorKind::TypeMismatch, code))
                            }
                        };
                        *var = WasmUnsafeValue::from(result);
                    } else {
                        return Err(self.error(WasmRuntimeErrorKind::TypeMismatch, code));
                    }
                }
                Ok(())
            })
        } else {
            Err(self.error(WasmRuntimeErrorKind::NoMethod, code))
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
            .ok_or(WasmRuntimeError::from(WasmRuntimeErrorKind::NoMethod))?;

        let local_len = usize::max(
            INITIAL_VALUE_STACK_SIZE,
            function.param_types().len() + code_block.local_types().len(),
        );
        let mut locals = Vec::with_capacity(local_len);
        locals.resize(local_len, WasmUnsafeValue::zero());

        for (index, param_type) in function.param_types().iter().enumerate() {
            let param = params.get(index).ok_or(WasmRuntimeError::from(
                WasmRuntimeErrorKind::InvalidParameter,
            ))?;
            if !param.is_valid_type(*param_type) {
                return Err(WasmRuntimeErrorKind::InvalidParameter.into());
            }
            unsafe {
                *locals.get_unchecked_mut(index) = WasmUnsafeValue::from(param.clone());
            }
        }

        let result_types = function.result_types();

        let mut interp = WasmInterpreter::new(self.module());
        interp.invoke(
            function.index(),
            code_block,
            locals.as_mut_slice(),
            result_types,
        )
    }
}

pub struct WasmRuntimeError {
    kind: WasmRuntimeErrorKind,
    file_position: usize,
    function: usize,
    function_name: Option<String>,
    position: usize,
    opcode: WasmOpcode,
}

impl WasmRuntimeError {
    #[inline]
    pub const fn kind(&self) -> WasmRuntimeErrorKind {
        self.kind
    }

    #[inline]
    pub const fn file_position(&self) -> usize {
        self.file_position
    }

    #[inline]
    pub const fn function(&self) -> usize {
        self.function
    }

    #[inline]
    pub fn function_name(&self) -> Option<&str> {
        self.function_name.as_ref().map(|v| v.as_str())
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

impl From<WasmRuntimeErrorKind> for WasmRuntimeError {
    #[inline]
    fn from(kind: WasmRuntimeErrorKind) -> Self {
        Self {
            kind,
            file_position: 0,
            function: 0,
            function_name: None,
            position: 0,
            opcode: WasmOpcode::Unreachable,
        }
    }
}

impl fmt::Debug for WasmRuntimeError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let opcode = self.opcode();
        if let Some(function_name) = self.function_name() {
            write!(
                f,
                "{:?} (at 0x{:x} [${}:{}] opcode {:02x} {} function {})",
                self.kind(),
                self.file_position(),
                self.function(),
                self.position(),
                opcode as usize,
                opcode.to_str(),
                function_name,
            )
        } else {
            write!(
                f,
                "{:?} (at 0x{:x} [${}:{}] opcode {:02x} {})",
                self.kind(),
                self.file_position(),
                self.function(),
                self.position(),
                opcode as usize,
                opcode.to_str(),
            )
        }
    }
}
