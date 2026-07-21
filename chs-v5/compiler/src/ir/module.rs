use std::collections::HashMap;

use super::function::{Function, RunBlock, RunId};
use crate::types::TypeDatabase;

#[derive(Debug)]
pub struct Module {
    pub type_db: TypeDatabase,
    pub functions: HashMap<String, Function>,
    pub run_blocks: Vec<RunBlock>,
}

impl Module {
    pub fn new(type_db: TypeDatabase) -> Self {
        Self {
            type_db,
            functions: HashMap::new(),
            run_blocks: Vec::new(),
        }
    }

    pub fn add_function(&mut self, function: Function) {
        self.functions.insert(function.name().to_string(), function);
    }

    pub fn add_run_block(&mut self, run_block: RunBlock) {
        self.run_blocks.push(run_block);
    }

    pub fn next_run_id(&self) -> RunId {
        RunId(self.run_blocks.len() as u32)
    }
}
