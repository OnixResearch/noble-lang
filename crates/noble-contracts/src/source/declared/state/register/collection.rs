pub(super) struct Collected {
    pub schemas: alloc::vec::Vec<crate::source::declared::Schema>,
    pub definitions: alloc::vec::Vec<(
        alloc::string::String,
        alloc::vec::Vec<u8>,
        crate::source::Tree,
    )>,
    pub requirements: alloc::vec::Vec<(
        alloc::string::String,
        alloc::string::String,
        alloc::string::String,
    )>,
    pub exports: alloc::vec::Vec<alloc::string::String>,
    names: alloc::vec::Vec<alloc::string::String>,
}

struct OpaqueDeclaration {
    name: alloc::string::String,
    base: alloc::string::String,
    public: bool,
}

impl Collected {
    pub(super) fn collect(
        members: alloc::vec::Vec<crate::source::declared::parsing::Member>,
        limits: crate::Limits,
    ) -> Result<Self, crate::source::Error> {
        let count = members.len();
        let mut collected = Ok(Self {
            schemas: alloc::vec::Vec::with_capacity(count),
            definitions: alloc::vec::Vec::with_capacity(count),
            requirements: alloc::vec::Vec::with_capacity(count),
            exports: alloc::vec::Vec::with_capacity(count),
            names: alloc::vec::Vec::with_capacity(count),
        });
        let mut members = members;
        members.reverse();
        while let Some(member) = members.pop() {
            collected = match collected {
                Ok(collected) => match member {
                    crate::source::declared::parsing::Member::Export(name) => {
                        collected.add_export(name)
                    }
                    crate::source::declared::parsing::Member::Opaque { name, base, public } => {
                        collected.add_opaque(OpaqueDeclaration { name, base, public })
                    }
                    crate::source::declared::parsing::Member::Variant {
                        name,
                        left,
                        left_type,
                        left_public,
                        right,
                        right_type,
                        right_public,
                    } => collected.add_variant(
                        name,
                        (left, left_type, left_public),
                        (right, right_type, right_public),
                    ),
                    crate::source::declared::parsing::Member::Require {
                        name,
                        input,
                        operation,
                    } => collected.add_requirement((name, input, operation)),
                    crate::source::declared::parsing::Member::Definition(bytes) => {
                        collected.add_definition(bytes, limits)
                    }
                },
                Err(problem) => Err(problem),
            };
            if collected.is_err() {
                break;
            }
        }
        let collected = attempt!(collected);
        attempt!(collected.validate());
        Ok(collected)
    }

    fn add_export(mut self, name: alloc::string::String) -> Result<Self, crate::source::Error> {
        if self.exports.contains(&name) {
            return Err(crate::source::declared::error(
                crate::source::Stage::Resolve,
                "duplicate export",
            ));
        }
        self.exports.push(name);
        Ok(self)
    }

    fn add_opaque(self, declaration: OpaqueDeclaration) -> Result<Self, crate::source::Error> {
        let OpaqueDeclaration { name, base, public } = declaration;
        if self.names.contains(&name) {
            return Err(crate::source::declared::error(
                crate::source::Stage::Resolve,
                "duplicate module declaration",
            ));
        }
        Ok(self.insert_schema(
            name,
            crate::source::declared::SchemaKind::Opaque { base, public },
        ))
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; Vec<String>::contains, String equality, insert_schema and owned destructor require runtime on both pinned compilers (E0277/E0658/E0015/E0493); reassess when these APIs become const-capable."
    )]
    fn add_variant(
        self,
        name: alloc::string::String,
        left_arm: (alloc::string::String, alloc::string::String, bool),
        right_arm: (alloc::string::String, alloc::string::String, bool),
    ) -> Result<Self, crate::source::Error> {
        let (left, left_type, left_public) = left_arm;
        let (right, right_type, right_public) = right_arm;
        if self.names.contains(&name) {
            return Err(crate::source::declared::error(
                crate::source::Stage::Resolve,
                "duplicate constructor or module declaration",
            ));
        }
        if left == right {
            return Err(crate::source::declared::error(
                crate::source::Stage::Resolve,
                "duplicate constructor or module declaration",
            ));
        }
        Ok(self.insert_schema(
            name,
            crate::source::declared::SchemaKind::Variant {
                left,
                left_type,
                left_public,
                right,
                right_type,
                right_public,
            },
        ))
    }

    fn insert_schema(
        mut self,
        name: alloc::string::String,
        kind: crate::source::declared::SchemaKind,
    ) -> Self {
        let registered_name = name.clone();
        self.names.push(registered_name);
        self.schemas
            .push(crate::source::declared::Schema { name, kind });
        self
    }

    fn add_requirement(
        mut self,
        requirement: (
            alloc::string::String,
            alloc::string::String,
            alloc::string::String,
        ),
    ) -> Result<Self, crate::source::Error> {
        if self.names.contains(&requirement.0) {
            return Err(crate::source::declared::error(
                crate::source::Stage::Resolve,
                "duplicate required operation",
            ));
        }
        self.names.push(requirement.0.clone());
        self.requirements.push(requirement);
        Ok(self)
    }

    fn add_definition(
        mut self,
        bytes: alloc::vec::Vec<u8>,
        limits: crate::Limits,
    ) -> Result<Self, crate::source::Error> {
        let mut meter = crate::Meter::new(limits);
        let parsed = crate::source::parsing::parse_declared(&bytes, &mut meter);
        let (tree, parsed_name) = attempt!(parsed.map_err(|problem| {
            crate::source::declared::diagnostic(crate::source::Stage::Parse, problem)
        }));
        let parsed_name = attempt!(parsed_name.ok_or_else(|| crate::source::declared::error(
            crate::source::Stage::Parse,
            "module definition missing name"
        )));
        if self.names.contains(&parsed_name) {
            return Err(crate::source::declared::error(
                crate::source::Stage::Resolve,
                "duplicate module definition",
            ));
        }
        self.names.push(parsed_name.clone());
        self.definitions.push((parsed_name, bytes, tree));
        Ok(self)
    }

    fn validate(&self) -> Result<(), crate::source::Error> {
        let mut constructors = 0usize;
        let mut schema_index = 0;
        while schema_index < self.schemas.len() {
            constructors += match &self.schemas[schema_index].kind {
                crate::source::declared::SchemaKind::Opaque { .. } => 2,
                crate::source::declared::SchemaKind::Variant { .. } => 3,
            };
            schema_index += 1;
        }
        if constructors > crate::source::declared::CONSTRUCTOR_CAP
            || self.definitions.len() > crate::source::declared::CONSTRUCTOR_CAP
        {
            return Err(crate::source::declared::error(
                crate::source::Stage::Check,
                "module schema/definition count exceeded",
            ));
        }
        let mut position = 0;
        let mut has_missing_export = false;
        while position < self.exports.len() {
            let exported = &self.exports[position];
            let exported_bytes = exported.as_bytes();
            let mut split = 0;
            while split < exported_bytes.len() {
                if exported_bytes[split] == b'.' {
                    break;
                }
                split += 1;
            }
            let schema = &exported[..split];
            let mut has_declaration = false;
            let mut index = 0;
            while index < self.schemas.len() {
                if self.schemas[index].name == schema {
                    has_declaration = true;
                    break;
                }
                index += 1;
            }
            if !has_declaration {
                index = 0;
                while index < self.definitions.len() {
                    if self.definitions[index].0 == *exported {
                        has_declaration = true;
                        break;
                    }
                    index += 1;
                }
            }
            if !has_declaration {
                has_missing_export = true;
                break;
            }
            position += 1;
        }
        if has_missing_export {
            return Err(crate::source::declared::error(
                crate::source::Stage::Resolve,
                "export references missing declaration",
            ));
        }
        Ok(())
    }
}
