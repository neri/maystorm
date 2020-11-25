// WebAssembly Runtime (pre-alpha)

use super::opcode::*;
use super::wasm::*;
use crate::*;
use alloc::vec::Vec;
use core::fmt;
// use super::*;
// use alloc::sync::Arc;
// use core::cell::RefCell;

#[allow(dead_code)]
pub struct WasmRuntimeContext<'a> {
    module: &'a WasmModule,
    value_stack: Vec<WasmValue>,
    call_stack: Vec<FunctionContext<'a>>,
}

impl<'a> WasmRuntimeContext<'a> {
    pub fn from_module(module: &'a WasmModule) -> Self {
        Self {
            module,
            value_stack: Vec::new(),
            call_stack: Vec::new(),
        }
    }

    pub fn start(&mut self) -> Result<WasmValue, WasmRuntimeError> {
        self.module
            .start()
            .ok_or(WasmRuntimeError::NoMethod)
            .and_then(|v| self.invoke_func(v, &[]))
    }

    pub fn invoke(
        &mut self,
        name: &str,
        params: &[WasmValue],
    ) -> Result<WasmValue, WasmRuntimeError> {
        self.module
            .function(name)
            .ok_or(WasmRuntimeError::NoMethod)
            .and_then(|v| self.invoke_func(v, params))
    }

    pub fn invoke_func(
        &mut self,
        index: usize,
        params: &[WasmValue],
    ) -> Result<WasmValue, WasmRuntimeError> {
        let function = self
            .module
            .func_by_ref(index)
            .ok_or(WasmRuntimeError::NoMethod)?;

        let func_type = self
            .module
            .type_by_ref(function.type_ref())
            .ok_or(WasmRuntimeError::NoMethod)?;

        let body = function.body().ok_or(WasmRuntimeError::NoMethod)?;

        let mut locals = Vec::new();
        for param in params {
            locals.push(*param);
        }
        for local in body.locals() {
            locals.push(WasmValue::default_for(*local));
        }

        let result_types = func_type.result_types();

        let code_block = body.code_block();
        let code_ref = code_block.borrow();
        let mut context = FunctionContext::from_slice(&code_ref);
        self.run(&mut context, locals, result_types)
    }

    fn run(
        &mut self,
        block: &mut FunctionContext,
        locals: Vec<WasmValue>,
        result_types: &[WasmValType],
    ) -> Result<WasmValue, WasmRuntimeError> {
        println!("locals: {:?}", locals);
        let mut locals = locals;
        loop {
            let position = block.position();
            let opcode = block.get_opcode()?;
            println!("{:04x} {:02x} {}", position, opcode as u8, opcode.to_str());
            match opcode {
                WasmOpcode::End => {
                    break;
                }
                WasmOpcode::Drop => {
                    let val = self.value_stack.pop().ok_or(WasmRuntimeError::OutOfStack)?;
                    println!("drop {} -> []", val);
                }
                WasmOpcode::LocalGet => {
                    let local_ref = block.get_uint()? as usize;
                    let val = locals
                        .get(local_ref)
                        .ok_or(WasmRuntimeError::InvalidLocal)?;
                    println!("local.get {} -> {}", local_ref, val);
                    self.value_stack.push(*val);
                }
                WasmOpcode::LocalSet => {
                    let local_ref = block.get_uint()? as usize;
                    let var = locals
                        .get_mut(local_ref)
                        .ok_or(WasmRuntimeError::InvalidLocal)?;
                    let val = self.value_stack.pop().ok_or(WasmRuntimeError::OutOfStack)?;
                    *var = val;
                    println!("local.set {} -> {}", local_ref, val);
                }
                WasmOpcode::LocalTee => {
                    let local_ref = block.get_uint()? as usize;
                    let var = locals
                        .get_mut(local_ref)
                        .ok_or(WasmRuntimeError::InvalidLocal)?;
                    let val = self
                        .value_stack
                        .last()
                        .ok_or(WasmRuntimeError::OutOfStack)?;
                    *var = *val;
                    println!("local.tee {} -> {}", local_ref, val);
                }
                WasmOpcode::I32Const => {
                    let val = block.get_sint()? as i32;
                    println!("i32.const {} ;; 0x{:x}", val, val);
                    self.value_stack.push(WasmValue::I32(val))
                }
                WasmOpcode::I32Add => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    let c = a.add(b)?;
                    println!("add {} + {} -> {}", a, b, c);
                    self.push(c)?;
                }
                _ => return Err(WasmRuntimeError::InvalidOpcode),
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

#[allow(dead_code)]
#[derive(Debug, Copy, Clone)]
pub enum WasmRuntimeError {
    OutOfBounds,
    OutOfMemory,
    OutOfStack,
    InvalidLocal,
    UnexpectedEof,
    UNexpectedToken,
    InvalidParameter,
    InvalidOpcode,
    NoMethod,
    TypeMismatch,
}

#[derive(Debug, Copy, Clone)]
pub enum WasmValue {
    Empty,
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
}

impl WasmValue {
    pub fn default_for(val_type: WasmValType) -> Self {
        match val_type {
            WasmValType::I32 => Self::I32(0),
            WasmValType::I64 => Self::I64(0),
            WasmValType::F32 => Self::F32(0.0),
            WasmValType::F64 => Self::F64(0.0),
        }
    }

    pub fn add(self, other: WasmValue) -> Result<WasmValue, WasmRuntimeError> {
        match self {
            Self::I32(a) => {
                let b = match other {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(a + b))
            }
            Self::I64(a) => {
                let b = match other {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I64(a + b))
            }
            _ => return Err(WasmRuntimeError::InvalidOpcode),
        }
    }
}

impl fmt::Display for WasmValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            WasmValue::Empty => write!(f, "()"),
            Self::I32(v) => write!(f, "{}", v),
            Self::I64(v) => write!(f, "{}", v),
            Self::F32(_) => write!(f, "(#!F32)"),
            Self::F64(_) => write!(f, "(#!F64)"),
        }
    }
}

struct FunctionContext<'a> {
    code: Leb128Stream<'a>,
}

impl<'a> FunctionContext<'a> {
    fn from_slice(slice: &'a [u8]) -> Self {
        Self {
            code: Leb128Stream::from_slice(slice),
        }
    }

    const fn position(&self) -> usize {
        self.code.position()
    }

    fn get_opcode(&mut self) -> Result<WasmOpcode, WasmRuntimeError> {
        self.code
            .read_byte()
            .map(|v| WasmOpcode::from_u8(v))
            .map_err(|err| Self::map_err(err))
    }

    fn get_sint(&mut self) -> Result<i64, WasmRuntimeError> {
        self.code.read_sint().map_err(|err| Self::map_err(err))
    }

    fn get_uint(&mut self) -> Result<u64, WasmRuntimeError> {
        self.code.read_uint().map_err(|err| Self::map_err(err))
    }

    fn map_err(err: WasmDecodeError) -> WasmRuntimeError {
        match err {
            WasmDecodeError::UnexpectedEof => WasmRuntimeError::UnexpectedEof,
            _ => WasmRuntimeError::UNexpectedToken,
        }
    }
}
