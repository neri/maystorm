// WebAssembly Loader

use super::opcode::*;
use crate::intcode::*;
use crate::wasmintr::*;
use crate::*;
use _core::borrow::BorrowMut;
use alloc::{boxed::Box, string::*, sync::Arc, vec::Vec};
use bitflags::*;
use byteorder::*;
use core::{
    cell::{RefCell, UnsafeCell},
    convert::TryFrom,
    fmt,
    ops::*,
    slice, str,
};

pub struct WasmLoader {
    module: WasmModule,
}

pub type WasmDynFunc = fn(&WasmModule, &[WasmValue]) -> Result<WasmValue, WasmRuntimeError>;

impl WasmLoader {
    /// Minimal valid module size, Magic(4) + Version(4) + Empty sections(0) = 8
    const MINIMAL_MOD_SIZE: usize = 8;
    /// Magic number of WebAssembly Binary Format
    const MAGIC: u32 = 0x6D736100;
    /// Current Version
    const VER_CURRENT: u32 = 0x0000_0001;

    #[inline]
    pub const fn new() -> Self {
        Self {
            module: WasmModule::new(),
        }
    }

    /// Identify the file format
    #[inline]
    pub fn identity(blob: &[u8]) -> bool {
        blob.len() >= Self::MINIMAL_MOD_SIZE
            && LE::read_u32(&blob[0..4]) == Self::MAGIC
            && LE::read_u32(&blob[4..8]) == Self::VER_CURRENT
    }

    /// Instantiate wasm modules from slice
    pub fn instantiate<F>(blob: &[u8], resolver: F) -> Result<WasmModule, WasmDecodeError>
    where
        F: FnMut(&str, &str, &WasmType) -> Result<WasmDynFunc, WasmDecodeError> + Copy,
    {
        if Self::identity(blob) {
            let mut loader = Self::new();
            loader.load(blob, resolver).map(|_| loader.module)
        } else {
            return Err(WasmDecodeError::BadExecutable);
        }
    }

    /// Load wasm from slice
    pub fn load<F>(&mut self, blob: &[u8], resolver: F) -> Result<(), WasmDecodeError>
    where
        F: FnMut(&str, &str, &WasmType) -> Result<WasmDynFunc, WasmDecodeError> + Copy,
    {
        let mut blob = Leb128Stream::from_slice(&blob[8..]);
        while let Some(mut section) = blob.next_section()? {
            match section.section_type {
                WasmSectionType::Custom => Ok(()),
                WasmSectionType::Type => self.parse_sec_type(&mut section),
                WasmSectionType::Import => self.parse_sec_import(&mut section, resolver),
                WasmSectionType::Table => self.parse_sec_table(&mut section),
                WasmSectionType::Memory => self.parse_sec_memory(&mut section),
                WasmSectionType::Element => self.parse_sec_elem(&mut section),
                WasmSectionType::Function => self.parse_sec_func(&mut section),
                WasmSectionType::Export => self.parse_sec_export(&mut section),
                WasmSectionType::Code => self.parse_sec_code(&mut section),
                WasmSectionType::Data => self.parse_sec_data(&mut section),
                WasmSectionType::Start => self.parse_sec_start(&mut section),
                WasmSectionType::Global => self.parse_sec_global(&mut section),
                // _ => Err(WasmDecodeError::UnexpectedToken),
            }?;
        }

        self.module.types.shrink_to_fit();
        self.module.imports.shrink_to_fit();
        self.module.functions.shrink_to_fit();
        self.module.tables.shrink_to_fit();
        self.module.memories.shrink_to_fit();
        self.module.exports.shrink_to_fit();
        self.module.globals.shrink_to_fit();

        Ok(())
    }

    #[inline]
    pub const fn module(&self) -> &WasmModule {
        &self.module
    }

    #[inline]
    pub fn into_module(self) -> WasmModule {
        self.module
    }

    /// Parse "type" section
    fn parse_sec_type(&mut self, section: &mut WasmSection) -> Result<(), WasmDecodeError> {
        let n_items = section.stream.read_unsigned()? as usize;
        for _ in 0..n_items {
            let ft = WasmType::from_stream(&mut section.stream)?;
            self.module.types.push(ft);
        }
        Ok(())
    }

    /// Parse "import" section
    fn parse_sec_import<F>(
        &mut self,
        section: &mut WasmSection,
        mut resolver: F,
    ) -> Result<(), WasmDecodeError>
    where
        F: FnMut(&str, &str, &WasmType) -> Result<WasmDynFunc, WasmDecodeError> + Copy,
    {
        let n_items = section.stream.read_unsigned()? as usize;
        for _ in 0..n_items {
            let mut import = WasmImport::from_stream(&mut section.stream)?;
            match import.index {
                WasmImportIndex::Type(index) => {
                    import.func_ref = self.module.n_ext_func;
                    let func_type = self
                        .module
                        .types
                        .get(index)
                        .ok_or(WasmDecodeError::InvalidType)?;
                    let dlink = resolver(import.mod_name(), import.name(), func_type)?;
                    self.module.functions.push(WasmFunction::from_import(
                        index,
                        func_type,
                        self.module.n_ext_func,
                        dlink,
                    ));
                    self.module.n_ext_func += 1;
                }
                WasmImportIndex::Memory(memtype) => {
                    // TODO: import memory
                    self.module.memories.push(WasmMemory::new(memtype));
                }
            }
            self.module.imports.push(import);
        }
        Ok(())
    }

    /// Parse "func" section
    fn parse_sec_func(&mut self, section: &mut WasmSection) -> Result<(), WasmDecodeError> {
        let n_items = section.stream.read_unsigned()? as usize;
        let base_index = self.module.imports.len();
        for index in 0..n_items {
            let type_index = section.stream.read_unsigned()? as usize;
            let func_type = self
                .module
                .types
                .get(type_index)
                .ok_or(WasmDecodeError::InvalidType)?;
            self.module.functions.push(WasmFunction::internal(
                base_index + index,
                type_index,
                func_type,
            ));
        }
        Ok(())
    }

    /// Parse "export" section
    fn parse_sec_export(&mut self, section: &mut WasmSection) -> Result<(), WasmDecodeError> {
        let n_items = section.stream.read_unsigned()? as usize;
        for i in 0..n_items {
            let export = WasmExport::from_stream(&mut section.stream)?;
            if let WasmExportIndex::Function(index) = export.index {
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
        let n_items = section.stream.read_unsigned()?;
        for _ in 0..n_items {
            let limit = WasmLimit::from_stream(&mut section.stream)?;
            self.module.memories.push(WasmMemory::new(limit));
        }
        Ok(())
    }

    /// Parse "table" section
    fn parse_sec_table(&mut self, section: &mut WasmSection) -> Result<(), WasmDecodeError> {
        let n_items = section.stream.read_unsigned()?;
        for _ in 0..n_items {
            let table = WasmTable::from_stream(&mut section.stream)?;
            self.module.tables.push(table);
        }
        Ok(())
    }

    /// Parse "elem" section
    fn parse_sec_elem(&mut self, section: &mut WasmSection) -> Result<(), WasmDecodeError> {
        let n_items = section.stream.read_unsigned()?;
        for _ in 0..n_items {
            let tabidx = section.stream.read_unsigned()? as usize;
            let offset = self.eval_offset(&mut section.stream)? as usize;
            let n_elements = section.stream.read_unsigned()? as usize;
            let table = self
                .module
                .tables
                .get_mut(tabidx)
                .ok_or(WasmDecodeError::InvalidParameter)?;
            for i in offset..offset + n_elements {
                let elem = section.stream.read_unsigned()? as usize;
                table.table.get_mut(i).map(|v| *v = elem);
            }
        }
        Ok(())
    }

    /// Parse "code" section
    fn parse_sec_code(&mut self, section: &mut WasmSection) -> Result<(), WasmDecodeError> {
        let n_items = section.stream.read_unsigned()? as usize;
        for i in 0..n_items {
            let index = i + self.module.n_ext_func;
            let module = &mut self.module;
            let func_def = module
                .functions
                .get(index)
                .ok_or(WasmDecodeError::InvalidParameter)?;
            let body = WasmFunctionBody::from_stream(
                index,
                &mut section.stream,
                func_def.param_types(),
                func_def.result_types(),
                module,
            )?;
            self.module.functions[index].body = Some(body);
        }
        Ok(())
    }

    /// Parse "data" section
    fn parse_sec_data(&mut self, section: &mut WasmSection) -> Result<(), WasmDecodeError> {
        let n_items = section.stream.read_unsigned()?;
        for _ in 0..n_items {
            let memidx = section.stream.read_unsigned()? as usize;
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
        let index = section.stream.read_unsigned()? as usize;
        self.module.start = Some(index);
        Ok(())
    }

    /// Parse "global" section
    fn parse_sec_global(&mut self, section: &mut WasmSection) -> Result<(), WasmDecodeError> {
        let n_items = section.stream.read_unsigned()? as usize;
        for _ in 0..n_items {
            let val_type = section
                .stream
                .read_byte()
                .and_then(|v| WasmValType::from_u64(v as u64))?;
            let is_mutable = section.stream.read_byte()? == 1;
            let value = self.eval_expr(&mut section.stream)?;

            if !value.is_valid_type(val_type) {
                return Err(WasmDecodeError::InvalidGlobal);
            }

            let global = WasmGlobal {
                val_type,
                is_mutable,
                value: RefCell::new(value),
            };
            self.module.globals.push(global);
        }
        Ok(())
    }

    fn eval_offset(&self, mut stream: &mut Leb128Stream) -> Result<usize, WasmDecodeError> {
        self.eval_expr(&mut stream)
            .and_then(|v| v.get_i32().map_err(|_| WasmDecodeError::InvalidParameter))
            .map(|v| v as usize)
    }

    fn eval_expr(&self, stream: &mut Leb128Stream) -> Result<WasmValue, WasmDecodeError> {
        stream
            .read_byte()
            .and_then(|opc| match WasmOpcode::from_u8(opc) {
                WasmOpcode::I32Const => stream.read_signed().and_then(|r| {
                    match stream.read_byte().map(|v| WasmOpcode::from_u8(v)) {
                        Ok(WasmOpcode::End) => Ok(WasmValue::I32(r as i32)),
                        _ => Err(WasmDecodeError::UnexpectedToken),
                    }
                }),
                WasmOpcode::I64Const => stream.read_signed().and_then(|r| {
                    match stream.read_byte().map(|v| WasmOpcode::from_u8(v)) {
                        Ok(WasmOpcode::End) => Ok(WasmValue::I64(r)),
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
    globals: Vec<WasmGlobal>,
    n_ext_func: usize,
}

impl WasmModule {
    #[inline]
    pub const fn new() -> Self {
        Self {
            types: Vec::new(),
            memories: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            tables: Vec::new(),
            functions: Vec::new(),
            start: None,
            globals: Vec::new(),
            n_ext_func: 0,
        }
    }

    #[inline]
    pub fn types(&self) -> &[WasmType] {
        self.types.as_slice()
    }

    #[inline]
    pub fn type_by_ref(&self, index: usize) -> Option<&WasmType> {
        self.types.get(index)
    }

    #[inline]
    pub fn imports(&self) -> &[WasmImport] {
        self.imports.as_slice()
    }

    #[inline]
    pub fn exports(&self) -> &[WasmExport] {
        self.exports.as_slice()
    }

    #[inline]
    pub fn memories(&mut self) -> &mut [WasmMemory] {
        self.memories.as_mut_slice()
    }

    #[inline]
    pub fn memory(&self, index: usize) -> Option<&WasmMemory> {
        self.memories.get(index)
    }

    #[inline]
    pub fn tables(&mut self) -> &mut [WasmTable] {
        self.tables.as_mut_slice()
    }

    #[inline]
    pub fn elem_by_index(&self, index: usize) -> Option<&WasmFunction> {
        self.tables
            .get(0)
            .and_then(|v| v.table.get(index))
            .and_then(|v| self.functions.get(*v))
    }

    #[inline]
    pub fn functions(&self) -> &[WasmFunction] {
        self.functions.as_slice()
    }

    #[inline]
    pub fn func_by_index(&self, index: usize) -> Result<WasmRunnable, WasmRuntimeError> {
        self.functions
            .get(index)
            .map(|v| WasmRunnable::from_function(v, self))
            .ok_or(WasmRuntimeError::NoMethod)
    }

    #[inline]
    pub fn entry_point(&self) -> Result<WasmRunnable, WasmRuntimeError> {
        self.start
            .ok_or(WasmRuntimeError::NoMethod)
            .and_then(|v| self.func_by_index(v))
    }

    /// Get a reference to the exported function with the specified name
    #[inline]
    pub fn func(&self, name: &str) -> Result<WasmRunnable, WasmRuntimeError> {
        for export in &self.exports {
            if let WasmExportIndex::Function(v) = export.index {
                if export.name == name {
                    return self.func_by_index(v);
                }
            }
        }
        Err(WasmRuntimeError::NoMethod)
    }

    #[inline]
    pub fn globals(&self) -> &[WasmGlobal] {
        self.globals.as_slice()
    }

    #[inline]
    pub fn global(&self, index: usize) -> Option<&WasmGlobal> {
        self.globals.get(index)
    }
}

pub struct Leb128Stream<'a> {
    blob: &'a [u8],
    position: usize,
}

impl<'a> Leb128Stream<'a> {
    /// Instantiates from a slice
    #[inline]
    pub const fn from_slice(slice: &'a [u8]) -> Self {
        Self {
            blob: slice,
            position: 0,
        }
    }
}

#[allow(dead_code)]
impl Leb128Stream<'_> {
    /// Returns to the origin of the stream
    #[inline]
    pub fn reset(&mut self) {
        self.position = 0;
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.blob.len()
    }

    /// Gets current position of stream
    #[inline]
    pub const fn position(&self) -> usize {
        self.position
    }

    #[inline]
    pub fn set_position(&mut self, val: usize) {
        self.position = val;
    }

    /// Returns whether the end of the stream has been reached
    #[inline]
    pub const fn is_eof(&self) -> bool {
        self.position >= self.blob.len()
    }

    /// Reads one byte from a stream
    #[inline]
    pub fn read_byte(&mut self) -> Result<u8, WasmDecodeError> {
        if self.is_eof() {
            return Err(WasmDecodeError::UnexpectedEof);
        }
        let d = self.blob[self.position];
        self.position += 1;
        Ok(d)
    }

    /// Returns a slice of the specified number of bytes from the stream
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

    /// Reads multiple bytes from the stream
    #[inline]
    pub fn read_bytes(&mut self) -> Result<&[u8], WasmDecodeError> {
        self.read_unsigned()
            .and_then(move |size| self.get_bytes(size as usize))
    }

    /// Reads an unsigned integer from a stream
    pub fn read_unsigned(&mut self) -> Result<u64, WasmDecodeError> {
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

    /// Reads a signed integer from a stream
    pub fn read_signed(&mut self) -> Result<i64, WasmDecodeError> {
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

    /// Reads the UTF-8 encoded string from the stream
    #[inline]
    pub fn get_string(&mut self) -> Result<&str, WasmDecodeError> {
        self.read_bytes()
            .and_then(|v| str::from_utf8(v).map_err(|_| WasmDecodeError::UnexpectedToken))
    }

    #[inline]
    pub fn read_opcode(&mut self) -> Result<WasmOpcode, WasmDecodeError> {
        self.read_byte()
            .and_then(|v| WasmOpcode::try_from(v).map_err(|_| WasmDecodeError::InvalidBytecode))
    }

    #[inline]
    pub fn read_memarg(&mut self) -> Result<WasmMemArg, WasmDecodeError> {
        let a = self.read_unsigned()? as u32;
        let o = self.read_unsigned()? as u32;
        Ok(WasmMemArg::new(o, a))
    }

    fn next_section(&mut self) -> Result<Option<WasmSection>, WasmDecodeError> {
        let section_type = match self.read_byte().ok() {
            Some(v) => v,
            None => return Ok(None),
        };

        let blob = self.read_bytes()?;
        let stream = Leb128Stream::from_slice(blob);
        Ok(Some(WasmSection {
            section_type: section_type.into(),
            stream,
        }))
    }
}

#[derive(Debug, Copy, Clone)]
pub struct WasmMemArg {
    pub align: u32,
    pub offset: u32,
}

impl WasmMemArg {
    #[inline]
    pub const fn new(offset: u32, align: u32) -> Self {
        Self { offset, align }
    }

    #[inline]
    pub const fn offset_by(&self, base: u32) -> usize {
        (self.offset as u64 + base as u64) as usize
    }
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
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
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

#[repr(isize)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WasmBlockType {
    Empty = -64,
    I32 = -1,
    I64 = -2,
    F32 = -3,
    F64 = -4,
}

impl WasmBlockType {
    pub const fn from_i64(v: i64) -> Result<Self, WasmDecodeError> {
        match v {
            -64 => Ok(Self::Empty),
            -1 => Ok(Self::I32),
            -2 => Ok(Self::I64),
            -3 => Ok(Self::F32),
            -4 => Ok(Self::F64),
            _ => Err(WasmDecodeError::InvalidParameter),
        }
    }

    pub const fn into_type(self) -> Option<WasmValType> {
        match self {
            WasmBlockType::Empty => None,
            WasmBlockType::I32 => Some(WasmValType::I32),
            WasmBlockType::I64 => Some(WasmValType::I64),
            WasmBlockType::F32 => Some(WasmValType::F32),
            WasmBlockType::F64 => Some(WasmValType::F64),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct WasmLimit {
    min: u32,
    max: u32,
}

impl WasmLimit {
    #[inline]
    fn from_stream(stream: &mut Leb128Stream) -> Result<Self, WasmDecodeError> {
        match stream.read_unsigned() {
            Ok(0) => stream.read_unsigned().map(|min| Self {
                min: min as u32,
                max: min as u32,
            }),
            Ok(1) => {
                let min = stream.read_unsigned()? as u32;
                let max = stream.read_unsigned()? as u32;
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

    #[inline]
    fn new(limit: WasmLimit) -> Self {
        let size = limit.min as usize * Self::PAGE_SIZE;
        let mut memory = Vec::with_capacity(size);
        memory.resize(size, 0);
        Self {
            limit,
            memory: Arc::new(UnsafeCell::new(memory)),
        }
    }

    #[inline]
    pub const fn limit(&self) -> WasmLimit {
        self.limit
    }

    // fn memory_arc(&self) -> Arc<UnsafeCell<Vec<u8>>> {
    //     self.memory.clone()
    // }

    #[inline]
    fn memory(&self) -> &[u8] {
        unsafe { self.memory.get().as_ref().unwrap() }
    }

    #[inline]
    fn memory_mut(&self) -> &mut [u8] {
        unsafe { self.memory.get().as_mut().unwrap() }
    }

    pub fn grow(&self, delta: usize) -> isize {
        if Arc::strong_count(&self.memory) > 1 {
            return -1;
        }
        let memory = unsafe { self.memory.get().as_mut().unwrap() };
        let old_size = memory.len();
        let additional = delta * Self::PAGE_SIZE;
        if memory.try_reserve_exact(additional).is_err() {
            return -1;
        }
        memory.resize(old_size + additional, 0);
        (old_size / Self::PAGE_SIZE) as isize
    }

    #[inline]
    pub fn size(&self) -> usize {
        let memory = self.memory();
        memory.len() / Self::PAGE_SIZE
    }

    /// Read the specified range of memory
    #[inline]
    pub fn read_bytes(&self, offset: usize, size: usize) -> Result<&[u8], WasmRuntimeError> {
        let memory = self.memory();
        let limit = memory.len();
        if offset < limit && size < limit && offset + size < limit {
            unsafe { Ok(slice::from_raw_parts(&memory[offset] as *const _, size)) }
        } else {
            Err(WasmRuntimeError::OutOfBounds)
        }
    }

    #[inline]
    pub fn read_u32_array(&self, offset: usize, len: usize) -> Result<&[u32], WasmRuntimeError> {
        let memory = self.memory();
        let limit = memory.len();
        let size = len * 4;
        if offset < limit && size < limit && offset + size < limit {
            unsafe {
                Ok(slice::from_raw_parts(
                    &memory[offset] as *const _ as *const u32,
                    len,
                ))
            }
        } else {
            Err(WasmRuntimeError::OutOfBounds)
        }
    }

    /// Write slice to memory
    #[inline]
    pub fn write_bytes(&mut self, offset: usize, src: &[u8]) -> Result<(), WasmRuntimeError> {
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
            Err(WasmRuntimeError::OutOfBounds)
        }
    }

    #[inline]
    pub fn read_u8(&self, offset: usize) -> Result<u8, WasmRuntimeError> {
        let slice = self.memory();
        slice
            .get(offset)
            .map(|v| *v)
            .ok_or(WasmRuntimeError::OutOfBounds)
    }

    #[inline]
    pub fn write_u8(&self, offset: usize, val: u8) -> Result<(), WasmRuntimeError> {
        let slice = self.memory_mut();
        slice
            .get_mut(offset)
            .map(|v| *v = val)
            .ok_or(WasmRuntimeError::OutOfBounds)
    }

    #[inline]
    pub fn read_u16(&self, offset: usize) -> Result<u16, WasmRuntimeError> {
        let slice = self.memory();
        let limit = slice.len();
        if offset + 1 < limit {
            Ok(LE::read_u16(&slice[offset..offset + 2]))
        } else {
            Err(WasmRuntimeError::OutOfBounds)
        }
    }

    #[inline]
    pub fn write_u16(&self, offset: usize, val: u16) -> Result<(), WasmRuntimeError> {
        let slice = self.memory_mut();
        let limit = slice.len();
        if offset + 1 < limit {
            LE::write_u16(&mut slice[offset..offset + 2], val);
            Ok(())
        } else {
            Err(WasmRuntimeError::OutOfBounds)
        }
    }

    #[inline]
    pub fn read_u32(&self, offset: usize) -> Result<u32, WasmRuntimeError> {
        let slice = self.memory();
        let limit = slice.len();
        if offset + 3 < limit {
            Ok(LE::read_u32(&slice[offset..offset + 4]))
        } else {
            Err(WasmRuntimeError::OutOfBounds)
        }
    }

    #[inline]
    pub fn write_u32(&self, offset: usize, val: u32) -> Result<(), WasmRuntimeError> {
        let slice = self.memory_mut();
        let limit = slice.len();
        if offset + 3 < limit {
            LE::write_u32(&mut slice[offset..offset + 4], val);
            Ok(())
        } else {
            Err(WasmRuntimeError::OutOfBounds)
        }
    }

    #[inline]
    pub fn read_u64(&self, offset: usize) -> Result<u64, WasmRuntimeError> {
        let slice = self.memory();
        let limit = slice.len();
        if offset + 7 < limit {
            Ok(LE::read_u64(&slice[offset..offset + 8]))
        } else {
            Err(WasmRuntimeError::OutOfBounds)
        }
    }

    #[inline]
    pub fn write_u64(&self, offset: usize, val: u64) -> Result<(), WasmRuntimeError> {
        let slice = self.memory_mut();
        let limit = slice.len();
        if offset + 7 < limit {
            LE::write_u64(&mut slice[offset..offset + 8], val);
            Ok(())
        } else {
            Err(WasmRuntimeError::OutOfBounds)
        }
    }
}

pub struct WasmTable {
    limit: WasmLimit,
    table: Vec<usize>,
}

impl WasmTable {
    #[inline]
    fn from_stream(stream: &mut Leb128Stream) -> Result<Self, WasmDecodeError> {
        match stream.read_unsigned() {
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

    #[inline]
    pub const fn limit(&self) -> WasmLimit {
        self.limit
    }

    #[inline]
    pub fn table(&mut self) -> &mut [usize] {
        self.table.as_mut_slice()
    }
}

pub struct WasmFunction {
    index: usize,
    type_index: usize,
    func_type: WasmType,
    origin: WasmFunctionOrigin,
    body: Option<WasmFunctionBody>,
    dlink: Option<WasmDynFunc>,
}

impl WasmFunction {
    #[inline]
    fn from_import(
        type_index: usize,
        func_type: &WasmType,
        index: usize,
        dlink: WasmDynFunc,
    ) -> Self {
        Self {
            index,
            type_index,
            func_type: func_type.clone(),
            origin: WasmFunctionOrigin::Import(index),
            body: None,
            dlink: Some(dlink),
        }
    }

    #[inline]
    fn internal(index: usize, type_index: usize, func_type: &WasmType) -> Self {
        Self {
            index,
            type_index,
            func_type: func_type.clone(),
            origin: WasmFunctionOrigin::Internal,
            body: None,
            dlink: None,
        }
    }

    #[inline]
    pub const fn index(&self) -> usize {
        self.index
    }

    #[inline]
    pub const fn type_index(&self) -> usize {
        self.type_index
    }

    #[inline]
    pub const fn param_types(&self) -> &[WasmValType] {
        &self.func_type.param_types
    }

    #[inline]
    pub const fn result_types(&self) -> &[WasmValType] {
        &self.func_type.result_types
    }

    #[inline]
    pub const fn origin(&self) -> WasmFunctionOrigin {
        self.origin
    }

    #[inline]
    pub const fn body(&self) -> Option<&WasmFunctionBody> {
        self.body.as_ref()
    }

    #[inline]
    pub fn dlink(&self) -> Option<WasmDynFunc> {
        self.dlink
    }
}

#[derive(Debug, Copy, Clone)]
pub enum WasmFunctionOrigin {
    Internal,
    Export(usize),
    Import(usize),
}

#[derive(Debug, Clone)]
pub struct WasmType {
    param_types: Box<[WasmValType]>,
    result_types: Box<[WasmValType]>,
}

impl WasmType {
    fn from_stream(stream: &mut Leb128Stream) -> Result<Self, WasmDecodeError> {
        match stream.read_unsigned() {
            Ok(0x60) => (),
            Err(err) => return Err(err),
            _ => return Err(WasmDecodeError::UnexpectedToken),
        };
        let n_params = stream.read_unsigned()? as usize;
        let mut params = Vec::with_capacity(n_params);
        for _ in 0..n_params {
            stream
                .read_unsigned()
                .and_then(|v| WasmValType::from_u64(v))
                .map(|v| params.push(v))?;
        }
        let n_result = stream.read_unsigned()? as usize;
        let mut result = Vec::with_capacity(n_result);
        for _ in 0..n_result {
            stream
                .read_unsigned()
                .and_then(|v| WasmValType::from_u64(v))
                .map(|v| result.push(v))?;
        }
        Ok(Self {
            param_types: params.into_boxed_slice(),
            result_types: result.into_boxed_slice(),
        })
    }

    #[inline]
    pub fn param_types(&self) -> &[WasmValType] {
        &self.param_types
    }

    #[inline]
    pub fn result_types(&self) -> &[WasmValType] {
        &self.result_types
    }
}

impl fmt::Display for WasmType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.param_types.len() > 0 {
            write!(f, " (param")?;
            for param in self.param_types.into_iter() {
                write!(f, " {}", param)?;
            }
            write!(f, ")")?;
        }
        if self.result_types.len() > 0 {
            write!(f, " (result")?;
            for result in self.result_types.into_iter() {
                write!(f, " {}", result)?;
            }
            write!(f, ")")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct WasmImport {
    mod_name: String,
    name: String,
    index: WasmImportIndex,
    func_ref: usize,
}

impl WasmImport {
    #[inline]
    fn from_stream(stream: &mut Leb128Stream) -> Result<Self, WasmDecodeError> {
        let mod_name = stream.get_string()?.to_string();
        let name = stream.get_string()?.to_string();
        let index = WasmImportIndex::from_stream(stream)?;

        Ok(Self {
            mod_name,
            name,
            index,
            func_ref: 0,
        })
    }

    #[inline]
    pub fn mod_name(&self) -> &str {
        self.mod_name.as_ref()
    }

    #[inline]
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    #[inline]
    pub const fn index(&self) -> WasmImportIndex {
        self.index
    }
}

#[derive(Debug, Copy, Clone)]
pub enum WasmImportIndex {
    Type(usize),
    // Table(usize),
    Memory(WasmLimit),
    // Global(usize),
}

impl WasmImportIndex {
    #[inline]
    fn from_stream(mut stream: &mut Leb128Stream) -> Result<Self, WasmDecodeError> {
        stream.read_unsigned().and_then(|v| match v {
            0 => stream.read_unsigned().map(|v| Self::Type(v as usize)),
            // 1 => stream.read_unsigned().map(|v| Self::Table(v as usize)),
            2 => WasmLimit::from_stream(&mut stream).map(|v| Self::Memory(v)),
            // 3 => stream.read_unsigned().map(|v| Self::Global(v as usize)),
            _ => Err(WasmDecodeError::UnexpectedToken),
        })
    }
}

pub struct WasmExport {
    name: String,
    index: WasmExportIndex,
}

impl WasmExport {
    #[inline]
    fn from_stream(stream: &mut Leb128Stream) -> Result<Self, WasmDecodeError> {
        let name = stream.get_string()?.to_string();
        let index = WasmExportIndex::from_stream(stream)?;
        Ok(Self { name, index })
    }

    #[inline]
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    #[inline]
    pub const fn index(&self) -> WasmExportIndex {
        self.index
    }
}

#[derive(Debug, Copy, Clone)]
pub enum WasmExportIndex {
    Function(usize),
    Table(usize),
    Memory(usize),
    Global(usize),
}

impl WasmExportIndex {
    #[inline]
    fn from_stream(stream: &mut Leb128Stream) -> Result<Self, WasmDecodeError> {
        stream.read_unsigned().and_then(|v| match v {
            0 => stream.read_unsigned().map(|v| Self::Function(v as usize)),
            1 => stream.read_unsigned().map(|v| Self::Table(v as usize)),
            2 => stream.read_unsigned().map(|v| Self::Memory(v as usize)),
            3 => stream.read_unsigned().map(|v| Self::Global(v as usize)),
            _ => Err(WasmDecodeError::UnexpectedToken),
        })
    }
}

pub struct WasmFunctionBody {
    local_types: Box<[WasmValType]>,
    code_block: Box<[u8]>,
    block_info: WasmBlockInfo,
}

impl WasmFunctionBody {
    fn from_stream(
        func_index: usize,
        stream: &mut Leb128Stream,
        param_types: &[WasmValType],
        result_types: &[WasmValType],
        module: &WasmModule,
    ) -> Result<Self, WasmDecodeError> {
        let blob = stream.read_bytes()?;
        let mut stream = Leb128Stream::from_slice(blob);
        let n_locals = stream.read_unsigned()? as usize;
        let mut local_types = Vec::with_capacity(n_locals);
        for _ in 0..n_locals {
            let repeat = stream.read_unsigned()?;
            let val = stream
                .read_unsigned()
                .and_then(|v| WasmValType::from_u64(v))?;
            for _ in 0..repeat {
                local_types.push(val);
            }
        }
        let code_block = blob[stream.position..].to_vec().into_boxed_slice();

        let block_info = {
            let mut running_local_types = Vec::with_capacity(param_types.len() + local_types.len());
            for param_type in param_types {
                running_local_types.push(param_type.clone());
            }
            running_local_types.extend_from_slice(local_types.as_slice());
            let mut code_block = Leb128Stream::from_slice(&code_block);
            WasmBlockInfo::analyze(
                func_index,
                &mut code_block,
                &running_local_types,
                result_types,
                module,
            )
            .map_err(|err| err)
        }?;

        Ok(Self {
            local_types: local_types.into_boxed_slice(),
            code_block,
            block_info,
        })
    }

    #[inline]
    pub const fn local_types(&self) -> &[WasmValType] {
        &self.local_types
    }

    #[inline]
    pub const fn block_info(&self) -> &WasmBlockInfo {
        &self.block_info
    }

    #[inline]
    pub const fn code_block(&self) -> &[u8] {
        &self.code_block
    }
}

#[allow(dead_code)]
pub struct WasmGlobal {
    val_type: WasmValType,
    is_mutable: bool,
    value: RefCell<WasmValue>,
}

impl WasmGlobal {
    #[inline]
    pub const fn val_type(&self) -> WasmValType {
        self.val_type
    }

    #[inline]
    pub const fn is_mutable(&self) -> bool {
        self.is_mutable
    }

    #[inline]
    pub const fn value(&self) -> &RefCell<WasmValue> {
        &self.value
    }
}

#[derive(Debug, Copy, Clone)]
pub enum WasmDecodeError {
    UnexpectedEof,
    UnexpectedToken,
    InvalidParameter,
    InvalidBytecode,
    InvalidStackLevel,
    InvalidType,
    InvalidGlobal,
    InvalidLocal,
    OutOfStack,
    OutOfBranch,
    TypeMismatch,
    BlockMismatch,
    ElseWithoutIf,
    UnreachableTrap,
    DynamicLinkError,
    NotSupprted,
    BadExecutable,
}

#[allow(dead_code)]
#[derive(Debug, Copy, Clone)]
pub enum WasmRuntimeError {
    NoError,
    UnexpectedEof,
    UnexpectedToken,
    InvalidParameter,
    InvalidBytecode,
    Unreachable,
    OutOfBounds,
    OutOfMemory,
    NoMethod,
    DivideByZero,
    TypeMismatch,
    InternalInconsistency,
    WriteProtected,
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
    #[inline]
    pub const fn default_for(val_type: WasmValType) -> Self {
        match val_type {
            WasmValType::I32 => Self::I32(0),
            WasmValType::I64 => Self::I64(0),
            WasmValType::F32 => Self::F32(0.0),
            WasmValType::F64 => Self::F64(0.0),
        }
    }

    #[inline]
    pub fn is_valid_type(&self, val_type: WasmValType) -> bool {
        match *self {
            WasmValue::Empty => false,
            WasmValue::I32(_) => val_type == WasmValType::I32,
            WasmValue::I64(_) => val_type == WasmValType::I64,
            WasmValue::F32(_) => val_type == WasmValType::F32,
            WasmValue::F64(_) => val_type == WasmValType::F64,
        }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        match *self {
            Self::Empty => true,
            _ => false,
        }
    }

    #[inline]
    pub const fn get_i32(self) -> Result<i32, WasmRuntimeError> {
        match self {
            Self::I32(a) => Ok(a),
            _ => return Err(WasmRuntimeError::TypeMismatch),
        }
    }

    #[inline]
    pub const fn get_u32(self) -> Result<u32, WasmRuntimeError> {
        match self {
            Self::I32(a) => Ok(a as u32),
            _ => return Err(WasmRuntimeError::TypeMismatch),
        }
    }

    #[inline]
    pub const fn get_i64(self) -> Result<i64, WasmRuntimeError> {
        match self {
            Self::I64(a) => Ok(a),
            _ => return Err(WasmRuntimeError::TypeMismatch),
        }
    }

    #[inline]
    pub const fn get_u64(self) -> Result<u64, WasmRuntimeError> {
        match self {
            Self::I64(a) => Ok(a as u64),
            _ => return Err(WasmRuntimeError::TypeMismatch),
        }
    }

    #[inline]
    pub fn map_i32<F>(self, f: F) -> Result<WasmValue, WasmRuntimeError>
    where
        F: FnOnce(i32) -> i32,
    {
        match self {
            Self::I32(a) => Ok(f(a).into()),
            _ => return Err(WasmRuntimeError::TypeMismatch),
        }
    }

    #[inline]
    pub fn map_i64<F>(self, f: F) -> Result<WasmValue, WasmRuntimeError>
    where
        F: FnOnce(i64) -> i64,
    {
        match self {
            Self::I64(a) => Ok(f(a).into()),
            _ => return Err(WasmRuntimeError::TypeMismatch),
        }
    }
}

impl From<i32> for WasmValue {
    #[inline]
    fn from(v: i32) -> Self {
        Self::I32(v)
    }
}

impl From<u32> for WasmValue {
    #[inline]
    fn from(v: u32) -> Self {
        Self::I32(v as i32)
    }
}

impl From<i64> for WasmValue {
    #[inline]
    fn from(v: i64) -> Self {
        Self::I64(v)
    }
}

impl From<u64> for WasmValue {
    #[inline]
    fn from(v: u64) -> Self {
        Self::I64(v as i64)
    }
}

impl From<f32> for WasmValue {
    #[inline]
    fn from(v: f32) -> Self {
        Self::F32(v)
    }
}

impl From<f64> for WasmValue {
    #[inline]
    fn from(v: f64) -> Self {
        Self::F64(v)
    }
}

impl From<bool> for WasmValue {
    #[inline]
    fn from(v: bool) -> Self {
        Self::I32(if v { 1 } else { 0 })
    }
}

impl fmt::Display for WasmValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Empty => write!(f, "()"),
            Self::I32(v) => write!(f, "{}", v),
            Self::I64(v) => write!(f, "{}", v),
            Self::F32(_) => write!(f, "(#!F32)"),
            Self::F64(_) => write!(f, "(#!F64)"),
        }
    }
}

pub struct WasmCodeBlock<'a> {
    code: Leb128Stream<'a>,
    info: &'a WasmBlockInfo,
}

impl<'a> WasmCodeBlock<'a> {
    #[inline]
    pub fn from_slice(slice: &'a [u8], info: &'a WasmBlockInfo) -> Self {
        Self {
            code: Leb128Stream::from_slice(slice),
            info,
        }
    }

    #[inline]
    pub const fn info(&self) -> &WasmBlockInfo {
        self.info
    }

    #[inline]
    pub fn reset(&mut self) {
        self.code.reset();
    }

    #[inline]
    pub const fn position(&self) -> usize {
        self.code.position()
    }

    #[inline]
    pub fn set_position(&mut self, val: usize) {
        self.code.set_position(val);
    }

    #[inline]
    pub fn read_signed(&mut self) -> Result<i64, WasmRuntimeError> {
        self.code.read_signed().map_err(|err| Self::map_err(err))
    }

    #[inline]
    pub fn read_unsigned(&mut self) -> Result<u64, WasmRuntimeError> {
        self.code.read_unsigned().map_err(|err| Self::map_err(err))
    }

    #[inline]
    pub fn read_byte(&mut self) -> Result<u8, WasmRuntimeError> {
        self.code.read_byte().map_err(|err| Self::map_err(err))
    }

    #[inline]
    pub fn read_memarg(&mut self) -> Result<WasmMemArg, WasmRuntimeError> {
        self.code.read_memarg().map_err(|err| Self::map_err(err))
    }

    #[inline]
    fn map_err(err: WasmDecodeError) -> WasmRuntimeError {
        match err {
            WasmDecodeError::UnexpectedEof => WasmRuntimeError::UnexpectedEof,
            WasmDecodeError::InvalidBytecode => WasmRuntimeError::InvalidBytecode,
            _ => WasmRuntimeError::UnexpectedToken,
        }
    }
}

#[derive(Debug)]
pub struct WasmBlockInfo {
    func_index: usize,
    max_stack: usize,
    max_block_level: usize,
    flags: WasmBlockFlag,
    // blocks: BTreeMap<usize, WasmBlockContext>,
    int_codes: Box<[WasmImc]>,
    ext_params: Box<[usize]>,
}

bitflags! {
    pub struct WasmBlockFlag: usize {
        const LEAF_FUNCTION     = 0b0000_0000_0000_0001;
    }
}

impl WasmBlockInfo {
    #[inline]
    pub const fn func_index(&self) -> usize {
        self.func_index
    }

    #[inline]
    pub const fn max_stack(&self) -> usize {
        self.max_stack
    }

    #[inline]
    pub const fn max_block_level(&self) -> usize {
        self.max_block_level
    }

    #[inline]
    pub fn is_leaf(&self) -> bool {
        self.flags.contains(WasmBlockFlag::LEAF_FUNCTION)
    }

    #[inline]
    pub fn intermediate_codes<'a>(&'a self) -> &'a [WasmImc] {
        self.int_codes.as_ref()
    }

    #[inline]
    pub fn ext_params<'a>(&'a self) -> &'a [usize] {
        self.ext_params.as_ref()
    }

    /// Analyze block info
    pub fn analyze(
        func_index: usize,
        code_block: &mut Leb128Stream,
        local_types: &[WasmValType],
        result_types: &[WasmValType],
        module: &WasmModule,
    ) -> Result<Self, WasmDecodeError> {
        let mut blocks = Vec::new();
        let mut block_stack = Vec::new();
        let mut value_stack = Vec::new();
        let mut max_stack = 0;
        let mut max_block_level = 0;
        let mut flags = WasmBlockFlag::LEAF_FUNCTION;

        let mut int_codes: Vec<WasmImc> = Vec::new();
        let mut ext_params = Vec::new();

        loop {
            max_stack = usize::max(max_stack, value_stack.len());
            max_block_level = usize::max(max_block_level, block_stack.len());
            let position = code_block.position();
            let opcode = code_block.read_opcode()?;
            // let old_values = value_stack.clone();

            match opcode.proposal_type() {
                WasmProposalType::Mvp => {}
                WasmProposalType::MvpI64 => {}
                WasmProposalType::SignExtend => {}
                #[cfg(feature = "float")]
                WasmProposalType::MvpF32 | WasmProposalType::MvpF64 => {}
                _ => return Err(WasmDecodeError::NotSupprted),
            }

            match opcode {
                WasmOpcode::Unreachable => {
                    int_codes.push(WasmIntMnemonic::Unreachable.into());
                }

                WasmOpcode::Nop => (),

                WasmOpcode::Block => {
                    let target = blocks.len();
                    let block_type = code_block
                        .read_signed()
                        .and_then(|v| WasmBlockType::from_i64(v))?;
                    let block = RefCell::new(WasmBlockContext {
                        inst_type: BlockInstType::Block,
                        block_type,
                        stack_level: value_stack.len(),
                        start_position: 0,
                        end_position: 0,
                        else_position: 0,
                    });
                    block_stack.push(target);
                    blocks.push(block);
                    if block_type == WasmBlockType::Empty {
                        int_codes.push(WasmImc::new(
                            position,
                            WasmIntMnemonic::Block,
                            value_stack.len(),
                            target as u64,
                        ));
                    } else {
                        int_codes.push(WasmIntMnemonic::Undefined.into());
                    }
                }
                WasmOpcode::Loop => {
                    let target = blocks.len();
                    let block_type = code_block
                        .read_signed()
                        .and_then(|v| WasmBlockType::from_i64(v))?;
                    let block = RefCell::new(WasmBlockContext {
                        inst_type: BlockInstType::Loop,
                        block_type,
                        stack_level: value_stack.len(),
                        start_position: 0,
                        end_position: 0,
                        else_position: 0,
                    });
                    block_stack.push(target);
                    blocks.push(block);
                    if block_type == WasmBlockType::Empty {
                        int_codes.push(WasmImc::new(
                            position,
                            WasmIntMnemonic::Block,
                            value_stack.len(),
                            target as u64,
                        ));
                    } else {
                        int_codes.push(WasmIntMnemonic::Undefined.into());
                    }
                }
                WasmOpcode::If => {
                    let cc = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if cc != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    let block_type = code_block
                        .read_signed()
                        .and_then(|v| WasmBlockType::from_i64(v))?;
                    let block = RefCell::new(WasmBlockContext {
                        inst_type: BlockInstType::If,
                        block_type,
                        stack_level: value_stack.len(),
                        start_position: 0,
                        end_position: 0,
                        else_position: 0,
                    });
                    block_stack.push(blocks.len());
                    blocks.push(block);
                    int_codes.push(WasmIntMnemonic::Undefined.into());
                }
                WasmOpcode::Else => {
                    let block_ref = block_stack.last().ok_or(WasmDecodeError::ElseWithoutIf)?;
                    let block = blocks.get(*block_ref).unwrap().borrow();
                    if block.inst_type != BlockInstType::If {
                        return Err(WasmDecodeError::ElseWithoutIf);
                    }
                    let n_drops = value_stack.len() - block.stack_level;
                    for _ in 0..n_drops {
                        value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    }
                    int_codes.push(WasmIntMnemonic::Undefined.into());
                }
                WasmOpcode::End => {
                    if block_stack.len() > 0 {
                        let block_ref = block_stack.pop().ok_or(WasmDecodeError::BlockMismatch)?;
                        let block = blocks.get(block_ref).unwrap().borrow();
                        let n_drops = value_stack.len() - block.stack_level;
                        for _ in 0..n_drops {
                            value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                        }
                        block.block_type.into_type().map(|v| {
                            value_stack.push(v);
                        });
                        int_codes.push(WasmImc::new(
                            position,
                            WasmIntMnemonic::End,
                            value_stack.len(),
                            block_ref as u64,
                        ));
                    // TODO: type check
                    } else {
                        int_codes.push(WasmImc::new(
                            position,
                            WasmIntMnemonic::Return,
                            value_stack.len() - 1,
                            0,
                        ));
                        break;
                    }
                }

                WasmOpcode::Br => {
                    let br = code_block.read_unsigned()? as usize;
                    let target = block_stack
                        .get(block_stack.len() - br - 1)
                        .ok_or(WasmDecodeError::OutOfBranch)?;
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::Br,
                        value_stack.len(),
                        *target as u64,
                    ));
                }
                WasmOpcode::BrIf => {
                    let br = code_block.read_unsigned()? as usize;
                    let target = block_stack
                        .get(block_stack.len() - br - 1)
                        .ok_or(WasmDecodeError::OutOfBranch)?;
                    let cc = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::BrIf,
                        value_stack.len(),
                        *target as u64,
                    ));
                    if cc != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                }
                WasmOpcode::BrTable => {
                    let table_len = 1 + code_block.read_unsigned()? as usize;
                    let param_position = ext_params.len();
                    ext_params.push(table_len);
                    for _ in 0..table_len {
                        let br = code_block.read_unsigned()? as usize;
                        let target = block_stack
                            .get(block_stack.len() - br - 1)
                            .ok_or(WasmDecodeError::OutOfBranch)?;
                        ext_params.push(*target);
                    }
                    let cc = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if cc != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::BrTable,
                        value_stack.len(),
                        param_position as u64,
                    ));
                }

                WasmOpcode::Return => {
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::Return,
                        value_stack.len() - 1,
                        0,
                    ));
                    // TODO: type check
                }

                WasmOpcode::Call => {
                    flags.remove(WasmBlockFlag::LEAF_FUNCTION);
                    let func_index = code_block.read_unsigned()? as usize;
                    let function = module
                        .functions
                        .get(func_index)
                        .ok_or(WasmDecodeError::InvalidParameter)?;
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::Call,
                        value_stack.len(),
                        func_index as u64,
                    ));
                    // TODO: type check
                    for _param in function.param_types() {
                        value_stack.pop();
                    }
                    for result in function.result_types() {
                        value_stack.push(result.clone());
                    }
                }
                WasmOpcode::CallIndirect => {
                    flags.remove(WasmBlockFlag::LEAF_FUNCTION);
                    let type_ref = code_block.read_unsigned()? as usize;
                    let _reserved = code_block.read_unsigned()? as usize;
                    let func_type = module
                        .type_by_ref(type_ref)
                        .ok_or(WasmDecodeError::InvalidParameter)?;
                    let index = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if index != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::CallIndirect,
                        value_stack.len(),
                        type_ref as u64,
                    ));
                    // TODO: type check
                    for _param in func_type.param_types() {
                        value_stack.pop();
                    }
                    for result in func_type.result_types() {
                        value_stack.push(result.clone());
                    }
                }

                // WasmOpcode::ReturnCall
                // WasmOpcode::ReturnCallIndirect
                WasmOpcode::Drop => {
                    value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                }

                WasmOpcode::Select => {
                    let cc = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || cc != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::Select,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(a);
                }

                WasmOpcode::LocalGet => {
                    let local_ref = code_block.read_unsigned()? as usize;
                    let val = *local_types
                        .get(local_ref)
                        .ok_or(WasmDecodeError::InvalidLocal)?;
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::LocalGet,
                        value_stack.len(),
                        local_ref as u64,
                    ));
                    value_stack.push(val);
                }
                WasmOpcode::LocalSet => {
                    let local_ref = code_block.read_unsigned()? as usize;
                    let val = *local_types
                        .get(local_ref)
                        .ok_or(WasmDecodeError::InvalidLocal)?;
                    let stack = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if stack != val {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::LocalSet,
                        value_stack.len(),
                        local_ref as u64,
                    ));
                }
                WasmOpcode::LocalTee => {
                    let local_ref = code_block.read_unsigned()? as usize;
                    let val = *local_types
                        .get(local_ref)
                        .ok_or(WasmDecodeError::InvalidLocal)?;
                    let stack = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if stack != val {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::LocalTee,
                        value_stack.len() - 1,
                        local_ref as u64,
                    ));
                }

                WasmOpcode::GlobalGet => {
                    let global_ref = code_block.read_unsigned()? as usize;
                    let global = module
                        .global(global_ref)
                        .ok_or(WasmDecodeError::InvalidGlobal)?;
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::GlobalGet,
                        value_stack.len(),
                        global_ref as u64,
                    ));
                    value_stack.push(global.val_type);
                }
                WasmOpcode::GlobalSet => {
                    let global_ref = code_block.read_unsigned()? as usize;
                    let global = module
                        .global(global_ref)
                        .ok_or(WasmDecodeError::InvalidGlobal)?;
                    if !global.is_mutable() {
                        return Err(WasmDecodeError::InvalidGlobal);
                    }
                    let stack = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if stack != global.val_type {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::GlobalSet,
                        value_stack.len(),
                        global_ref as u64,
                    ));
                }

                WasmOpcode::I32Load => {
                    let arg = code_block.read_memarg()?;
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Load,
                        value_stack.len(),
                        arg.offset as u64,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I32Load8S => {
                    let arg = code_block.read_memarg()?;
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Load8S,
                        value_stack.len(),
                        arg.offset as u64,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I32Load8U => {
                    let arg = code_block.read_memarg()?;
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Load8U,
                        value_stack.len(),
                        arg.offset as u64,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I32Load16S => {
                    let arg = code_block.read_memarg()?;
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Load16S,
                        value_stack.len(),
                        arg.offset as u64,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I32Load16U => {
                    let arg = code_block.read_memarg()?;
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Load16U,
                        value_stack.len(),
                        arg.offset as u64,
                    ));
                    value_stack.push(WasmValType::I32);
                }

                WasmOpcode::I64Load => {
                    let arg = code_block.read_memarg()?;
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Load,
                        value_stack.len(),
                        arg.offset as u64,
                    ));
                    value_stack.push(WasmValType::I64);
                }
                WasmOpcode::I64Load8S => {
                    let arg = code_block.read_memarg()?;
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Load8S,
                        value_stack.len(),
                        arg.offset as u64,
                    ));
                    value_stack.push(WasmValType::I64);
                }
                WasmOpcode::I64Load8U => {
                    let arg = code_block.read_memarg()?;
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Load8U,
                        value_stack.len(),
                        arg.offset as u64,
                    ));
                    value_stack.push(WasmValType::I64);
                }
                WasmOpcode::I64Load16S => {
                    let arg = code_block.read_memarg()?;
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Load16S,
                        value_stack.len(),
                        arg.offset as u64,
                    ));
                    value_stack.push(WasmValType::I64);
                }
                WasmOpcode::I64Load16U => {
                    let arg = code_block.read_memarg()?;
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Load16U,
                        value_stack.len(),
                        arg.offset as u64,
                    ));
                    value_stack.push(WasmValType::I64);
                }
                WasmOpcode::I64Load32S => {
                    let arg = code_block.read_memarg()?;
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Load32S,
                        value_stack.len(),
                        arg.offset as u64,
                    ));
                    value_stack.push(WasmValType::I64);
                }
                WasmOpcode::I64Load32U => {
                    let arg = code_block.read_memarg()?;
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Load32U,
                        value_stack.len(),
                        arg.offset as u64,
                    ));
                    value_stack.push(WasmValType::I64);
                }

                WasmOpcode::I32Store => {
                    let arg = code_block.read_memarg()?;
                    let d = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let i = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if i != d && i != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Store,
                        value_stack.len(),
                        arg.offset as u64,
                    ));
                }
                WasmOpcode::I32Store8 => {
                    let arg = code_block.read_memarg()?;
                    let d = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let i = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if i != d && i != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Store8,
                        value_stack.len(),
                        arg.offset as u64,
                    ));
                }
                WasmOpcode::I32Store16 => {
                    let arg = code_block.read_memarg()?;
                    let d = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let i = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if i != d && i != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Store16,
                        value_stack.len(),
                        arg.offset as u64,
                    ));
                }

                WasmOpcode::I64Store => {
                    let arg = code_block.read_memarg()?;
                    let d = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let i = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if i != WasmValType::I32 && d != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Store,
                        value_stack.len(),
                        arg.offset as u64,
                    ));
                }
                WasmOpcode::I64Store8 => {
                    let arg = code_block.read_memarg()?;
                    let d = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let i = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if i != WasmValType::I32 && d != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Store8,
                        value_stack.len(),
                        arg.offset as u64,
                    ));
                }
                WasmOpcode::I64Store16 => {
                    let arg = code_block.read_memarg()?;
                    let d = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let i = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if i != WasmValType::I32 && d != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Store16,
                        value_stack.len(),
                        arg.offset as u64,
                    ));
                }
                WasmOpcode::I64Store32 => {
                    let arg = code_block.read_memarg()?;
                    let d = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let i = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if i != WasmValType::I32 && d != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Store32,
                        value_stack.len(),
                        arg.offset as u64,
                    ));
                }

                #[cfg(feature = "float")]
                WasmOpcode::F32Load => {
                    let _ = code_block.read_memarg()?;
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    value_stack.push(WasmValType::F32);
                }
                #[cfg(feature = "float")]
                WasmOpcode::F64Load => {
                    let _ = code_block.read_memarg()?;
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    value_stack.push(WasmValType::F64);
                }
                #[cfg(feature = "float")]
                WasmOpcode::F32Store => {
                    let _ = code_block.read_memarg()?;
                    let d = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let i = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if i != WasmValType::I32 && d != WasmValType::F32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                }
                #[cfg(feature = "float")]
                WasmOpcode::F64Store => {
                    let _ = code_block.read_memarg()?;
                    let d = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let i = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if i != WasmValType::I32 && d != WasmValType::F64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                }

                WasmOpcode::MemorySize => {
                    let _ = code_block.read_unsigned()?;
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::MemorySize,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }

                WasmOpcode::MemoryGrow => {
                    let _ = code_block.read_unsigned()?;
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::MemoryGrow,
                        value_stack.len(),
                        0,
                    ));
                    let a = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                }

                WasmOpcode::I32Const => {
                    let val = code_block.read_signed()?;
                    if val < (i32::MIN as i64) || val > (i32::MAX as i64) {
                        return Err(WasmDecodeError::InvalidParameter);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Const,
                        value_stack.len(),
                        val as u64,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I64Const => {
                    let val = code_block.read_signed()?;
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Const,
                        value_stack.len(),
                        val as u64,
                    ));
                    value_stack.push(WasmValType::I64);
                }
                #[cfg(feature = "float")]
                WasmOpcode::F32Const => {
                    let _ = code_block.get_bytes(4)?;
                    value_stack.push(WasmValType::F32);
                }
                #[cfg(feature = "float")]
                WasmOpcode::F64Const => {
                    let _ = code_block.get_bytes(8)?;
                    value_stack.push(WasmValType::F64);
                }

                // unary operator [i32] -> [i32]
                WasmOpcode::I32Eqz => {
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Eqz,
                        value_stack.len() - 1,
                        0,
                    ));
                    let a = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                }
                WasmOpcode::I32Clz => {
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Clz,
                        value_stack.len() - 1,
                        0,
                    ));
                    let a = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                }
                WasmOpcode::I32Ctz => {
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Ctz,
                        value_stack.len() - 1,
                        0,
                    ));
                    let a = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                }
                WasmOpcode::I32Popcnt => {
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Popcnt,
                        value_stack.len() - 1,
                        0,
                    ));
                    let a = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                }
                WasmOpcode::I32Extend8S => {
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Extend8S,
                        value_stack.len() - 1,
                        0,
                    ));
                    let a = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                }
                WasmOpcode::I32Extend16S => {
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Extend16S,
                        value_stack.len() - 1,
                        0,
                    ));
                    let a = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                }

                // binary operator [i32, i32] -> [i32]
                WasmOpcode::I32Eq => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Eq,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I32Ne => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Ne,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I32LtS => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32LtS,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I32LtU => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32LtU,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I32GtS => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32GtS,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I32GtU => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32GtU,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I32LeS => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32LeS,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I32LeU => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32LeU,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I32GeS => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32GeS,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I32GeU => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32GeU,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I32Add => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Add,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I32Sub => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Sub,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I32Mul => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Mul,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I32DivS => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32DivS,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I32DivU => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32DivU,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I32RemS => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32RemS,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I32RemU => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32RemU,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I32And => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32And,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I32Or => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Or,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I32Xor => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Xor,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I32Shl => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Shl,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I32ShrS => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32ShrS,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I32ShrU => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32ShrU,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I32Rotl => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Rotl,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I32Rotr => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32Rotr,
                        value_stack.len() - 1,
                        0,
                    ));
                }

                // binary operator [i64, i64] -> [i32]
                WasmOpcode::I64Eq => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Eq,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I64Ne => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Ne,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I64LtS => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64LtS,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I64LtU => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64LtU,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I64GtS => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64GtS,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I64GtU => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64GtU,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I64LeS => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64LeS,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I64LeU => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64LeU,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I64GeS => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64GeS,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I64GeU => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64GeU,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }

                // unary operator [i64] -> [i64]
                WasmOpcode::I64Clz => {
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Clz,
                        value_stack.len() - 1,
                        0,
                    ));
                    let a = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                }
                WasmOpcode::I64Ctz => {
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Ctz,
                        value_stack.len() - 1,
                        0,
                    ));
                    let a = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                }
                WasmOpcode::I64Popcnt => {
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Popcnt,
                        value_stack.len() - 1,
                        0,
                    ));
                    let a = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                }
                WasmOpcode::I64Extend8S => {
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Extend8S,
                        value_stack.len() - 1,
                        0,
                    ));
                    let a = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                }
                WasmOpcode::I64Extend16S => {
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Extend16S,
                        value_stack.len() - 1,
                        0,
                    ));
                    let a = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                }
                WasmOpcode::I64Extend32S => {
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Extend32S,
                        value_stack.len() - 1,
                        0,
                    ));
                    let a = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                }

                // binary operator [i64, i64] -> [i64]
                WasmOpcode::I64Add => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Add,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I64Sub => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Sub,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I64Mul => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Mul,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I64DivS => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64DivS,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I64DivU => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64DivU,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I64RemS => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64RemS,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I64RemU => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64RemU,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I64And => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64And,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I64Or => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Or,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I64Xor => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Xor,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I64Shl => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Shl,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I64ShrS => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64ShrS,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I64ShrU => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64ShrU,
                        value_stack.len() - 1,
                        0,
                    ));
                }
                WasmOpcode::I64Rotl => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Rotl,
                        value_stack.len() - 1,
                        0,
                    ));
                }

                WasmOpcode::I64Rotr => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Rotr,
                        value_stack.len() - 1,
                        0,
                    ));
                }

                // [i64] -> [i32]
                WasmOpcode::I64Eqz => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64Eqz,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }
                WasmOpcode::I32WrapI64 => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I32WrapI64,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I32);
                }

                // [i32] -> [i64]
                WasmOpcode::I64ExtendI32S => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64ExtendI32S,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I64);
                }
                WasmOpcode::I64ExtendI32U => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    int_codes.push(WasmImc::new(
                        position,
                        WasmIntMnemonic::I64ExtendI32U,
                        value_stack.len(),
                        0,
                    ));
                    value_stack.push(WasmValType::I64);
                }

                // [f32] -> [i32]
                #[cfg(feature = "float")]
                WasmOpcode::I32TruncF32S
                | WasmOpcode::I32TruncF32U
                | WasmOpcode::I32ReinterpretF32 => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::F32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    value_stack.push(WasmValType::I32);
                }

                // [f32, f32] -> [i32]
                #[cfg(feature = "float")]
                WasmOpcode::F32Eq
                | WasmOpcode::F32Ne
                | WasmOpcode::F32Lt
                | WasmOpcode::F32Gt
                | WasmOpcode::F32Le
                | WasmOpcode::F32Ge => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::F32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    value_stack.push(WasmValType::I32);
                }

                // [f32] -> [f32]
                #[cfg(feature = "float")]
                WasmOpcode::F32Abs
                | WasmOpcode::F32Neg
                | WasmOpcode::F32Ceil
                | WasmOpcode::F32Floor
                | WasmOpcode::F32Trunc
                | WasmOpcode::F32Nearest
                | WasmOpcode::F32Sqrt => {
                    let a = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                }

                // [f32, f32] -> [f32]
                #[cfg(feature = "float")]
                WasmOpcode::F32Add
                | WasmOpcode::F32Sub
                | WasmOpcode::F32Mul
                | WasmOpcode::F32Div
                | WasmOpcode::F32Min
                | WasmOpcode::F32Max
                | WasmOpcode::F32Copysign => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::F32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                }

                // [f64] -> [i32]
                #[cfg(feature = "float")]
                WasmOpcode::I32TruncF64S | WasmOpcode::I32TruncF64U => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::F64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    value_stack.push(WasmValType::I32);
                }

                // [f64] -> [i64]
                #[cfg(feature = "float")]
                WasmOpcode::I64TruncF32S
                | WasmOpcode::I64TruncF32U
                | WasmOpcode::I64TruncF64S
                | WasmOpcode::I64TruncF64U
                | WasmOpcode::I64ReinterpretF64 => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::F64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    value_stack.push(WasmValType::I32);
                }

                // [f64, f64] -> [i32]
                #[cfg(feature = "float")]
                WasmOpcode::F64Eq
                | WasmOpcode::F64Ne
                | WasmOpcode::F64Lt
                | WasmOpcode::F64Gt
                | WasmOpcode::F64Le
                | WasmOpcode::F64Ge => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::F64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    value_stack.push(WasmValType::I32);
                }

                // [f64] -> [f64]
                #[cfg(feature = "float")]
                WasmOpcode::F64Abs
                | WasmOpcode::F64Neg
                | WasmOpcode::F64Ceil
                | WasmOpcode::F64Floor
                | WasmOpcode::F64Trunc
                | WasmOpcode::F64Nearest
                | WasmOpcode::F64Sqrt => {
                    let a = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::F64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                }

                // [f64, f64] -> [f64]
                #[cfg(feature = "float")]
                WasmOpcode::F64Add
                | WasmOpcode::F64Sub
                | WasmOpcode::F64Mul
                | WasmOpcode::F64Div
                | WasmOpcode::F64Min
                | WasmOpcode::F64Max
                | WasmOpcode::F64Copysign => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    let b = *value_stack.last().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != b || a != WasmValType::F64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                }

                // [i32] -> [f32]
                #[cfg(feature = "float")]
                WasmOpcode::F32ConvertI32S
                | WasmOpcode::F32ConvertI32U
                | WasmOpcode::F32ReinterpretI32 => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    value_stack.push(WasmValType::F32);
                }

                // [i64] -> [f64]
                #[cfg(feature = "float")]
                WasmOpcode::F32ConvertI64S | WasmOpcode::F32ConvertI64U => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    value_stack.push(WasmValType::F32);
                }

                // [f64] -> [f32]
                #[cfg(feature = "float")]
                WasmOpcode::F32DemoteF64 => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::F64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    value_stack.push(WasmValType::F32);
                }

                // [i32] -> [f64]
                #[cfg(feature = "float")]
                WasmOpcode::F64ConvertI32S | WasmOpcode::F64ConvertI32U => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    value_stack.push(WasmValType::F64);
                }

                // [i64] -> [f64]
                #[cfg(feature = "float")]
                WasmOpcode::F64ConvertI64S
                | WasmOpcode::F64ConvertI64U
                | WasmOpcode::F64ReinterpretI64 => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::I64 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    value_stack.push(WasmValType::F64);
                }

                // [f32] -> [f64]
                #[cfg(feature = "float")]
                WasmOpcode::F64PromoteF32 => {
                    let a = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                    if a != WasmValType::F32 {
                        return Err(WasmDecodeError::TypeMismatch);
                    }
                    value_stack.push(WasmValType::F64);
                }

                #[allow(unreachable_patterns)]
                _ => return Err(WasmDecodeError::UnreachableTrap),
            }
        }

        if result_types.len() > 0 {
            if result_types.len() != value_stack.len() {
                return Err(WasmDecodeError::TypeMismatch);
            }

            for result_type in result_types {
                let val = value_stack.pop().ok_or(WasmDecodeError::OutOfStack)?;
                if *result_type != val {
                    return Err(WasmDecodeError::TypeMismatch);
                }
            }
        } else {
            if value_stack.len() > 0 {
                return Err(WasmDecodeError::InvalidStackLevel);
            }
        }

        // fused instructions
        if int_codes.len() > 2 {
            let limit = int_codes.len() - 1;
            for i in 0..limit {
                use WasmIntMnemonic::*;
                let j = i + 1;
                let current = int_codes[i].mnemonic();
                let next = int_codes[j].mnemonic();
                match (current, next) {
                    (I32Const, I32Add) => {
                        let param1 = int_codes[i].param1();
                        int_codes[i].mnemonic = Nop;
                        int_codes[j].mnemonic = FusedI32AddI;
                        int_codes[j].param1 = param1;
                    }
                    (I32Const, I32Sub) => {
                        let param1 = int_codes[i].param1();
                        int_codes[i].mnemonic = Nop;
                        int_codes[j].mnemonic = FusedI32SubI;
                        int_codes[j].param1 = param1;
                    }
                    (I64Const, I64Add) => {
                        let param1 = int_codes[i].param1();
                        int_codes[i].mnemonic = Nop;
                        int_codes[j].mnemonic = FusedI64AddI;
                        int_codes[j].param1 = param1;
                    }
                    (I64Const, I64Sub) => {
                        let param1 = int_codes[i].param1();
                        int_codes[i].mnemonic = Nop;
                        int_codes[j].mnemonic = FusedI64SubI;
                        int_codes[j].param1 = param1;
                    }
                    (I32Eqz, BrIf) => {
                        int_codes[i].mnemonic = Nop;
                        int_codes[j].mnemonic = FusedI32BrZ;
                    }
                    (I64Eqz, BrIf) => {
                        int_codes[i].mnemonic = Nop;
                        int_codes[j].mnemonic = FusedI64BrZ;
                    }
                    _ => (),
                }
            }
        }

        // compaction
        let mut actual_len = 0;
        for index in 0..int_codes.len() {
            use WasmIntMnemonic::*;
            let code = int_codes[index];
            match code.mnemonic() {
                Nop => (),
                Block => {
                    let target = code.param1() as usize;
                    let ref mut block = blocks[target].borrow_mut();
                    block.start_position = actual_len;
                }
                End => {
                    let target = code.param1() as usize;
                    let ref mut block = blocks[target].borrow_mut();
                    block.end_position = actual_len;
                }
                _ => {
                    int_codes[actual_len] = code;
                    actual_len += 1;
                }
            }
        }
        int_codes.resize(
            actual_len,
            WasmImc::from_mnemonic(WasmIntMnemonic::Unreachable),
        );

        // fixes branching targets
        for code in int_codes.iter_mut() {
            use WasmIntMnemonic::*;
            let mnemonic = code.mnemonic();
            if mnemonic.is_branch() {
                let target = code.param1() as usize;
                let block = blocks.get(target).ok_or(WasmDecodeError::OutOfBranch)?;
                code.set_param1(block.borrow().preferred_target() as u64);
            } else {
                match code.mnemonic() {
                    BrTable => {
                        let table_position = code.param1() as usize;
                        let table_len = ext_params[table_position];
                        for i in 0..table_len {
                            let index = table_position + i + 1;
                            let target = ext_params[index];
                            let block = blocks.get(target).ok_or(WasmDecodeError::OutOfBranch)?;
                            ext_params[index] = block.borrow().preferred_target();
                        }
                    }
                    _ => (),
                }
            }
        }

        Ok(Self {
            func_index,
            max_stack,
            max_block_level,
            flags,
            int_codes: int_codes.into_boxed_slice(),
            ext_params: ext_params.into_boxed_slice(),
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum BlockInstType {
    Block,
    Loop,
    If,
}

#[derive(Debug, Copy, Clone)]
pub struct WasmBlockContext {
    pub inst_type: BlockInstType,
    pub block_type: WasmBlockType,
    pub stack_level: usize,
    pub start_position: usize,
    pub end_position: usize,
    pub else_position: usize,
}

impl WasmBlockContext {
    #[inline]
    pub fn preferred_target(&self) -> usize {
        if self.inst_type == BlockInstType::Loop {
            self.start_position
        } else {
            self.end_position
        }
    }
}

#[derive(Copy, Clone)]
pub struct WasmRunnable<'a> {
    function: &'a WasmFunction,
    module: &'a WasmModule,
}

impl<'a> WasmRunnable<'a> {
    #[inline]
    const fn from_function(function: &'a WasmFunction, module: &'a WasmModule) -> Self {
        Self { function, module }
    }
}

impl WasmRunnable<'_> {
    #[inline]
    pub const fn function(&self) -> &WasmFunction {
        &self.function
    }

    #[inline]
    pub const fn module(&self) -> &WasmModule {
        &self.module
    }
}

#[cfg(test)]
mod tests {
    use super::{Leb128Stream, WasmLoader};

    #[test]
    fn instantiate() {
        let minimal = [0, 97, 115, 109, 1, 0, 0, 0];
        WasmLoader::instantiate(&minimal, |_, _, _| unreachable!()).unwrap();
    }

    #[test]
    #[should_panic(expected = "BadExecutable")]
    fn instantiate_bad_exec1() {
        let too_small = [0, 97, 115, 109, 1, 0, 0];
        WasmLoader::instantiate(&too_small, |_, _, _| unreachable!()).unwrap();
    }

    #[test]
    #[should_panic(expected = "BadExecutable")]
    fn instantiate_bad_exec2() {
        let too_small = [0, 97, 115, 109, 2, 0, 0, 0];
        WasmLoader::instantiate(&too_small, |_, _, _| unreachable!()).unwrap();
    }

    #[test]
    #[should_panic(expected = "UnexpectedEof")]
    fn instantiate_unexpected_eof() {
        let minimal_bad = [0, 97, 115, 109, 1, 0, 0, 0, 1];
        WasmLoader::instantiate(&minimal_bad, |_, _, _| unreachable!()).unwrap();
    }

    #[test]
    fn instantiate_fibonacci() {
        let slice = [
            0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00, 0x01, 0x06, 0x01, 0x60, 0x01, 0x7F,
            0x01, 0x7F, 0x03, 0x02, 0x01, 0x00, 0x0A, 0x31, 0x01, 0x2F, 0x01, 0x01, 0x7F, 0x41,
            0x00, 0x21, 0x01, 0x02, 0x40, 0x03, 0x40, 0x20, 0x00, 0x41, 0x02, 0x49, 0x0D, 0x01,
            0x20, 0x00, 0x41, 0x7F, 0x6A, 0x10, 0x00, 0x20, 0x01, 0x6A, 0x21, 0x01, 0x20, 0x00,
            0x41, 0x7E, 0x6A, 0x21, 0x00, 0x0C, 0x00, 0x0B, 0x0B, 0x20, 0x00, 0x20, 0x01, 0x6A,
            0x0B,
        ];
        let module = WasmLoader::instantiate(&slice, |_, _, _| unreachable!()).unwrap();
        let _ = module.func_by_index(0).unwrap();
    }

    #[test]
    fn leb128() {
        let data = [
            0x7F, 0xFF, 0x00, 0xEF, 0xFD, 0xB6, 0xF5, 0x0D, 0xEF, 0xFD, 0xB6, 0xF5, 0x7D,
        ];
        let mut stream = Leb128Stream::from_slice(&data);

        stream.reset();
        assert_eq!(stream.position(), 0);
        let test = stream.read_unsigned().unwrap();
        assert_eq!(test, 127);
        let test = stream.read_unsigned().unwrap();
        assert_eq!(test, 127);
        let test = stream.read_unsigned().unwrap();
        assert_eq!(test, 0xdeadbeef);
        let test = stream.read_unsigned().unwrap();
        assert_eq!(test, 0x7deadbeef);

        stream.reset();
        assert_eq!(stream.position(), 0);
        let test = stream.read_signed().unwrap();
        assert_eq!(test, -1);
        let test = stream.read_signed().unwrap();
        assert_eq!(test, 127);
        let test = stream.read_signed().unwrap();
        assert_eq!(test, 0xdeadbeef);
        let test = stream.read_signed().unwrap();
        assert_eq!(test, -559038737);
    }
}
