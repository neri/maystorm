// WebAssembly Interpreter

use super::opcode::*;
use super::wasm::*;
use crate::*;
use alloc::vec::Vec;

pub struct WasmInterpreter {}

impl WasmInterpreter {
    /// Interpret WebAssembly code blocks
    pub fn run(
        mut code_block: &mut WasmCodeBlock,
        locals: &[WasmValue],
        result_types: &[WasmValType],
        module: &WasmModule,
    ) -> Result<WasmValue, WasmRuntimeError> {
        let mut locals = {
            let mut output = Vec::with_capacity(locals.len());
            for local in locals {
                output.push(WasmStackValue::from(*local));
            }
            output
        };
        let mut value_stack: Vec<WasmStackValue> =
            Vec::with_capacity(code_block.info().max_stack());
        let mut block_stack = Vec::with_capacity(code_block.info().max_block_level());

        code_block.reset();
        loop {
            let opcode = code_block.read_opcode()?;

            // println!(
            //     "{}:{:04x} {:02x} {}",
            //     code_block.info().func_index(),
            //     code_block.fetch_position(),
            //     opcode as u8,
            //     opcode.to_str()
            // );

            match opcode {
                WasmOpcode::Nop => (),

                WasmOpcode::Block => {
                    let _ = code_block.read_unsigned()?;
                    block_stack.push(code_block.fetch_position());
                }
                WasmOpcode::Loop => {
                    let _ = code_block.read_unsigned()?;
                    block_stack.push(code_block.fetch_position());
                }
                WasmOpcode::If => {
                    let _ = code_block.read_unsigned()?;
                    let position = code_block.fetch_position();
                    let cc = value_stack
                        .pop()
                        .map(|v| v.get_bool())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    if cc {
                        block_stack.push(position);
                    } else {
                        let block = code_block
                            .info()
                            .block_info(position)
                            .ok_or(WasmRuntimeError::InternalInconsistency)?;
                        let end_position = block.end_position;
                        let else_position = block.else_position;
                        if else_position != 0 {
                            block_stack.push(position);
                            code_block.set_position(else_position);
                        } else {
                            code_block.set_position(end_position);
                        }
                    }
                }
                WasmOpcode::Else => {
                    Self::branch(0, &mut block_stack, &mut value_stack, &mut code_block)?;
                }
                WasmOpcode::End => {
                    if block_stack.pop().is_none() {
                        break;
                    }
                }
                WasmOpcode::Br => {
                    let target = code_block.read_unsigned()? as usize;
                    Self::branch(target, &mut block_stack, &mut value_stack, &mut code_block)?;
                }
                WasmOpcode::BrIf => {
                    let target = code_block.read_unsigned()? as usize;
                    let cc = value_stack
                        .pop()
                        .map(|v| v.get_bool())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    if cc {
                        Self::branch(target, &mut block_stack, &mut value_stack, &mut code_block)?;
                    }
                }
                WasmOpcode::BrTable => {
                    let mut index = value_stack
                        .pop()
                        .map(|v| v.get_i32() as usize)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let n_vec = code_block.read_unsigned()? as usize;
                    if index >= n_vec {
                        index = n_vec;
                    }
                    for _ in 1..=index {
                        let _ = code_block.read_unsigned()?;
                    }
                    let target = code_block.read_unsigned()? as usize;
                    Self::branch(target, &mut block_stack, &mut value_stack, &mut code_block)?;
                }

                WasmOpcode::Return => {
                    break;
                }

                WasmOpcode::Call => {
                    let index = code_block.read_unsigned()? as usize;
                    let func = module
                        .functions()
                        .get(index)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    Self::call(func, &mut value_stack, module)?;
                }
                WasmOpcode::CallIndirect => {
                    let type_index = code_block.read_unsigned()? as usize;
                    let _reserved = code_block.read_unsigned()? as usize;
                    let index = value_stack
                        .pop()
                        .map(|v| v.get_i32() as usize)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let func = module
                        .elem_by_index(index)
                        .ok_or(WasmRuntimeError::NoMethod)?;
                    if func.type_index() != type_index {
                        return Err(WasmRuntimeError::TypeMismatch);
                    }
                    Self::call(func, &mut value_stack, module)?;
                }

                WasmOpcode::Drop => {
                    let _ = value_stack.pop();
                }
                WasmOpcode::Select => {
                    let cc = value_stack
                        .pop()
                        .map(|v| v.get_bool())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let b = value_stack
                        .pop()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .pop()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let c = if cc { a } else { b };
                    value_stack.push(c);
                }

                WasmOpcode::LocalGet => {
                    let local_ref = code_block.read_unsigned()? as usize;
                    let val = *locals
                        .get(local_ref)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    value_stack.push(val.into());
                }
                WasmOpcode::LocalSet => {
                    let local_ref = code_block.read_unsigned()? as usize;
                    let var = locals
                        .get_mut(local_ref)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let val = value_stack
                        .pop()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *var = val;
                }
                WasmOpcode::LocalTee => {
                    let local_ref = code_block.read_unsigned()? as usize;
                    let var = locals
                        .get_mut(local_ref)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let val = *value_stack
                        .last()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *var = val;
                }

                WasmOpcode::GlobalGet => {
                    let global_ref = code_block.read_unsigned()? as usize;
                    let global = module
                        .global(global_ref)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let val = global
                        .value()
                        .try_borrow()
                        .map_err(|_| WasmRuntimeError::InternalInconsistency)?;
                    value_stack.push(WasmStackValue::from(*val));
                }
                WasmOpcode::GlobalSet => {
                    let global_ref = code_block.read_unsigned()? as usize;
                    let global = module
                        .global(global_ref)
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let val = value_stack
                        .pop()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let mut var = global
                        .value()
                        .try_borrow_mut()
                        .map_err(|_| WasmRuntimeError::WriteProtected)?;
                    *var = val.into_value(global.val_type());
                }

                WasmOpcode::I32Load => {
                    let memarg = code_block.read_memarg()?;
                    let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let offset = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let val = memory.read_u32(memarg.offset_by(offset))?;
                    value_stack.push(WasmStackValue::from(val))
                }
                WasmOpcode::I32Store => {
                    let memarg = code_block.read_memarg()?;
                    let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let val = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let offset = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    memory.write_u32(memarg.offset_by(offset), val)?;
                }
                WasmOpcode::I64Load => {
                    let memarg = code_block.read_memarg()?;
                    let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let offset = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let val = memory.read_u64(memarg.offset_by(offset))?;
                    value_stack.push(WasmStackValue::from(val))
                }
                WasmOpcode::I64Store => {
                    let memarg = code_block.read_memarg()?;
                    let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let val = value_stack
                        .pop()
                        .map(|v| v.get_u64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let offset = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    memory.write_u64(memarg.offset_by(offset), val)?;
                }

                WasmOpcode::I32Load8S => {
                    let memarg = code_block.read_memarg()?;
                    let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let offset = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let val = memory.read_u8(memarg.offset_by(offset))?;
                    value_stack.push(WasmStackValue::from(val as i8 as i32))
                }
                WasmOpcode::I32Load8U => {
                    let memarg = code_block.read_memarg()?;
                    let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let offset = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let val = memory.read_u8(memarg.offset_by(offset))?;
                    value_stack.push(WasmStackValue::from(val as u32))
                }
                WasmOpcode::I32Load16S => {
                    let memarg = code_block.read_memarg()?;
                    let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let offset = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let val = memory.read_u16(memarg.offset_by(offset))?;
                    value_stack.push(WasmStackValue::from(val as i16 as i32))
                }
                WasmOpcode::I32Load16U => {
                    let memarg = code_block.read_memarg()?;
                    let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let offset = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let val = memory.read_u16(memarg.offset_by(offset))?;
                    value_stack.push(WasmStackValue::from(val as u32))
                }

                WasmOpcode::I32Store8 => {
                    let memarg = code_block.read_memarg()?;
                    let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let val = value_stack
                        .pop()
                        .map(|v| v.get_u8())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let offset = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    memory.write_u8(memarg.offset_by(offset), val)?;
                }
                WasmOpcode::I32Store16 => {
                    let memarg = code_block.read_memarg()?;
                    let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let val = value_stack
                        .pop()
                        .map(|v| v.get_u16())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let offset = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    memory.write_u16(memarg.offset_by(offset), val)?;
                }

                WasmOpcode::I64Load8S => {
                    let memarg = code_block.read_memarg()?;
                    let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let offset = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let val = memory.read_u8(memarg.offset_by(offset))?;
                    value_stack.push(WasmStackValue::from(val as i8 as i64))
                }
                WasmOpcode::I64Load8U => {
                    let memarg = code_block.read_memarg()?;
                    let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let offset = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let val = memory.read_u8(memarg.offset_by(offset))?;
                    value_stack.push(WasmStackValue::from(val as u64))
                }
                WasmOpcode::I64Load16S => {
                    let memarg = code_block.read_memarg()?;
                    let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let offset = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let val = memory.read_u16(memarg.offset_by(offset))?;
                    value_stack.push(WasmStackValue::from(val as i16 as i64))
                }
                WasmOpcode::I64Load16U => {
                    let memarg = code_block.read_memarg()?;
                    let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let offset = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let val = memory.read_u16(memarg.offset_by(offset))?;
                    value_stack.push(WasmStackValue::from(val as u64))
                }
                WasmOpcode::I64Load32S => {
                    let memarg = code_block.read_memarg()?;
                    let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let offset = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let val = memory.read_u32(memarg.offset_by(offset))?;
                    value_stack.push(WasmStackValue::from(val as i32 as i64))
                }
                WasmOpcode::I64Load32U => {
                    let memarg = code_block.read_memarg()?;
                    let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let offset = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let val = memory.read_u32(memarg.offset_by(offset))?;
                    value_stack.push(WasmStackValue::from(val as u64))
                }

                WasmOpcode::I64Store8 => {
                    let memarg = code_block.read_memarg()?;
                    let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let val = value_stack
                        .pop()
                        .map(|v| v.get_u8())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let offset = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    memory.write_u8(memarg.offset_by(offset), val)?;
                }
                WasmOpcode::I64Store16 => {
                    let memarg = code_block.read_memarg()?;
                    let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let val = value_stack
                        .pop()
                        .map(|v| v.get_u16())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let offset = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    memory.write_u16(memarg.offset_by(offset), val)?;
                }
                WasmOpcode::I64Store32 => {
                    let memarg = code_block.read_memarg()?;
                    let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let val = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let offset = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    memory.write_u32(memarg.offset_by(offset), val)?;
                }

                WasmOpcode::MemorySize => {
                    let _ = code_block.read_unsigned()?;
                    let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    value_stack.push(WasmStackValue::from(memory.size()));
                }

                WasmOpcode::MemoryGrow => {
                    let _ = code_block.read_unsigned()?;
                    let val = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let memory = module.memory(0).ok_or(WasmRuntimeError::OutOfMemory)?;
                    let result = memory.grow(val as usize);
                    value_stack.push(WasmStackValue { i32: result as i32 });
                }

                WasmOpcode::I32Const => {
                    let val = code_block.read_signed()? as i32;
                    value_stack.push(WasmStackValue { i32: val });
                }
                WasmOpcode::I64Const => {
                    let val = code_block.read_signed()?;
                    value_stack.push(WasmStackValue { i64: val });
                }

                WasmOpcode::I32Eqz => {
                    let last = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *last = WasmStackValue::from(last.get_i32() == 0);
                }
                WasmOpcode::I32Eq => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_i32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *a = WasmStackValue::from(a.get_i32() == b);
                }
                WasmOpcode::I32Ne => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_i32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *a = WasmStackValue::from(a.get_i32() == b);
                }
                WasmOpcode::I32LtS => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_i32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *a = WasmStackValue::from(a.get_i32() < b);
                }
                WasmOpcode::I32LtU => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *a = WasmStackValue::from(a.get_u32() < b);
                }
                WasmOpcode::I32LeS => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_i32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *a = WasmStackValue::from(a.get_i32() <= b);
                }
                WasmOpcode::I32LeU => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *a = WasmStackValue::from(a.get_u32() <= b);
                }
                WasmOpcode::I32GtS => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_i32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *a = WasmStackValue::from(a.get_i32() > b);
                }
                WasmOpcode::I32GtU => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *a = WasmStackValue::from(a.get_u32() > b);
                }
                WasmOpcode::I32GeS => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_i32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *a = WasmStackValue::from(a.get_i32() >= b);
                }
                WasmOpcode::I32GeU => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *a = WasmStackValue::from(a.get_u32() >= b);
                }

                WasmOpcode::I64Eqz => {
                    let last = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *last = WasmStackValue::from(last.get_i64() == 0);
                }
                WasmOpcode::I64Eq => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_i64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *a = WasmStackValue::from(a.get_i64() == b);
                }
                WasmOpcode::I64Ne => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_i64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *a = WasmStackValue::from(a.get_i64() == b);
                }
                WasmOpcode::I64LtS => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_i64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *a = WasmStackValue::from(a.get_i64() < b);
                }
                WasmOpcode::I64LtU => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *a = WasmStackValue::from(a.get_u64() < b);
                }
                WasmOpcode::I64LeS => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_i64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *a = WasmStackValue::from(a.get_i64() <= b);
                }
                WasmOpcode::I64LeU => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *a = WasmStackValue::from(a.get_u64() <= b);
                }
                WasmOpcode::I64GtS => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_i64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *a = WasmStackValue::from(a.get_i64() > b);
                }
                WasmOpcode::I64GtU => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *a = WasmStackValue::from(a.get_u64() > b);
                }
                WasmOpcode::I64GeS => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_i64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *a = WasmStackValue::from(a.get_i64() >= b);
                }
                WasmOpcode::I64GeU => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *a = WasmStackValue::from(a.get_u64() >= b);
                }

                WasmOpcode::I32Clz => {
                    let last = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    last.map_u32(|v| v.leading_zeros());
                }
                WasmOpcode::I32Ctz => {
                    let last = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    last.map_u32(|v| v.trailing_zeros());
                }
                WasmOpcode::I32Popcnt => {
                    let last = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    last.map_u32(|v| v.count_ones());
                }

                WasmOpcode::I32Add => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_u32(|v| v.wrapping_add(b));
                }
                WasmOpcode::I32Sub => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_u32(|v| v.wrapping_sub(b));
                }
                WasmOpcode::I32Mul => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_u32(|v| v.wrapping_mul(b));
                }
                WasmOpcode::I32DivS => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_i32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    if b == 0 {
                        return Err(WasmRuntimeError::DivideByZero);
                    }
                    a.map_i32(|v| v.wrapping_div(b));
                }
                WasmOpcode::I32DivU => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    if b == 0 {
                        return Err(WasmRuntimeError::DivideByZero);
                    }
                    a.map_u32(|v| v.wrapping_div(b));
                }
                WasmOpcode::I32RemS => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_i32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    if b == 0 {
                        return Err(WasmRuntimeError::DivideByZero);
                    }
                    a.map_i32(|v| v.wrapping_rem(b));
                }
                WasmOpcode::I32RemU => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    if b == 0 {
                        return Err(WasmRuntimeError::DivideByZero);
                    }
                    a.map_u32(|v| v.wrapping_rem(b));
                }

                WasmOpcode::I32And => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_u32(|v| v & b);
                }
                WasmOpcode::I32Or => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_u32(|v| v | b);
                }
                WasmOpcode::I32Xor => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_u32(|v| v ^ b);
                }

                WasmOpcode::I32Shl => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_u32(|v| v << b);
                }
                WasmOpcode::I32ShrS => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_u32(|v| v >> b);
                }
                WasmOpcode::I32ShrU => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_i32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_i32(|v| v >> b);
                }
                WasmOpcode::I32Rotl => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_u32(|v| v.rotate_left(b));
                }
                WasmOpcode::I32Rotr => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u32())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_u32(|v| v.rotate_right(b));
                }

                WasmOpcode::I64Clz => {
                    let last = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    last.map_u64(|v| v.leading_zeros() as u64);
                }
                WasmOpcode::I64Ctz => {
                    let last = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    last.map_u64(|v| v.trailing_zeros() as u64);
                }
                WasmOpcode::I64Popcnt => {
                    let last = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    last.map_u64(|v| v.count_ones() as u64);
                }

                WasmOpcode::I64Add => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_u64(|v| v.wrapping_add(b));
                }
                WasmOpcode::I64Sub => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_u64(|v| v.wrapping_sub(b));
                }
                WasmOpcode::I64Mul => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_u64(|v| v.wrapping_mul(b));
                }
                WasmOpcode::I64DivS => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_i64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    if b == 0 {
                        return Err(WasmRuntimeError::DivideByZero);
                    }
                    a.map_i64(|v| v.wrapping_div(b));
                }
                WasmOpcode::I64DivU => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    if b == 0 {
                        return Err(WasmRuntimeError::DivideByZero);
                    }
                    a.map_u64(|v| v.wrapping_div(b));
                }
                WasmOpcode::I64RemS => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_i64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    if b == 0 {
                        return Err(WasmRuntimeError::DivideByZero);
                    }
                    a.map_i64(|v| v.wrapping_rem(b));
                }
                WasmOpcode::I64RemU => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    if b == 0 {
                        return Err(WasmRuntimeError::DivideByZero);
                    }
                    a.map_u64(|v| v.wrapping_rem(b));
                }

                WasmOpcode::I64And => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_u64(|v| v & b);
                }
                WasmOpcode::I64Or => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_u64(|v| v | b);
                }
                WasmOpcode::I64Xor => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_u64(|v| v ^ b);
                }

                WasmOpcode::I64Shl => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_u64(|v| v << b);
                }
                WasmOpcode::I64ShrS => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_i64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_i64(|v| v >> b);
                }
                WasmOpcode::I64ShrU => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_u64(|v| v >> b);
                }
                WasmOpcode::I64Rotl => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_u64(|v| v.rotate_left(b as u32));
                }
                WasmOpcode::I64Rotr => {
                    let b = value_stack
                        .pop()
                        .map(|v| v.get_u64())
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    let a = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    a.map_u64(|v| v.rotate_right(b as u32));
                }

                WasmOpcode::I32WrapI64 => {
                    // NOP
                }
                WasmOpcode::I64ExtendI32S => {
                    let last = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *last = WasmStackValue::from_i64(last.get_i32() as i64);
                }
                WasmOpcode::I64ExtendI32U => {
                    let last = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *last = WasmStackValue::from_u64(last.get_u32() as u64);
                }

                WasmOpcode::I32Extend8S => {
                    let last = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *last = WasmStackValue::from_i32((last.get_i32() as i8) as i32);
                }
                WasmOpcode::I32Extend16S => {
                    let last = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *last = WasmStackValue::from_i32((last.get_i32() as i16) as i32);
                }

                WasmOpcode::I64Extend8S => {
                    let last = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *last = WasmStackValue::from_i64((last.get_i64() as i8) as i64);
                }
                WasmOpcode::I64Extend16S => {
                    let last = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *last = WasmStackValue::from_i64((last.get_i64() as i16) as i64);
                }
                WasmOpcode::I64Extend32S => {
                    let last = value_stack
                        .last_mut()
                        .ok_or(WasmRuntimeError::InternalInconsistency)?;
                    *last = WasmStackValue::from_i64((last.get_i64() as i32) as i64);
                }

                _ => return Err(WasmRuntimeError::InvalidBytecode),
            }
        }
        if let Some(result_type) = result_types.first() {
            let val = value_stack
                .pop()
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

    fn call(
        func: &WasmFunction,
        value_stack: &mut Vec<WasmStackValue>,
        module: &WasmModule,
    ) -> Result<(), WasmRuntimeError> {
        let result_types = func.result_types();

        let mut locals = Vec::new();
        let param_len = func.param_types().len();
        if value_stack.len() < param_len {
            return Err(WasmRuntimeError::InternalInconsistency);
        }
        let new_stack_len = value_stack.len() - param_len;
        let params = &value_stack[new_stack_len..];
        for (index, val_type) in func.param_types().iter().enumerate() {
            locals.push(params[index].get_by_type(*val_type));
        }
        value_stack.resize(new_stack_len, WasmStackValue::from_usize(0));

        if let Some(body) = func.body() {
            for local in body.local_types() {
                locals.push(WasmValue::default_for(*local));
            }
            let cb = body.code_block();
            let cb_ref = cb.borrow();
            let slice = cb_ref.as_slice();
            let mut code_block = WasmCodeBlock::from_slice(slice, body.block_info());
            let result = Self::run(&mut code_block, &locals, result_types, module)?;
            if !result.is_empty() {
                value_stack.push(WasmStackValue::from(result));
            }
            Ok(())
        } else if let Some(dlink) = func.dlink() {
            let result = dlink(module, &locals)?;
            if let Some(t) = result_types.first() {
                if result.is_valid_type(*t) {
                    value_stack.push(WasmStackValue::from(result));
                } else {
                    return Err(WasmRuntimeError::TypeMismatch);
                }
            }
            Ok(())
        } else {
            return Err(WasmRuntimeError::NoMethod);
        }
    }

    fn branch(
        target: usize,
        block_stack: &mut Vec<usize>,
        value_stack: &mut Vec<WasmStackValue>,
        code_block: &mut WasmCodeBlock,
    ) -> Result<(), WasmRuntimeError> {
        block_stack.resize(block_stack.len() - target, 0);
        let block_position = block_stack
            .pop()
            .ok_or(WasmRuntimeError::InternalInconsistency)?;
        let block = code_block
            .info()
            .block_info(block_position)
            .ok_or(WasmRuntimeError::InternalInconsistency)?;

        let block_type = block.block_type;
        let new_len = block.stack_level;
        let new_position = block.preferred_target();
        if block_type == WasmBlockType::Empty {
            value_stack.resize(new_len, WasmStackValue::from_usize(0));
        } else {
            let top_val = value_stack
                .pop()
                .ok_or(WasmRuntimeError::InternalInconsistency)?;
            value_stack.resize(new_len, WasmStackValue::from_usize(0));
            value_stack.push(top_val);
        }
        code_block.set_position(new_position);
        Ok(())
    }
}

#[derive(Copy, Clone)]
pub union WasmStackValue {
    i32: i32,
    u32: u32,
    i64: i64,
    u64: u64,
    f32: f32,
    f64: f64,
    usize: usize,
    isize: isize,
}

impl WasmStackValue {
    #[inline]
    pub const fn from_bool(v: bool) -> Self {
        if v {
            Self::from_usize(1)
        } else {
            Self::from_usize(0)
        }
    }

    #[inline]
    pub const fn from_usize(v: usize) -> Self {
        Self { usize: v }
    }

    #[inline]
    pub const fn from_isize(v: isize) -> Self {
        Self { isize: v }
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
    pub fn get_u8(&self) -> u8 {
        unsafe { self.usize as u8 }
    }

    #[inline]
    pub fn get_u16(&self) -> u16 {
        unsafe { self.usize as u16 }
    }

    #[inline]
    pub fn map_i32<F>(&mut self, f: F)
    where
        F: FnOnce(i32) -> i32,
    {
        let val = unsafe { self.i32 };
        self.i32 = f(val);
    }

    #[inline]
    pub fn map_u32<F>(&mut self, f: F)
    where
        F: FnOnce(u32) -> u32,
    {
        let val = unsafe { self.u32 };
        self.u32 = f(val);
    }

    #[inline]
    pub fn map_i64<F>(&mut self, f: F)
    where
        F: FnOnce(i64) -> i64,
    {
        let val = unsafe { self.i64 };
        self.i64 = f(val);
    }

    #[inline]
    pub fn map_u64<F>(&mut self, f: F)
    where
        F: FnOnce(u64) -> u64,
    {
        let val = unsafe { self.u64 };
        self.u64 = f(val);
    }

    pub fn get_by_type(&self, val_type: WasmValType) -> WasmValue {
        match val_type {
            WasmValType::I32 => WasmValue::I32(self.get_i32()),
            WasmValType::I64 => WasmValue::I64(self.get_i64()),
            // WasmValType::F32 => {}
            // WasmValType::F64 => {}
            _ => todo!(),
        }
    }

    pub fn into_value(&self, val_type: WasmValType) -> WasmValue {
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
    fn from(v: bool) -> Self {
        Self::from_bool(v)
    }
}

impl From<usize> for WasmStackValue {
    fn from(v: usize) -> Self {
        Self::from_usize(v)
    }
}

impl From<u32> for WasmStackValue {
    fn from(v: u32) -> Self {
        Self::from_u32(v)
    }
}

impl From<i32> for WasmStackValue {
    fn from(v: i32) -> Self {
        Self::from_i32(v)
    }
}

impl From<u64> for WasmStackValue {
    fn from(v: u64) -> Self {
        Self::from_u64(v)
    }
}

impl From<i64> for WasmStackValue {
    fn from(v: i64) -> Self {
        Self::from_i64(v)
    }
}

impl From<WasmValue> for WasmStackValue {
    fn from(v: WasmValue) -> Self {
        match v {
            WasmValue::Empty => Self::from_i64(0),
            WasmValue::I32(v) => Self::from_i64(v as i64),
            WasmValue::I64(v) => Self::from_i64(v),
            _ => todo!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WasmInterpreter;
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
        let block_info =
            WasmBlockInfo::analyze(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut code_block = super::WasmCodeBlock::from_slice(&slice, &block_info);

        let params = [1234.into(), 5678.into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 6912);

        let params = [0xDEADBEEFu32.into(), 0x55555555.into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
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
        let block_info =
            WasmBlockInfo::analyze(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut code_block = super::WasmCodeBlock::from_slice(&slice, &block_info);

        let params = [1234.into(), 5678.into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, -4444);

        let params = [0x55555555.into(), 0xDEADBEEFu32.into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
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
        let block_info =
            WasmBlockInfo::analyze(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut code_block = super::WasmCodeBlock::from_slice(&slice, &block_info);

        let params = [1234.into(), 5678.into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 7006652);

        let params = [0x55555555.into(), 0xDEADBEEFu32.into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
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
        let block_info =
            WasmBlockInfo::analyze(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut code_block = super::WasmCodeBlock::from_slice(&slice, &block_info);

        let params = [7006652.into(), 5678.into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 1234);

        let params = [42.into(), (-6).into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, -7);

        let params = [(-42).into(), (6).into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, -7);

        let params = [(-42).into(), (-6).into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
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
        let block_info =
            WasmBlockInfo::analyze(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut code_block = super::WasmCodeBlock::from_slice(&slice, &block_info);

        let params = [7006652.into(), 5678.into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 1234);

        let params = [42.into(), (-6).into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 0);

        let params = [(-42).into(), (6).into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 715827875);
    }

    #[test]
    fn br_table() {
        let slice = [
            0x02, 0x40, 0x02, 0x40, 0x02, 0x40, 0x20, 0x00, 0x0e, 0x02, 0x00, 0x01, 0x02, 0x0b,
            0x41, 0xfb, 0x00, 0x0f, 0x0b, 0x41, 0xc8, 0x03, 0x0f, 0x0b, 0x41, 0x95, 0x06, 0x0b,
        ];
        let local_types = [WasmValType::I32];
        let result_types = [WasmValType::I32];
        let mut stream = Leb128Stream::from_slice(&slice);
        let module = WasmModule::new();
        let block_info =
            WasmBlockInfo::analyze(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut code_block = super::WasmCodeBlock::from_slice(&slice, &block_info);

        let params = [0.into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 123);

        let params = [1.into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 456);

        let params = [2.into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 789);

        let params = [3.into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 789);

        let params = [4.into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 789);

        let params = [5.into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 789);

        let params = [(-1).into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
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
        let block_info =
            WasmBlockInfo::analyze(0, &mut stream, &local_types, &result_types, &module).unwrap();
        let mut code_block = super::WasmCodeBlock::from_slice(&slice, &block_info);

        let params = [7.into(), 0.into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
            .unwrap()
            .get_i32()
            .unwrap();
        assert_eq!(result, 5040);

        let params = [10.into(), 0.into()];
        let result = WasmInterpreter::run(&mut code_block, &params, &result_types, &module)
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
            WasmLoader::instantiate(&slice, &|_, _, _| Err(WasmDecodeError::DynamicLinkError))
                .unwrap();
        let runnable = module.func_by_index(0).unwrap();

        let result = runnable.invoke(&[7.into()]).unwrap().get_i32().unwrap();
        assert_eq!(result, 13);

        let result = runnable.invoke(&[20.into()]).unwrap().get_i32().unwrap();
        assert_eq!(result, 6765);
    }
}
