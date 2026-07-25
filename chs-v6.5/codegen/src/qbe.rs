use ir::{
    EnumLayout, Function, InstId, Instruction, Module, Operand, StructLayout, Type as TypeID,
    type_layout,
};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use types::{self as t, Type as LangType, TypeDatabase};

pub struct QbeTranspiler<'a> {
    module: &'a Module,
    output: String,
    string_map: HashMap<std::rc::Rc<str>, usize>,
}

fn escape_string(s: &str) -> String {
    let mut escaped = String::new();
    for c in s.chars() {
        match c {
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\x08' => escaped.push_str("\\b"),
            '\x0c' => escaped.push_str("\\f"),
            c if (c as u32) < 32 || (c as u32) == 127 => {
                escaped.push_str(&format!("\\{:03o}", c as u32));
            }
            _ => escaped.push(c),
        }
    }
    escaped
}

fn collect_string_literals(module: &Module) -> HashMap<std::rc::Rc<str>, usize> {
    let mut map = HashMap::new();
    let mut next_id = 0;

    let mut add_str = |s: &std::rc::Rc<str>| {
        map.entry(s.clone()).or_insert_with(|| {
            let id = next_id;
            next_id += 1;
            id
        });
    };

    // Collect reflection strings
    let mut visited = std::collections::HashSet::new();
    let mut queue: Vec<t::TypeID> = module.type_db.queried_types.iter().copied().collect();
    while let Some(ty) = queue.pop() {
        let canonical = module.type_db.resolve(ty);
        if visited.insert(canonical) {
            let type_name = module.type_db.type_to_string(canonical);
            add_str(&std::rc::Rc::from(type_name));

            match module.type_db.get_type(canonical) {
                t::Type::Pointer(inner) => {
                    queue.push(*inner);
                }
                t::Type::Array(inner, _) => {
                    queue.push(*inner);
                }
                t::Type::Slice(inner) => {
                    queue.push(*inner);
                }
                t::Type::Struct {
                    fields: Some(fields),
                    ..
                } => {
                    for field in fields {
                        add_str(&std::rc::Rc::from(field.name.clone()));
                        queue.push(field.ty);
                    }
                }
                t::Type::Enum { variants, .. } => {
                    for variant in variants {
                        add_str(&std::rc::Rc::from(variant.name.clone()));
                    }
                }
                t::Type::FnPointer {
                    params,
                    return_type,
                } => {
                    for param in params {
                        queue.push(*param);
                    }
                    queue.push(*return_type);
                }
                _ => {}
            }
        }
    }

    for func in module.functions.values() {
        if let Function::Default { instructions, .. } = func {
            for inst_data in instructions {
                match &inst_data.inst {
                    Instruction::Add(l, r)
                    | Instruction::Sub(l, r)
                    | Instruction::Mul(l, r)
                    | Instruction::Div(l, r)
                    | Instruction::Mod(l, r)
                    | Instruction::Eq(_, l, r)
                    | Instruction::NotEq(_, l, r)
                    | Instruction::Lt(_, l, r)
                    | Instruction::LtEq(_, l, r)
                    | Instruction::Gt(_, l, r)
                    | Instruction::GtEq(_, l, r)
                    | Instruction::And(l, r)
                    | Instruction::Or(l, r)
                    | Instruction::BitAnd(l, r)
                    | Instruction::BitOr(l, r)
                    | Instruction::BitXor(l, r)
                    | Instruction::Index(l, r)
                    | Instruction::GetIndexPtr(l, r)
                    | Instruction::Store(_, l, r) => {
                        if let Operand::String(s) = l {
                            add_str(s);
                        }
                        if let Operand::String(s) = r {
                            add_str(s);
                        }
                    }
                    Instruction::Neg(o)
                    | Instruction::Not(o)
                    | Instruction::Load(o)
                    | Instruction::CondBr(o, _, _)
                    | Instruction::GetMemberPtr(o, _)
                    | Instruction::Cast(o) => {
                        if let Operand::String(s) = o {
                            add_str(s);
                        }
                    }
                    Instruction::Return(Some(Operand::String(s))) => {
                        add_str(s);
                    }
                    Instruction::Call(callee, args) => {
                        if let Operand::String(s) = callee {
                            add_str(s);
                        }
                        for arg in args.as_ref() {
                            if let Operand::String(s) = arg {
                                add_str(s);
                            }
                        }
                    }
                    Instruction::OOBCheck(msg, ..) => {
                        if let Operand::String(s) = msg {
                            add_str(s);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    map
}

impl<'a> QbeTranspiler<'a> {
    pub fn new(module: &'a Module) -> Self {
        let string_map = collect_string_literals(module);
        Self {
            module,
            output: String::new(),
            string_map,
        }
    }

    pub fn transpile(mut self) -> String {
        self.emit_types();
        self.emit_strings();
        self.emit_reflection_data();

        let mut funcs: Vec<&Function> = self.module.functions.values().collect();
        funcs.sort_by_key(|f| f.name());
        for func in funcs {
            if func.is_default() {
                self.emit_function(func);
            }
        }

        self.emit_globals();

        if self.module.functions.contains_key("main") {
            self.emit_main_wrapper();
        }

        self.output
    }

    fn emit_globals(&mut self) {
        let globals = self.module.globals.clone();
        for g in globals {
            let (size, align) = ir::types::type_layout(g.ty, &self.module.type_db);
            let prefix = if g.is_thread_local { "thread " } else { "" };

            write!(self.output, "{}data ${} = align {} {{ ", prefix, g.name, align).unwrap();
            match g.init_val {
                ir::module::ConstVal::Int(v) => {
                    if size == 8 {
                        write!(self.output, "l {}", v).unwrap();
                    } else if size == 4 {
                        write!(self.output, "w {}", v).unwrap();
                    } else if size == 2 {
                        write!(self.output, "h {}", v).unwrap();
                    } else if size == 1 {
                        write!(self.output, "b {}", v).unwrap();
                    } else {
                        write!(self.output, "z {}", size).unwrap();
                    }
                }
                ir::module::ConstVal::Bool(v) => {
                    let val = if v { 1 } else { 0 };
                    write!(self.output, "b {}", val).unwrap();
                }
                ir::module::ConstVal::Zero => {
                    write!(self.output, "z {}", size).unwrap();
                }
            }
            writeln!(self.output, " }}").unwrap();
        }
    }

    fn emit_types(&mut self) {
        writeln!(self.output, "type :string = align 8 {{ l, w }}").unwrap();

        // 1. Emit all slice types first (since they don't have layout dependencies on other types)
        let mut slices = Vec::new();
        for &id in self.module.type_db.types.keys() {
            let canonical = self.module.type_db.resolve(id);
            if let LangType::Slice(inner) = self.module.type_db.get_type(canonical) {
                slices.push(*inner);
            }
        }
        slices.sort();
        slices.dedup();

        for &inner in &slices {
            let mangled_inner = self.mangle_type(inner);
            writeln!(
                self.output,
                "type :chs_slice_{} = align 8 {{ l, w }}",
                mangled_inner
            )
            .unwrap();
        }

        // 2. Gather all structs and arrays to define
        let mut types_to_define = Vec::new();
        // Add all structs and enums
        for (name, &type_id) in &self.module.type_db.names {
            if name == "string" {
                continue;
            }
            let canonical = self.module.type_db.resolve(type_id);
            match self.module.type_db.get_type(canonical) {
                LangType::Struct {
                    name: canonical_name,
                    fields: Some(_),
                    ..
                } if name == canonical_name => {
                    types_to_define.push(canonical);
                }
                LangType::Enum {
                    name: canonical_name,
                    ..
                } if name == canonical_name => {
                    types_to_define.push(canonical);
                }
                _ => {}
            }
        }
        // Add all arrays and tuples
        for &id in self.module.type_db.types.keys() {
            let canonical = self.module.type_db.resolve(id);
            match self.module.type_db.get_type(canonical) {
                LangType::Array(..) | LangType::Tuple(..) => {
                    types_to_define.push(canonical);
                }
                _ => {}
            }
        }
        // Deduplicate
        types_to_define.sort();
        types_to_define.dedup();

        // 3. Perform topological sort
        let mut visited = HashSet::new();
        let mut temp_visited = HashSet::new();
        let mut order = Vec::new();

        fn visit<'b>(
            ty: t::TypeID,
            type_db: &'b TypeDatabase,
            visited: &mut HashSet<t::TypeID>,
            temp_visited: &mut HashSet<t::TypeID>,
            order: &mut Vec<t::TypeID>,
        ) {
            let canonical = type_db.resolve(ty);
            if temp_visited.contains(&canonical) {
                return;
            }
            if !visited.contains(&canonical) {
                temp_visited.insert(canonical);

                match type_db.get_type(canonical) {
                    LangType::Struct {
                        fields: Some(fields),
                        ..
                    } => {
                        for field in fields {
                            let field_canon = type_db.resolve(field.ty);
                            match type_db.get_type(field_canon) {
                                LangType::Struct { name, .. } if name != "string" => {
                                    visit(field_canon, type_db, visited, temp_visited, order);
                                }
                                LangType::Array(..)
                                | LangType::Tuple(..)
                                | LangType::Enum { .. } => {
                                    visit(field_canon, type_db, visited, temp_visited, order);
                                }
                                _ => {}
                            }
                        }
                    }
                    LangType::Array(inner, _) => {
                        let inner_canon = type_db.resolve(*inner);
                        match type_db.get_type(inner_canon) {
                            LangType::Struct { name, .. } if name != "string" => {
                                visit(inner_canon, type_db, visited, temp_visited, order);
                            }
                            LangType::Array(..) | LangType::Tuple(..) | LangType::Enum { .. } => {
                                visit(inner_canon, type_db, visited, temp_visited, order);
                            }
                            _ => {}
                        }
                    }
                    LangType::Tuple(elements) => {
                        for &elem in elements {
                            let elem_canon = type_db.resolve(elem);
                            match type_db.get_type(elem_canon) {
                                LangType::Struct { name, .. } if name != "string" => {
                                    visit(elem_canon, type_db, visited, temp_visited, order);
                                }
                                LangType::Array(..)
                                | LangType::Tuple(..)
                                | LangType::Enum { .. } => {
                                    visit(elem_canon, type_db, visited, temp_visited, order);
                                }
                                _ => {}
                            }
                        }
                    }
                    LangType::Enum { repr, variants, .. } => {
                        let repr_canon = type_db.resolve(*repr);
                        match type_db.get_type(repr_canon) {
                            LangType::Struct { name, .. } if name != "string" => {
                                visit(repr_canon, type_db, visited, temp_visited, order);
                            }
                            LangType::Array(..) | LangType::Tuple(..) | LangType::Enum { .. } => {
                                visit(repr_canon, type_db, visited, temp_visited, order);
                            }
                            _ => {}
                        }
                        for variant in variants {
                            if let Some(payload_ty) = variant.payload {
                                let payload_canon = type_db.resolve(payload_ty);
                                match type_db.get_type(payload_canon) {
                                    LangType::Struct { name, .. } if name != "string" => {
                                        visit(payload_canon, type_db, visited, temp_visited, order);
                                    }
                                    LangType::Array(..)
                                    | LangType::Tuple(..)
                                    | LangType::Enum { .. } => {
                                        visit(payload_canon, type_db, visited, temp_visited, order);
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    _ => {}
                }

                temp_visited.remove(&canonical);
                visited.insert(canonical);
                order.push(canonical);
            }
        }

        for &ty in &types_to_define {
            visit(
                ty,
                &self.module.type_db,
                &mut visited,
                &mut temp_visited,
                &mut order,
            );
        }

        // 4. Emit the sorted types
        for ty in order {
            let canonical = self.module.type_db.resolve(ty);
            match self.module.type_db.get_type(canonical) {
                LangType::Struct { name, .. } => {
                    let layout = StructLayout::compute(canonical, &self.module.type_db);
                    write!(self.output, "type :{} = align {} {{ ", name, layout.align).unwrap();
                    if let LangType::Struct {
                        fields: Some(fields),
                        ..
                    } = self.module.type_db.get_type(canonical)
                    {
                        for (i, field) in fields.iter().enumerate() {
                            if i > 0 {
                                self.output.push_str(", ");
                            }
                            self.output.push_str(&self.map_extended_type(field.ty));
                        }
                    }
                    self.output.push_str(" }\n");
                }
                LangType::Tuple(elements) => {
                    // Lower structural tuples as anonymous, flat, byte-aligned struct definitions in QBE.
                    // QBE's ABI handler automatically decides whether small tuples/structs are passed/returned
                    // via registers or whether large aggregates are passed via caller-allocated stack storage
                    // with a pointer passed as an implicit first argument, conforming to the target C ABI.
                    let name = self.mangle_type(canonical);
                    let layout = StructLayout::compute(canonical, &self.module.type_db);
                    write!(self.output, "type :{} = align {} {{ ", name, layout.align).unwrap();
                    for (i, &elem) in elements.iter().enumerate() {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        self.output.push_str(&self.map_extended_type(elem));
                    }
                    self.output.push_str(" }\n");
                }
                LangType::Array(inner, size) => {
                    let mangled_inner = self.mangle_type(*inner);
                    let elem_qbe = self.map_extended_type(*inner);
                    let (_, elem_align) = type_layout(*inner, &self.module.type_db);
                    writeln!(
                        self.output,
                        "type :chs_array_{}_{} = align {} {{ {} {} }}",
                        size, mangled_inner, elem_align, elem_qbe, size
                    )
                    .unwrap();
                }
                LangType::Enum { name, repr, .. } => {
                    let layout = EnumLayout::compute(canonical, &self.module.type_db);
                    let tag_qbe = self.map_extended_type(*repr);
                    let (tag_size, _) = type_layout(*repr, &self.module.type_db);
                    if layout.size > tag_size {
                        let payload_size = layout.size - tag_size;
                        writeln!(
                            self.output,
                            "type :{} = align {} {{ {}, b {} }}",
                            name, layout.align, tag_qbe, payload_size
                        )
                        .unwrap();
                    } else {
                        writeln!(
                            self.output,
                            "type :{} = align {} {{ {} }}",
                            name, layout.align, tag_qbe
                        )
                        .unwrap();
                    }
                }
                _ => {}
            }
        }
        self.output.push('\n');
    }

    fn emit_strings(&mut self) {
        let mut strings: Vec<(&std::rc::Rc<str>, &usize)> = self.string_map.iter().collect();
        strings.sort_by_key(|&(_, id)| *id);
        for (val, &id) in strings {
            let escaped = escape_string(val);
            writeln!(
                self.output,
                "data $str_data_{} = {{ b \"{}\", b 0 }}",
                id, escaped
            )
            .unwrap();
            writeln!(
                self.output,
                "data $str_{} = align 8 {{ l $str_data_{}, w {} }}",
                id,
                id,
                val.len()
            )
            .unwrap();
        }
        if !self.string_map.is_empty() {
            self.output.push('\n');
        }
    }

    fn map_abi_type(&self, ty: t::TypeID) -> String {
        let canonical = self.module.type_db.resolve(ty);
        if canonical == self.module.type_db.float() {
            "d".to_string()
        } else if canonical == self.module.type_db.void()
            || canonical == self.module.type_db.u8()
            || canonical == self.module.type_db.bool()
            || canonical == self.module.type_db.int()
        {
            "w".to_string()
        } else {
            match self.module.type_db.get_type(canonical) {
                LangType::Primitive(_) => "w".to_string(),
                LangType::Pointer(_) | LangType::FnPointer { .. } => "l".to_string(),
                LangType::Array(..) => {
                    let mangled = self.mangle_type(canonical);
                    format!(":chs_{}", mangled)
                }
                LangType::Slice(..) => {
                    let mangled = self.mangle_type(canonical);
                    format!(":chs_{}", mangled)
                }
                LangType::Struct { name, .. } => format!(":{}", name),
                LangType::Tuple(..) => {
                    let mangled = self.mangle_type(canonical);
                    format!(":{}", mangled)
                }
                LangType::Enum { name, .. } => format!(":{}", name),
                LangType::TypeVar(_) => "l".to_string(),
                LangType::Any(_) => ":Any".to_string(),
                LangType::String(_) => ":string".to_string(),
                LangType::Distinct { base, .. } => self.map_abi_type(*base),
            }
        }
    }

    fn map_extended_type(&self, ty: t::TypeID) -> String {
        let canonical = self.module.type_db.resolve(ty);
        if canonical == self.module.type_db.float() {
            "d".to_string()
        } else if canonical == self.module.type_db.void()
            || canonical == self.module.type_db.u8()
            || canonical == self.module.type_db.bool()
        {
            "b".to_string()
        } else if canonical == self.module.type_db.int() {
            "w".to_string()
        } else {
            match self.module.type_db.get_type(canonical) {
                LangType::Primitive(_) => "w".to_string(),
                LangType::Pointer(_) | LangType::FnPointer { .. } => "l".to_string(),
                LangType::Array(..) => {
                    let mangled = self.mangle_type(canonical);
                    format!(":chs_{}", mangled)
                }
                LangType::Slice(..) => {
                    let mangled = self.mangle_type(canonical);
                    format!(":chs_{}", mangled)
                }
                LangType::Struct { name, .. } => format!(":{}", name),
                LangType::Tuple(..) => {
                    let mangled = self.mangle_type(canonical);
                    format!(":{}", mangled)
                }
                LangType::Enum { name, .. } => format!(":{}", name),
                LangType::TypeVar(_) => "l".to_string(),
                LangType::Any(_) => ":Any".to_string(),
                LangType::String(_) => ":string".to_string(),
                LangType::Distinct { base, .. } => self.map_extended_type(*base),
            }
        }
    }

    fn map_base_type(&self, ty: t::TypeID) -> String {
        let canonical = self.module.type_db.resolve(ty);
        if canonical == self.module.type_db.float() {
            "d".to_string()
        } else if canonical == self.module.type_db.void() {
            "".to_string()
        } else if canonical == self.module.type_db.u8()
            || canonical == self.module.type_db.bool()
            || canonical == self.module.type_db.int()
        {
            "w".to_string()
        } else {
            match self.module.type_db.get_type(canonical) {
                LangType::Primitive(_) => "w".to_string(),
                LangType::Pointer(_) | LangType::FnPointer { .. } => "l".to_string(),
                LangType::Array(..) => "l".to_string(),
                LangType::Slice(..) => "l".to_string(),
                LangType::Struct { .. } | LangType::Tuple(..) => "l".to_string(),
                LangType::Enum { .. } => "l".to_string(),
                LangType::TypeVar(_) => "l".to_string(),
                LangType::Any(_) => "l".to_string(),
                LangType::String(_) => "l".to_string(),
                LangType::Distinct { base, .. } => self.map_base_type(*base),
            }
        }
    }

    fn emit_function_signature(&mut self, func: &Function) {
        let ret_ty = &func.signature().return_type;
        let canonical_ret = self.module.type_db.resolve(*ret_ty);
        let ret_str = if canonical_ret == self.module.type_db.void()
            || canonical_ret == self.module.type_db.noreturn()
        {
            "".to_string()
        } else {
            format!("{} ", self.map_abi_type(*ret_ty))
        };

        write!(
            self.output,
            "export function {}$chs_{}(",
            ret_str,
            func.name()
        )
        .unwrap();
        for (i, param) in func.signature().params.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            write!(self.output, "{} %param_{}", self.map_abi_type(*param), i).unwrap();
        }
        self.output.push(')');
    }

    fn emit_function(&mut self, func: &Function) {
        self.emit_function_signature(func);
        let Function::Default {
            blocks,
            instructions,
            ..
        } = func
        else {
            return;
        };
        self.output.push_str(" {\n");

        writeln!(self.output, "@start").unwrap();
        writeln!(self.output, "    jmp @block_0").unwrap();

        for block in blocks {
            writeln!(self.output, "@block_{}", block.id.0).unwrap();
            for inst_id in &block.instructions {
                let inst_data = &instructions[inst_id.0 as usize];
                self.emit_instruction(func, inst_id, &inst_data.inst, &inst_data.ty);
            }
        }

        self.output.push_str("}\n\n");
    }

    fn emit_instruction(&mut self, func: &Function, id: &InstId, inst: &Instruction, ty: &TypeID) {
        let base_ty = self.map_base_type(*ty);

        match inst {
            Instruction::Add(l, r) => {
                let l_ty = self.get_operand_type(l, func);
                let r_ty = self.get_operand_type(r, func);
                let l_canon = self.module.type_db.resolve(l_ty);
                let r_canon = self.module.type_db.resolve(r_ty);

                let l_is_ptr =
                    matches!(self.module.type_db.get_type(l_canon), LangType::Pointer(_));
                let r_is_ptr =
                    matches!(self.module.type_db.get_type(r_canon), LangType::Pointer(_));

                if l_is_ptr {
                    let LangType::Pointer(inner_ty) = self.module.type_db.get_type(l_canon) else {
                        unreachable!()
                    };
                    let (raw_size, _) = type_layout(*inner_ty, &self.module.type_db);
                    let elem_size = std::cmp::max(raw_size, 1);

                    let ext_inst = if r_canon == self.module.type_db.int() {
                        "extsw"
                    } else {
                        "extuw"
                    };
                    writeln!(
                        self.output,
                        "    %idx_l_{} =l {} {}",
                        id.0,
                        ext_inst,
                        self.op(r)
                    )
                    .unwrap();
                    writeln!(
                        self.output,
                        "    %offset_{} =l mul %idx_l_{}, {}",
                        id.0, id.0, elem_size
                    )
                    .unwrap();
                    writeln!(
                        self.output,
                        "    %r_{} =l add {}, %offset_{}",
                        id.0,
                        self.op(l),
                        id.0
                    )
                    .unwrap();
                } else if r_is_ptr {
                    let LangType::Pointer(inner_ty) = self.module.type_db.get_type(r_canon) else {
                        unreachable!()
                    };
                    let (raw_size, _) = type_layout(*inner_ty, &self.module.type_db);
                    let elem_size = std::cmp::max(raw_size, 1);

                    let ext_inst = if l_canon == self.module.type_db.int() {
                        "extsw"
                    } else {
                        "extuw"
                    };
                    writeln!(
                        self.output,
                        "    %idx_l_{} =l {} {}",
                        id.0,
                        ext_inst,
                        self.op(l)
                    )
                    .unwrap();
                    writeln!(
                        self.output,
                        "    %offset_{} =l mul %idx_l_{}, {}",
                        id.0, id.0, elem_size
                    )
                    .unwrap();
                    writeln!(
                        self.output,
                        "    %r_{} =l add {}, %offset_{}",
                        id.0,
                        self.op(r),
                        id.0
                    )
                    .unwrap();
                } else {
                    let op_ty = self.get_operand_type(l, func);
                    let op_base_ty = self.map_base_type(op_ty);
                    writeln!(
                        self.output,
                        "    %r_{} ={} add {}, {}",
                        id.0,
                        op_base_ty,
                        self.op(l),
                        self.op(r)
                    )
                    .unwrap();
                }
            }
            Instruction::Sub(l, r) => {
                let l_ty = self.get_operand_type(l, func);
                let r_ty = self.get_operand_type(r, func);
                let l_canon = self.module.type_db.resolve(l_ty);
                let r_canon = self.module.type_db.resolve(r_ty);

                let l_is_ptr =
                    matches!(self.module.type_db.get_type(l_canon), LangType::Pointer(_));
                let r_is_ptr =
                    matches!(self.module.type_db.get_type(r_canon), LangType::Pointer(_));

                if l_is_ptr && r_is_ptr {
                    let LangType::Pointer(inner_ty) = self.module.type_db.get_type(l_canon) else {
                        unreachable!()
                    };
                    let (raw_size, _) = type_layout(*inner_ty, &self.module.type_db);
                    let elem_size = std::cmp::max(raw_size, 1);

                    let dest_base = self.map_base_type(*ty);
                    if dest_base == "w" {
                        writeln!(
                            self.output,
                            "    %diff_l_{} =l sub {}, {}",
                            id.0,
                            self.op(l),
                            self.op(r)
                        )
                        .unwrap();
                        writeln!(
                            self.output,
                            "    %div_l_{} =l div %diff_l_{}, {}",
                            id.0, id.0, elem_size
                        )
                        .unwrap();
                        writeln!(self.output, "    %r_{} =w copy %div_l_{}", id.0, id.0).unwrap();
                    } else {
                        writeln!(
                            self.output,
                            "    %diff_{} =l sub {}, {}",
                            id.0,
                            self.op(l),
                            self.op(r)
                        )
                        .unwrap();
                        writeln!(
                            self.output,
                            "    %r_{} =l div %diff_{}, {}",
                            id.0, id.0, elem_size
                        )
                        .unwrap();
                    }
                } else if l_is_ptr {
                    let LangType::Pointer(inner_ty) = self.module.type_db.get_type(l_canon) else {
                        unreachable!()
                    };
                    let (raw_size, _) = type_layout(*inner_ty, &self.module.type_db);
                    let elem_size = std::cmp::max(raw_size, 1);

                    let ext_inst = if r_canon == self.module.type_db.int() {
                        "extsw"
                    } else {
                        "extuw"
                    };
                    writeln!(
                        self.output,
                        "    %idx_l_{} =l {} {}",
                        id.0,
                        ext_inst,
                        self.op(r)
                    )
                    .unwrap();
                    writeln!(
                        self.output,
                        "    %offset_{} =l mul %idx_l_{}, {}",
                        id.0, id.0, elem_size
                    )
                    .unwrap();
                    writeln!(
                        self.output,
                        "    %r_{} =l sub {}, %offset_{}",
                        id.0,
                        self.op(l),
                        id.0
                    )
                    .unwrap();
                } else {
                    let op_ty = self.get_operand_type(l, func);
                    let op_base_ty = self.map_base_type(op_ty);
                    writeln!(
                        self.output,
                        "    %r_{} ={} sub {}, {}",
                        id.0,
                        op_base_ty,
                        self.op(l),
                        self.op(r)
                    )
                    .unwrap();
                }
            }
            Instruction::Mul(l, r) => {
                let op_ty = self.get_operand_type(l, func);
                let op_base_ty = self.map_base_type(op_ty);
                let op = "mul";
                writeln!(
                    self.output,
                    "    %r_{} ={} {} {}, {}",
                    id.0,
                    op_base_ty,
                    op,
                    self.op(l),
                    self.op(r)
                )
                .unwrap();
            }
            Instruction::Div(l, r) => {
                let op_ty = self.get_operand_type(l, func);
                let op_base_ty = self.map_base_type(op_ty);
                let op = "div";
                writeln!(
                    self.output,
                    "    %r_{} ={} {} {}, {}",
                    id.0,
                    op_base_ty,
                    op,
                    self.op(l),
                    self.op(r)
                )
                .unwrap();
            }
            Instruction::Mod(l, r) => {
                let op_ty = self.get_operand_type(l, func);
                let op_base_ty = self.map_base_type(op_ty);
                writeln!(
                    self.output,
                    "    %r_{} ={} rem {}, {}",
                    id.0,
                    op_base_ty,
                    self.op(l),
                    self.op(r)
                )
                .unwrap();
            }

            Instruction::Eq(comp_ty, l, r)
            | Instruction::NotEq(comp_ty, l, r)
            | Instruction::Lt(comp_ty, l, r)
            | Instruction::LtEq(comp_ty, l, r)
            | Instruction::Gt(comp_ty, l, r)
            | Instruction::GtEq(comp_ty, l, r) => {
                let canonical_comp = self.module.type_db.resolve(*comp_ty);
                if let LangType::Enum { repr, .. } = self.module.type_db.get_type(canonical_comp) {
                    let repr_canon = self.module.type_db.resolve(*repr);
                    let load_inst = if repr_canon == self.module.type_db.int() {
                        "loadw"
                    } else if repr_canon == self.module.type_db.u8()
                        || repr_canon == self.module.type_db.bool()
                    {
                        "loadub"
                    } else {
                        "loadw"
                    };

                    writeln!(
                        self.output,
                        "    %tag_l_{} =w {} {}",
                        id.0,
                        load_inst,
                        self.op(l)
                    )
                    .unwrap();
                    writeln!(
                        self.output,
                        "    %tag_r_{} =w {} {}",
                        id.0,
                        load_inst,
                        self.op(r)
                    )
                    .unwrap();

                    let op_name = match inst {
                        Instruction::Eq(..) => "ceqw",
                        Instruction::NotEq(..) => "cnew",
                        Instruction::Lt(..) => "csltw",
                        Instruction::LtEq(..) => "cslew",
                        Instruction::Gt(..) => "csgtw",
                        Instruction::GtEq(..) => "csgew",
                        _ => unreachable!(),
                    };
                    writeln!(
                        self.output,
                        "    %r_{} =w {} %tag_l_{}, %tag_r_{}",
                        id.0, op_name, id.0, id.0
                    )
                    .unwrap();
                } else {
                    let is_float = canonical_comp == self.module.type_db.float();
                    let comp_suffix = if is_float {
                        "d".to_string()
                    } else if canonical_comp == self.module.type_db.int() {
                        "w".to_string()
                    } else if matches!(
                        self.module.type_db.get_type(canonical_comp),
                        LangType::Pointer(_) | LangType::FnPointer { .. }
                    ) {
                        "l".to_string()
                    } else {
                        "w".to_string()
                    };

                    let op_name = match inst {
                        Instruction::Eq(..) => format!("ceq{}", comp_suffix),
                        Instruction::NotEq(..) => format!("cne{}", comp_suffix),
                        Instruction::Lt(..) => {
                            if is_float {
                                format!("clt{}", comp_suffix)
                            } else {
                                format!("cslt{}", comp_suffix)
                            }
                        }
                        Instruction::LtEq(..) => {
                            if is_float {
                                format!("cle{}", comp_suffix)
                            } else {
                                format!("csle{}", comp_suffix)
                            }
                        }
                        Instruction::Gt(..) => {
                            if is_float {
                                format!("cgt{}", comp_suffix)
                            } else {
                                format!("csgt{}", comp_suffix)
                            }
                        }
                        Instruction::GtEq(..) => {
                            if is_float {
                                format!("cge{}", comp_suffix)
                            } else {
                                format!("csge{}", comp_suffix)
                            }
                        }
                        _ => unreachable!(),
                    };

                    writeln!(
                        self.output,
                        "    %r_{} =w {} {}, {}",
                        id.0,
                        op_name,
                        self.op(l),
                        self.op(r)
                    )
                    .unwrap();
                }
            }

            Instruction::And(l, r) => {
                writeln!(
                    self.output,
                    "    %r_{} =w and {}, {}",
                    id.0,
                    self.op(l),
                    self.op(r)
                )
                .unwrap();
            }
            Instruction::Or(l, r) => {
                writeln!(
                    self.output,
                    "    %r_{} =w or {}, {}",
                    id.0,
                    self.op(l),
                    self.op(r)
                )
                .unwrap();
            }
            Instruction::BitAnd(l, r) => {
                writeln!(
                    self.output,
                    "    %r_{} =w and {}, {}",
                    id.0,
                    self.op(l),
                    self.op(r)
                )
                .unwrap();
            }
            Instruction::BitOr(l, r) => {
                writeln!(
                    self.output,
                    "    %r_{} =w or {}, {}",
                    id.0,
                    self.op(l),
                    self.op(r)
                )
                .unwrap();
            }
            Instruction::BitXor(l, r) => {
                writeln!(
                    self.output,
                    "    %r_{} =w xor {}, {}",
                    id.0,
                    self.op(l),
                    self.op(r)
                )
                .unwrap();
            }

            Instruction::Neg(op) => {
                let op_ty = self.get_operand_type(op, func);
                let op_base_ty = self.map_base_type(op_ty);
                if op_base_ty == "d" {
                    writeln!(self.output, "    %r_{} =d sub d_0.0, {}", id.0, self.op(op)).unwrap();
                } else {
                    writeln!(
                        self.output,
                        "    %r_{} ={} sub 0, {}",
                        id.0,
                        op_base_ty,
                        self.op(op)
                    )
                    .unwrap();
                }
            }
            Instruction::Not(op) => {
                writeln!(self.output, "    %r_{} =w ceqw {}, 0", id.0, self.op(op)).unwrap();
            }
            Instruction::Cast(op) => {
                let src_ty = self.get_operand_type(op, func);
                let dest_ty = *ty;

                let is_src_agg = self.is_aggregate(src_ty);
                let is_dest_agg = self.is_aggregate(dest_ty);

                if is_src_agg && is_dest_agg {
                    // Aggregate-to-aggregate cast (e.g. distinct struct type casts)
                    let (size, align) = type_layout(dest_ty, &self.module.type_db);
                    let alloc_inst = if align >= 16 {
                        "alloc16"
                    } else if align == 8 {
                        "alloc8"
                    } else {
                        "alloc4"
                    };
                    writeln!(self.output, "    %r_{} =l {} {}", id.0, alloc_inst, size).unwrap();
                    writeln!(
                        self.output,
                        "    call $memcpy(l %r_{}, l {}, l {})",
                        id.0,
                        self.op(op),
                        size
                    )
                    .unwrap();
                } else if !is_src_agg && is_dest_agg {
                    // cast primitive to aggregate (e.g. int -> Enum)
                    let (size, align) = type_layout(dest_ty, &self.module.type_db);
                    let alloc_inst = if align >= 16 {
                        "alloc16"
                    } else if align == 8 {
                        "alloc8"
                    } else {
                        "alloc4"
                    };
                    writeln!(self.output, "    %r_{} =l {} {}", id.0, alloc_inst, size).unwrap();

                    let canonical_dest = self.module.type_db.resolve(dest_ty);
                    if let LangType::Enum { repr, .. } =
                        self.module.type_db.get_type(canonical_dest)
                    {
                        let repr_canon = self.module.type_db.resolve(*repr);
                        let store_inst = if repr_canon == self.module.type_db.int() {
                            "storew"
                        } else if repr_canon == self.module.type_db.u8()
                            || repr_canon == self.module.type_db.bool()
                        {
                            "storeb"
                        } else {
                            "storew"
                        };
                        writeln!(
                            self.output,
                            "    {} {}, %r_{}",
                            store_inst,
                            self.op(op),
                            id.0
                        )
                        .unwrap();
                    } else {
                        writeln!(self.output, "    storew {}, %r_{}", self.op(op), id.0).unwrap();
                    }
                } else {
                    // normal cast or aggregate-to-primitive cast
                    let src_val = if is_src_agg {
                        let canonical_src = self.module.type_db.resolve(src_ty);
                        let load_inst = if let LangType::Enum { repr, .. } =
                            self.module.type_db.get_type(canonical_src)
                        {
                            let repr_canon = self.module.type_db.resolve(*repr);
                            if repr_canon == self.module.type_db.int() {
                                "loadw"
                            } else if repr_canon == self.module.type_db.u8()
                                || repr_canon == self.module.type_db.bool()
                            {
                                "loadub"
                            } else {
                                "loadw"
                            }
                        } else {
                            "loadw"
                        };
                        let temp_reg = format!("%cast_src_{}", id.0);
                        writeln!(
                            self.output,
                            "    {} =w {} {}",
                            temp_reg,
                            load_inst,
                            self.op(op)
                        )
                        .unwrap();
                        temp_reg
                    } else {
                        self.op(op)
                    };

                    let src_base = if is_src_agg {
                        "w".to_string()
                    } else {
                        self.map_base_type(src_ty)
                    };
                    let dest_base = self.map_base_type(dest_ty);

                    if src_base == dest_base {
                        writeln!(
                            self.output,
                            "    %r_{} ={} copy {}",
                            id.0, dest_base, src_val
                        )
                        .unwrap();
                    } else if src_base == "w" && dest_base == "l" {
                        let canonical_src = self.module.type_db.resolve(src_ty);
                        let repr_is_int = if let LangType::Enum { repr, .. } =
                            self.module.type_db.get_type(canonical_src)
                        {
                            self.module.type_db.resolve(*repr) == self.module.type_db.int()
                        } else {
                            canonical_src == self.module.type_db.int()
                        };
                        let ext_inst = if repr_is_int { "extsw" } else { "extuw" };
                        writeln!(self.output, "    %r_{} =l {} {}", id.0, ext_inst, src_val)
                            .unwrap();
                    } else if src_base == "l" && dest_base == "w" {
                        writeln!(self.output, "    %r_{} =w copy {}", id.0, src_val).unwrap();
                    } else if src_base == "w" && dest_base == "d" {
                        writeln!(self.output, "    %r_{} =d swtof {}", id.0, src_val).unwrap();
                    } else if src_base == "l" && dest_base == "d" {
                        writeln!(self.output, "    %r_{} =d sltod {}", id.0, src_val).unwrap();
                    } else if src_base == "d" && dest_base == "w" {
                        writeln!(self.output, "    %r_{} =w dtosi {}", id.0, src_val).unwrap();
                    } else if src_base == "d" && dest_base == "l" {
                        writeln!(self.output, "    %r_{} =l dtosl {}", id.0, src_val).unwrap();
                    } else {
                        panic!("Unsupported QBE cast from {} to {}", src_base, dest_base);
                    }
                }
            }

            Instruction::Alloca(alloc_ty) => {
                let (size, align) = type_layout(*alloc_ty, &self.module.type_db);
                let size = std::cmp::max(size, 1);
                let alloc_inst = if align >= 16 {
                    "alloc16"
                } else if align == 8 {
                    "alloc8"
                } else {
                    "alloc4"
                };
                writeln!(self.output, "    %r_{} =l {} {}", id.0, alloc_inst, size).unwrap();
            }

            Instruction::Load(ptr) => {
                let val_ty = *ty;
                if self.is_aggregate(val_ty) {
                    let (size, align) = type_layout(val_ty, &self.module.type_db);
                    let alloc_inst = if align >= 16 {
                        "alloc16"
                    } else if align == 8 {
                        "alloc8"
                    } else {
                        "alloc4"
                    };
                    writeln!(self.output, "    %r_{} =l {} {}", id.0, alloc_inst, size).unwrap();
                    writeln!(
                        self.output,
                        "    call $memcpy(l %r_{}, l {}, l {})",
                        id.0,
                        self.op(ptr),
                        size
                    )
                    .unwrap();
                } else {
                    let underlying_val = self.module.type_db.get_underlying_type(val_ty);
                    let load_inst = if underlying_val == self.module.type_db.int() {
                        "loadw"
                    } else if underlying_val == self.module.type_db.float() {
                        "loadd"
                    } else if underlying_val == self.module.type_db.u8()
                        || underlying_val == self.module.type_db.bool()
                    {
                        "loadub"
                    } else if matches!(
                        self.module.type_db.get_type(underlying_val),
                        LangType::Pointer(_) | LangType::FnPointer { .. }
                    ) {
                        "loadl"
                    } else if let LangType::Enum { .. } =
                        self.module.type_db.get_type(underlying_val)
                    {
                        "loadw"
                    } else {
                        "loadw"
                    };
                    writeln!(
                        self.output,
                        "    %r_{} ={} {} {}",
                        id.0,
                        base_ty,
                        load_inst,
                        self.op(ptr)
                    )
                    .unwrap();
                }
            }

            Instruction::Store(val_ty, ptr, val) => {
                let underlying_val = self.module.type_db.get_underlying_type(*val_ty);
                if self.is_aggregate(*val_ty) {
                    let (size, _) = type_layout(*val_ty, &self.module.type_db);
                    writeln!(
                        self.output,
                        "    call $memcpy(l {}, l {}, l {})",
                        self.op(ptr),
                        self.op(val),
                        size
                    )
                    .unwrap();
                } else {
                    let store_inst = if underlying_val == self.module.type_db.int() {
                        "storew"
                    } else if underlying_val == self.module.type_db.float() {
                        "stored"
                    } else if underlying_val == self.module.type_db.u8()
                        || underlying_val == self.module.type_db.bool()
                    {
                        "storeb"
                    } else if matches!(
                        self.module.type_db.get_type(underlying_val),
                        LangType::Pointer(_) | LangType::FnPointer { .. }
                    ) {
                        "storel"
                    } else if let LangType::Enum { .. } =
                        self.module.type_db.get_type(underlying_val)
                    {
                        "storew"
                    } else {
                        "storew"
                    };
                    writeln!(
                        self.output,
                        "    {} {}, {}",
                        store_inst,
                        self.op(val),
                        self.op(ptr)
                    )
                    .unwrap();
                }
            }

            Instruction::Index(arr, idx) => {
                let arr_ty = self.get_operand_type(arr, func);
                let canonical_arr = self.module.type_db.resolve(arr_ty);
                let elem_ty = match self.module.type_db.get_type(canonical_arr) {
                    LangType::Pointer(inner) => *inner,
                    _ => panic!("Expected pointer type for Index"),
                };
                let (elem_size, _) = type_layout(elem_ty, &self.module.type_db);

                writeln!(self.output, "    %idx_l_{} =l extsw {}", id.0, self.op(idx)).unwrap();
                writeln!(
                    self.output,
                    "    %offset_{} =l mul %idx_l_{}, {}",
                    id.0, id.0, elem_size
                )
                .unwrap();
                writeln!(
                    self.output,
                    "    %elem_ptr_{} =l add {}, %offset_{}",
                    id.0,
                    self.op(arr),
                    id.0
                )
                .unwrap();

                let canonical_elem = self.module.type_db.resolve(elem_ty);
                if self.is_aggregate(elem_ty) {
                    writeln!(self.output, "    %r_{} =l copy %elem_ptr_{}", id.0, id.0).unwrap();
                } else {
                    let load_inst = if canonical_elem == self.module.type_db.int() {
                        "loadw"
                    } else if canonical_elem == self.module.type_db.u8()
                        || canonical_elem == self.module.type_db.bool()
                    {
                        "loadub"
                    } else if matches!(
                        self.module.type_db.get_type(canonical_elem),
                        LangType::Pointer(_) | LangType::FnPointer { .. }
                    ) {
                        "loadl"
                    } else if let LangType::Enum { .. } =
                        self.module.type_db.get_type(canonical_elem)
                    {
                        "loadw"
                    } else {
                        "loadw"
                    };
                    writeln!(
                        self.output,
                        "    %r_{} ={} {} %elem_ptr_{}",
                        id.0, base_ty, load_inst, id.0
                    )
                    .unwrap();
                }
            }

            Instruction::GetMemberPtr(obj, offset) => {
                writeln!(
                    self.output,
                    "    %r_{} =l add {}, {}",
                    id.0,
                    self.op(obj),
                    offset
                )
                .unwrap();
            }

            Instruction::GetIndexPtr(arr, idx) => {
                let arr_ty = self.get_operand_type(arr, func);
                let canonical_arr = self.module.type_db.resolve(arr_ty);
                let elem_ty = match self.module.type_db.get_type(canonical_arr) {
                    LangType::Pointer(inner) => {
                        let canonical_inner = self.module.type_db.resolve(*inner);
                        match self.module.type_db.get_type(canonical_inner) {
                            LangType::Array(inner_elem, _) => *inner_elem,
                            LangType::Slice(inner_elem) => *inner_elem,
                            _ => *inner,
                        }
                    }
                    _ => panic!("Expected pointer type for GetIndexPtr"),
                };
                let (elem_size, _) = type_layout(elem_ty, &self.module.type_db);

                writeln!(self.output, "    %idx_l_{} =l extsw {}", id.0, self.op(idx)).unwrap();
                writeln!(
                    self.output,
                    "    %offset_{} =l mul %idx_l_{}, {}",
                    id.0, id.0, elem_size
                )
                .unwrap();
                writeln!(
                    self.output,
                    "    %r_{} =l add {}, %offset_{}",
                    id.0,
                    self.op(arr),
                    id.0
                )
                .unwrap();
            }
            Instruction::OOBCheck(msg, idx, len) => {
                let mut arg_strings = Vec::new();
                for arg in [msg, idx, len] {
                    let arg_ty = self.get_operand_type(arg, func);
                    let arg_abi = self.map_abi_type(arg_ty);
                    arg_strings.push(format!("{} {}", arg_abi, self.op(arg)));
                }

                let args_formatted = arg_strings.join(", ");

                writeln!(self.output, "    call $chs__oob_check({})", args_formatted).unwrap();
            }
            Instruction::Call(callee, args) => {
                let is_indirect = !matches!(callee, Operand::String(_)|Operand::Global(_));

                let (return_type, symbol_name) = if is_indirect {
                    let callee_ty = self.get_operand_type(callee, func);
                    let canonical = self.module.type_db.resolve(callee_ty);
                    if let LangType::FnPointer { return_type, .. } =
                        self.module.type_db.get_type(canonical).clone()
                    {
                        (return_type, self.op(callee))
                    } else {
                        panic!("Expected function pointer type for indirect QBE call");
                    }
                } else {
                    let callee_name = match callee {
                        Operand::String(name) => name.clone(),
                        Operand::Global(name) => name.clone(),
                        _ => unreachable!(),
                    };
                    let callee_func = self
                        .module
                        .functions
                        .get(callee_name.as_ref())
                        .expect("Callee function signature not found");
                    let mangle = if callee_func.is_default() { "chs_" } else { "" };
                    (
                        callee_func.signature().return_type,
                        format!("${mangle}{}", callee_func.symbol_name()),
                    )
                };

                let canonical_ret = self.module.type_db.resolve(return_type);
                let mut arg_strings = Vec::new();

                if is_indirect {
                    for arg in args.iter() {
                        let arg_ty = self.get_operand_type(arg, func);
                        let arg_abi = self.map_abi_type(arg_ty);
                        arg_strings.push(format!("{} {}", arg_abi, self.op(arg)));
                    }
                } else {
                    let callee_name = match callee {
                        Operand::String(name) => name.clone(),
                        Operand::Global(name) => name.clone(),
                        _ => unreachable!(),
                    };
                    let callee_func = self
                        .module
                        .functions
                        .get(callee_name.as_ref())
                        .expect("Callee function signature not found");
                    let params_len = callee_func.signature().params.len();
                    for (i, arg) in args.iter().enumerate() {
                        if callee_func.signature().has_va_args && i >= params_len {
                            if i == params_len {
                                arg_strings.push("...".to_string());
                            }
                            let arg_ty = self.get_operand_type(arg, func);
                            let arg_abi = self.map_abi_type(arg_ty);
                            arg_strings.push(format!("{} {}", arg_abi, self.op(arg)));
                        } else {
                            let param_ty = callee_func.signature().params[i];
                            let param_abi = self.map_abi_type(param_ty);
                            arg_strings.push(format!("{} {}", param_abi, self.op(arg)));
                        }
                    }
                }

                let args_formatted = arg_strings.join(", ");

                if canonical_ret == self.module.type_db.void() {
                    writeln!(self.output, "    call {}({})", symbol_name, args_formatted).unwrap();
                } else {
                    let ret_abi = self.map_abi_type(return_type);
                    writeln!(
                        self.output,
                        "    %r_{} ={} call {}({})",
                        id.0, ret_abi, symbol_name, args_formatted
                    )
                    .unwrap();
                }
            }

            Instruction::Br(target) => {
                writeln!(self.output, "    jmp @block_{}", target.0).unwrap();
            }
            Instruction::CondBr(cond, true_block, false_block) => {
                writeln!(
                    self.output,
                    "    jnz {}, @block_{}, @block_{}",
                    self.op(cond),
                    true_block.0,
                    false_block.0
                )
                .unwrap();
            }
            Instruction::Return(val_opt) => {
                if let Some(val) = val_opt {
                    writeln!(self.output, "    ret {}", self.op(val)).unwrap();
                } else {
                    writeln!(self.output, "    ret").unwrap();
                }
            }
        }
    }

    fn get_operand_type(&self, op: &Operand, func: &Function) -> t::TypeID {
        match op {
            Operand::Null => self.module.type_db.void(),
            Operand::Reg(reg_id) => {
                let Function::Default { instructions, .. } = func else {
                    return self.module.type_db.void();
                };
                instructions[reg_id.0 as usize].ty
            }
            Operand::Int(_) => self.module.type_db.int(),
            Operand::Bool(_) => self.module.type_db.bool(),
            Operand::Float(_) => self.module.type_db.float(),
            Operand::String(_) => self.module.type_db.string(),
            Operand::Param(i) => func.signature().params[*i as usize],
            Operand::Global(_) | Operand::ThreadLocalGlobal(_) => {
                let type_info_id = self.module.type_db.u8();
                self.module.type_db.pointer_type(type_info_id).unwrap()
            }
        }
    }

    fn mangle_type(&self, ty: t::TypeID) -> String {
        let canonical = self.module.type_db.resolve(ty);
        if canonical == self.module.type_db.void() {
            "void".to_string()
        } else if canonical == self.module.type_db.u8() {
            "u8".to_string()
        } else if canonical == self.module.type_db.int() {
            "int".to_string()
        } else if canonical == self.module.type_db.bool() {
            "bool".to_string()
        } else if canonical == self.module.type_db.float() {
            "float".to_string()
        } else if canonical == self.module.type_db.string() {
            "string".to_string()
        } else {
            match self.module.type_db.get_type(canonical) {
                LangType::Pointer(inner) => {
                    format!("ptr_{}", self.mangle_type(*inner))
                }
                LangType::Array(inner, size) => {
                    format!("array_{}_{}", self.mangle_type(*inner), size)
                }
                LangType::Slice(inner) => {
                    format!("slice_{}", self.mangle_type(*inner))
                }
                LangType::Tuple(elements) => {
                    let mut parts = vec!["tuple".to_string()];
                    for &e in elements {
                        parts.push(self.mangle_type(e));
                    }
                    parts.join("_")
                }
                LangType::Struct { name, .. } => {
                    format!("struct_{}", name)
                }
                LangType::Enum { name, .. } => {
                    format!("enum_{}", name)
                }
                _ => panic!("Cannot mangle type"),
            }
        }
    }

    fn is_aggregate(&self, ty: t::TypeID) -> bool {
        let canonical = self.module.type_db.resolve(ty);
        matches!(
            self.module.type_db.get_type(canonical),
            LangType::Struct { .. }
                | LangType::Enum { .. }
                | LangType::Array(..)
                | LangType::Slice(..)
                | LangType::Tuple(..)
        )
    }

    fn op(&self, op: &Operand) -> String {
        match op {
            Operand::Null => "0".to_string(),
            Operand::Reg(reg_id) => format!("%r_{}", reg_id.0),
            Operand::Int(v) => format!("{}", v),
            Operand::Float(v) => {
                let mut s = format!("{}", v);
                if !s.contains('.') && !s.contains('e') && !s.contains('E') {
                    s.push_str(".0");
                }
                format!("d_{}", s)
            }
            Operand::Bool(v) => {
                if *v {
                    "1".to_string()
                } else {
                    "0".to_string()
                }
            }
            Operand::String(s) => {
                let id = self
                    .string_map
                    .get(s)
                    .expect("String literal not found in map");
                format!("$str_{}", id)
            }
            Operand::Param(i) => format!("%param_{}", i),
            Operand::Global(name) => {
                if let Some(func) = self.module.functions.get(name.as_ref()) {
                    if func.is_default() {
                        format!("$chs_{}", name)
                    } else {
                        format!("${}", name)
                    }
                } else {
                    format!("${}", name)
                }
            }
            Operand::ThreadLocalGlobal(name) => {
                format!("thread ${}", name)
            }
        }
    }

    fn emit_reflection_data(&mut self) {
        let mut visited = std::collections::HashSet::new();
        let mut queue: Vec<t::TypeID> = self.module.type_db.queried_types.iter().copied().collect();
        let mut reflection_types = Vec::new();

        while let Some(ty) = queue.pop() {
            let canonical = self.module.type_db.resolve(ty);
            if visited.insert(canonical) {
                reflection_types.push(canonical);
                match self.module.type_db.get_type(canonical) {
                    t::Type::Pointer(inner) => {
                        queue.push(*inner);
                    }
                    t::Type::Array(inner, _) => {
                        queue.push(*inner);
                    }
                    t::Type::Slice(inner) => {
                        queue.push(*inner);
                    }
                    t::Type::Struct {
                        fields: Some(fields),
                        ..
                    } => {
                        for field in fields {
                            queue.push(field.ty);
                        }
                    }
                    t::Type::FnPointer {
                        params,
                        return_type,
                    } => {
                        queue.push(*return_type);
                        for param in params {
                            queue.push(*param);
                        }
                    }
                    _ => {}
                }
            }
        }

        if reflection_types.is_empty() {
            return;
        }

        writeln!(self.output, "# --- Reflection Metadata ---").unwrap();

        // Helper to format string slice in QBE data block
        let get_str_data = |s: &str| -> String {
            let id = self.string_map.get(s).expect("Reflection string not found");
            format!("l $str_data_{}, w {}", id, s.len())
        };

        // Emit arrays of fields and variants
        for &ty in &reflection_types {
            let canonical = self.module.type_db.resolve(ty);
            match self.module.type_db.get_type(canonical) {
                t::Type::Struct {
                    fields: Some(fields),
                    ..
                } => {
                    if !fields.is_empty() {
                        let layout = StructLayout::compute(canonical, &self.module.type_db);
                        writeln!(self.output, "data $chs_type_fields_{} = align 8 {{", ty.0)
                            .unwrap();
                        for (idx, field) in fields.iter().enumerate() {
                            let field_offset = layout.fields[idx].offset;
                            let field_canon = self.module.type_db.resolve(field.ty);
                            writeln!(self.output, "    {}, z 4,", get_str_data(&field.name))
                                .unwrap();
                            writeln!(self.output, "    w {}, z 4,", field_offset).unwrap();
                            writeln!(self.output, "    l $chs_type_info_{},", field_canon.0)
                                .unwrap();
                        }
                        writeln!(self.output, "}}").unwrap();
                    }
                }
                t::Type::Enum { variants, .. } => {
                    if !variants.is_empty() {
                        writeln!(self.output, "data $chs_type_variants_{} = align 8 {{", ty.0)
                            .unwrap();
                        for variant in variants {
                            writeln!(self.output, "    {}, z 4,", get_str_data(&variant.name))
                                .unwrap();
                            writeln!(self.output, "    w {}, z 4,", variant.default_value).unwrap();
                        }
                        writeln!(self.output, "}}").unwrap();
                    }
                }
                _ => {}
            }
        }

        // Emit TypeInfo structures
        for &ty in &reflection_types {
            let canonical = self.module.type_db.resolve(ty);
            fn get_type_kind(db: &t::TypeDatabase, ty: t::TypeID) -> u32 {
                let canonical = db.resolve(ty);
                match db.types.get(&canonical).unwrap() {
                    t::Type::Primitive(_) => 1,
                    t::Type::Pointer(_) => 2,
                    t::Type::Array(_, _) => 3,
                    t::Type::Slice(_) => 4,
                    t::Type::Struct { .. } => 5,
                    t::Type::Enum { .. } => 6,
                    t::Type::String(_) => 7,
                    t::Type::Any(_) => 8,
                    t::Type::FnPointer { .. } => 9,
                    t::Type::Distinct { base, .. } => get_type_kind(db, *base),
                    _ => 0,
                }
            }
            let kind = get_type_kind(&self.module.type_db, ty);
            let name = self.module.type_db.type_to_string(canonical);
            let (size, align) = type_layout(canonical, &self.module.type_db);

            let elem_ptr = match self.module.type_db.get_type(canonical) {
                t::Type::Pointer(inner) | t::Type::Array(inner, _) | t::Type::Slice(inner) => {
                    format!("l $chs_type_info_{}", self.module.type_db.resolve(*inner).0)
                }
                t::Type::FnPointer { return_type, .. } => {
                    format!(
                        "l $chs_type_info_{}",
                        self.module.type_db.resolve(*return_type).0
                    )
                }
                _ => "l 0".to_string(),
            };

            let arr_len = match self.module.type_db.get_type(canonical) {
                t::Type::Array(_, size) => *size as u64,
                _ => 0,
            };

            let fields_slice = match self.module.type_db.get_type(canonical) {
                t::Type::Struct {
                    fields: Some(fields),
                    ..
                } if !fields.is_empty() => {
                    format!("l $chs_type_fields_{}, w {}", ty.0, fields.len())
                }
                _ => "l 0, w 0".to_string(),
            };

            let variants_slice = match self.module.type_db.get_type(canonical) {
                t::Type::Enum { variants, .. } if !variants.is_empty() => {
                    format!("l $chs_type_variants_{}, w {}", ty.0, variants.len())
                }
                _ => "l 0, w 0".to_string(),
            };

            writeln!(self.output, "data $chs_type_info_{} = align 8 {{", ty.0).unwrap();
            writeln!(self.output, "    w {}, z 4,", kind).unwrap();
            writeln!(self.output, "    {}, z 4,", get_str_data(&name)).unwrap();
            writeln!(self.output, "    w {},", size).unwrap();
            writeln!(self.output, "    w {},", align).unwrap();
            writeln!(self.output, "    {},", elem_ptr).unwrap();
            writeln!(self.output, "    w {}, z 4,", arr_len).unwrap();
            writeln!(self.output, "    {}, z 4,", fields_slice).unwrap();
            writeln!(self.output, "    {}, z 4", variants_slice).unwrap();
            writeln!(self.output, "}}").unwrap();
        }
        writeln!(self.output).unwrap();
    }

    fn emit_main_wrapper(&mut self) {
        writeln!(self.output, "export function w $main() {{").unwrap();
        writeln!(self.output, "@start").unwrap();
        writeln!(self.output, "    call $chs_main()").unwrap();
        writeln!(self.output, "    ret 0").unwrap();
        writeln!(self.output, "}}").unwrap();
    }
}
