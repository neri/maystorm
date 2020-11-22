// WebAssembly Binary Loader

use super::super::*;
use crate::*;
use alloc::string::*;
use alloc::vec::Vec;
use byteorder::*;
use core::fmt;
use core::str;

#[allow(dead_code)]
pub(super) struct WasmBinaryLoader {
    lio: LoadedImageOption,
    ctx: WasmContext,
}

impl WasmBinaryLoader {
    const MINIMAL_BIN_SIZE: usize = 8;
    const MAGIC: u32 = 0x6D736100;
    const VER_CURRENT: u32 = 0x0000_0001;

    pub fn identity(blob: &[u8]) -> Option<WasmBinaryLoader> {
        if blob.len() >= Self::MINIMAL_BIN_SIZE
            && LE::read_u32(&blob[0..4]) == Self::MAGIC
            && LE::read_u32(&blob[4..8]) == Self::VER_CURRENT
        {
            Some(Self {
                lio: LoadedImageOption::default(),
                ctx: WasmContext::new(),
            })
        } else {
            None
        }
    }

    #[allow(dead_code)]
    fn start(_: usize) {
        MyScheduler::yield_thread();
        Timer::msleep(1000);
        RuntimeEnvironment::exit(0);
    }

    /// Parse "type" section
    fn parse_sec_type(&mut self, section: &mut WasmSection) {
        let n_items = section.stream.read_uint().unwrap();
        for _ in 0..n_items {
            let ft = WasmType::from_stream(&mut section.stream).unwrap();
            self.ctx.types.push(ft);
        }
    }

    /// Parse "import" section
    fn parse_sec_import(&mut self, section: &mut WasmSection) {
        let n_items = section.stream.read_uint().unwrap() as usize;
        for i in 0..n_items {
            let import = WasmImport::from_stream(&mut section.stream).unwrap();
            if let WasmImportType::Type(index) = import.index {
                self.ctx.functions.push(WasmFunction::from_import(index, i));
            }
            self.ctx.imports.push(import);
        }
    }

    /// Parse "func" section
    fn parse_sec_func(&mut self, section: &mut WasmSection) {
        let n_items = section.stream.read_uint().unwrap();
        for _ in 0..n_items {
            let index = section.stream.read_uint().unwrap() as usize;
            self.ctx.functions.push(WasmFunction::internal(index));
        }
    }

    /// Parse "export" section
    fn parse_sec_export(&mut self, section: &mut WasmSection) {
        let n_items = section.stream.read_uint().unwrap() as usize;
        for i in 0..n_items {
            let export = WasmExport::from_stream(&mut section.stream).unwrap();
            if let WasmExportType::Function(index) = export.index {
                self.ctx
                    .functions
                    .get_mut(index)
                    .map(|v| v.origin = WasmFunctionOrigin::Export(i));
            }
            self.ctx.exports.push(export);
        }
    }

    /// Parse "memory" section
    fn parse_sec_memory(&mut self, section: &mut WasmSection) {
        let n_items = section.stream.read_uint().unwrap();
        for _ in 0..n_items {
            match WasmLimit::from_stream(&mut section.stream) {
                Ok(limit) => {
                    self.ctx.memory.push(WasmMemory::new(limit));
                }
                _ => (),
            }
        }
    }

    /// Parse "table" section
    fn parse_sec_table(&mut self, section: &mut WasmSection) {
        let n_items = section.stream.read_uint().unwrap();
        for _ in 0..n_items {
            match WasmTable::from_stream(&mut section.stream) {
                Ok(table) => {
                    self.ctx.tables.push(table);
                }
                _ => (),
            }
        }
    }

    /// Parse "elem" section
    fn parse_sec_elem(&mut self, section: &mut WasmSection) {
        let n_items = section.stream.read_uint().unwrap();
        for _ in 0..n_items {
            let tabidx = section.stream.read_uint().unwrap() as usize;
            let offset = self.eval_offset(&mut section.stream).unwrap() as usize;
            let n_elements = section.stream.read_uint().unwrap() as usize;
            let table = self.ctx.tables.get_mut(tabidx).unwrap();
            for i in offset..offset + n_elements {
                let elem = section.stream.read_uint().unwrap() as usize;
                table.table[i] = elem;
            }
        }
    }

    /// Parse "data" section
    fn parse_sec_data(&mut self, section: &mut WasmSection) {
        let n_items = section.stream.read_uint().unwrap();
        for _ in 0..n_items {
            let memidx = section.stream.read_uint().unwrap() as usize;
            let offset = self.eval_offset(&mut section.stream).unwrap() as usize;
            let src = section.stream.read_bytes().unwrap();
            let memory = self.ctx.memory.get_mut(memidx).unwrap();
            memory.write_bytes(offset, src).unwrap();
        }
    }

    fn eval_offset(&mut self, stream: &mut Leb128Stream) -> Result<u64, Leb128Error> {
        stream
            .read_uint()
            .and_then(|opc| match WasmOpcode::from(opc) {
                WasmOpcode::I32Const => stream.read_uint().and_then(|r| {
                    match stream.read_uint().map(|v| WasmOpcode::from(v)) {
                        Ok(WasmOpcode::End) => Ok(r),
                        _ => Err(Leb128Error::UnexpectedToken),
                    }
                }),
                _ => Err(Leb128Error::UnexpectedToken),
            })
    }
}

impl BinaryLoader for WasmBinaryLoader {
    fn option(&mut self) -> &mut LoadedImageOption {
        &mut self.lio
    }

    fn load(&mut self, blob: &[u8]) {
        println!("WASM version 1 size {}", blob.len());
        let mut blob = Leb128Stream::new(&blob[8..]);
        while let Some(mut section) = blob.next_section() {
            match section.section_type {
                WasmSectionType::Type => self.parse_sec_type(&mut section),
                WasmSectionType::Import => self.parse_sec_import(&mut section),
                WasmSectionType::Table => self.parse_sec_table(&mut section),
                WasmSectionType::Memory => self.parse_sec_memory(&mut section),
                WasmSectionType::Element => self.parse_sec_elem(&mut section),
                WasmSectionType::Function => self.parse_sec_func(&mut section),
                WasmSectionType::Export => self.parse_sec_export(&mut section),
                WasmSectionType::Data => self.parse_sec_data(&mut section),
                _ => (),
            }
        }

        for (index, ty) in self.ctx.types.iter().enumerate() {
            println!(
                " type ${} params {:?} result {:?}",
                index, ty.params, ty.result
            );
        }
        for import in &self.ctx.imports {
            println!(
                " import \"{}\" \"{}\" {:?}",
                import.mod_name, import.name, import.index
            );
        }
        for export in &self.ctx.exports {
            println!(" export \"{}\" {:?}", export.name, export.index);
        }
        for (index, function) in self.ctx.functions.iter().enumerate() {
            println!(
                " function ${} type ${} {:?}",
                index, function.type_ref, function.origin
            );
        }
        for (index, memory) in self.ctx.memory.iter().enumerate() {
            println!(
                " memory ${} {} {}",
                index, memory.limit.min, memory.limit.max
            );
        }
        for (index, table) in self.ctx.tables.iter().enumerate() {
            println!(" table ${} {} {}", index, table.limit.min, table.limit.max);
            for i in 0..table.table.len() {
                println!(" table[{}] = func ${}", i, table.table[i]);
            }
        }
    }

    fn invoke_start(&mut self) -> Option<ThreadHandle> {
        // SpawnOption::new().spawn(Self::start, 0, self.lio.name.as_ref())
        None
    }
}

#[allow(dead_code)]
struct WasmContext {
    types: Vec<WasmType>,
    imports: Vec<WasmImport>,
    exports: Vec<WasmExport>,
    memory: Vec<WasmMemory>,
    tables: Vec<WasmTable>,
    functions: Vec<WasmFunction>,
}

impl WasmContext {
    fn new() -> Self {
        Self {
            types: Vec::new(),
            memory: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            tables: Vec::new(),
            functions: Vec::new(),
        }
    }
}

struct Leb128Stream<'a> {
    blob: &'a [u8],
    cursor: usize,
}

impl<'a> Leb128Stream<'a> {
    fn new(blob: &'a [u8]) -> Self {
        Self { blob, cursor: 0 }
    }
}

#[allow(dead_code)]
impl Leb128Stream<'_> {
    fn is_eof(&self) -> bool {
        self.cursor >= self.blob.len()
    }

    fn read_byte(&mut self) -> Result<u8, Leb128Error> {
        if self.is_eof() {
            return Err(Leb128Error::UnexpectedEof);
        }
        let d = self.blob[self.cursor];
        self.cursor += 1;
        Ok(d)
    }

    fn get_bytes(&mut self, size: usize) -> Result<&[u8], Leb128Error> {
        let limit = self.blob.len();
        if self.cursor <= limit && size <= limit && self.cursor + size <= limit {
            let offset = self.cursor;
            self.cursor += size;
            Ok(&self.blob[offset..offset + size])
        } else {
            Err(Leb128Error::UnexpectedEof)
        }
    }

    fn read_bytes(&mut self) -> Result<&[u8], Leb128Error> {
        self.read_uint()
            .and_then(move |size| self.get_bytes(size as usize))
    }

    fn read_uint(&mut self) -> Result<u64, Leb128Error> {
        let mut value: u64 = 0;
        let mut scale = 0;
        let mut cursor = self.cursor;
        loop {
            if self.is_eof() {
                return Err(Leb128Error::UnexpectedEof);
            }
            let d = self.blob[cursor];
            cursor += 1;
            value |= (d as u64 & 0x7F) << scale;
            scale += 7;
            if (d & 0x80) == 0 {
                break;
            }
        }
        self.cursor = cursor;
        Ok(value)
    }

    fn get_string(&mut self) -> Result<&str, Leb128Error> {
        self.read_bytes()
            .and_then(|v| str::from_utf8(v).map_err(|_| Leb128Error::UnexpectedToken))
    }

    fn next_section(&mut self) -> Option<WasmSection> {
        let section_type = match self.read_uint().ok() {
            Some(v) => v,
            None => return None,
        };
        let size = match self.read_uint().ok() {
            Some(v) => v as usize,
            None => return None,
        };
        let offset = self.cursor;
        self.cursor += size;
        let stream = Leb128Stream {
            blob: &self.blob[offset..offset + size],
            cursor: 0,
        };
        Some(WasmSection {
            section_type: section_type.into(),
            stream,
        })
    }
}

#[derive(Debug, Copy, Clone)]
enum Leb128Error {
    UnexpectedEof,
    UnexpectedToken,
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

impl From<u64> for WasmSectionType {
    fn from(v: u64) -> Self {
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

#[derive(Debug, Clone, Copy)]
enum WasmValType {
    I32 = 0x7F,
    I64 = 0x7E,
    F32 = 0x7D,
    F64 = 0x7C,
}

impl WasmValType {
    const fn from_u64(v: u64) -> Option<Self> {
        match v {
            0x7F => Some(WasmValType::I32),
            0x7E => Some(WasmValType::I64),
            0x7D => Some(WasmValType::F32),
            0x7C => Some(WasmValType::F64),
            _ => None,
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

#[allow(dead_code)]
struct WasmLimit {
    min: u32,
    max: u32,
}

impl WasmLimit {
    fn from_stream(stream: &mut Leb128Stream) -> Result<Self, Leb128Error> {
        match stream.read_uint() {
            Ok(0) => stream.read_uint().map(|min| Self {
                min: min as u32,
                max: min as u32,
            }),
            Ok(1) => {
                let min = match stream.read_uint() {
                    Ok(v) => v as u32,
                    Err(err) => return Err(err),
                };
                let max = match stream.read_uint() {
                    Ok(v) => v as u32,
                    Err(err) => return Err(err),
                };
                Ok(Self { min, max })
            }
            Err(err) => Err(err),
            _ => Err(Leb128Error::UnexpectedToken),
        }
    }
}

#[allow(dead_code)]
pub struct WasmMemory {
    limit: WasmLimit,
    memory: Vec<u8>,
}

impl WasmMemory {
    const PAGE_SIZE: usize = 0x10000;

    fn new(limit: WasmLimit) -> Self {
        let size = limit.min as usize * Self::PAGE_SIZE;
        let mut memory = Vec::with_capacity(size);
        memory.resize(size, 0);
        Self { limit, memory }
    }

    pub fn write_bytes(&mut self, offset: usize, src: &[u8]) -> Result<(), WasmMemoryError> {
        let size = src.len();
        let limit = self.memory.len();
        if offset < limit && size < limit && offset + size < limit {
            let dest = &mut self.memory[0] as *mut u8;
            let src = &src[0] as *const u8;
            unsafe {
                dest.copy_from_nonoverlapping(src, size);
            }
            Ok(())
        } else {
            Err(WasmMemoryError::OutOfBounds)
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Copy, Clone)]
pub enum WasmMemoryError {
    OutOfBounds,
    OutOfMemory,
}

struct WasmTable {
    limit: WasmLimit,
    table: Vec<usize>,
}

impl WasmTable {
    fn from_stream(stream: &mut Leb128Stream) -> Result<Self, Leb128Error> {
        match stream.read_uint() {
            Ok(0x70) => (),
            Err(err) => return Err(err),
            _ => return Err(Leb128Error::UnexpectedToken),
        };
        WasmLimit::from_stream(stream).map(|limit| {
            let size = limit.min as usize;
            let mut table = Vec::with_capacity(size);
            table.resize(size, 0);
            Self { limit, table }
        })
    }
}

#[derive(Debug, Copy, Clone)]
struct WasmFunction {
    type_ref: usize,
    origin: WasmFunctionOrigin,
}

impl WasmFunction {
    fn from_import(type_ref: usize, index: usize) -> Self {
        Self {
            type_ref,
            origin: WasmFunctionOrigin::Import(index),
        }
    }

    fn internal(type_ref: usize) -> Self {
        Self {
            type_ref,
            origin: WasmFunctionOrigin::Internal,
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum WasmFunctionOrigin {
    Internal,
    Import(usize),
    Export(usize),
}

#[derive(Debug)]
struct WasmType {
    params: Vec<WasmValType>,
    result: Vec<WasmValType>,
}

impl WasmType {
    fn from_stream(stream: &mut Leb128Stream) -> Result<Self, Leb128Error> {
        match stream.read_uint() {
            Ok(0x60) => (),
            Err(err) => return Err(err),
            _ => return Err(Leb128Error::UnexpectedToken),
        };
        let n_params = match stream.read_uint() {
            Ok(v) => v as usize,
            Err(err) => return Err(err),
        };
        let mut params = Vec::with_capacity(n_params);
        for _ in 0..n_params {
            match stream.read_uint() {
                Ok(v) => {
                    WasmValType::from_u64(v).map(|v| params.push(v));
                }
                Err(err) => return Err(err),
            }
        }
        let n_result = match stream.read_uint() {
            Ok(v) => v as usize,
            Err(err) => return Err(err),
        };
        let mut result = Vec::with_capacity(n_result);
        for _ in 0..n_result {
            match stream.read_uint() {
                Ok(v) => {
                    WasmValType::from_u64(v).map(|v| result.push(v));
                }
                Err(err) => return Err(err),
            }
        }
        Ok(Self { params, result })
    }
}

#[allow(dead_code)]
struct WasmImport {
    mod_name: String,
    name: String,
    index: WasmImportType,
}

impl WasmImport {
    fn from_stream(stream: &mut Leb128Stream) -> Result<Self, Leb128Error> {
        let mod_name = match stream.get_string() {
            Ok(v) => v.to_string(),
            Err(err) => return Err(err),
        };
        let name = match stream.get_string() {
            Ok(v) => v.to_string(),
            Err(err) => return Err(err),
        };
        let index = match WasmImportType::from_stream(stream) {
            Ok(v) => v,
            Err(err) => return Err(err),
        };

        Ok(Self {
            mod_name,
            name,
            index,
        })
    }
}

#[derive(Debug, Copy, Clone)]
enum WasmImportType {
    Type(usize),
    Table(usize),
    Memory(usize),
    Global(usize),
}

impl WasmImportType {
    fn from_stream(stream: &mut Leb128Stream) -> Result<Self, Leb128Error> {
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
            _ => Err(Leb128Error::UnexpectedToken),
        })
    }
}

#[allow(dead_code)]
struct WasmExport {
    name: String,
    index: WasmExportType,
}

impl WasmExport {
    fn from_stream(stream: &mut Leb128Stream) -> Result<Self, Leb128Error> {
        let name = match stream.get_string() {
            Ok(v) => v.to_string(),
            Err(err) => return Err(err),
        };
        let index = match WasmExportType::from_stream(stream) {
            Ok(v) => v,
            Err(err) => return Err(err),
        };

        Ok(Self { name, index })
    }
}

#[derive(Debug, Copy, Clone)]
enum WasmExportType {
    Function(usize),
    Table(usize),
    Memory(usize),
    Global(usize),
}

impl WasmExportType {
    fn from_stream(stream: &mut Leb128Stream) -> Result<Self, Leb128Error> {
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
            _ => Err(Leb128Error::UnexpectedToken),
        })
    }
}

#[derive(Debug, Copy, Clone)]
enum WasmOpcode {
    Unreachable = 0x00,
    End = 0x0B,
    I32Const = 0x41,
}

impl From<u64> for WasmOpcode {
    fn from(v: u64) -> Self {
        match v {
            0x0B => WasmOpcode::End,
            0x41 => WasmOpcode::I32Const,
            _ => WasmOpcode::Unreachable,
        }
    }
}
