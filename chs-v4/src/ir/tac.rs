use crate::types::MethodId;

#[derive(Debug)]
pub struct Program {
    pub procs: Vec<Proc>,
}

impl Program {
    pub fn new() -> Self {
        Self { procs: Vec::new() }
    }

    pub fn push(&mut self, proc: Proc) {
        self.procs.push(proc);
    }
}

#[derive(Debug)]
pub struct Proc {
    pub method_id: MethodId,
    pub name: String,
    pub instrs: Vec<Instr>,
}

impl Proc {
    pub fn new(method_id: MethodId, name: String) -> Self {
        Self {
            method_id,
            name,
            instrs: Vec::new(),
        }
    }

    pub fn push(&mut self, ret: Instr) {
        self.instrs.push(ret);
    }
}

#[derive(Debug)]
pub enum Instr {
    Ret,
}
