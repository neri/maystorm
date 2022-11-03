use super::*;

/// Multi Bytes Opcodes (FC)
#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum WasmOpcodeFC {
    /// `FC 08 memory.init segment memory` (bulk_memory_operations)
    MemoryInit = 0x08,
    /// `FC 09 data.drop segment` (bulk_memory_operations)
    DataDrop = 0x09,
    /// `FC 0A memory.copy memory_dst memory_src` (bulk_memory_operations)
    MemoryCopy = 0x0A,
    /// `FC 0B memory.fill memory` (bulk_memory_operations)
    MemoryFill = 0x0B,
    /// `FC 0C table.init segment table` (bulk_memory_operations)
    TableInit = 0x0C,
    /// `FC 0D elem.drop segment` (bulk_memory_operations)
    ElemDrop = 0x0D,
    /// `FC 0E table.copy table_dst table_src` (bulk_memory_operations)
    TableCopy = 0x0E,
}

impl WasmOpcodeFC {
    pub const fn new(value: u32) -> Option<Self> {
        match value {
            0x08 => Some(Self::MemoryInit),
            0x09 => Some(Self::DataDrop),
            0x0A => Some(Self::MemoryCopy),
            0x0B => Some(Self::MemoryFill),
            0x0C => Some(Self::TableInit),
            0x0D => Some(Self::ElemDrop),
            0x0E => Some(Self::TableCopy),
            _ => None,
        }
    }

    pub const fn to_str(&self) -> &str {
        match *self {
            Self::MemoryInit => "memory.init",
            Self::DataDrop => "data.drop",
            Self::MemoryCopy => "memory.copy",
            Self::MemoryFill => "memory.fill",
            Self::TableInit => "table.init",
            Self::ElemDrop => "elem.drop",
            Self::TableCopy => "table.copy",
        }
    }

    pub const fn proposal_type(&self) -> WasmProposalType {
        match *self {
            Self::MemoryInit => WasmProposalType::BulkMemoryOperations,
            Self::DataDrop => WasmProposalType::BulkMemoryOperations,
            Self::MemoryCopy => WasmProposalType::BulkMemoryOperations,
            Self::MemoryFill => WasmProposalType::BulkMemoryOperations,
            Self::TableInit => WasmProposalType::BulkMemoryOperations,
            Self::ElemDrop => WasmProposalType::BulkMemoryOperations,
            Self::TableCopy => WasmProposalType::BulkMemoryOperations,
        }
    }
}
