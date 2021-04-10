// Wasm Intermediate Code Interpreter

use super::{intcode::*, stack::*, wasm::*};
use crate::*;
use alloc::vec::Vec;
use core::fmt::Debug;

type StackType = usize;

/// Wasm Intermediate Code
#[derive(Debug, Clone, Copy)]
pub struct WasmImc {
    pub position: usize,
    pub mnemonic: WasmIntMnemonic,
    pub stack_level: StackType,
    pub param1: u64,
}

impl WasmImc {
    #[inline]
    pub fn from_mnemonic(mnemonic: WasmIntMnemonic) -> Self {
        Self {
            position: 0,
            mnemonic,
            stack_level: StackType::default(),
            param1: 0,
        }
    }

    #[inline]
    pub const fn new(
        position: usize,
        opcode: WasmIntMnemonic,
        stack_level: usize,
        param1: u64,
    ) -> Self {
        Self {
            position,
            mnemonic: opcode,
            stack_level: stack_level as StackType,
            param1,
        }
    }

    #[inline]
    pub const fn position(&self) -> usize {
        self.position
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
    last_postion: usize,
    last_code: WasmImc,
}

impl<'a> WasmInterpreter<'a> {
    #[inline]
    pub fn new(module: &'a WasmModule) -> Self {
        Self {
            module,
            func_index: 0,
            last_postion: 0,
            last_code: WasmImc::from_mnemonic(WasmIntMnemonic::Unreachable),
        }
    }
}

impl WasmInterpreter<'_> {
    pub fn invoke(
        &mut self,
        func_index: usize,
        info: &WasmBlockInfo,
        locals: &[WasmStackValue],
        result_types: &[WasmValType],
    ) -> Result<WasmValue, WasmIntrError> {
        let mut stack = SharedStack::with_capacity(0x10000);

        let mut locals = {
            let output = stack.alloc(locals.len());
            output.copy_from_slice(locals);
            output
        };

        self.func_index = func_index;

        self.interpret(info, &mut locals, result_types, &mut stack)
            .map_err(|v| WasmIntrError {
                kind: v,
                function: self.func_index,
                code: self.last_code,
            })
    }

    fn interpret(
        &mut self,
        info: &WasmBlockInfo,
        locals: &mut [WasmStackValue],
        result_types: &[WasmValType],
        stack: &mut SharedStack,
    ) -> Result<WasmValue, WasmRuntimeError> {
        let mut codes = WasmIntermediateCodeBlock::from_codes(info.intermediate_codes());

        let value_stack = stack.alloc(info.max_stack());
        for value in value_stack.iter_mut() {
            *value = WasmStackValue::zero();
        }

        let mut result_stack_level = 0;

        while let Some(code) = codes.fetch() {
            // self.last_postion = code.position();
            // self.last_code = code;
            match code.mnemonic() {
                WasmIntMnemonic::Unreachable => return Err(WasmRuntimeError::Unreachable),

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
                    let ext_params = info.ext_params();
                    let table_position = code.param1() as usize;
                    let table_len = ext_params[table_position] - 1;
                    if index >= table_len {
                        index = table_len;
                    }
                    let target = ext_params[table_position + index + 1];
                    codes.set_position(target);
                }

                WasmIntMnemonic::Return => {
                    result_stack_level = code.stack_level();
                    break;
                }

                WasmIntMnemonic::Call => {
                    let func = self
                        .module
                        .functions()
                        .get(code.param1() as usize)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    self.call(func, code.stack_level(), value_stack, stack)?;
                }
                WasmIntMnemonic::CallIndirect => {
                    let stack_level = code.stack_level();
                    let type_index = code.param1() as usize;
                    let index = value_stack
                        .get(stack_level)
                        .map(|v| v.get_i32() as usize)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let func = self
                        .module
                        .elem_by_index(index)
                        .ok_or(WasmRuntimeError::NoMethod)?;
                    if func.type_index() != type_index {
                        return Err(WasmRuntimeError::TypeMismatch);
                    }
                    self.call(func, stack_level, value_stack, stack)?;
                }

                WasmIntMnemonic::Select => {
                    let stack_level = code.stack_level();
                    let cc = value_stack
                        .get(stack_level + 2)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?
                        .get_bool();
                    if !cc {
                        let b = *value_stack
                            .get(stack_level + 1)
                            .ok_or(WasmRuntimeError::InternalInconsistency)?;
                        let ref_a = value_stack
                            .get_mut(stack_level)
                            .ok_or(WasmRuntimeError::InternalInconsistency)?;
                        *ref_a = b;
                    }
                }

                WasmIntMnemonic::LocalGet => {
                    let local = locals
                        .get(code.param1() as usize)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let ref_a = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *ref_a = *local;
                }
                WasmIntMnemonic::LocalSet | WasmIntMnemonic::LocalTee => {
                    let local = locals
                        .get_mut(code.param1() as usize)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .get(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *local = *a;
                }

                WasmIntMnemonic::GlobalGet => {
                    let global = self
                        .module
                        .global(code.param1() as usize)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?
                        .value()
                        .try_borrow()
                        .map_err(|_| WasmRuntimeError::InternalInconsistency)?;
                    let ref_a = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *ref_a = WasmStackValue::from(*global);
                }
                WasmIntMnemonic::GlobalSet => {
                    let global = self
                        .module
                        .global(code.param1() as usize)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let mut var = global
                        .value()
                        .try_borrow_mut()
                        .map_err(|_| WasmRuntimeError::InternalInconsistency)?;
                    let ref_a = value_stack
                        .get(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *var = ref_a.into_value(global.val_type())
                }

                WasmIntMnemonic::I32Load => {
                    let memory = self.module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = WasmStackValue::from(memory.read_u32(offset)?);
                }
                WasmIntMnemonic::I32Load8S => {
                    let memory = self.module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = WasmStackValue::from(memory.read_u8(offset)? as i8 as i32);
                }
                WasmIntMnemonic::I32Load8U => {
                    let memory = self.module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = WasmStackValue::from(memory.read_u8(offset)? as u32);
                }
                WasmIntMnemonic::I32Load16S => {
                    let memory = self.module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = WasmStackValue::from(memory.read_u16(offset)? as i16 as i32);
                }
                WasmIntMnemonic::I32Load16U => {
                    let memory = self.module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = WasmStackValue::from(memory.read_u16(offset)? as u32);
                }

                WasmIntMnemonic::I64Load => {
                    let memory = self.module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = WasmStackValue::from(memory.read_u64(offset)?);
                }
                WasmIntMnemonic::I64Load8S => {
                    let memory = self.module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = WasmStackValue::from(memory.read_u8(offset)? as i8 as i64);
                }
                WasmIntMnemonic::I64Load8U => {
                    let memory = self.module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = WasmStackValue::from(memory.read_u8(offset)? as u64);
                }
                WasmIntMnemonic::I64Load16S => {
                    let memory = self.module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = WasmStackValue::from(memory.read_u16(offset)? as i16 as i64);
                }
                WasmIntMnemonic::I64Load16U => {
                    let memory = self.module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = WasmStackValue::from(memory.read_u16(offset)? as u64);
                }
                WasmIntMnemonic::I64Load32S => {
                    let memory = self.module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = WasmStackValue::from(memory.read_u32(offset)? as i32 as i64);
                }
                WasmIntMnemonic::I64Load32U => {
                    let memory = self.module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let offset = code.param1() as usize + var.get_u32() as usize;
                    *var = WasmStackValue::from(memory.read_u32(offset)? as u64);
                }

                WasmIntMnemonic::I64Store32 | WasmIntMnemonic::I32Store => {
                    let stack_level = code.stack_level();
                    let memory = self.module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let index = value_stack
                        .get(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?
                        .get_u32() as usize;
                    let data = value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?
                        .get_u32();
                    let offset = code.param1() as usize + index;
                    memory.write_u32(offset, data)?;
                }
                WasmIntMnemonic::I64Store8 | WasmIntMnemonic::I32Store8 => {
                    let stack_level = code.stack_level();
                    let memory = self.module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let index = value_stack
                        .get(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?
                        .get_u32() as usize;
                    let data = value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?
                        .get_u8();
                    let offset = code.param1() as usize + index;
                    memory.write_u8(offset, data)?;
                }
                WasmIntMnemonic::I64Store16 | WasmIntMnemonic::I32Store16 => {
                    let stack_level = code.stack_level();
                    let memory = self.module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let index = value_stack
                        .get(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?
                        .get_u32() as usize;
                    let data = value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?
                        .get_u16();
                    let offset = code.param1() as usize + index;
                    memory.write_u16(offset, data)?;
                }
                WasmIntMnemonic::I64Store => {
                    let stack_level = code.stack_level();
                    let memory = self.module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let index = value_stack
                        .get(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?
                        .get_u32() as usize;
                    let data = value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?
                        .get_u64();
                    let offset = code.param1() as usize + index;
                    memory.write_u64(offset, data)?;
                }

                WasmIntMnemonic::MemorySize => {
                    let memory = self.module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    value_stack
                        .get_mut(code.stack_level())
                        .map(|v| *v = WasmStackValue::from(memory.size()))
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                }
                WasmIntMnemonic::MemoryGrow => {
                    let memory = self.module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *var = WasmStackValue::from(memory.grow(var.get_u32() as usize) as u32);
                }

                WasmIntMnemonic::I32Const => {
                    *value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)? =
                        WasmStackValue::from_u32(code.param1() as u32);
                }
                WasmIntMnemonic::I64Const => {
                    *value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)? =
                        WasmStackValue::from_u64(code.param1());
                }

                WasmIntMnemonic::I32Eqz => {
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *var = WasmStackValue::from_bool(var.get_i32() == 0);
                }
                WasmIntMnemonic::I32Eq => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *lhs = WasmStackValue::from(lhs.get_u32() == rhs.get_u32());
                }
                WasmIntMnemonic::I32Ne => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *lhs = WasmStackValue::from(lhs.get_u32() != rhs.get_u32());
                }
                WasmIntMnemonic::I32LtS => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *lhs = WasmStackValue::from(lhs.get_i32() < rhs.get_i32());
                }
                WasmIntMnemonic::I32LtU => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *lhs = WasmStackValue::from(lhs.get_u32() < rhs.get_u32());
                }
                WasmIntMnemonic::I32GtS => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *lhs = WasmStackValue::from(lhs.get_i32() > rhs.get_i32());
                }
                WasmIntMnemonic::I32GtU => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *lhs = WasmStackValue::from(lhs.get_u32() > rhs.get_u32());
                }
                WasmIntMnemonic::I32LeS => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *lhs = WasmStackValue::from(lhs.get_i32() <= rhs.get_i32());
                }
                WasmIntMnemonic::I32LeU => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *lhs = WasmStackValue::from(lhs.get_u32() <= rhs.get_u32());
                }
                WasmIntMnemonic::I32GeS => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *lhs = WasmStackValue::from(lhs.get_i32() >= rhs.get_i32());
                }
                WasmIntMnemonic::I32GeU => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *lhs = WasmStackValue::from(lhs.get_u32() >= rhs.get_u32());
                }

                WasmIntMnemonic::I32Clz => {
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    var.map_u32(|v| v.leading_zeros());
                }
                WasmIntMnemonic::I32Ctz => {
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    var.map_u32(|v| v.trailing_zeros());
                }
                WasmIntMnemonic::I32Popcnt => {
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    var.map_u32(|v| v.count_ones());
                }
                WasmIntMnemonic::I32Add => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_i32(|lhs| lhs.wrapping_add(rhs.get_i32()));
                }
                WasmIntMnemonic::I32Sub => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_i32(|lhs| lhs.wrapping_sub(rhs.get_i32()));
                }
                WasmIntMnemonic::I32Mul => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_i32(|lhs| lhs.wrapping_mul(rhs.get_i32()));
                }
                WasmIntMnemonic::I32DivS => {
                    let stack_level = code.stack_level();
                    let rhs = value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?
                        .get_i32();
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    if rhs == 0 {
                        return Err(WasmRuntimeError::DivideByZero);
                    }
                    lhs.map_i32(|lhs| lhs.wrapping_div(rhs));
                }
                WasmIntMnemonic::I32DivU => {
                    let stack_level = code.stack_level();
                    let rhs = value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?
                        .get_u32();
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    if rhs == 0 {
                        return Err(WasmRuntimeError::DivideByZero);
                    }
                    lhs.map_u32(|lhs| lhs.wrapping_div(rhs));
                }
                WasmIntMnemonic::I32RemS => {
                    let stack_level = code.stack_level();
                    let rhs = value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?
                        .get_i32();
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    if rhs == 0 {
                        return Err(WasmRuntimeError::DivideByZero);
                    }
                    lhs.map_i32(|lhs| lhs.wrapping_rem(rhs));
                }
                WasmIntMnemonic::I32RemU => {
                    let stack_level = code.stack_level();
                    let rhs = value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?
                        .get_u32();
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    if rhs == 0 {
                        return Err(WasmRuntimeError::DivideByZero);
                    }
                    lhs.map_u32(|lhs| lhs.wrapping_rem(rhs));
                }
                WasmIntMnemonic::I32And => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_u32(|lhs| lhs & rhs.get_u32());
                }
                WasmIntMnemonic::I32Or => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_u32(|lhs| lhs | rhs.get_u32());
                }
                WasmIntMnemonic::I32Xor => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_u32(|lhs| lhs ^ rhs.get_u32());
                }
                WasmIntMnemonic::I32Shl => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_u32(|lhs| lhs << rhs.get_u32());
                }
                WasmIntMnemonic::I32ShrS => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_i32(|lhs| lhs >> rhs.get_i32());
                }
                WasmIntMnemonic::I32ShrU => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_u32(|lhs| lhs >> rhs.get_u32());
                }
                WasmIntMnemonic::I32Rotl => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_u32(|lhs| lhs.rotate_left(rhs.get_u32()));
                }
                WasmIntMnemonic::I32Rotr => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_u32(|lhs| lhs.rotate_right(rhs.get_u32()));
                }

                WasmIntMnemonic::I64Eqz => {
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *var = WasmStackValue::from_bool(var.get_i64() == 0);
                }
                WasmIntMnemonic::I64Eq => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *lhs = WasmStackValue::from(lhs.get_u64() == rhs.get_u64());
                }
                WasmIntMnemonic::I64Ne => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *lhs = WasmStackValue::from(lhs.get_u64() != rhs.get_u64());
                }
                WasmIntMnemonic::I64LtS => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *lhs = WasmStackValue::from(lhs.get_i64() < rhs.get_i64());
                }
                WasmIntMnemonic::I64LtU => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *lhs = WasmStackValue::from(lhs.get_u64() < rhs.get_u64());
                }
                WasmIntMnemonic::I64GtS => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *lhs = WasmStackValue::from(lhs.get_i64() > rhs.get_i64());
                }
                WasmIntMnemonic::I64GtU => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *lhs = WasmStackValue::from(lhs.get_u64() > rhs.get_u64());
                }
                WasmIntMnemonic::I64LeS => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *lhs = WasmStackValue::from(lhs.get_i64() <= rhs.get_i64());
                }
                WasmIntMnemonic::I64LeU => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *lhs = WasmStackValue::from(lhs.get_u64() <= rhs.get_u64());
                }
                WasmIntMnemonic::I64GeS => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *lhs = WasmStackValue::from(lhs.get_i64() >= rhs.get_i64());
                }
                WasmIntMnemonic::I64GeU => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    *lhs = WasmStackValue::from(lhs.get_u64() >= rhs.get_u64());
                }

                WasmIntMnemonic::I64Clz => {
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    var.map_u64(|v| v.leading_zeros() as u64);
                }
                WasmIntMnemonic::I64Ctz => {
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    var.map_u64(|v| v.trailing_zeros() as u64);
                }
                WasmIntMnemonic::I64Popcnt => {
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    var.map_u64(|v| v.count_ones() as u64);
                }
                WasmIntMnemonic::I64Add => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_i64(|lhs| lhs.wrapping_add(rhs.get_i64()));
                }
                WasmIntMnemonic::I64Sub => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_i64(|lhs| lhs.wrapping_sub(rhs.get_i64()));
                }
                WasmIntMnemonic::I64Mul => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_i64(|lhs| lhs.wrapping_mul(rhs.get_i64()));
                }
                WasmIntMnemonic::I64DivS => {
                    let stack_level = code.stack_level();
                    let rhs = value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?
                        .get_i64();
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    if rhs == 0 {
                        return Err(WasmRuntimeError::DivideByZero);
                    }
                    lhs.map_i64(|lhs| lhs.wrapping_div(rhs));
                }
                WasmIntMnemonic::I64DivU => {
                    let stack_level = code.stack_level();
                    let rhs = value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?
                        .get_u64();
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    if rhs == 0 {
                        return Err(WasmRuntimeError::DivideByZero);
                    }
                    lhs.map_u64(|lhs| lhs.wrapping_div(rhs));
                }
                WasmIntMnemonic::I64RemS => {
                    let stack_level = code.stack_level();
                    let rhs = value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?
                        .get_i64();
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    if rhs == 0 {
                        return Err(WasmRuntimeError::DivideByZero);
                    }
                    lhs.map_i64(|lhs| lhs.wrapping_rem(rhs));
                }
                WasmIntMnemonic::I64RemU => {
                    let stack_level = code.stack_level();
                    let rhs = value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?
                        .get_u64();
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    if rhs == 0 {
                        return Err(WasmRuntimeError::DivideByZero);
                    }
                    lhs.map_u64(|lhs| lhs.wrapping_rem(rhs));
                }
                WasmIntMnemonic::I64And => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_u64(|lhs| lhs & rhs.get_u64());
                }
                WasmIntMnemonic::I64Or => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_u64(|lhs| lhs | rhs.get_u64());
                }
                WasmIntMnemonic::I64Xor => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_u64(|lhs| lhs ^ rhs.get_u64());
                }
                WasmIntMnemonic::I64Shl => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_u64(|lhs| lhs << rhs.get_u64());
                }
                WasmIntMnemonic::I64ShrS => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_i64(|lhs| lhs >> rhs.get_i64());
                }
                WasmIntMnemonic::I64ShrU => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_u64(|lhs| lhs >> rhs.get_u64());
                }
                WasmIntMnemonic::I64Rotl => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_u64(|lhs| lhs.rotate_left(rhs.get_u32()));
                }
                WasmIntMnemonic::I64Rotr => {
                    let stack_level = code.stack_level();
                    let rhs = *value_stack
                        .get(stack_level + 1)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let lhs = value_stack
                        .get_mut(stack_level)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_u64(|lhs| lhs.rotate_right(rhs.get_u32()));
                }

                WasmIntMnemonic::I64Extend8S => {
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *var = WasmStackValue::from_i64(var.get_i8() as i64);
                }
                WasmIntMnemonic::I64Extend16S => {
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *var = WasmStackValue::from_i64(var.get_i16() as i64);
                }
                WasmIntMnemonic::I64Extend32S | WasmIntMnemonic::I64ExtendI32S => {
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *var = WasmStackValue::from_i64(var.get_i32() as i64);
                }
                WasmIntMnemonic::I64ExtendI32U => {
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *var = WasmStackValue::from_u64(var.get_u32() as u64);
                }
                WasmIntMnemonic::I32WrapI64 => {
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *var = WasmStackValue::from_i32(var.get_i64() as i32);
                }
                WasmIntMnemonic::I32Extend8S => {
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *var = WasmStackValue::from_i32(var.get_i8() as i32);
                }
                WasmIntMnemonic::I32Extend16S => {
                    let var = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *var = WasmStackValue::from_i32(var.get_i16() as i32);
                }

                WasmIntMnemonic::FusedI32AddI => {
                    let lhs = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_i32(|lhs| lhs.wrapping_add(code.param1() as i32));
                }
                WasmIntMnemonic::FusedI32SubI => {
                    let lhs = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_i32(|lhs| lhs.wrapping_sub(code.param1() as i32));
                }
                WasmIntMnemonic::FusedI64AddI => {
                    let lhs = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_i64(|lhs| lhs.wrapping_add(code.param1() as i64));
                }
                WasmIntMnemonic::FusedI64SubI => {
                    let lhs = value_stack
                        .get_mut(code.stack_level())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;

                    lhs.map_i64(|lhs| lhs.wrapping_sub(code.param1() as i64));
                }
                WasmIntMnemonic::FusedI32BrZ => {
                    let cc = value_stack[code.stack_level()].get_i32() == 0;
                    if cc {
                        let br = code.param1() as usize;
                        codes.set_position(br);
                    }
                }
                WasmIntMnemonic::FusedI64BrZ => {
                    let cc = value_stack[code.stack_level()].get_i64() == 0;
                    if cc {
                        let br = code.param1() as usize;
                        codes.set_position(br);
                    }
                }

                #[allow(unreachable_patterns)]
                _ => return Err(WasmRuntimeError::InvalidBytecode),
            }
        }
        if let Some(result_type) = result_types.first() {
            let val = value_stack
                .get(result_stack_level)
                .ok_or(WasmRuntimeError::InternalInconsistency)?;
            match result_type {
                WasmValType::I32 => Ok(WasmValue::I32(val.get_i32())),
                WasmValType::I64 => Ok(WasmValue::I64(val.get_i64())),
                // WasmValType::F32 => {}
                // WasmValType::F64 => {}
                _ => Err(WasmRuntimeError::InvalidParameter),
            }
        } else {
            Ok(WasmValue::Empty)
        }
    }

    #[inline]
    fn call(
        &mut self,
        func: &WasmFunction,
        stack_pointer: usize,
        value_stack: &mut [WasmStackValue],
        stack: &mut SharedStack,
    ) -> Result<(), WasmRuntimeError> {
        let current_function = self.func_index;
        let module = self.module;
        let result_types = func.result_types();

        let param_len = func.param_types().len();
        if stack_pointer < param_len {
            return Err(WasmRuntimeError::InternalInconsistency);
        }

        if let Some(body) = func.body() {
            stack.snapshot(|stack| {
                let info = body.block_info();
                let mut locals = stack.alloc_stack(param_len + body.local_types().len());
                let stack_under = stack_pointer - param_len;

                locals.extend_from_slice(&value_stack[stack_under..stack_under + param_len]);
                for _ in body.local_types() {
                    locals
                        .push(WasmStackValue::zero())
                        .map_err(|_| WasmRuntimeError::InternalInconsistency)?;
                }

                self.func_index = func.index();
                let result = self.interpret(info, locals.as_mut_slice(), result_types, stack)?;
                if !result.is_empty() {
                    let var = value_stack
                        .get_mut(stack_under)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *var = WasmStackValue::from(result);
                }
                self.func_index = current_function;
                Ok(())
            })
        } else if let Some(dlink) = func.dlink() {
            stack.snapshot(|stack| {
                let mut locals = stack.alloc_stack(param_len);
                let stack_under = stack_pointer - param_len;
                let params = &value_stack[stack_under..stack_under + param_len];
                for (index, val_type) in func.param_types().iter().enumerate() {
                    locals
                        .push(params[index].get_by_type(*val_type))
                        .map_err(|_| WasmRuntimeError::InternalInconsistency)?;
                }

                let result = dlink(module, locals.as_slice())?;

                if let Some(t) = result_types.first() {
                    if result.is_valid_type(*t) {
                        let var = value_stack
                            .get_mut(stack_under)
                            .ok_or(WasmRuntimeError::InternalInconsistency)?;
                        *var = WasmStackValue::from(result);
                    } else {
                        return Err(WasmRuntimeError::TypeMismatch);
                    }
                }
                Ok(())
            })
        } else {
            return Err(WasmRuntimeError::NoMethod);
        }
    }
}

struct WasmIntermediateCodeBlock<'a> {
    codes: &'a [WasmImc],
    position: usize,
}

impl<'a> WasmIntermediateCodeBlock<'a> {
    #[inline]
    fn from_codes(codes: &'a [WasmImc]) -> Self {
        Self { codes, position: 0 }
    }
}

impl WasmIntermediateCodeBlock<'_> {
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
    fn invoke(&self, params: &[WasmValue]) -> Result<WasmValue, WasmIntrError>;
}

impl WasmInvocation for WasmRunnable<'_> {
    fn invoke(&self, params: &[WasmValue]) -> Result<WasmValue, WasmIntrError> {
        let function = self.function();
        let body = function
            .body()
            .ok_or(WasmIntrError::from(WasmRuntimeError::NoMethod))?;

        let mut locals =
            Vec::with_capacity(function.param_types().len() + body.local_types().len());
        for (index, param_type) in function.param_types().iter().enumerate() {
            let param = params
                .get(index)
                .ok_or(WasmIntrError::from(WasmRuntimeError::InvalidParameter))?;
            if !param.is_valid_type(*param_type) {
                return Err(WasmRuntimeError::InvalidParameter.into());
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

#[derive(Debug)]
pub struct WasmIntrError {
    kind: WasmRuntimeError,
    function: usize,
    code: WasmImc,
}

impl WasmIntrError {
    #[inline]
    pub const fn kind(&self) -> WasmRuntimeError {
        self.kind
    }
}

impl From<WasmRuntimeError> for WasmIntrError {
    #[inline]
    fn from(kind: WasmRuntimeError) -> Self {
        Self {
            kind,
            function: 0,
            code: WasmImc::from_mnemonic(WasmIntMnemonic::Unreachable),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::{WasmInterpreter, WasmInvocation};
    use crate::wasm::{
        Leb128Stream, WasmBlockInfo, WasmDecodeError, WasmLoader, WasmModule, WasmValType,
    };

    #[test]
    fn add() {
        let slice = [0x20, 0, 0x20, 1, 0x6A, 0x0B];
        let local_types = [WasmValType::I32, WasmValType::I32];
        let result_types = [WasmValType::I32];
        let mut stream = Leb128Stream::from_slice(&slice);
        let module = WasmModule::new();
        let info =
            WasmBlockInfo::analyze(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [1234.into(), 5678.into()];

        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 6912);

        let params = [0xDEADBEEFu32.into(), 0x55555555.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0x34031444);
    }

    #[test]
    fn sub() {
        let slice = [0x20, 0, 0x20, 1, 0x6B, 0x0B];
        let local_types = [WasmValType::I32, WasmValType::I32];
        let result_types = [WasmValType::I32];
        let mut stream = Leb128Stream::from_slice(&slice);
        let module = WasmModule::new();
        let info =
            WasmBlockInfo::analyze(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [1234.into(), 5678.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, -4444);

        let params = [0x55555555.into(), 0xDEADBEEFu32.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
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
            WasmBlockInfo::analyze(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [1234.into(), 5678.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 7006652);

        let params = [0x55555555.into(), 0xDEADBEEFu32.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
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
            WasmBlockInfo::analyze(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [7006652.into(), 5678.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 1234);

        let params = [42.into(), (-6).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, -7);

        let params = [(-42).into(), (6).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, -7);

        let params = [(-42).into(), (-6).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 7);
    }

    #[test]
    fn div_u() {
        let slice = [0x20, 0, 0x20, 1, 0x6E, 0x0B];
        let local_types = [WasmValType::I32, WasmValType::I32];
        let result_types = [WasmValType::I32];
        let mut stream = Leb128Stream::from_slice(&slice);
        let module = WasmModule::new();
        let info =
            WasmBlockInfo::analyze(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [7006652.into(), 5678.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 1234);

        let params = [42.into(), (-6).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0);

        let params = [(-42).into(), (6).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 715827875);
    }

    #[test]
    fn select() {
        let slice = [0x20, 0, 0x20, 1, 0x20, 2, 0x1B, 0x0B];
        let local_types = [WasmValType::I32, WasmValType::I32, WasmValType::I32];
        let result_types = [WasmValType::I32];
        let mut stream = Leb128Stream::from_slice(&slice);
        let module = WasmModule::new();
        let info =
            WasmBlockInfo::analyze(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [123.into(), 456.into(), 789.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 123);

        let params = [123.into(), 456.into(), 0.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
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
            WasmBlockInfo::analyze(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [123.into(), 456.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 1);

        let params = [123.into(), 123.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0);

        let params = [456.into(), 123.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0);

        let params = [123.into(), (-456).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0);

        let params = [456.into(), (-123).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
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
            WasmBlockInfo::analyze(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [123.into(), 456.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 1);

        let params = [123.into(), 123.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0);

        let params = [456.into(), 123.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0);

        let params = [123.into(), (-456).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 1);

        let params = [456.into(), (-123).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
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
            WasmBlockInfo::analyze(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [123.into(), 456.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 1);

        let params = [123.into(), 123.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 1);

        let params = [456.into(), 123.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0);

        let params = [123.into(), (-456).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0);

        let params = [456.into(), (-123).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
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
            WasmBlockInfo::analyze(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [0.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 123);

        let params = [1.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 456);

        let params = [2.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 789);

        let params = [3.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 789);

        let params = [4.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 789);

        let params = [5.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 789);

        let params = [(-1).into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
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
            WasmBlockInfo::analyze(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut interp = WasmInterpreter::new(&module);

        let params = [7.into(), 0.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 5040);

        let params = [10.into(), 0.into()];
        let result = interp
            .invoke(0, &info, &params, &result_types)
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
            WasmLoader::instantiate(&slice, |_, _, _| Err(WasmDecodeError::DynamicLinkError))
                .unwrap();
        let runnable = module.func_by_index(0).unwrap();

        let result = runnable.invoke(&[5.into()]).unwrap().get_i32().unwrap();
        assert_eq!(result, 5);

        let result = runnable.invoke(&[10.into()]).unwrap().get_i32().unwrap();
        assert_eq!(result, 55);

        let result = runnable.invoke(&[20.into()]).unwrap().get_i32().unwrap();
        assert_eq!(result, 6765);
    }
}
