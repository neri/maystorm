//! WebAssembly Intermediate Code Interpreter

use super::{intcode::*, opcode::WasmOpcode, stack::*, wasm::*};
use alloc::{borrow::ToOwned, string::String, vec::Vec};
use core::fmt;

const INITIAL_VALUE_STACK_SIZE: usize = 512;

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
            match *code.mnemonic() {
                WasmIntMnemonic::Unreachable
                | WasmIntMnemonic::Nop
                | WasmIntMnemonic::Undefined
                | WasmIntMnemonic::Block(_)
                | WasmIntMnemonic::End(_) => {
                    // Currently, NOP is unreachable
                    return Err(self.error(WasmRuntimeErrorKind::Unreachable, code));
                }

                WasmIntMnemonic::Br(target) => {
                    codes.set_position(target);
                }
                WasmIntMnemonic::BrIf(target) => {
                    let cc = unsafe { value_stack.get_unchecked(code.stack_level()).get_bool() };
                    if cc {
                        codes.set_position(target);
                    }
                }
                WasmIntMnemonic::BrTable(ref table) => {
                    let table_len = table.len() - 1;
                    let index = usize::min(table_len, unsafe {
                        value_stack.get_unchecked(code.stack_level()).get_u32() as usize
                    });
                    let target = unsafe { *table.get_unchecked(index) };
                    codes.set_position(target);
                }

                WasmIntMnemonic::Return => {
                    // last_code = *code;
                    result_stack_level = code.stack_level();
                    break;
                }

                WasmIntMnemonic::Call(func_index) => {
                    let func = unsafe { self.module.functions().get_unchecked(func_index) };
                    self.call(func, code, value_stack, heap)?;
                }
                WasmIntMnemonic::CallIndirect(type_index) => {
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

                WasmIntMnemonic::LocalGet32(local_index) => {
                    let local = unsafe { locals.get_unchecked(local_index) };
                    let ref_a = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        ref_a.copy_from_i32(local);
                    }
                }
                WasmIntMnemonic::LocalGet(local_index) => {
                    let local = unsafe { locals.get_unchecked(local_index) };
                    let ref_a = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *ref_a = *local;
                }
                WasmIntMnemonic::LocalSet32(local_index)
                | WasmIntMnemonic::LocalTee32(local_index) => {
                    let local = unsafe { locals.get_unchecked_mut(local_index) };
                    let ref_a = unsafe { value_stack.get_unchecked(code.stack_level()) };
                    unsafe {
                        local.copy_from_i32(ref_a);
                    }
                }
                WasmIntMnemonic::LocalSet(local_index) | WasmIntMnemonic::LocalTee(local_index) => {
                    let local = unsafe { locals.get_unchecked_mut(local_index) };
                    let ref_a = unsafe { value_stack.get_unchecked(code.stack_level()) };
                    *local = *ref_a;
                }

                WasmIntMnemonic::GlobalGet(global_ref) => {
                    let global = unsafe { self.module.globals().get_unchecked(global_ref) };
                    let ref_a = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *ref_a = global.value().into();
                }
                WasmIntMnemonic::GlobalSet(global_ref) => {
                    let global = unsafe { self.module.globals().get_unchecked(global_ref) };
                    let ref_a = unsafe { value_stack.get_unchecked(code.stack_level()) };
                    global.set_value(*ref_a);
                }

                WasmIntMnemonic::I32Load(offset) => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = memory
                        .read_u32(offset, unsafe { var.get_u32() })
                        .map(|v| WasmUnsafeValue::from(v))
                        .map_err(|e| self.error(e, code))?;
                }
                WasmIntMnemonic::I32Load8S(offset) => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = memory
                        .read_u8(offset, unsafe { var.get_u32() })
                        .map(|v| WasmUnsafeValue::from(v as i8 as i32))
                        .map_err(|e| self.error(e, code))?;
                }
                WasmIntMnemonic::I32Load8U(offset) => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = memory
                        .read_u8(offset, unsafe { var.get_u32() })
                        .map(|v| WasmUnsafeValue::from(v as u32))
                        .map_err(|e| self.error(e, code))?;
                }
                WasmIntMnemonic::I32Load16S(offset) => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = memory
                        .read_u16(offset, unsafe { var.get_u32() })
                        .map(|v| WasmUnsafeValue::from(v as i16 as i32))
                        .map_err(|e| self.error(e, code))?;
                }
                WasmIntMnemonic::I32Load16U(offset) => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = memory
                        .read_u16(offset, unsafe { var.get_u32() })
                        .map(|v| WasmUnsafeValue::from(v as u32))
                        .map_err(|e| self.error(e, code))?;
                }

                WasmIntMnemonic::I64Load(offset) => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = memory
                        .read_u64(offset, unsafe { var.get_u32() })
                        .map(|v| WasmUnsafeValue::from(v))
                        .map_err(|e| self.error(e, code))?;
                }
                WasmIntMnemonic::I64Load8S(offset) => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = memory
                        .read_u8(offset, unsafe { var.get_u32() })
                        .map(|v| WasmUnsafeValue::from(v as i8 as i64))
                        .map_err(|e| self.error(e, code))?;
                }
                WasmIntMnemonic::I64Load8U(offset) => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = memory
                        .read_u8(offset, unsafe { var.get_u32() })
                        .map(|v| WasmUnsafeValue::from(v as u64))
                        .map_err(|e| self.error(e, code))?;
                }
                WasmIntMnemonic::I64Load16S(offset) => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = memory
                        .read_u16(offset, unsafe { var.get_u32() })
                        .map(|v| WasmUnsafeValue::from(v as i16 as i64))
                        .map_err(|e| self.error(e, code))?;
                }
                WasmIntMnemonic::I64Load16U(offset) => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = memory
                        .read_u16(offset, unsafe { var.get_u32() })
                        .map(|v| WasmUnsafeValue::from(v as u64))
                        .map_err(|e| self.error(e, code))?;
                }
                WasmIntMnemonic::I64Load32S(offset) => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = memory
                        .read_u32(offset, unsafe { var.get_u32() })
                        .map(|v| WasmUnsafeValue::from(v as i32 as i64))
                        .map_err(|e| self.error(e, code))?;
                }
                WasmIntMnemonic::I64Load32U(offset) => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *var = memory
                        .read_u32(offset, unsafe { var.get_u32() })
                        .map(|v| WasmUnsafeValue::from(v as u64))
                        .map_err(|e| self.error(e, code))?;
                }

                WasmIntMnemonic::I64Store32(offset) | WasmIntMnemonic::I32Store(offset) => {
                    let stack_level = code.stack_level();
                    let index = unsafe { value_stack.get_unchecked(stack_level).get_u32() };
                    let data = unsafe { value_stack.get_unchecked(stack_level + 1).get_u32() };
                    memory
                        .write_u32(offset, index, data)
                        .map_err(|e| self.error(e, code))?;
                }
                WasmIntMnemonic::I64Store8(offset) | WasmIntMnemonic::I32Store8(offset) => {
                    let stack_level = code.stack_level();
                    let index = unsafe { value_stack.get_unchecked(stack_level).get_u32() };
                    let data = unsafe { value_stack.get_unchecked(stack_level + 1).get_u8() };
                    memory
                        .write_u8(offset, index, data)
                        .map_err(|e| self.error(e, code))?;
                }
                WasmIntMnemonic::I64Store16(offset) | WasmIntMnemonic::I32Store16(offset) => {
                    let stack_level = code.stack_level();
                    let index = unsafe { value_stack.get_unchecked(stack_level).get_u32() };
                    let data = unsafe { value_stack.get_unchecked(stack_level + 1).get_u16() };
                    memory
                        .write_u16(offset, index, data)
                        .map_err(|e| self.error(e, code))?;
                }
                WasmIntMnemonic::I64Store(offset) => {
                    let stack_level = code.stack_level();
                    let index = unsafe { value_stack.get_unchecked(stack_level).get_u32() };
                    let data = unsafe { value_stack.get_unchecked(stack_level + 1).get_u64() };
                    memory
                        .write_u64(offset, index, data)
                        .map_err(|e| self.error(e, code))?;
                }

                WasmIntMnemonic::MemorySize => {
                    let ref_a = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *ref_a = WasmUnsafeValue::from(memory.size());
                }
                WasmIntMnemonic::MemoryGrow => {
                    let ref_a = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    *ref_a = WasmUnsafeValue::from(memory.grow(unsafe { ref_a.get_i32() }));
                }

                WasmIntMnemonic::I32Const(val) => {
                    let ref_a = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        ref_a.write_i32(val);
                    }
                }
                WasmIntMnemonic::I64Const(val) => {
                    let ref_a = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        ref_a.write_i64(val);
                    }
                }

                WasmIntMnemonic::I32Eqz => {
                    let var = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe { var.write_bool(var.get_i32() == 0) }
                }
                WasmIntMnemonic::I32Eq => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe { lhs.write_bool(lhs.get_u32() == rhs.get_u32()) }
                }
                WasmIntMnemonic::I32Ne => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe { lhs.write_bool(lhs.get_u32() != rhs.get_u32()) }
                }
                WasmIntMnemonic::I32LtS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe { lhs.write_bool(lhs.get_i32() < rhs.get_i32()) }
                }
                WasmIntMnemonic::I32LtU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe { lhs.write_bool(lhs.get_u32() < rhs.get_u32()) }
                }
                WasmIntMnemonic::I32GtS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe { lhs.write_bool(lhs.get_i32() > rhs.get_i32()) }
                }
                WasmIntMnemonic::I32GtU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe { lhs.write_bool(lhs.get_u32() > rhs.get_u32()) }
                }
                WasmIntMnemonic::I32LeS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe { lhs.write_bool(lhs.get_i32() <= rhs.get_i32()) }
                }
                WasmIntMnemonic::I32LeU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe { lhs.write_bool(lhs.get_u32() <= rhs.get_u32()) }
                }
                WasmIntMnemonic::I32GeS => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe { lhs.write_bool(lhs.get_i32() >= rhs.get_i32()) }
                }
                WasmIntMnemonic::I32GeU => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { value_stack.get_unchecked_mut(stack_level) };
                    unsafe { lhs.write_bool(lhs.get_u32() >= rhs.get_u32()) }
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

                WasmIntMnemonic::FusedI32SetConst(local_index, val) => {
                    let local = unsafe { locals.get_unchecked_mut(local_index) };
                    unsafe {
                        local.write_i32(val);
                    }
                }
                WasmIntMnemonic::FusedI32AddI(val) => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        lhs.map_i32(|lhs| lhs.wrapping_add(val));
                    }
                }
                WasmIntMnemonic::FusedI32SubI(val) => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        lhs.map_i32(|lhs| lhs.wrapping_sub(val));
                    }
                }
                WasmIntMnemonic::FusedI32AndI(val) => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        lhs.map_i32(|lhs| lhs & val);
                    }
                }
                WasmIntMnemonic::FusedI32OrI(val) => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        lhs.map_i32(|lhs| lhs | val);
                    }
                }
                WasmIntMnemonic::FusedI32XorI(val) => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        lhs.map_i32(|lhs| lhs ^ val);
                    }
                }
                WasmIntMnemonic::FusedI32ShlI(val) => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        lhs.map_u32(|lhs| lhs << (val));
                    }
                }
                WasmIntMnemonic::FusedI32ShrUI(val) => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        lhs.map_u32(|lhs| lhs >> (val as u32));
                    }
                }
                WasmIntMnemonic::FusedI32ShrSI(val) => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        lhs.map_i32(|lhs| lhs >> val);
                    }
                }

                WasmIntMnemonic::FusedI64SetConst(local_index, val) => {
                    let local = unsafe { locals.get_unchecked_mut(local_index) };
                    *local = val.into();
                }
                WasmIntMnemonic::FusedI64AddI(val) => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        lhs.map_i64(|lhs| lhs.wrapping_add(val));
                    }
                }
                WasmIntMnemonic::FusedI64SubI(val) => {
                    let lhs = unsafe { value_stack.get_unchecked_mut(code.stack_level()) };
                    unsafe {
                        lhs.map_i64(|lhs| lhs.wrapping_sub(val));
                    }
                }

                WasmIntMnemonic::FusedI32BrZ(target) => {
                    let cc =
                        unsafe { value_stack.get_unchecked_mut(code.stack_level()).get_i32() == 0 };
                    if cc {
                        codes.set_position(target);
                    }
                }
                WasmIntMnemonic::FusedI32BrEq(target) => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { *value_stack.get_unchecked(stack_level) };
                    if unsafe { lhs.get_u32() == rhs.get_u32() } {
                        codes.set_position(target);
                    }
                }
                WasmIntMnemonic::FusedI32BrNe(target) => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { *value_stack.get_unchecked(stack_level) };
                    if unsafe { lhs.get_u32() != rhs.get_u32() } {
                        codes.set_position(target);
                    }
                }

                WasmIntMnemonic::FusedI64BrZ(target) => {
                    let cc =
                        unsafe { value_stack.get_unchecked_mut(code.stack_level()).get_i64() == 0 };
                    if cc {
                        codes.set_position(target);
                    }
                }
                WasmIntMnemonic::FusedI64BrEq(target) => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { *value_stack.get_unchecked(stack_level) };
                    if unsafe { lhs.get_u64() == rhs.get_u64() } {
                        codes.set_position(target);
                    }
                }
                WasmIntMnemonic::FusedI64BrNe(target) => {
                    let stack_level = code.stack_level();
                    let rhs = unsafe { *value_stack.get_unchecked(stack_level + 1) };
                    let lhs = unsafe { *value_stack.get_unchecked(stack_level) };
                    if unsafe { lhs.get_u64() != rhs.get_u64() } {
                        codes.set_position(target);
                    }
                } // _ => return Err(self.error(WasmRuntimeErrorKind::NotSupprted, code)),
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
        let stack_under = stack_pointer - param_len;
        // if stack_pointer < param_len {
        //     return Err(self.error(WasmRuntimeError::InternalInconsistency, code));
        // }

        if let Some(code_block) = target.code_block() {
            heap.snapshot(|heap| {
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
        } else if let Some(f) = target.dlink() {
            let (_, locals) = value_stack.split_at_mut(stack_under);
            let (locals, _) = locals.split_at_mut(param_len);

            let result = match f(module, locals) {
                Ok(v) => v,
                Err(e) => return Err(self.error(e, code)),
            };

            if let Some(t) = result_types.first() {
                if result.is_valid_type(*t) {
                    let var = match value_stack.get_mut(stack_under) {
                        Some(v) => v,
                        None => return Err(self.error(WasmRuntimeErrorKind::TypeMismatch, code)),
                    };
                    *var = WasmUnsafeValue::from(result);
                } else {
                    return Err(self.error(WasmRuntimeErrorKind::TypeMismatch, code));
                }
            }
            Ok(())
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
                "{:?} (at 0x{:x} [{}({}):{}] opcode {:02x} {})",
                self.kind(),
                self.file_position(),
                function_name,
                self.function(),
                self.position(),
                opcode as usize,
                opcode.to_str(),
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
