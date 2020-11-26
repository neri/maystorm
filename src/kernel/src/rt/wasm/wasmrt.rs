// WebAssembly Runtime (pre-alpha)

use super::opcode::*;
use super::wasm::*;
use crate::*;
use alloc::vec::Vec;
// use super::*;
// use alloc::sync::Arc;
// use core::cell::RefCell;

#[allow(dead_code)]
pub struct WasmRuntimeContext<'a> {
    value_stack: Vec<WasmValue>,
    call_stack: Vec<WasmCodeBlock<'a>>,
}

impl<'a> WasmRuntimeContext<'a> {
    pub fn new() -> Self {
        Self {
            value_stack: Vec::new(),
            call_stack: Vec::new(),
        }
    }

    pub fn run(
        &mut self,
        code_block: &mut WasmCodeBlock,
        locals: &[WasmValue],
        result_types: &[WasmValType],
    ) -> Result<WasmValue, WasmRuntimeError> {
        let mut local0 = Vec::new();
        for local in locals {
            local0.push(*local);
        }
        let mut locals = local0;
        loop {
            let position = code_block.position();
            let opcode = code_block.get_opcode()?;
            println!("{:04x} {:02x} {}", position, opcode as u8, opcode.to_str());
            match opcode {
                WasmOpcode::End => {
                    break;
                }
                WasmOpcode::Drop => {
                    let _ = self.value_stack.pop().ok_or(WasmRuntimeError::OutOfStack)?;
                }
                WasmOpcode::LocalGet => {
                    let local_ref = code_block.get_uint()? as usize;
                    let val = locals
                        .get(local_ref)
                        .ok_or(WasmRuntimeError::InvalidLocal)?;
                    self.value_stack.push(*val);
                }
                WasmOpcode::LocalSet => {
                    let local_ref = code_block.get_uint()? as usize;
                    let var = locals
                        .get_mut(local_ref)
                        .ok_or(WasmRuntimeError::InvalidLocal)?;
                    let val = self.value_stack.pop().ok_or(WasmRuntimeError::OutOfStack)?;
                    *var = val;
                }
                WasmOpcode::LocalTee => {
                    let local_ref = code_block.get_uint()? as usize;
                    let var = locals
                        .get_mut(local_ref)
                        .ok_or(WasmRuntimeError::InvalidLocal)?;
                    let val = self
                        .value_stack
                        .last()
                        .ok_or(WasmRuntimeError::OutOfStack)?;
                    *var = *val;
                }
                WasmOpcode::I32Const => {
                    let val = code_block.get_sint()? as i32;
                    self.value_stack.push(val.into())
                }
                WasmOpcode::I64Const => {
                    let val = code_block.get_sint()?;
                    self.value_stack.push(val.into())
                }

                WasmOpcode::I32Clz | WasmOpcode::I64Clz => {
                    let a = self.pop()?;
                    let c = a.clz()?;
                    self.push(c)?;
                }
                WasmOpcode::I32Ctz | WasmOpcode::I64Ctz => {
                    let a = self.pop()?;
                    let c = a.ctz()?;
                    self.push(c)?;
                }
                WasmOpcode::I32Popcnt | WasmOpcode::I64Popcnt => {
                    let a = self.pop()?;
                    let c = a.popcnt()?;
                    self.push(c)?;
                }
                WasmOpcode::I32Add | WasmOpcode::I64Add => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = (a + b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32Sub | WasmOpcode::I64Sub => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = (a - b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32Mul | WasmOpcode::I64Mul => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = (a * b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32DivS | WasmOpcode::I64DivS => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = (a / b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32DivU | WasmOpcode::I64DivU => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = a.div_u(b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32RemS | WasmOpcode::I64RemS => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = (a % b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32RemU | WasmOpcode::I64RemU => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = a.rem_u(b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32And | WasmOpcode::I64And => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = (a & b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32Or | WasmOpcode::I64Or => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = (a | b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32Xor | WasmOpcode::I64Xor => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = (a ^ b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32Shl | WasmOpcode::I64Shl => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = (a << b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32ShrS | WasmOpcode::I64ShrS => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = (a >> b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32ShrU | WasmOpcode::I64ShrU => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = a.shr_u(b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32Rotl | WasmOpcode::I64Rotl => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = a.rotl(b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32Rotr | WasmOpcode::I64Rotr => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = a.rotr(b)?;
                    self.push(c)?;
                }

                WasmOpcode::I32Eqz | WasmOpcode::I64Eqz => {
                    let a = self.pop()?;
                    let c = a.eqz()?;
                    self.push(c)?;
                }
                WasmOpcode::I32Eq | WasmOpcode::I64Eq => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = a.eq(b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32Ne | WasmOpcode::I64Ne => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = a.ne(b)?;
                    self.push(c)?;
                }

                WasmOpcode::I32LtS | WasmOpcode::I64LtS => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = a.lt_s(b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32LtU | WasmOpcode::I64LtU => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = a.lt_u(b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32LeS | WasmOpcode::I64LeS => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = a.le_s(b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32LeU | WasmOpcode::I64LeU => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = a.le_u(b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32GtS | WasmOpcode::I64GtS => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = a.gt_s(b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32GtU | WasmOpcode::I64GtU => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = a.gt_u(b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32GeS | WasmOpcode::I64GeS => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = a.ge_s(b)?;
                    self.push(c)?;
                }
                WasmOpcode::I32GeU | WasmOpcode::I64GeU => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = a.ge_u(b)?;
                    self.push(c)?;
                }

                WasmOpcode::I32WrapI64 => {
                    let a = self.pop()?;
                    let c = (a.get_i64()? as i32).into();
                    self.push(c)?;
                }
                WasmOpcode::I64ExtendI32S => {
                    let a = self.pop()?;
                    let c = (a.get_i32()? as i64).into();
                    self.push(c)?;
                }
                WasmOpcode::I64ExtendI32U => {
                    let a = self.pop()?;
                    let c = (a.get_i32()? as u32 as u64 as i64).into();
                    self.push(c)?;
                }
                WasmOpcode::I32Extend8S => {
                    let a = self.pop()?;
                    let c = a.map_i32(|v| (v as i8) as i32)?;
                    self.push(c)?;
                }
                WasmOpcode::I32Extend16S => {
                    let a = self.pop()?;
                    let c = a.map_i32(|v| (v as i16) as i32)?;
                    self.push(c)?;
                }
                WasmOpcode::I64Extend8S => {
                    let a = self.pop()?;
                    let c = a.map_i64(|v| (v as i8) as i64)?;
                    self.push(c)?;
                }
                WasmOpcode::I64Extend16S => {
                    let a = self.pop()?;
                    let c = a.map_i64(|v| (v as i16) as i64)?;
                    self.push(c)?;
                }
                WasmOpcode::I64Extend32S => {
                    let a = self.pop()?;
                    let c = a.map_i64(|v| (v as i32) as i64)?;
                    self.push(c)?;
                }

                _ => return Err(WasmRuntimeError::InvalidBytecode),
            }
        }
        if result_types.len() > 0 {
            let val = self.pop()?;
            Ok(val)
        } else {
            Ok(WasmValue::Empty)
        }
    }

    #[inline]
    pub fn push(&mut self, value: WasmValue) -> Result<(), WasmRuntimeError> {
        Ok(self.value_stack.push(value))
    }

    #[inline]
    pub fn pop(&mut self) -> Result<WasmValue, WasmRuntimeError> {
        self.value_stack.pop().ok_or(WasmRuntimeError::OutOfStack)
    }
}
