use std::collections::HashMap;

/// A lightweight, copyable handle to a registered type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeID(pub u32);

/// A field within a struct type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructField {
    pub name: String,
    pub ty: TypeID,
}

/// A variant within an enum type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnumVariant {
    pub name: String,
    pub default_value: u64,
    pub payload: Option<TypeID>,
}

/// The internal representation of all language types.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Primitive(TypeID),
    Pointer(TypeID),
    Array(TypeID, usize),
    // NOTE: Maybe Slice should be a struct type
    // ```
    // // experimental syntax
    // struct Slice($T) {
    //      data: *$T,
    //      len: int
    // }
    // ```
    Slice(TypeID),
    Struct {
        name: String,
        fields: Option<Vec<StructField>>, // None means the struct is a forward-declared placeholder
    },
    Enum {
        name: String,
        repr: TypeID,
        variants: Vec<EnumVariant>, // None means the enum is a forward-declared placeholder
    },
    TypeVar(u32),
    Tuple(Vec<TypeID>),
    Any(TypeID),
    String(TypeID),
    Distinct {
        name: String,
        base: TypeID,
    },
    FnPointer {
        params: Vec<TypeID>,
        return_type: TypeID,
    },
}

impl Type {
    /// Returns `true` if the type is [`Primitive`].
    ///
    /// [`Primitive`]: Type::Primitive
    #[must_use]
    pub fn is_primitive(&self) -> bool {
        matches!(self, Self::Primitive(..))
    }

    /// Returns `true` if the type is [`Pointer`].
    ///
    /// [`Pointer`]: Type::Pointer
    #[must_use]
    pub fn is_pointer(&self) -> bool {
        matches!(self, Self::Pointer(..))
    }
}

/// The central type registry and unification arena.
#[derive(Debug, Clone)]
pub struct TypeDatabase {
    pub types: HashMap<TypeID, Type>,
    pub substitutions: HashMap<u32, TypeID>,
    pub names: HashMap<String, TypeID>,
    pub generic_instantiations: HashMap<TypeID, (String, Vec<TypeID>)>,
    pub queried_types: std::collections::HashSet<TypeID>,
    pub aliases: HashMap<TypeID, TypeID>,
    next_var_id: u32,
    void_id: TypeID,
    int_id: TypeID,
    bool_id: TypeID,
    u8_id: TypeID,
    float_id: TypeID,
    string_id: TypeID,
    // type_kind_id: TypeID,
    // type_info_id: TypeID,
    // any_id: TypeID,
    noreturn_id: TypeID,
}

impl Default for TypeDatabase {
    fn default() -> Self {
        let mut db = Self {
            types: HashMap::new(),
            substitutions: HashMap::new(),
            names: HashMap::new(),
            generic_instantiations: HashMap::new(),
            next_var_id: 0,
            void_id: TypeID(0),
            int_id: TypeID(0),
            bool_id: TypeID(0),
            u8_id: TypeID(0),
            float_id: TypeID(0),
            string_id: TypeID(0),
            // type_info_id: TypeID(0),
            // type_kind_id: TypeID(0),
            // any_id: TypeID(0),
            noreturn_id: TypeID(0),
            queried_types: std::collections::HashSet::new(),
            aliases: HashMap::new(),
        };

        db.void_id = db.insert_named_type("void".to_string(), Type::Primitive(TypeID(0)));
        db.int_id = db.insert_named_type("int".to_string(), Type::Primitive(TypeID(1)));
        db.bool_id = db.insert_named_type("bool".to_string(), Type::Primitive(TypeID(2)));
        db.u8_id = db.insert_named_type("u8".to_string(), Type::Primitive(TypeID(3)));
        db.float_id = db.insert_named_type("float".to_string(), Type::Primitive(TypeID(4)));
        db.noreturn_id = db.insert_named_type("noreturn".to_string(), Type::Primitive(TypeID(5)));

        // string
        // NOTE: Maybe string should be a #distinct []char
        //      with this syntax `type string #distinct []char`
        {
            let fields = Some(vec![
                StructField {
                    name: "data".to_string(),
                    ty: db.pointer(db.u8_id),
                },
                StructField {
                    name: "len".to_string(),
                    ty: db.int_id,
                },
            ]);
            let name = "string".to_string();
            let string_inner_id = db.insert_type(Type::Struct {
                name: name.clone(),
                fields,
            });
            db.string_id = db.insert_named_type(name.clone(), Type::String(string_inner_id));
        }

        // // 1. Forward-declare TypeInfo
        // let type_info_placeholder = db.insert_named_type(
        //     "TypeInfo".to_string(),
        //     Type::Struct {
        //         name: "TypeInfo".to_string(),
        //         fields: None,
        //     },
        // );
        // db.type_info_id = type_info_placeholder;

        // let variants = vec![
        //     EnumVariant {
        //         name: "Primitive".to_string(),
        //         default_value: 1,
        //         payload: None,
        //     },
        //     EnumVariant {
        //         name: "Pointer".to_string(),
        //         default_value: 2,
        //         payload: None,
        //     },
        //     EnumVariant {
        //         name: "Array".to_string(),
        //         default_value: 3,
        //         payload: None,
        //     },
        //     EnumVariant {
        //         name: "Slice".to_string(),
        //         default_value: 4,
        //         payload: None,
        //     },
        //     EnumVariant {
        //         name: "Struct".to_string(),
        //         default_value: 5,
        //         payload: None,
        //     },
        //     EnumVariant {
        //         name: "Enum".to_string(),
        //         default_value: 6,
        //         payload: None,
        //     },
        //     EnumVariant {
        //         name: "String".to_string(),
        //         default_value: 7,
        //         payload: None,
        //     },
        //     EnumVariant {
        //         name: "Any".to_string(),
        //         default_value: 8,
        //         payload: None,
        //     },
        // ];
        // let type_kind = db.insert_named_type(
        //     "TypeKind".to_string(),
        //     Type::Enum {
        //         name: "TypeKind".to_string(),
        //         repr: db.int_id,
        //         variants,
        //     },
        // );
        // db.type_kind_id = type_kind;

        // // 2. Define TypeEnumVariant
        // let type_enum_variant = db.insert_named_type(
        //     "TypeEnumVariant".to_string(),
        //     Type::Struct {
        //         name: "TypeEnumVariant".to_string(),
        //         fields: Some(vec![
        //             StructField {
        //                 name: "name".to_string(),
        //                 ty: db.string_id,
        //             },
        //             StructField {
        //                 name: "value".to_string(),
        //                 ty: db.int_id,
        //             },
        //         ]),
        //     },
        // );

        // // 3. Define TypeField
        // let type_info_ptr = db.pointer(type_info_placeholder);
        // let type_field = db.insert_named_type(
        //     "TypeField".to_string(),
        //     Type::Struct {
        //         name: "TypeField".to_string(),
        //         fields: Some(vec![
        //             StructField {
        //                 name: "name".to_string(),
        //                 ty: db.string_id,
        //             },
        //             StructField {
        //                 name: "offset".to_string(),
        //                 ty: db.int_id,
        //             },
        //             StructField {
        //                 name: "type_info".to_string(),
        //                 ty: type_info_ptr,
        //             },
        //         ]),
        //     },
        // );

        // // 4. Define TypeInfo fields
        // let type_field_slice = db.slice(type_field);
        // let type_enum_variant_slice = db.slice(type_enum_variant);

        // let type_info_fields = Some(vec![
        //     StructField {
        //         name: "kind".to_string(),
        //         ty: db.type_kind_id,
        //     },
        //     StructField {
        //         name: "name".to_string(),
        //         ty: db.string_id,
        //     },
        //     StructField {
        //         name: "size".to_string(),
        //         ty: db.int_id,
        //     },
        //     StructField {
        //         name: "align".to_string(),
        //         ty: db.int_id,
        //     },
        //     StructField {
        //         name: "element_type".to_string(),
        //         ty: type_info_ptr,
        //     },
        //     StructField {
        //         name: "array_len".to_string(),
        //         ty: db.int_id,
        //     },
        //     StructField {
        //         name: "fields".to_string(),
        //         ty: type_field_slice,
        //     },
        //     StructField {
        //         name: "variants".to_string(),
        //         ty: type_enum_variant_slice,
        //     },
        // ]);

        // // 5. Update TypeInfo definition in-place!
        // if let Some(t) = db.types.get_mut(&type_info_placeholder) {
        //     *t = Type::Struct {
        //         name: "TypeInfo".to_string(),
        //         fields: type_info_fields,
        //     };
        // }

        // // Any
        // let fields = Some(vec![
        //     StructField {
        //         name: "value".to_string(),
        //         ty: db.pointer(db.void_id),
        //     },
        //     StructField {
        //         name: "type_info".to_string(),
        //         ty: db.pointer(db.type_info_id),
        //     },
        // ]);
        // let name = "Any".to_string();
        // let any_inner_id = db.insert_type(Type::Struct {
        //     name: name.clone(),
        //     fields,
        // });
        // db.any_id = db.insert_named_type(name, Type::Any(any_inner_id));

        db
    }
}

impl TypeDatabase {
    /// Creates a new TypeDatabase pre-populated with primitive types.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn void(&self) -> TypeID {
        self.void_id
    }
    pub fn int(&self) -> TypeID {
        self.int_id
    }
    pub fn noreturn(&self) -> TypeID {
        self.noreturn_id
    }
    pub fn bool(&self) -> TypeID {
        self.bool_id
    }
    pub fn u8(&self) -> TypeID {
        self.u8_id
    }
    pub fn float(&self) -> TypeID {
        self.float_id
    }
    pub fn string(&self) -> TypeID {
        self.string_id
    }
    pub fn type_kind(&self) -> TypeID {
        self.lookup_by_name("TypeKind").expect("runtime")
    }
    pub fn type_info(&self) -> TypeID {
        self.lookup_by_name("TypeInfo").expect("runtime")
    }
    pub fn any(&self) -> TypeID {
        self.lookup_by_name("Any").expect("runtime")
    }
    pub fn pointer_type(&self, inner: TypeID) -> Option<TypeID> {
        for (&id, ty) in &self.types {
            if let Type::Pointer(existing_inner) = ty
                && *existing_inner == inner
            {
                return Some(id);
            }
        }
        None
    }

    pub fn is_primitive_castable(&self, ty: TypeID) -> bool {
        if ty == self.u8() || ty == self.bool() || ty == self.int() || ty == self.float() {
            return true;
        }
        return false;
    }

    /// Inserts a new type into the registry and returns its unique TypeID.
    pub fn insert_type(&mut self, ty: Type) -> TypeID {
        let id = TypeID(self.types.len() as u32);
        self.types.insert(id, ty);
        id
    }

    /// Inserts a named type (struct or enum) into the registry, registering its name.
    pub fn insert_named_type(&mut self, name: String, ty: Type) -> TypeID {
        let id = self.insert_type(ty);
        self.names.insert(name, id);
        id
    }

    /// Looks up a registered type by name.
    pub fn lookup_by_name(&self, name: &str) -> Option<TypeID> {
        self.names.get(name).copied()
    }

    /// Registers a type ID as a generic instantiation of a base template with argument types.
    pub fn register_generic_instantiation(
        &mut self,
        id: TypeID,
        base_name: String,
        args: Vec<TypeID>,
    ) {
        let canonical = self.resolve(id);
        self.generic_instantiations
            .insert(canonical, (base_name, args));
    }

    /// Returns a TypeID for a Pointer to the inner type, interning it.
    pub fn pointer(&mut self, inner: TypeID) -> TypeID {
        self.insert_type(Type::Pointer(inner))
    }

    /// Returns a TypeID for an Array of the element type and size, interning it.
    pub fn array(&mut self, element: TypeID, size: usize) -> TypeID {
        for (&id, ty) in &self.types {
            if let Type::Array(existing_element, existing_size) = ty
                && *existing_element == element
                && *existing_size == size
            {
                return id;
            }
        }
        self.insert_type(Type::Array(element, size))
    }

    /// Returns a TypeID for a Slice of the element type, interning it.
    pub fn slice(&mut self, element: TypeID) -> TypeID {
        for (&id, ty) in &self.types {
            if let Type::Slice(existing_element) = ty
                && *existing_element == element
            {
                return id;
            }
        }
        self.insert_type(Type::Slice(element))
    }

    /// Returns a TypeID for a Tuple of the given element types, interning it.
    pub fn tuple(&mut self, elements: Vec<TypeID>) -> TypeID {
        for (&id, ty) in &self.types {
            if let Type::Tuple(existing_elements) = ty
                && *existing_elements == elements
            {
                return id;
            }
        }
        self.insert_type(Type::Tuple(elements))
    }

    pub fn fn_pointer(&mut self, params: Vec<TypeID>, return_type: TypeID) -> TypeID {
        let ty = Type::FnPointer {
            params,
            return_type,
        };
        for (id, existing_ty) in &self.types {
            if existing_ty == &ty {
                return *id;
            }
        }
        self.insert_type(ty)
    }

    /// Retrieves a reference to the Type associated with a TypeID.
    pub fn get_type(&self, id: TypeID) -> &Type {
        match self.types.get(&id).unwrap() {
            Type::Any(any_id) => self.get_type(*any_id),
            Type::String(string_id) => self.get_type(*string_id),
            ty => ty,
        }
    }

    pub fn get_inner_type_id(&self, id: TypeID) -> TypeID {
        match self.types.get(&id).unwrap() {
            Type::Any(any_id) => *any_id,
            Type::String(string_id) => *string_id,
            Type::Slice(inner_id) => *inner_id,
            Type::Array(inner_id, ..) => *inner_id,
            _ => id,
        }
    }

    pub fn get_underlying_type(&self, id: TypeID) -> TypeID {
        let canonical = self.resolve(id);
        match self.types.get(&canonical).unwrap() {
            Type::Distinct { base, .. } => self.get_underlying_type(*base),
            Type::Any(any_id) => self.get_underlying_type(*any_id),
            Type::String(string_id) => self.get_underlying_type(*string_id),
            _ => canonical,
        }
    }

    /// Retrieves a mutable reference to the Type associated with a TypeID.
    pub fn get_type_mut(&mut self, id: TypeID) -> &mut Type {
        match self.types.get(&id).unwrap() {
            Type::Any(any_id) => self.get_type_mut(*any_id),
            Type::String(string_id) => self.get_type_mut(*string_id),
            _ => self.types.get_mut(&id).unwrap(),
        }
    }

    /// Creates and registers a new unique inference type variable.
    pub fn new_inference_var(&mut self) -> TypeID {
        let var_id = self.next_var_id;
        self.next_var_id += 1;
        self.insert_type(Type::TypeVar(var_id))
    }

    /// Resolves a TypeID to its canonical representative, following substitution chains.
    pub fn resolve(&self, mut id: TypeID) -> TypeID {
        loop {
            let mut updated = false;
            while let Type::TypeVar(var_id) = self.get_type(id) {
                if let Some(&resolved) = self.substitutions.get(var_id) {
                    if resolved == id {
                        break;
                    }
                    id = resolved;
                    updated = true;
                } else {
                    break;
                }
            }
            if let Some(&aliased) = self.aliases.get(&id)
                && aliased != id
            {
                id = aliased;
                updated = true;
            }
            if !updated {
                break;
            }
        }
        id
    }

    /// Unifies two types, updating the internal substitutions if type variables are involved.
    pub fn unify(&mut self, id1: TypeID, id2: TypeID) -> Result<(), String> {
        let canonical1 = self.resolve(id1);
        let canonical2 = self.resolve(id2);

        if canonical1 == canonical2 {
            return Ok(());
        }

        // Check if both are generic instantiations of the same base type
        if let (Some((base1, args1)), Some((base2, args2))) = (
            self.generic_instantiations.get(&canonical1).cloned(),
            self.generic_instantiations.get(&canonical2).cloned(),
        ) {
            if base1 == base2 && args1.len() == args2.len() {
                for (&a1, &a2) in args1.iter().zip(args2.iter()) {
                    self.unify(a1, a2)?;
                }
                self.aliases.insert(canonical2, canonical1);
                return Ok(());
            }
        }

        let void_id = self.void_id;

        match (
            self.get_type(canonical1).clone(),
            self.get_type(canonical2).clone(),
        ) {
            (Type::TypeVar(v1), _) => {
                self.substitutions.insert(v1, canonical2);
                Ok(())
            }
            (_, Type::TypeVar(v2)) => {
                self.substitutions.insert(v2, canonical1);
                Ok(())
            }
            (Type::Primitive(p1), Type::Primitive(p2)) => {
                if p1 == p2 {
                    Ok(())
                } else {
                    Err(format!("Cannot unify primitive {:?} with {:?}", p1, p2))
                }
            }
            (Type::Pointer(p1), Type::Pointer(..)) if p1 == void_id => Ok(()),
            (Type::Pointer(..), Type::Pointer(p2)) if p2 == void_id => Ok(()),
            (Type::Pointer(inner1), Type::Pointer(inner2)) => self.unify(inner1, inner2),
            (Type::Array(inner1, size1), Type::Array(inner2, size2)) => {
                if size1 == size2 {
                    self.unify(inner1, inner2)
                } else {
                    Err(format!(
                        "Cannot unify array of size {} with array of size {}",
                        size1, size2
                    ))
                }
            }
            (Type::Slice(inner1), Type::Slice(inner2)) => self.unify(inner1, inner2),
            (Type::Tuple(elems1), Type::Tuple(elems2)) => {
                if elems1.len() != elems2.len() {
                    return Err(format!(
                        "Cannot unify tuples of different lengths ({} and {})",
                        elems1.len(),
                        elems2.len()
                    ));
                }
                for (&e1, &e2) in elems1.iter().zip(elems2.iter()) {
                    self.unify(e1, e2)?;
                }
                Ok(())
            }
            (
                Type::FnPointer {
                    params: params1,
                    return_type: ret1,
                },
                Type::FnPointer {
                    params: params2,
                    return_type: ret2,
                },
            ) => {
                if params1.len() != params2.len() {
                    return Err(format!(
                        "Cannot unify function pointer types with different parameter counts ({} and {})",
                        params1.len(),
                        params2.len()
                    ));
                }
                for (&p1, &p2) in params1.iter().zip(params2.iter()) {
                    self.unify(p1, p2)?;
                }
                self.unify(ret1, ret2)
            }
            (t1, t2) => Err(format!("Cannot unify {:?} with {:?}", t1, t2)),
        }
    }

    /// Populates/updates fields for a placeholder Struct.
    pub fn set_struct_fields(
        &mut self,
        id: TypeID,
        fields: Vec<StructField>,
    ) -> Result<(), String> {
        let canonical = self.resolve(id);
        match self.get_type_mut(canonical) {
            Type::Struct { fields: f, .. } => {
                *f = Some(fields);
                Ok(())
            }
            _ => Err("Type is not a struct".to_string()),
        }
    }

    /// Populates/updates variants for a placeholder Enum.
    pub fn set_enum_variants(
        &mut self,
        id: TypeID,
        variants: Vec<EnumVariant>,
    ) -> Result<(), String> {
        let canonical = self.resolve(id);
        match self.get_type_mut(canonical) {
            Type::Enum { variants: v, .. } => {
                *v = variants;
                Ok(())
            }
            _ => Err("Type is not an enum".to_string()),
        }
    }

    /// Returns a user-friendly string representation of a type.
    pub fn type_to_string(&self, id: TypeID) -> String {
        let canonical = self.resolve(id);
        match self.get_type(canonical) {
            Type::Primitive(p) if *p == self.void_id => "void".to_string(),
            Type::Primitive(p) if *p == self.int_id => "int".to_string(),
            Type::Primitive(p) if *p == self.bool_id => "bool".to_string(),
            Type::Primitive(p) if *p == self.u8_id => "u8".to_string(),
            Type::Primitive(p) if *p == self.float_id => "float".to_string(),
            Type::Primitive(p) if *p == self.noreturn_id => "noreturn".to_string(),
            Type::Primitive(_) => "unknown".to_string(),
            Type::Pointer(inner) => {
                format!("*{}", self.type_to_string(*inner))
            }
            Type::Array(inner, size) => format!("[{}]{}", size, self.type_to_string(*inner)),
            Type::Slice(inner) => format!("[]{}", self.type_to_string(*inner)),
            Type::Struct { name, .. } => name.clone(),
            Type::Enum { name, .. } => name.clone(),
            Type::TypeVar(v) => format!("_t{}", v),
            Type::Tuple(elems) => {
                let elems_str: Vec<String> =
                    elems.iter().map(|&e| self.type_to_string(e)).collect();
                format!("({})", elems_str.join(", "))
            }
            Type::Any(_) => "Any".to_string(),
            Type::String(_) => "string".to_string(),
            Type::Distinct { name, .. } => name.clone(),
            Type::FnPointer {
                params,
                return_type,
            } => {
                let params_str: Vec<String> =
                    params.iter().map(|&p| self.type_to_string(p)).collect();
                format!(
                    "fn({}) -> {}",
                    params_str.join(", "),
                    self.type_to_string(*return_type)
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_types() {
        let db = TypeDatabase::new();
        assert_eq!(db.get_type(db.int()), &Type::Primitive(db.int_id));
    }

    #[test]
    fn test_recursive_struct() {
        let mut db = TypeDatabase::new();

        // Forward declare struct Node
        let node_id = db.insert_named_type(
            "Node".to_string(),
            Type::Struct {
                name: "Node".to_string(),
                fields: None,
            },
        );

        let int_id = db.int();
        let next_ptr_id = db.pointer(node_id);

        let fields = vec![
            StructField {
                name: "value".to_string(),
                ty: int_id,
            },
            StructField {
                name: "next".to_string(),
                ty: next_ptr_id,
            },
        ];

        assert!(db.set_struct_fields(node_id, fields.clone()).is_ok());

        // Retrieve Node type and verify fields are correct
        if let Type::Struct {
            name,
            fields: Some(f),
        } = db.get_type(node_id)
        {
            assert_eq!(name, "Node");
            assert_eq!(f, &fields);
        } else {
            panic!("Expected struct type Node with fields populated");
        }
    }

    #[test]
    fn test_unification_and_resolution() {
        let mut db = TypeDatabase::new();

        let var1 = db.new_inference_var();
        let var2 = db.new_inference_var();
        let int_id = db.int();

        // Unify two inference variables
        assert!(db.unify(var1, var2).is_ok());
        // Unify one of them with int
        assert!(db.unify(var2, int_id).is_ok());

        // Both variables should now resolve to int_id
        assert_eq!(db.resolve(var1), int_id);
        assert_eq!(db.resolve(var2), int_id);
    }

    #[test]
    fn test_unification_mismatch() {
        let mut db = TypeDatabase::new();
        let int_id = db.int();
        let bool_id = db.bool();

        assert!(db.unify(int_id, bool_id).is_err());
    }

    #[test]
    fn test_enum_variants() {
        let mut db = TypeDatabase::new();
        let int_id = db.int();

        // Forward declare Enum Option
        let option_id = db.insert_named_type(
            "Option".to_string(),
            Type::Enum {
                name: "Option".to_string(),
                repr: int_id,
                variants: Vec::new(),
            },
        );

        let variants = vec![
            EnumVariant {
                name: "None".to_string(),
                default_value: 0,
                payload: None,
            },
            EnumVariant {
                name: "Some".to_string(),
                default_value: 1,
                payload: None,
            },
        ];

        assert!(db.set_enum_variants(option_id, variants.clone()).is_ok());

        if let Type::Enum {
            name,
            repr,
            variants: v,
        } = db.get_type(option_id)
        {
            assert_eq!(name, "Option");
            assert_eq!(*repr, int_id);
            assert_eq!(v, &variants);
        } else {
            panic!("Expected enum type Option with variants populated");
        }
    }

    #[test]
    fn test_fn_pointer_unification() {
        let mut db = TypeDatabase::new();
        let int_id = db.int();
        let bool_id = db.bool();

        let fn1 = db.fn_pointer(vec![int_id], bool_id);
        let fn2 = db.fn_pointer(vec![int_id], bool_id);

        // They should be identical IDs (interned)
        assert_eq!(fn1, fn2);

        // Unifying identical function pointer types should succeed
        assert!(db.unify(fn1, fn2).is_ok());

        // Unifying with a different parameter count or type should fail
        let fn3 = db.fn_pointer(vec![int_id, bool_id], bool_id);
        assert!(db.unify(fn1, fn3).is_err());

        let fn4 = db.fn_pointer(vec![bool_id], bool_id);
        assert!(db.unify(fn1, fn4).is_err());

        // Type variable unification
        let var1 = db.new_inference_var();
        let fn_var = db.fn_pointer(vec![var1], bool_id);
        assert!(db.unify(fn1, fn_var).is_ok());
        assert_eq!(db.resolve(var1), int_id);
    }
}
