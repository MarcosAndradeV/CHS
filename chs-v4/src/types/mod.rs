use std::collections::{HashMap, hash_map};
use std::path::{Path, PathBuf};

use crate::parser::lexer::Token;

#[derive(Debug)]
pub struct Module {
    file_path: PathBuf,
    symbols: HashMap<String, Symbol>,
}

impl Module {
    fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            symbols: HashMap::new(),
        }
    }
    pub fn register(db: &mut Database, file_path: PathBuf) -> ModuleId {
        match db.modules_mapping.entry(file_path) {
            hash_map::Entry::Occupied(occupied_entry) => *occupied_entry.get(),
            hash_map::Entry::Vacant(vacant_entry) => {
                let file_path = vacant_entry.key().clone();
                let m = Self::new(file_path);
                let id = ModuleId(db.modules.len() as u32);
                vacant_entry.insert(id);
                db.modules.push(m);
                id
            }
        }
    }
}

#[derive(Debug)]
pub struct Method {
    name: Token,
    arguments: Vec<TypeId>,
    return_type: Option<TypeId>,
}

impl Method {
    pub fn register(db: &mut Database, module: ModuleId, name: Token) -> Result<MethodId, String> {
        if let Some(_) = module.has_symbol(db, name.source()) {
            return Err(format!(
                "{}:{}: Cannot redefine '{}'",
                module.file_path(db).display(),
                name.loc,
                name
            ));
        }
        let m = Self {
            name,
            arguments: vec![],
            return_type: None,
        };
        let id = MethodId(db.methods.len() as u32);
        module.add_symbol(db, m.name.source(), Symbol::Method(id));
        db.methods.push(m);
        Ok(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Symbol {
    Module(ModuleId),
    Method(MethodId),
    Trait(TraitId),
    Type(TypeId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModuleId(u32);
impl ModuleId {
    pub fn get_method(self, db: &Database, source: &str) -> Option<MethodId> {
        if let Some(Symbol::Method(id)) = self.has_symbol(db, source) {
            Some(id)
        } else {
            None
        }
    }
    fn file_path(self, db: &Database) -> &Path {
        db.modules[self.0 as usize].file_path.as_path()
    }
    fn has_symbol(self, db: &Database, source: &str) -> Option<Symbol> {
        db.modules[self.0 as usize].symbols.get(source).copied()
    }
    fn add_symbol(self, db: &mut Database, source: &str, sym: Symbol) {
        assert!(
            db.modules[self.0 as usize]
                .symbols
                .insert(source.to_string(), sym)
                .is_none()
        );
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MethodId(u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TraitId(u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeId(u32);

#[derive(Debug)]
pub struct Database {
    modules_mapping: HashMap<PathBuf, ModuleId>,
    modules: Vec<Module>,
    methods: Vec<Method>,
}

impl Database {
    pub fn new() -> Self {
        Self {
            modules_mapping: HashMap::new(),
            modules: Vec::new(),
            methods: Vec::new(),
        }
    }
    pub fn get_module(&self, name: &Path) -> Option<ModuleId> {
        self.modules_mapping.get(name).copied()
    }
    pub unsafe fn get_module_unchecked(&self, name: &Path) -> ModuleId {
        self.modules_mapping.get(name).copied().unwrap()
    }
}
