// WebAssembly Binary Loader

use super::opcode::*;
use super::wasmrt::*;
use crate::*;
use alloc::string::*;
use alloc::sync::Arc;
use alloc::vec::Vec;
use byteorder::*;
use core::cell::{RefCell, UnsafeCell};
use core::fmt;
use core::ops::*;
use core::slice;
use core::str;

pub struct WasmLoader {
    module: WasmModule,
}

impl WasmLoader {
    /// Minimal valid module size, Magic(4) + Version(4) + Empty sections(0) = 8
    const MINIMAL_MOD_SIZE: usize = 8;
    /// Magic number of WebAssembly Binary Format
    const MAGIC: u32 = 0x6D736100;
    /// Current Version
    const VER_CURRENT: u32 = 0x0000_0001;

    pub(super) fn new() -> Self {
        Self {
            module: WasmModule::new(),
        }
    }

    /// Identify the file format
    pub fn identity(blob: &[u8]) -> bool {
        blob.len() >= Self::MINIMAL_MOD_SIZE
            && LE::read_u32(&blob[0..4]) == Self::MAGIC
            && LE::read_u32(&blob[4..8]) == Self::VER_CURRENT
    }

    /// Instantiate wasm modules from slice
    pub fn instantiate(blob: &[u8]) -> Result<WasmModule, WasmDecodeError> {
        if Self::identity(blob) {
            let mut loader = Self::new();
            loader.load(blob).map(|_| loader.module)
        } else {
            return Err(WasmDecodeError::InvalidParameter);
        }
    }

    pub(super) fn load(&mut self, blob: &[u8]) -> Result<(), WasmDecodeError> {
        let mut blob = Leb128Stream::from_slice(&blob[8..]);
        while let Some(mut section) = blob.next_section() {
            match section.section_type {
                WasmSectionType::Custom => Ok(()),
                WasmSectionType::Type => self.parse_sec_type(&mut section),
                WasmSectionType::Import => self.parse_sec_import(&mut section),
                WasmSectionType::Table => self.parse_sec_table(&mut section),
                WasmSectionType::Memory => self.parse_sec_memory(&mut section),
                WasmSectionType::Element => self.parse_sec_elem(&mut section),
                WasmSectionType::Function => self.parse_sec_func(&mut section),
                WasmSectionType::Export => self.parse_sec_export(&mut section),
                WasmSectionType::Code => self.parse_sec_code(&mut section),
                WasmSectionType::Data => self.parse_sec_data(&mut section),
                WasmSectionType::Start => self.parse_sec_start(&mut section),
                // WasmSectionType::Global => todo!();
                _ => Err(WasmDecodeError::UnexpectedToken),
            }?;
        }
        Ok(())
    }

    pub fn print_stat(&mut self) {
        self.module.print_stat();
    }

    pub fn module(&mut self) -> &WasmModule {
        &self.module
    }

    /// Parse "type" section
    fn parse_sec_type(&mut self, section: &mut WasmSection) -> Result<(), WasmDecodeError> {
        let n_items = section.stream.read_uint()? as usize;
        for _ in 0..n_items {
            let ft = WasmType::from_stream(&mut section.stream)?;
            self.module.types.push(ft);
        }
        Ok(())
    }

    /// Parse "import" section
    fn parse_sec_import(&mut self, section: &mut WasmSection) -> Result<(), WasmDecodeError> {
        let n_items = section.stream.read_uint()? as usize;
        for _ in 0..n_items {
            let import = WasmImport::from_stream(&mut section.stream)?;
            if let WasmImportType::Type(index) = import.index {
                self.module
                    .functions
                    .push(WasmFunction::from_import(index, self.module.n_ext_func));
                self.module.n_ext_func += 1;
            }
            self.module.imports.push(import);
        }
        Ok(())
    }

    /// Parse "func" section
    fn parse_sec_func(&mut self, section: &mut WasmSection) -> Result<(), WasmDecodeError> {
        let n_items = section.stream.read_uint()?;
        for _ in 0..n_items {
            let index = section.stream.read_uint()? as usize;
            self.module.functions.push(WasmFunction::internal(index));
        }
        Ok(())
    }

    /// Parse "export" section
    fn parse_sec_export(&mut self, section: &mut WasmSection) -> Result<(), WasmDecodeError> {
        let n_items = section.stream.read_uint()? as usize;
        for i in 0..n_items {
            let export = WasmExport::from_stream(&mut section.stream)?;
            if let WasmExportType::Function(index) = export.index {
                self.module
                    .functions
                    .get_mut(index)
                    .map(|v| v.origin = WasmFunctionOrigin::Export(i));
            }
            self.module.exports.push(export);
        }
        Ok(())
    }

    /// Parse "memory" section
    fn parse_sec_memory(&mut self, section: &mut WasmSection) -> Result<(), WasmDecodeError> {
        let n_items = section.stream.read_uint()?;
        for _ in 0..n_items {
            let limit = WasmLimit::from_stream(&mut section.stream)?;
            self.module.memories.push(WasmMemory::new(limit));
        }
        Ok(())
    }

    /// Parse "table" section
    fn parse_sec_table(&mut self, section: &mut WasmSection) -> Result<(), WasmDecodeError> {
        let n_items = section.stream.read_uint()?;
        for _ in 0..n_items {
            let table = WasmTable::from_stream(&mut section.stream)?;
            self.module.tables.push(table);
        }
        Ok(())
    }

    /// Parse "elem" section
    fn parse_sec_elem(&mut self, section: &mut WasmSection) -> Result<(), WasmDecodeError> {
        let n_items = section.stream.read_uint()?;
        for _ in 0..n_items {
            let tabidx = section.stream.read_uint()? as usize;
            let offset = self.eval_offset(&mut section.stream)? as usize;
            let n_elements = section.stream.read_uint()? as usize;
            let table = self
                .module
                .tables
                .get_mut(tabidx)
                .ok_or(WasmDecodeError::InvalidParameter)?;
            for i in offset..offset + n_elements {
                let elem = section.stream.read_uint()? as usize;
                table.table.get_mut(i).map(|v| *v = elem);
            }
        }
        Ok(())
    }

    /// Parse "code" section
    fn parse_sec_code(&mut self, section: &mut WasmSection) -> Result<(), WasmDecodeError> {
        let n_items = section.stream.read_uint()? as usize;
        for i in 0..n_items {
            let index = i + self.module.n_ext_func;
            let func_def = self
                .module
                .functions
                .get(index)
                .ok_or(WasmDecodeError::InvalidParameter)?;
            let type_def = self
                .module
                .type_by_ref(func_def.type_ref)
                .ok_or(WasmDecodeError::InvalidParameter)?;
            let body = WasmFunctionBody::from_stream(&mut section.stream, type_def)?;
            self.module.functions[index].body = Some(body);
        }
        Ok(())
    }

    /// Parse "data" section
    fn parse_sec_data(&mut self, section: &mut WasmSection) -> Result<(), WasmDecodeError> {
        let n_items = section.stream.read_uint()?;
        for _ in 0..n_items {
            let memidx = section.stream.read_uint()? as usize;
            let offset = self.eval_offset(&mut section.stream)?;
            let src = section.stream.read_bytes()?;
            let memory = self
                .module
                .memories
                .get_mut(memidx)
                .ok_or(WasmDecodeError::InvalidParameter)?;
            memory.write_bytes(offset, src).unwrap();
        }
        Ok(())
    }

    /// Parse "start" section
    fn parse_sec_start(&mut self, section: &mut WasmSection) -> Result<(), WasmDecodeError> {
        let index = section.stream.read_uint()? as usize;
        self.module.start = Some(index);
        Ok(())
    }

    fn eval_offset(&mut self, stream: &mut Leb128Stream) -> Result<usize, WasmDecodeError> {
        stream
            .read_byte()
            .and_then(|opc| match WasmOpcode::from_u8(opc) {
                WasmOpcode::I32Const => stream.read_sint().and_then(|r| {
                    match stream.read_byte().map(|v| WasmOpcode::from_u8(v)) {
                        Ok(WasmOpcode::End) => Ok((r as u32) as usize),
                        _ => Err(WasmDecodeError::UnexpectedToken),
                    }
                }),
                _ => Err(WasmDecodeError::UnexpectedToken),
            })
    }
}

pub struct WasmModule {
    types: Vec<WasmType>,
    imports: Vec<WasmImport>,
    exports: Vec<WasmExport>,
    memories: Vec<WasmMemory>,
    tables: Vec<WasmTable>,
    functions: Vec<WasmFunction>,
    start: Option<usize>,
    n_ext_func: usize,
}

impl WasmModule {
    const fn new() -> Self {
        Self {
            types: Vec::new(),
            memories: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            tables: Vec::new(),
            functions: Vec::new(),
            start: None,
            n_ext_func: 0,
        }
    }

    pub fn types(&self) -> &[WasmType] {
        self.types.as_slice()
    }

    pub fn type_by_ref(&self, index: usize) -> Option<&WasmType> {
        self.types.get(index)
    }

    pub fn imports(&self) -> &[WasmImport] {
        self.imports.as_slice()
    }

    pub fn exports(&self) -> &[WasmExport] {
        self.exports.as_slice()
    }

    pub fn memories(&mut self) -> &mut [WasmMemory] {
        self.memories.as_mut_slice()
    }

    pub fn tables(&mut self) -> &mut [WasmTable] {
        self.tables.as_mut_slice()
    }

    pub fn func_by_ref(&self, index: usize) -> Result<&WasmFunction, WasmRuntimeError> {
        self.functions.get(index).ok_or(WasmRuntimeError::NoMethod)
    }

    pub fn start(&self) -> Result<&WasmFunction, WasmRuntimeError> {
        self.start
            .ok_or(WasmRuntimeError::NoMethod)
            .and_then(|v| self.func_by_ref(v))
    }

    /// Get a reference to the exported function with the specified name
    pub fn function(&self, name: &str) -> Result<&WasmFunction, WasmRuntimeError> {
        for export in &self.exports {
            if let WasmExportType::Function(v) = export.index {
                if export.name == name {
                    return self.func_by_ref(v);
                }
            }
        }
        Err(WasmRuntimeError::NoMethod)
    }

    pub fn print_stat(&mut self) {
        for (index, memory) in self.memories.iter().enumerate() {
            println!(
                " memory ${} {} {}",
                index, memory.limit.min, memory.limit.max
            );
        }
        for (index, table) in self.tables.iter().enumerate() {
            println!(" table ${} {} {}", index, table.limit.min, table.limit.max);
            for i in 0..table.table.len() {
                println!(" table[{}] = func ${}", i, table.table[i]);
            }
        }
        for (func_idx, function) in self.functions.iter().enumerate() {
            let type_ref = self.types.get(function.type_ref).unwrap();

            match function.origin {
                WasmFunctionOrigin::Internal => {
                    println!("func {}{}", func_idx, type_ref);
                    let _ = self.disassemble(func_idx);
                }
                WasmFunctionOrigin::Export(v) => {
                    let export = self.exports.get(v).unwrap();
                    println!("func {} (export {}){}", func_idx, export.name, type_ref);
                    let _ = self.disassemble(func_idx);
                }
                WasmFunctionOrigin::Import(v) => {
                    let import = self.imports.get(v).unwrap();
                    println!(
                        "func {} (import {}.{}){} ",
                        func_idx, import.mod_name, import.name, type_ref,
                    );
                }
            }
        }
    }

    pub fn disassemble(&self, func_idx: usize) -> Result<(), WasmDecodeError> {
        let func = self.functions.get(func_idx).unwrap();
        let type_ref = self.types.get(func.type_ref).unwrap();
        let body = func.body.as_ref().unwrap();
        let locals = body.local_types.as_slice();
        if locals.len() > 0 {
            let mut local_index = type_ref.params.len();
            for local in locals {
                println!(" (local ${}, {})", local_index, local);
                local_index += 1;
            }
        }
        let code_block = body.code_block.borrow();
        let mut stream = Leb128Stream::from_slice(&code_block);
        while let Ok(opcode) = stream.read_byte() {
            let op = WasmOpcode::from_u8(opcode);
            match op.mnemonic_type() {
                WasmMnemonicType::Local => {
                    let opr = stream.read_uint()?;
                    println!(" {} ${}", op.to_str(), opr);
                }
                WasmMnemonicType::Call => {
                    let opr = stream.read_uint()?;
                    println!(" {} ${}", op.to_str(), opr);
                }
                WasmMnemonicType::I32 => {
                    let opr = stream.read_sint()? as i32;
                    println!(" {} {} ;; 0x{:x}", op.to_str(), opr, opr);
                }
                WasmMnemonicType::I64 => {
                    let opr = stream.read_sint()?;
                    println!(" {} {} ;; 0x{:x}", op.to_str(), opr, opr);
                }
                _ => println!(" {}", op.to_str()),
            }
        }
        Ok(())
    }
}

pub struct Leb128Stream<'a> {
    blob: &'a [u8],
    position: usize,
}

impl<'a> Leb128Stream<'a> {
    pub const fn from_slice(slice: &'a [u8]) -> Self {
        Self {
            blob: slice,
            position: 0,
        }
    }
}

#[allow(dead_code)]
impl Leb128Stream<'_> {
    pub const fn position(&self) -> usize {
        self.position
    }

    pub const fn is_eof(&self) -> bool {
        self.position >= self.blob.len()
    }

    pub fn read_byte(&mut self) -> Result<u8, WasmDecodeError> {
        if self.is_eof() {
            return Err(WasmDecodeError::UnexpectedEof);
        }
        let d = self.blob[self.position];
        self.position += 1;
        Ok(d)
    }

    pub fn get_bytes(&mut self, size: usize) -> Result<&[u8], WasmDecodeError> {
        let limit = self.blob.len();
        if self.position <= limit && size <= limit && self.position + size <= limit {
            let offset = self.position;
            self.position += size;
            Ok(&self.blob[offset..offset + size])
        } else {
            Err(WasmDecodeError::UnexpectedEof)
        }
    }

    pub fn read_bytes(&mut self) -> Result<&[u8], WasmDecodeError> {
        self.read_uint()
            .and_then(move |size| self.get_bytes(size as usize))
    }

    pub fn read_uint(&mut self) -> Result<u64, WasmDecodeError> {
        let mut value: u64 = 0;
        let mut scale = 0;
        let mut cursor = self.position;
        loop {
            if self.is_eof() {
                return Err(WasmDecodeError::UnexpectedEof);
            }
            let d = self.blob[cursor];
            cursor += 1;
            value |= (d as u64 & 0x7F) << scale;
            scale += 7;
            if (d & 0x80) == 0 {
                break;
            }
        }
        self.position = cursor;
        Ok(value)
    }

    pub fn read_sint(&mut self) -> Result<i64, WasmDecodeError> {
        let mut value: u64 = 0;
        let mut scale = 0;
        let mut cursor = self.position;
        let signed = loop {
            if self.is_eof() {
                return Err(WasmDecodeError::UnexpectedEof);
            }
            let d = self.blob[cursor];
            cursor += 1;
            value |= (d as u64 & 0x7F) << scale;
            let signed = (d & 0x40) != 0;
            if (d & 0x80) == 0 {
                break signed;
            }
            scale += 7;
        };
        self.position = cursor;
        if signed {
            Ok((value | 0xFFFF_FFFF_FFFF_FFC0 << scale) as i64)
        } else {
            Ok(value as i64)
        }
    }

    pub fn get_string(&mut self) -> Result<&str, WasmDecodeError> {
        self.read_bytes()
            .and_then(|v| str::from_utf8(v).map_err(|_| WasmDecodeError::UnexpectedToken))
    }

    fn next_section(&mut self) -> Option<WasmSection> {
        let section_type = match self.read_byte().ok() {
            Some(v) => v,
            None => return None,
        };
        let size = match self.read_uint().ok() {
            Some(v) => v as usize,
            None => return None,
        };
        let offset = self.position;
        self.position += size;
        let stream = Leb128Stream {
            blob: &self.blob[offset..offset + size],
            position: 0,
        };
        Some(WasmSection {
            section_type: section_type.into(),
            stream,
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub enum WasmDecodeError {
    UnexpectedEof,
    UnexpectedToken,
    InvalidParameter,
}

struct WasmSection<'a> {
    section_type: WasmSectionType,
    stream: Leb128Stream<'a>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialOrd, PartialEq)]
enum WasmSectionType {
    Custom = 0,
    Type,
    Import,
    Function,
    Table,
    Memory,
    Global,
    Export,
    Start,
    Element,
    Code,
    Data,
}

impl From<u8> for WasmSectionType {
    fn from(v: u8) -> Self {
        match v {
            1 => WasmSectionType::Type,
            2 => WasmSectionType::Import,
            3 => WasmSectionType::Function,
            4 => WasmSectionType::Table,
            5 => WasmSectionType::Memory,
            6 => WasmSectionType::Global,
            7 => WasmSectionType::Export,
            8 => WasmSectionType::Start,
            9 => WasmSectionType::Element,
            10 => WasmSectionType::Code,
            11 => WasmSectionType::Data,
            _ => WasmSectionType::Custom,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum WasmValType {
    I32 = 0x7F,
    I64 = 0x7E,
    F32 = 0x7D,
    F64 = 0x7C,
}

impl WasmValType {
    const fn from_u64(v: u64) -> Result<Self, WasmDecodeError> {
        match v {
            0x7F => Ok(WasmValType::I32),
            0x7E => Ok(WasmValType::I64),
            0x7D => Ok(WasmValType::F32),
            0x7C => Ok(WasmValType::F64),
            _ => Err(WasmDecodeError::UnexpectedToken),
        }
    }
}

impl fmt::Display for WasmValType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                WasmValType::I32 => "i32",
                WasmValType::I64 => "i64",
                WasmValType::F32 => "f32",
                WasmValType::F64 => "f64",
            }
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub enum WasmBlockType {
    Empty = 0x40,
    I32 = 0x7F,
    I64 = 0x7E,
    F32 = 0x7D,
    F64 = 0x7C,
}

impl WasmBlockType {
    pub const fn from_i64(v: i64) -> Result<Self, WasmRuntimeError> {
        match v {
            0x40 => Ok(Self::Empty),
            0x7F => Ok(Self::I32),
            0x7E => Ok(Self::I64),
            0x7D => Ok(Self::F32),
            0x7C => Ok(Self::F64),
            _ => Err(WasmRuntimeError::InvalidParameter),
        }
    }

    pub const fn into_type<'a>(self) -> &'a [WasmValType] {
        match self {
            WasmBlockType::Empty => &[],
            WasmBlockType::I32 => &[WasmValType::I32],
            WasmBlockType::I64 => &[WasmValType::I64],
            WasmBlockType::F32 => &[WasmValType::F32],
            WasmBlockType::F64 => &[WasmValType::F64],
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct WasmLimit {
    min: u32,
    max: u32,
}

impl WasmLimit {
    fn from_stream(stream: &mut Leb128Stream) -> Result<Self, WasmDecodeError> {
        match stream.read_uint() {
            Ok(0) => stream.read_uint().map(|min| Self {
                min: min as u32,
                max: min as u32,
            }),
            Ok(1) => {
                let min = stream.read_uint()? as u32;
                let max = stream.read_uint()? as u32;
                Ok(Self { min, max })
            }
            Err(err) => Err(err),
            _ => Err(WasmDecodeError::UnexpectedToken),
        }
    }
}

#[allow(dead_code)]
pub struct WasmMemory {
    limit: WasmLimit,
    memory: Arc<UnsafeCell<Vec<u8>>>,
}

impl WasmMemory {
    const PAGE_SIZE: usize = 0x10000;

    fn new(limit: WasmLimit) -> Self {
        let size = limit.min as usize * Self::PAGE_SIZE;
        let mut memory = Vec::with_capacity(size);
        memory.resize(size, 0);
        Self {
            limit,
            memory: Arc::new(UnsafeCell::new(memory)),
        }
    }

    pub fn limit(&self) -> WasmLimit {
        self.limit
    }

    pub fn memory_arc(&mut self) -> Arc<UnsafeCell<Vec<u8>>> {
        self.memory.clone()
    }

    pub fn memory(&self) -> &[u8] {
        unsafe { self.memory.get().as_ref().unwrap() }
    }

    pub fn memory_mut(&mut self) -> &mut [u8] {
        unsafe { self.memory.get().as_mut().unwrap() }
    }

    /// Read the specified range of memory
    pub fn read_bytes(&self, offset: usize, size: usize) -> Result<&[u8], WasmMemoryError> {
        let memory = self.memory();
        let limit = memory.len();
        if offset < limit && size < limit && offset + size < limit {
            unsafe { Ok(slice::from_raw_parts(&memory[offset] as *const _, size)) }
        } else {
            Err(WasmMemoryError::OutOfBounds)
        }
    }

    /// Write slice to memory
    pub fn write_bytes(&mut self, offset: usize, src: &[u8]) -> Result<(), WasmMemoryError> {
        let memory = self.memory_mut();
        let size = src.len();
        let limit = memory.len();
        if offset < limit && size < limit && offset + size < limit {
            let dest = &mut memory[offset] as *mut u8;
            let src = &src[0] as *const u8;
            unsafe {
                dest.copy_from_nonoverlapping(src, size);
            }
            Ok(())
        } else {
            Err(WasmMemoryError::OutOfBounds)
        }
    }

    // pub fn grow(&mut self, delta: usize)
}

#[allow(dead_code)]
#[derive(Debug, Copy, Clone)]
pub enum WasmMemoryError {
    NullPointerException,
    OutOfBounds,
    OutOfMemory,
}

pub struct WasmTable {
    limit: WasmLimit,
    table: Vec<usize>,
}

impl WasmTable {
    fn from_stream(stream: &mut Leb128Stream) -> Result<Self, WasmDecodeError> {
        match stream.read_uint() {
            Ok(0x70) => (),
            Err(err) => return Err(err),
            _ => return Err(WasmDecodeError::UnexpectedToken),
        };
        WasmLimit::from_stream(stream).map(|limit| {
            let size = limit.min as usize;
            let mut table = Vec::with_capacity(size);
            table.resize(size, 0);
            Self { limit, table }
        })
    }

    pub fn limit(&self) -> WasmLimit {
        self.limit
    }

    pub fn table(&mut self) -> &mut [usize] {
        self.table.as_mut_slice()
    }
}

pub struct WasmFunction {
    type_ref: usize,
    origin: WasmFunctionOrigin,
    body: Option<WasmFunctionBody>,
}

impl WasmFunction {
    fn from_import(type_ref: usize, index: usize) -> Self {
        Self {
            type_ref,
            origin: WasmFunctionOrigin::Import(index),
            body: None,
        }
    }

    fn internal(type_ref: usize) -> Self {
        Self {
            type_ref,
            origin: WasmFunctionOrigin::Internal,
            body: None,
        }
    }

    pub fn type_ref(&self) -> usize {
        self.type_ref
    }

    pub fn origin(&self) -> WasmFunctionOrigin {
        self.origin
    }

    pub fn invoke(&self, params: &[WasmValue]) -> Result<WasmValue, WasmRuntimeError> {
        let body = self.body.as_ref().ok_or(WasmRuntimeError::NoMethod)?;

        let mut locals = Vec::new();
        for param in params {
            locals.push(*param);
        }
        for local in &body.local_types {
            locals.push(WasmValue::default_for(*local));
        }

        let result_types = body.result_types.as_slice();

        let code_ref = body.code_block.borrow();
        let mut code_block = WasmCodeBlock::from_slice(&code_ref);
        code_block.invoke(locals.as_slice(), result_types)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum WasmFunctionOrigin {
    Internal,
    Export(usize),
    Import(usize),
}

#[derive(Debug)]
pub struct WasmType {
    params: Vec<WasmValType>,
    result: Vec<WasmValType>,
}

impl WasmType {
    fn from_stream(stream: &mut Leb128Stream) -> Result<Self, WasmDecodeError> {
        match stream.read_uint() {
            Ok(0x60) => (),
            Err(err) => return Err(err),
            _ => return Err(WasmDecodeError::UnexpectedToken),
        };
        let n_params = stream.read_uint()? as usize;
        let mut params = Vec::with_capacity(n_params);
        for _ in 0..n_params {
            stream
                .read_uint()
                .and_then(|v| WasmValType::from_u64(v))
                .map(|v| params.push(v))?;
        }
        let n_result = stream.read_uint()? as usize;
        let mut result = Vec::with_capacity(n_result);
        for _ in 0..n_result {
            stream
                .read_uint()
                .and_then(|v| WasmValType::from_u64(v))
                .map(|v| result.push(v))?;
        }
        Ok(Self { params, result })
    }

    pub fn param_types(&self) -> &[WasmValType] {
        self.params.as_slice()
    }

    pub fn result_types(&self) -> &[WasmValType] {
        self.result.as_slice()
    }
}

impl fmt::Display for WasmType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.params.len() > 0 {
            write!(f, " (param")?;
            for param in &self.params {
                write!(f, " {}", param)?;
            }
            write!(f, ")")?;
        }
        if self.result.len() > 0 {
            write!(f, " (result")?;
            for result in &self.result {
                write!(f, " {}", result)?;
            }
            write!(f, ")")?;
        }
        Ok(())
    }
}

pub struct WasmImport {
    mod_name: String,
    name: String,
    index: WasmImportType,
}

impl WasmImport {
    fn from_stream(stream: &mut Leb128Stream) -> Result<Self, WasmDecodeError> {
        let mod_name = stream.get_string()?.to_string();
        let name = stream.get_string()?.to_string();
        let index = WasmImportType::from_stream(stream)?;

        Ok(Self {
            mod_name,
            name,
            index,
        })
    }

    pub fn mod_name(&self) -> &str {
        self.mod_name.as_ref()
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    pub const fn index(&self) -> WasmImportType {
        self.index
    }
}

#[derive(Debug, Copy, Clone)]
pub enum WasmImportType {
    Type(usize),
    Table(usize),
    Memory(usize),
    Global(usize),
}

impl WasmImportType {
    fn from_stream(stream: &mut Leb128Stream) -> Result<Self, WasmDecodeError> {
        stream.read_uint().and_then(|v| match v {
            0 => stream.read_uint().map(|v| WasmImportType::Type(v as usize)),
            1 => stream
                .read_uint()
                .map(|v| WasmImportType::Table(v as usize)),
            2 => stream
                .read_uint()
                .map(|v| WasmImportType::Memory(v as usize)),
            3 => stream
                .read_uint()
                .map(|v| WasmImportType::Global(v as usize)),
            _ => Err(WasmDecodeError::UnexpectedToken),
        })
    }
}

pub struct WasmExport {
    name: String,
    index: WasmExportType,
}

impl WasmExport {
    fn from_stream(stream: &mut Leb128Stream) -> Result<Self, WasmDecodeError> {
        let name = stream.get_string()?.to_string();
        let index = WasmExportType::from_stream(stream)?;
        Ok(Self { name, index })
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    pub const fn index(&self) -> WasmExportType {
        self.index
    }
}

#[derive(Debug, Copy, Clone)]
pub enum WasmExportType {
    Function(usize),
    Table(usize),
    Memory(usize),
    Global(usize),
}

impl WasmExportType {
    fn from_stream(stream: &mut Leb128Stream) -> Result<Self, WasmDecodeError> {
        stream.read_uint().and_then(|v| match v {
            0 => stream
                .read_uint()
                .map(|v| WasmExportType::Function(v as usize)),
            1 => stream
                .read_uint()
                .map(|v| WasmExportType::Table(v as usize)),
            2 => stream
                .read_uint()
                .map(|v| WasmExportType::Memory(v as usize)),
            3 => stream
                .read_uint()
                .map(|v| WasmExportType::Global(v as usize)),
            _ => Err(WasmDecodeError::UnexpectedToken),
        })
    }
}

pub struct WasmFunctionBody {
    #[allow(dead_code)]
    param_types: Vec<WasmValType>,
    local_types: Vec<WasmValType>,
    result_types: Vec<WasmValType>,
    code_block: Arc<RefCell<Vec<u8>>>,
}

impl WasmFunctionBody {
    fn from_stream(
        stream: &mut Leb128Stream,
        func_type: &WasmType,
    ) -> Result<Self, WasmDecodeError> {
        let blob = stream.read_bytes()?;
        let mut stream = Leb128Stream::from_slice(blob);
        let n_locals = stream.read_uint()? as usize;
        let mut locals = Vec::new();
        for _ in 0..n_locals {
            let repeat = stream.read_uint()?;
            let val = stream.read_uint().and_then(|v| WasmValType::from_u64(v))?;
            for _ in 0..repeat {
                locals.push(val);
            }
        }
        let code_block = Arc::new(RefCell::new(blob[stream.position..].to_vec()));
        Ok(Self {
            param_types: func_type.params.clone(),
            local_types: locals,
            result_types: func_type.result.clone(),
            code_block,
        })
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
    UnexpectedToken,
    InvalidParameter,
    InvalidBytecode,
    NoMethod,
    TypeMismatch,
    DivideByZero,
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

    pub fn get_i32(self) -> Result<i32, WasmRuntimeError> {
        match self {
            Self::I32(a) => Ok(a),
            _ => return Err(WasmRuntimeError::TypeMismatch),
        }
    }

    pub fn get_i64(self) -> Result<i64, WasmRuntimeError> {
        match self {
            Self::I64(a) => Ok(a),
            _ => return Err(WasmRuntimeError::TypeMismatch),
        }
    }

    pub fn map_i32<F>(self, f: F) -> Result<WasmValue, WasmRuntimeError>
    where
        F: FnOnce(i32) -> i32,
    {
        match self {
            Self::I32(a) => Ok(f(a).into()),
            _ => return Err(WasmRuntimeError::TypeMismatch),
        }
    }

    pub fn map_i64<F>(self, f: F) -> Result<WasmValue, WasmRuntimeError>
    where
        F: FnOnce(i64) -> i64,
    {
        match self {
            Self::I64(a) => Ok(f(a).into()),
            _ => return Err(WasmRuntimeError::TypeMismatch),
        }
    }

    pub fn clz(self) -> Result<WasmValue, WasmRuntimeError> {
        match self {
            Self::I32(a) => Ok(WasmValue::I32(a.leading_zeros() as i32)),
            Self::I64(a) => Ok(WasmValue::I64(a.leading_zeros() as i64)),
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }

    pub fn ctz(self) -> Result<WasmValue, WasmRuntimeError> {
        match self {
            Self::I32(a) => Ok(WasmValue::I32(a.trailing_zeros() as i32)),
            Self::I64(a) => Ok(WasmValue::I64(a.trailing_zeros() as i64)),
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }

    pub fn popcnt(self) -> Result<WasmValue, WasmRuntimeError> {
        match self {
            Self::I32(a) => Ok(WasmValue::I32(a.count_ones() as i32)),
            Self::I64(a) => Ok(WasmValue::I64(a.count_ones() as i64)),
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }

    pub fn div_u(self, rhs: Self) -> Result<WasmValue, WasmRuntimeError> {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                if b == 0 {
                    return Err(WasmRuntimeError::DivideByZero);
                }
                Ok(WasmValue::I32((a as u32 / b as u32) as i32))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                if b == 0 {
                    return Err(WasmRuntimeError::DivideByZero);
                }
                Ok(WasmValue::I64((a as u64 / b as u64) as i64))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }

    pub fn rem_u(self, rhs: Self) -> Result<WasmValue, WasmRuntimeError> {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                if b == 0 {
                    return Err(WasmRuntimeError::DivideByZero);
                }
                Ok(WasmValue::I32((a as u32 % b as u32) as i32))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                if b == 0 {
                    return Err(WasmRuntimeError::DivideByZero);
                }
                Ok(WasmValue::I64((a as u64 % b as u64) as i64))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }

    pub fn shr_u(self, rhs: Self) -> Result<WasmValue, WasmRuntimeError> {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32((a as u32 >> b as u32) as i32))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I64((a as u64 >> b as u64) as i64))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }

    pub fn rotl(self, rhs: Self) -> Result<WasmValue, WasmRuntimeError> {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(a.rotate_left(b as u32)))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I64(a.rotate_left(b as u32)))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }

    pub fn rotr(self, rhs: Self) -> Result<WasmValue, WasmRuntimeError> {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(a.rotate_right(b as u32)))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I64(a.rotate_right(b as u32)))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }

    pub fn eqz(self) -> Result<WasmValue, WasmRuntimeError> {
        match self {
            Self::I32(a) => Ok(WasmValue::I32(if a == 0 { 1 } else { 0 })),
            Self::I64(a) => Ok(WasmValue::I32(if a == 0 { 1 } else { 0 })),
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }

    pub fn eq(self, rhs: Self) -> Result<WasmValue, WasmRuntimeError> {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(if a == b { 1 } else { 0 }))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(if a == b { 1 } else { 0 }))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }

    pub fn ne(self, rhs: Self) -> Result<WasmValue, WasmRuntimeError> {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(if a != b { 1 } else { 0 }))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(if a != b { 1 } else { 0 }))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }

    pub fn lt_s(self, rhs: Self) -> Result<WasmValue, WasmRuntimeError> {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(if a < b { 1 } else { 0 }))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(if a < b { 1 } else { 0 }))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }

    pub fn lt_u(self, rhs: Self) -> Result<WasmValue, WasmRuntimeError> {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(if (a as u32) < (b as u32) { 1 } else { 0 }))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(if (a as u64) < (b as u64) { 1 } else { 0 }))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }

    pub fn le_s(self, rhs: Self) -> Result<WasmValue, WasmRuntimeError> {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(if a <= b { 1 } else { 0 }))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(if a <= b { 1 } else { 0 }))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }

    pub fn le_u(self, rhs: Self) -> Result<WasmValue, WasmRuntimeError> {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(if (a as u32) <= (b as u32) { 1 } else { 0 }))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(if (a as u64) <= (b as u64) { 1 } else { 0 }))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }

    pub fn gt_s(self, rhs: Self) -> Result<WasmValue, WasmRuntimeError> {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(if a > b { 1 } else { 0 }))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(if a > b { 1 } else { 0 }))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }

    pub fn gt_u(self, rhs: Self) -> Result<WasmValue, WasmRuntimeError> {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(if (a as u32) > (b as u32) { 1 } else { 0 }))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(if (a as u64) > (b as u64) { 1 } else { 0 }))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }

    pub fn ge_s(self, rhs: Self) -> Result<WasmValue, WasmRuntimeError> {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(if a >= b { 1 } else { 0 }))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(if a >= b { 1 } else { 0 }))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }

    pub fn ge_u(self, rhs: Self) -> Result<WasmValue, WasmRuntimeError> {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(if (a as u32) >= (b as u32) { 1 } else { 0 }))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(if (a as u64) >= (b as u64) { 1 } else { 0 }))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }
}

impl Add for WasmValue {
    type Output = Result<WasmValue, WasmRuntimeError>;
    fn add(self, rhs: Self) -> Self::Output {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(a + b))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I64(a + b))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }
}

impl Sub for WasmValue {
    type Output = Result<WasmValue, WasmRuntimeError>;
    fn sub(self, rhs: Self) -> Self::Output {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(a - b))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I64(a - b))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }
}

impl Mul for WasmValue {
    type Output = Result<WasmValue, WasmRuntimeError>;
    fn mul(self, rhs: Self) -> Self::Output {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(a * b))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I64(a * b))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }
}

impl Div for WasmValue {
    type Output = Result<WasmValue, WasmRuntimeError>;
    fn div(self, rhs: Self) -> Self::Output {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                if b == 0 {
                    return Err(WasmRuntimeError::DivideByZero);
                }
                Ok(WasmValue::I32(a / b))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                if b == 0 {
                    return Err(WasmRuntimeError::DivideByZero);
                }
                Ok(WasmValue::I64(a / b))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }
}

impl Rem for WasmValue {
    type Output = Result<WasmValue, WasmRuntimeError>;
    fn rem(self, rhs: Self) -> Self::Output {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                if b == 0 {
                    return Err(WasmRuntimeError::DivideByZero);
                }
                Ok(WasmValue::I32(a % b))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                if b == 0 {
                    return Err(WasmRuntimeError::DivideByZero);
                }
                Ok(WasmValue::I64(a % b))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }
}

impl BitAnd for WasmValue {
    type Output = Result<WasmValue, WasmRuntimeError>;
    fn bitand(self, rhs: Self) -> Self::Output {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(a & b))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I64(a & b))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }
}

impl BitOr for WasmValue {
    type Output = Result<WasmValue, WasmRuntimeError>;
    fn bitor(self, rhs: Self) -> Self::Output {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(a | b))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I64(a | b))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }
}

impl BitXor for WasmValue {
    type Output = Result<WasmValue, WasmRuntimeError>;
    fn bitxor(self, rhs: Self) -> Self::Output {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(a ^ b))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I64(a ^ b))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }
}

impl Shl for WasmValue {
    type Output = Result<WasmValue, WasmRuntimeError>;
    fn shl(self, rhs: Self) -> Self::Output {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(a << b))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I64(a << b))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }
}

impl Shr for WasmValue {
    type Output = Result<WasmValue, WasmRuntimeError>;
    fn shr(self, rhs: Self) -> Self::Output {
        match self {
            Self::I32(a) => {
                let b = match rhs {
                    Self::I32(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I32(a >> b))
            }
            Self::I64(a) => {
                let b = match rhs {
                    Self::I64(v) => v,
                    _ => return Err(WasmRuntimeError::TypeMismatch),
                };
                Ok(WasmValue::I64(a >> b))
            }
            _ => return Err(WasmRuntimeError::InvalidParameter),
        }
    }
}

impl From<i32> for WasmValue {
    fn from(v: i32) -> Self {
        Self::I32(v)
    }
}

impl From<u32> for WasmValue {
    fn from(v: u32) -> Self {
        Self::I32(v as i32)
    }
}

impl From<i64> for WasmValue {
    fn from(v: i64) -> Self {
        Self::I64(v)
    }
}

impl From<u64> for WasmValue {
    fn from(v: u64) -> Self {
        Self::I64(v as i64)
    }
}

impl From<f32> for WasmValue {
    fn from(v: f32) -> Self {
        Self::F32(v)
    }
}

impl From<f64> for WasmValue {
    fn from(v: f64) -> Self {
        Self::F64(v)
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

pub struct WasmCodeBlock<'a> {
    code: Leb128Stream<'a>,
}

impl<'a> WasmCodeBlock<'a> {
    pub fn from_slice(slice: &'a [u8]) -> Self {
        Self {
            code: Leb128Stream::from_slice(slice),
        }
    }

    pub const fn position(&self) -> usize {
        self.code.position()
    }

    pub fn get_opcode(&mut self) -> Result<WasmOpcode, WasmRuntimeError> {
        self.code
            .read_byte()
            .map(|v| WasmOpcode::from_u8(v))
            .map_err(|err| Self::map_err(err))
    }

    pub fn get_sint(&mut self) -> Result<i64, WasmRuntimeError> {
        self.code.read_sint().map_err(|err| Self::map_err(err))
    }

    pub fn get_uint(&mut self) -> Result<u64, WasmRuntimeError> {
        self.code.read_uint().map_err(|err| Self::map_err(err))
    }

    fn map_err(err: WasmDecodeError) -> WasmRuntimeError {
        match err {
            WasmDecodeError::UnexpectedEof => WasmRuntimeError::UnexpectedEof,
            _ => WasmRuntimeError::UnexpectedToken,
        }
    }

    pub fn invoke(
        &mut self,
        locals: &[WasmValue],
        result_types: &[WasmValType],
    ) -> Result<WasmValue, WasmRuntimeError> {
        let mut ctx = WasmRuntimeContext::new();
        ctx.run(self, locals, result_types)
    }
}
