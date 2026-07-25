use super::block::BasicBlock;
use super::inst::{BlockId, InstData};
use super::types::Type;

#[derive(Debug, Clone)]
pub struct Signature {
    pub name: String,
    pub has_va_args: bool,
    pub params: Vec<Type>,
    pub return_type: Type,
    pub mangled_name: String,
    pub is_private: bool,
}

#[derive(Debug, Clone)]
pub enum Function {
    Foreign {
        name: String,
        link_name: String,
        signature: Signature,
    },
    Default {
        name: String,
        signature: Signature,
        blocks: Vec<BasicBlock>,
        instructions: Vec<InstData>,
        entry_block: BlockId,
    },
}

impl Function {
    pub fn new(name: String, signature: Signature) -> Self {
        let entry_block = BasicBlock::new(BlockId(0));
        Self::Default {
            name,
            signature,
            blocks: vec![entry_block],
            instructions: Vec::new(),
            entry_block: BlockId(0),
        }
    }

    pub fn foreign(name: String, link_name: String, signature: Signature) -> Self {
        Self::Foreign {
            name,
            link_name,
            signature,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Function::Foreign { name, .. } => name,
            Function::Default { name, .. } => name,
        }
    }

    pub fn symbol_name(&self) -> &str {
        match self {
            Function::Foreign { link_name, .. } => link_name,
            Function::Default { name, .. } => name,
        }
    }

    pub fn signature(&self) -> &Signature {
        match self {
            Function::Foreign { signature, .. } => signature,
            Function::Default { signature, .. } => signature,
        }
    }

    pub fn is_default(&self) -> bool {
        matches!(self, Self::Default { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RunId(pub u32);

#[derive(Debug)]
pub struct RunBlock {
    pub id: RunId,
    pub return_type: Option<Type>,
    pub blocks: Vec<BasicBlock>,
    pub instructions: Vec<InstData>,
    pub entry_block: BlockId,
}

impl RunBlock {
    pub fn new(id: RunId) -> Self {
        let entry_block = BasicBlock::new(BlockId(0));
        Self {
            id,
            return_type: None,
            blocks: vec![entry_block],
            instructions: Vec::new(),
            entry_block: BlockId(0),
        }
    }
}
