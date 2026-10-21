//! Independently bind one source-classified returned Program to its complete
//! retained VM graph and the selected installed code closure. The hashes below
//! are local experimental identities, not portable ProgramValueIds.

use super::super::{Candidate, ProgramContract};
use super::{denied, number, rows, text};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

const DEFINITION_DOMAIN: &[u8] = b"noble returned pure-i64 DAG definition experimental v3\0";
const VALUE_DOMAIN: &[u8] = b"noble returned pure-i64 DAG value experimental v3\0";
const MANIFEST_DOMAIN: &[u8] = b"noble live-slot composite code donors v1\0";

fn types(source: &str) -> Option<Vec<String>> {
    if source.is_empty() {
        return Some(Vec::new());
    }
    let mut depth = 0usize;
    let mut start = 0;
    let mut result = Vec::new();
    for (at, character) in source.char_indices() {
        match character {
            '[' => depth += 1,
            ']' => depth = depth.checked_sub(1)?,
            ',' if depth == 0 => {
                result.push(source[start..at].to_owned());
                start = at + 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    result.push(source[start..].to_owned());
    if result.iter().all(|item| pure_type(item)) {
        Some(result)
    } else {
        None
    }
}

fn pure_type(source: &str) -> bool {
    source == "I64" || program_type(source).is_some()
}

fn program_type(source: &str) -> Option<(Vec<String>, Vec<String>)> {
    let body = source.strip_prefix("Program([")?.strip_suffix("]!{})")?;
    let mut depth = 0usize;
    for (at, character) in body.char_indices() {
        match character {
            '[' => depth += 1,
            ']' if depth == 0 && body[at..].starts_with("]->[") => {
                let input = types(&body[..at])?;
                let output = types(&body[at + 4..])?;
                return Some((input, output));
            }
            ']' => depth = depth.checked_sub(1)?,
            _ => {}
        }
    }
    None
}

fn canonical_i64(value: &str) -> Result<String, String> {
    let integer = value
        .parse::<i64>()
        .map_err(|_| denied("returned I64 is out of range"))?;
    Ok(integer.to_string())
}

fn signed(value: &str) -> Result<String, String> {
    let canonical = canonical_i64(value)?;
    if value != canonical {
        return Err(denied("returned I64 is not canonical signed decimal"));
    }
    Ok(canonical)
}

fn normalized_recipe(
    source: &Value,
    depth: usize,
    nodes: &mut usize,
) -> Result<(i64, Value), String> {
    *nodes = nodes
        .checked_add(1)
        .ok_or_else(|| denied("fixed recipe size overflow"))?;
    if depth > 32 || *nodes > 128 {
        return Err(denied(
            "computed fixed I64 recipe exceeds checked source bounds",
        ));
    }
    let (value, normalized, fields) = match text(source, "kind")? {
        "literal" => {
            let literal = signed(text(source, "value")?)?;
            (
                literal
                    .parse::<i64>()
                    .map_err(|_| denied("invalid fixed literal"))?,
                json!({"kind":"literal","value":literal}),
                2,
            )
        }
        kind @ ("add" | "sub" | "mul") => {
            let (left, left_recipe) = normalized_recipe(&source["left"], depth + 1, nodes)?;
            let (right, right_recipe) = normalized_recipe(&source["right"], depth + 1, nodes)?;
            let value = match kind {
                "add" => left.wrapping_add(right),
                "sub" => left.wrapping_sub(right),
                _ => left.wrapping_mul(right),
            };
            (
                value,
                json!({"kind":kind,"left":left_recipe,"right":right_recipe}),
                3,
            )
        }
        _ => return Err(denied("foreign fixed I64 recipe operation")),
    };
    if source
        .as_object()
        .is_none_or(|object| object.len() != fields)
    {
        return Err(denied("fixed I64 recipe has unchecked source fields"));
    }
    Ok((value, normalized))
}

fn digest(domain: &[u8], value: &Value) -> Result<String, String> {
    let encoded = serde_json::to_vec(value)
        .map_err(|_| denied("returned DAG descriptor cannot be canonically encoded"))?;
    let mut bytes = Vec::with_capacity(domain.len() + encoded.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&encoded);
    Ok(crate::workflow::intrinsic::sha256(&bytes))
}

fn bounded_semantics(value: &Value, depth: usize, visited: &mut usize) -> Result<(), String> {
    *visited = visited
        .checked_add(1)
        .ok_or_else(|| denied("semantic DAG size overflow"))?;
    if *visited > 4096 || depth > 32 {
        return Err(denied(
            "expanded returned Program template exceeds bounded size/depth",
        ));
    }
    match value {
        Value::Array(items) => {
            for child in items {
                bounded_semantics(child, depth + 1, visited)?;
            }
        }
        Value::Object(fields) => {
            for child in fields.values() {
                bounded_semantics(child, depth + 1, visited)?;
            }
        }
        _ => {}
    }
    Ok(())
}

struct FixedRecipe {
    kind: FixedRecipeKind,
    nodes: usize,
    depth: usize,
}

enum FixedRecipeKind {
    Literal(i64),
    Binary(&'static str, Rc<FixedRecipe>, Rc<FixedRecipe>),
}

impl FixedRecipe {
    fn literal(value: i64) -> Rc<Self> {
        Rc::new(Self {
            kind: FixedRecipeKind::Literal(value),
            nodes: 1,
            depth: 1,
        })
    }

    fn binary(kind: &'static str, left: Rc<Self>, right: Rc<Self>) -> Result<Rc<Self>, String> {
        let nodes = left
            .nodes
            .checked_add(right.nodes)
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| denied("computed I64 expression size overflow"))?;
        let depth = left.depth.max(right.depth) + 1;
        if nodes > 128 || depth > 32 {
            return Err(denied(
                "computed I64 expression exceeds selected source bounds",
            ));
        }
        Ok(Rc::new(Self {
            kind: FixedRecipeKind::Binary(kind, left, right),
            nodes,
            depth,
        }))
    }

    fn json(&self, depth: usize, nodes: &mut usize) -> Result<Value, String> {
        *nodes = nodes
            .checked_add(1)
            .ok_or_else(|| denied("fixed expression size overflow"))?;
        if depth > 32 || *nodes > 128 {
            return Err(denied("returned fixed expression exceeds bounded source"));
        }
        match &self.kind {
            FixedRecipeKind::Literal(value) => {
                Ok(json!({"kind":"literal","value":value.to_string()}))
            }
            FixedRecipeKind::Binary(kind, left, right) => {
                Ok(json!({"kind":kind,"left":left.json(depth+1,nodes)?,
                    "right":right.json(depth+1,nodes)?}))
            }
        }
    }
}

#[derive(Clone)]
enum ScalarOrigin {
    Root(usize),
    Fixed(i64, Rc<FixedRecipe>),
}

#[derive(Clone)]
enum Replayed {
    I64(ScalarOrigin),
    Program(Rc<ProgramOrigin>),
}

enum ProgramOrigin {
    Static {
        node: u32,
        index: usize,
    },
    QuoteI64 {
        node: u32,
        entry: u32,
        operand: ScalarOrigin,
    },
    QuoteProgram {
        node: u32,
        entry: u32,
        operand: Rc<ProgramOrigin>,
    },
    Compose {
        node: u32,
        entry: u32,
        left: Rc<ProgramOrigin>,
        right: Rc<ProgramOrigin>,
    },
}

fn replay_program_json(
    program: &Rc<ProgramOrigin>,
    aliases: &mut Vec<*const ProgramOrigin>,
    fixed_nodes: &mut usize,
) -> Result<Value, String> {
    let pointer = Rc::as_ptr(program);
    if let Some(index) = aliases.iter().position(|previous| *previous == pointer) {
        return Ok(json!({"kind":"reference","alias":index}));
    }
    if aliases.len() >= 32 {
        return Err(denied("replayed Program aliases exceed graph bound"));
    }
    let alias = aliases.len();
    aliases.push(pointer);
    match program.as_ref() {
        ProgramOrigin::Static { node, index } => Ok(json!({"kind":"static-program","alias":alias,
                "node":node,"program_index":index})),
        ProgramOrigin::QuoteI64 {
            node,
            entry,
            operand,
        } => {
            let operand = match operand {
                ScalarOrigin::Root(position) => json!({"kind":"root-i64","position":position}),
                ScalarOrigin::Fixed(value, recipe) => {
                    json!({"kind":"fixed-i64","value":value.to_string(),
                        "recipe":recipe.json(1,fixed_nodes)?})
                }
            };
            Ok(json!({"kind":"quote-i64","alias":alias,"node":node,
                "table_entry":entry,"operand":operand}))
        }
        ProgramOrigin::QuoteProgram {
            node,
            entry,
            operand,
        } => Ok(json!({"kind":"quote-program","alias":alias,"node":node,
                "table_entry":entry,"operand":replay_program_json(operand,aliases,fixed_nodes)?})),
        ProgramOrigin::Compose {
            node,
            entry,
            left,
            right,
        } => Ok(json!({"kind":"compose","alias":alias,"node":node,
                "table_entry":entry,"left":replay_program_json(left,aliases,fixed_nodes)?,
                "right":replay_program_json(right,aliases,fixed_nodes)?})),
    }
}

fn replay_execute(
    programs: &[Value],
    index: usize,
    stack: &mut Vec<Replayed>,
    static_programs: &mut [Option<Rc<ProgramOrigin>>],
    active: &mut Vec<usize>,
    steps: &mut usize,
) -> Result<(), String> {
    if active.len() >= 32 || active.contains(&index) {
        return Err(denied("recursive selected origin execution"));
    }
    let program = programs
        .get(index)
        .ok_or_else(|| denied("source Program is not installed"))?;
    if program["source_artifact"] != "definition" || program["effect_mask"] != 0 {
        return Err(denied(
            "selected origin executes foreign or effectful Program",
        ));
    }
    active.push(index);
    for operation in rows(program, "operations")? {
        *steps = steps
            .checked_add(1)
            .ok_or_else(|| denied("source replay step overflow"))?;
        if *steps > 4096 {
            return Err(denied("selected origin replay exceeds 4096 steps"));
        }
        if operation["effect_mask"] != 0 {
            return Err(denied("selected origin has an effectful instruction"));
        }
        let node = number(operation, "node")?;
        let entry = number(operation, "table_entry")?;
        match text(operation, "kind")? {
            "i64" => {
                let literal = signed(text(operation, "value")?)?
                    .parse::<i64>()
                    .map_err(|_| denied("invalid replayed source I64 literal"))?;
                stack.push(Replayed::I64(ScalarOrigin::Fixed(
                    literal,
                    FixedRecipe::literal(literal),
                )));
            }
            "dup" => stack.push(
                stack
                    .last()
                    .cloned()
                    .ok_or_else(|| denied("selected origin dup has no operand"))?,
            ),
            "drop" => {
                stack
                    .pop()
                    .ok_or_else(|| denied("selected origin drop has no operand"))?;
            }
            "swap" => {
                if stack.len() < 2 {
                    return Err(denied("selected origin swap lacks operands"));
                }
                let len = stack.len();
                stack.swap(len - 1, len - 2);
            }
            "add" | "sub" | "mul" => {
                let (
                    Some(Replayed::I64(ScalarOrigin::Fixed(right, right_recipe))),
                    Some(Replayed::I64(ScalarOrigin::Fixed(left, left_recipe))),
                ) = (stack.pop(), stack.pop())
                else {
                    return Err(denied(
                        "computed selected I64 has a root or foreign operand",
                    ));
                };
                let kind = text(operation, "kind")?;
                let (name, value) = match kind {
                    "add" => ("add", left.wrapping_add(right)),
                    "sub" => ("sub", left.wrapping_sub(right)),
                    _ => ("mul", left.wrapping_mul(right)),
                };
                stack.push(Replayed::I64(ScalarOrigin::Fixed(
                    value,
                    FixedRecipe::binary(name, left_recipe, right_recipe)?,
                )));
            }
            "static-program" => {
                let source = number(operation, "static_program_index")? as usize;
                let slot = static_programs
                    .get_mut(source)
                    .ok_or_else(|| denied("selected static Program index is absent"))?;
                let program = slot
                    .get_or_insert_with(|| {
                        Rc::new(ProgramOrigin::Static {
                            node,
                            index: source,
                        })
                    })
                    .clone();
                stack.push(Replayed::Program(program));
            }
            "quote-i64" => {
                let Some(Replayed::I64(operand)) = stack.pop() else {
                    return Err(denied("source quote does not consume an I64"));
                };
                stack.push(Replayed::Program(Rc::new(ProgramOrigin::QuoteI64 {
                    node,
                    entry,
                    operand,
                })));
            }
            "quote-program" => {
                let Some(Replayed::Program(operand)) = stack.pop() else {
                    return Err(denied("source Program quote has a foreign operand"));
                };
                stack.push(Replayed::Program(Rc::new(ProgramOrigin::QuoteProgram {
                    node,
                    entry,
                    operand,
                })));
            }
            "compose" => {
                let (Some(Replayed::Program(right)), Some(Replayed::Program(left))) =
                    (stack.pop(), stack.pop())
                else {
                    return Err(denied("selected origin composes foreign operands"));
                };
                stack.push(Replayed::Program(Rc::new(ProgramOrigin::Compose {
                    node,
                    entry,
                    left,
                    right,
                })));
            }
            "dip" => {
                let (Some(Replayed::Program(helper)), Some(hidden)) = (stack.pop(), stack.pop())
                else {
                    return Err(denied(
                        "selected generated dip has no static helper or hidden value",
                    ));
                };
                let ProgramOrigin::Static { index: helper, .. } = helper.as_ref() else {
                    return Err(denied("selected generated dip executes a dynamic Program"));
                };
                replay_execute(programs, *helper, stack, static_programs, active, steps)?;
                stack.push(hidden);
            }
            _ => {
                return Err(denied(
                    "selected source execution has an unsupported operation",
                ))
            }
        }
        if stack.len() > 128 {
            return Err(denied("selected origin replay stack exceeds bound"));
        }
    }
    active.pop();
    Ok(())
}

fn replay_returned_origin(
    programs: &[Value],
    contracts: &[ProgramContract],
    named: usize,
    inputs: &[Value],
    result: &Value,
    position: usize,
) -> Result<(), String> {
    let mut stack = (0..inputs.len())
        .map(|index| Replayed::I64(ScalarOrigin::Root(index)))
        .collect::<Vec<_>>();
    let mut static_programs = vec![None; programs.len()];
    replay_execute(
        programs,
        named,
        &mut stack,
        &mut static_programs,
        &mut Vec::new(),
        &mut 0,
    )?;
    let output = &contracts[named].output;
    if stack.len() != output.len() || output.get(position).is_none() {
        return Err(denied(
            "replayed returned Program interface differs from selected source",
        ));
    }
    for (index, (value, ty)) in stack.iter().zip(output).enumerate() {
        match (value, ty.as_str()) {
            (Replayed::I64(_), "I64") => {}
            (Replayed::Program(program), ty) if ty.starts_with("Program(") && index == position => {
                if &replay_program_json(program, &mut Vec::new(), &mut 0)? != result {
                    return Err(denied(
                        "returned Program DAG differs from source operation replay",
                    ));
                }
            }
            _ => {
                return Err(denied(
                    "selected source returns a foreign or ambiguous Program",
                ))
            }
        }
    }
    Ok(())
}

struct Plan<'a> {
    programs: &'a [Value],
    contracts: &'a [ProgramContract],
    inputs: &'a [Value],
    result: &'a Value,
    interface: Value,
    template: Value,
    captures: Vec<Value>,
    captured_positions: BTreeSet<usize>,
    fixed_nodes: usize,
    aliases: usize,
    semantic_aliases: Vec<Value>,
    graph_cells: usize,
    graph_statics: BTreeSet<usize>,
    constructed_sites: Vec<Value>,
    static_sites: Vec<Value>,
    program_indices: BTreeSet<usize>,
    entries: BTreeSet<u32>,
    dependencies: BTreeSet<String>,
    dip: bool,
    bodies: BTreeMap<usize, Value>,
}

impl<'a> Plan<'a> {
    fn new(
        origin: &'a Value,
        contracts: &'a [ProgramContract],
        position: usize,
        descriptor: &str,
        inputs: &'a [Value],
    ) -> Result<Option<Self>, String> {
        let Some((input_types, output_types)) = program_type(descriptor) else {
            return Ok(None);
        };
        if input_types
            .iter()
            .chain(&output_types)
            .any(|ty| ty != "I64")
            || output_types.is_empty()
            || inputs.iter().any(|input| input["kind"] != "i64")
        {
            return Ok(None);
        }
        let outputs = rows(origin, "outputs")?;
        if outputs.len() != 1 || number(&outputs[0], "stack_position")? as usize != position {
            return Ok(None);
        }
        let result = &outputs[0]["result"];
        // A pinned static Program alone has different ownership semantics; it
        // retains its ordinary saved owner rather than acquiring a new ID.
        if result["kind"] == "static-program" || result["kind"] == "reference" {
            return Ok(None);
        }
        let programs = rows(origin, "programs")?;
        if programs.is_empty()
            || programs.len() > 32
            || programs.len() != contracts.len()
            || programs
                .iter()
                .try_fold(0usize, |sum, program| {
                    sum.checked_add(rows(program, "operations").ok()?.len())
                })
                .is_none_or(|count| count > 128)
        {
            return Ok(None);
        }
        let named = number(origin, "named_program_index")? as usize;
        let Some(builder) = programs.get(named) else {
            return Ok(None);
        };
        if builder["source_artifact"] != "definition"
            || builder["effect_mask"] != 0
            || !contracts[named].effects.is_empty()
            || inputs.len() != contracts[named].input.len()
            || contracts[named].input.iter().any(|ty| ty != "I64")
            || contracts[named]
                .output
                .get(position)
                .is_none_or(|output| output.as_str() != descriptor)
        {
            return Ok(None);
        }
        for input in inputs {
            canonical_i64(text(input, "value")?)?;
        }
        let mut plan = Self {
            programs,
            contracts,
            inputs,
            result,
            interface: json!({"input":input_types,"output":output_types,"effects":[]}),
            template: Value::Null,
            captures: Vec::new(),
            captured_positions: BTreeSet::new(),
            fixed_nodes: 0,
            aliases: 0,
            semantic_aliases: Vec::new(),
            graph_cells: 0,
            graph_statics: BTreeSet::new(),
            constructed_sites: Vec::new(),
            static_sites: Vec::new(),
            program_indices: BTreeSet::new(),
            entries: BTreeSet::new(),
            dependencies: BTreeSet::new(),
            dip: false,
            bodies: BTreeMap::new(),
        };
        for operation in rows(builder, "operations")? {
            plan.record_entry(operation)?;
            if operation["kind"] == "static-program" {
                plan.static_body(
                    number(operation, "static_program_index")? as usize,
                    &mut BTreeSet::new(),
                )?;
            }
        }
        replay_returned_origin(programs, contracts, named, inputs, result, position)?;
        plan.template = plan.template_node(result, 0)?;
        bounded_semantics(&plan.template, 1, &mut 0)?;
        Ok(Some(plan))
    }

    fn record_entry(&mut self, operation: &Value) -> Result<(), String> {
        if operation["effect_mask"] != 0 {
            return Err(denied("effectful returned source operation"));
        }
        match text(operation, "kind")? {
            "dip" => self.dip = true,
            "add" | "sub" | "mul" => {
                self.dependencies
                    .insert(format!("builtin:i64.{}", text(operation, "kind")?));
            }
            "i64" | "dup" | "swap" | "drop" | "run" | "static-program" | "compose"
            | "quote-i64" | "quote-program" => {}
            _ => return Err(denied("unsupported returned source operation")),
        }
        self.entries.insert(number(operation, "table_entry")?);
        Ok(())
    }

    fn source_site(&self, value: &Value, kind: &str) -> Result<&'a Value, String> {
        let node = number(value, "node")?;
        let entry = value.get("table_entry").and_then(Value::as_u64);
        let index = value.get("program_index").and_then(Value::as_u64);
        let mut matched = self
            .programs
            .iter()
            .filter(|program| program["source_artifact"] == "definition")
            .flat_map(|program| program["operations"].as_array().into_iter().flatten())
            .filter(|operation| {
                operation["kind"] == kind
                    && operation["node"].as_u64() == Some(u64::from(node))
                    && (kind == "static-program"
                        && operation["static_program_index"].as_u64() == index
                        || kind != "static-program" && operation["table_entry"].as_u64() == entry)
            });
        let site = matched
            .next()
            .ok_or_else(|| denied("returned Program lacks checked source site"))?;
        if matched.next().is_some() || site["effect_mask"] != 0 {
            return Err(denied("ambiguous or effectful returned source site"));
        }
        Ok(site)
    }

    fn static_body(
        &mut self,
        index: usize,
        visiting: &mut BTreeSet<usize>,
    ) -> Result<Value, String> {
        if let Some(body) = self.bodies.get(&index) {
            return Ok(body.clone());
        }
        if visiting.len() >= 32 || !visiting.insert(index) {
            return Err(denied("recursive or unbounded static dependency"));
        }
        let programs = self.programs;
        let contracts = self.contracts;
        let program = programs
            .get(index)
            .ok_or_else(|| denied("static dependency is outside checked program map"))?;
        let contract = contracts
            .get(index)
            .ok_or_else(|| denied("static dependency lacks installed contract"))?;
        if program["source_artifact"] != "definition"
            || program["effect_mask"] != 0
            || !contract.effects.is_empty()
            || !contract
                .input
                .iter()
                .chain(&contract.output)
                .all(|ty| pure_type(ty))
        {
            return Err(denied(
                "static dependency is foreign, effectful or not pure I64/Program",
            ));
        }
        let operations = rows(program, "operations")?;
        let mut encoded = Vec::with_capacity(operations.len());
        self.program_indices.insert(index);
        for operation in operations {
            self.record_entry(operation)?;
            let kind = text(operation, "kind")?;
            let item = match kind {
                "i64" => json!({"kind":kind,"value":signed(text(operation, "value")?)?}),
                "static-program" => json!({"kind":kind,
                    "program":self.static_body(number(operation, "static_program_index")? as usize,
                        visiting)?}),
                _ => json!({"kind":kind}),
            };
            encoded.push(item);
        }
        visiting.remove(&index);
        let body = json!({"input":contract.input,"output":contract.output,
            "effects":[],"operations":encoded});
        self.bodies.insert(index, body.clone());
        Ok(body)
    }

    fn charge_graph(&mut self, count: usize) -> Result<(), String> {
        self.graph_cells = self
            .graph_cells
            .checked_add(count)
            .ok_or_else(|| denied("returned Program graph cell count overflow"))?;
        if self.graph_cells > 32 {
            return Err(denied("returned Program cannot fit bounded retained graph"));
        }
        Ok(())
    }

    fn static_graph(&mut self, index: usize) -> Result<(), String> {
        if !self.graph_statics.insert(index) {
            return Ok(());
        }
        let programs = self.programs;
        let operations = rows(
            programs
                .get(index)
                .ok_or_else(|| denied("missing returned static Program"))?,
            "operations",
        )?;
        // One Program, one atom per source operation, and a left-associated
        // concat for each operation following the first.
        let cells = operations
            .len()
            .checked_mul(2)
            .ok_or_else(|| denied("static recipe cell count overflow"))?;
        self.charge_graph(cells)?;
        for operation in operations {
            if operation["kind"] == "static-program" {
                self.static_graph(number(operation, "static_program_index")? as usize)?;
            }
        }
        Ok(())
    }

    fn template_node(&mut self, source: &Value, depth: usize) -> Result<Value, String> {
        if depth >= 32 {
            return Err(denied("returned source DAG exceeds depth/cell bound"));
        }
        let kind = text(source, "kind")?;
        let alias = number(source, "alias")? as usize;
        if kind == "reference" {
            if alias >= self.aliases || source.as_object().is_none_or(|fields| fields.len() != 2) {
                return Err(denied(
                    "returned source has a forward or malformed Program reference",
                ));
            }
            let prior = self
                .semantic_aliases
                .get(alias)
                .ok_or_else(|| denied("returned Program reference has no semantic origin"))?;
            if prior.is_null() {
                return Err(denied("cyclic returned Program reference"));
            }
            return Ok(prior.clone());
        }
        if self.aliases >= 32 {
            return Err(denied("returned source DAG exceeds cell bound"));
        }
        if alias != self.aliases {
            return Err(denied("returned source alias order differs from DFS"));
        }
        self.aliases += 1;
        self.semantic_aliases.push(Value::Null);
        let normalized = match kind {
            "static-program" => {
                let site = self.source_site(source, kind)?;
                let index = number(source, "program_index")? as usize;
                self.static_sites
                    .push(json!({"program_index":index,"node":number(site,"node")?,
                    "table_entry":number(site,"table_entry")?}));
                self.static_graph(index)?;
                Ok(json!({"kind":kind,
                    "program":self.static_body(index, &mut BTreeSet::new())?}))
            }
            "quote-i64" => {
                self.charge_graph(3)?;
                let site = self.source_site(source, kind)?;
                self.constructed_sites
                    .push(json!({"kind":kind,"node":number(site,"node")?,
                    "table_entry":number(site,"table_entry")?}));
                let operand = &source["operand"];
                let operand = match text(operand, "kind")? {
                    "root-i64" => {
                        let index = number(operand, "position")? as usize;
                        let input = self
                            .inputs
                            .get(index)
                            .ok_or_else(|| denied("quoted root binder does not exist"))?;
                        if self.captured_positions.insert(index) {
                            self.captures.push(json!({"type":"I64",
                                "value":canonical_i64(text(input,"value")?)?}));
                        }
                        json!({"kind":"root-i64","position":index})
                    }
                    "fixed-i64" => {
                        let value = signed(text(operand, "value")?)?;
                        let (evaluated, recipe) =
                            normalized_recipe(&operand["recipe"], 1, &mut self.fixed_nodes)?;
                        if evaluated.to_string() != value {
                            return Err(denied(
                                "fixed source recipe differs from saved I64 capture",
                            ));
                        }
                        json!({"kind":"fixed-i64","value":value,"recipe":recipe})
                    }
                    _ => return Err(denied("foreign Program quote I64 operand")),
                };
                Ok(json!({"kind":kind,"operand":operand}))
            }
            "quote-program" => {
                self.charge_graph(2)?;
                let site = self.source_site(source, kind)?;
                self.constructed_sites
                    .push(json!({"kind":kind,"node":number(site,"node")?,
                    "table_entry":number(site,"table_entry")?}));
                let operand = self.template_node(&source["operand"], depth + 1)?;
                Ok(json!({"kind":kind,"operand":operand}))
            }
            "compose" => {
                self.charge_graph(2)?;
                let site = self.source_site(source, kind)?;
                self.constructed_sites
                    .push(json!({"kind":kind,"node":number(site,"node")?,
                    "table_entry":number(site,"table_entry")?}));
                let left = self.template_node(&source["left"], depth + 1)?;
                let right = self.template_node(&source["right"], depth + 1)?;
                Ok(json!({"kind":kind,"left":left,"right":right}))
            }
            _ => Err(denied("unsupported constructed returned Program")),
        }?;
        bounded_semantics(&normalized, 1, &mut 0)?;
        self.semantic_aliases[alias] = normalized.clone();
        Ok(normalized)
    }

    fn identity(&self) -> Result<Value, String> {
        let descriptor = json!({"schema":"noble-returned-pure-i64-dag/experimental-v3",
            "interface":self.interface,"template":self.template,"lexical_recursive_owner":null});
        let definition_id = format!(
            "anonymous-template-experimental-v3-sha256:{}",
            digest(DEFINITION_DOMAIN, &descriptor)?
        );
        let program_value_id = format!(
            "anonymous-program-experimental-v3-sha256:{}",
            digest(
                VALUE_DOMAIN,
                &json!({"definition_id":definition_id,"captures":self.captures})
            )?
        );
        Ok(
            json!({"definition_id":definition_id,"program_value_id":program_value_id,
            "captures":self.captures,"interface":self.interface,"effects":[],
            "dependencies":self.dependencies,"artifact_sha256":null}),
        )
    }
}

pub(in crate::core::live_slot) fn supports_anonymous_target(
    origin: &Value,
    contracts: &[ProgramContract],
    position: usize,
    descriptor: &str,
    inputs: &[Value],
) -> Result<bool, String> {
    // Unsupported source never prevents saving or using the ordinary Program.
    Ok(Plan::new(origin, contracts, position, descriptor, inputs).is_ok_and(|plan| plan.is_some()))
}

struct Graph<'a> {
    cells: &'a [Value],
    seen: [bool; 32],
    aliases: Vec<usize>,
    constructed: BTreeSet<usize>,
}

#[derive(Clone, Copy)]
struct Shape {
    input: u32,
    output: u32,
    depth: u32,
    leaves: u32,
    recipe: usize,
}

impl<'a> Graph<'a> {
    fn new(value: &'a Value) -> Result<Self, String> {
        let cells = value
            .as_array()
            .ok_or_else(|| denied("no complete retained Program graph"))?;
        if cells.is_empty() || cells.len() > 32 {
            return Err(denied("retained DAG exceeds 32 graph cells"));
        }
        Ok(Self {
            cells,
            seen: [false; 32],
            aliases: Vec::new(),
            constructed: BTreeSet::new(),
        })
    }

    fn edge(row: &Value, field: &str) -> Result<usize, String> {
        row.get(field)
            .and_then(Value::as_u64)
            .and_then(|index| usize::try_from(index).ok())
            .ok_or_else(|| denied("missing or invalid returned graph edge"))
    }

    fn absent(row: &Value, field: &str) -> bool {
        row.get(field).is_some_and(Value::is_null)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "Owner: noble-maintainers; the expected cell mirrors the fixed ABI fields (kind, payload, x, y, z, w, n) that the host verifies one by one."
    )]
    fn cell(
        &mut self,
        index: usize,
        kind: u64,
        payload: &str,
        x: u32,
        y: u32,
        z: u32,
        w: u32,
        n: u32,
    ) -> Result<&'a Value, String> {
        let row = self
            .cells
            .get(index)
            .ok_or_else(|| denied("truncated retained graph edge"))?;
        if row.as_object().is_none_or(|fields| fields.len() != 11)
            || row["kind"].as_u64() != Some(kind)
            || row["payload"] != payload
            || row["x"].as_u64() != Some(u64::from(x))
            || row["y"].as_u64() != Some(u64::from(y))
            || row["z"].as_u64() != Some(u64::from(z))
            || row["w"].as_u64() != Some(u64::from(w))
            || row["n"].as_u64() != Some(u64::from(n))
            || !Self::absent(row, "textBytes")
            || ["a", "b", "c"].iter().any(|key| row.get(*key).is_none())
        {
            return Err(denied(
                "retained graph cell differs from independently checked source",
            ));
        }
        self.seen[index] = true;
        Ok(row)
    }

    fn atom(
        &mut self,
        index: usize,
        operation: &Value,
        static_child: Option<usize>,
    ) -> Result<(), String> {
        let kind = text(operation, "kind")?;
        let (tag, payload) = match kind {
            "i64" => (1, signed(text(operation, "value")?)?),
            "static-program" => (3, "0".to_owned()),
            "add" => (2, "4".to_owned()),
            "sub" => (2, "5".to_owned()),
            "mul" => (2, "6".to_owned()),
            "dup" => (2, "0".to_owned()),
            "swap" => (2, "2".to_owned()),
            "drop" => (2, "1".to_owned()),
            "dip" => (2, "3".to_owned()),
            "run" => (2, "10".to_owned()),
            "compose" => (2, "9".to_owned()),
            "quote-i64" | "quote-program" => (2, "8".to_owned()),
            _ => return Err(denied("unsupported static recipe opcode")),
        };
        let (input, output, effects) = if tag == 2 {
            (
                number(operation, "input_signature")?,
                number(operation, "output_signature")?,
                number(operation, "effect_mask")?,
            )
        } else {
            (0, 0, 0)
        };
        let row = self.cell(index, 8, &payload, tag, input, output, effects, 0)?;
        if !Self::absent(row, "b")
            || !Self::absent(row, "c")
            || static_child.is_some_and(|child| row["a"].as_u64() != Some(child as u64))
            || static_child.is_none() && !Self::absent(row, "a")
        {
            return Err(denied("static recipe atom has an unchecked child"));
        }
        Ok(())
    }

    fn concat(&mut self, index: usize, left: usize, right: usize) -> Result<(), String> {
        let row = self.cell(index, 9, "0", 0, 0, 0, 0, 0)?;
        if Self::edge(row, "a")? != left
            || Self::edge(row, "b")? != right
            || !Self::absent(row, "c")
        {
            return Err(denied("recipe concat has unchecked edges"));
        }
        Ok(())
    }

    fn static_recipe(
        &mut self,
        index: usize,
        operations: &[Value],
        plan: &Plan<'_>,
        depth: usize,
    ) -> Result<(), String> {
        if depth >= 32 {
            return Err(denied("static dependency recipe exceeds graph bound"));
        }
        let (last, earlier) = operations
            .split_last()
            .ok_or_else(|| denied("static dependency has no checked recipe operations"))?;
        if earlier.is_empty() {
            let child = if last["kind"] == "static-program" {
                let cells = self.cells;
                let row = cells
                    .get(index)
                    .ok_or_else(|| denied("truncated static recipe"))?;
                let child = Self::edge(row, "a")?;
                self.static_program(
                    child,
                    number(last, "static_program_index")? as usize,
                    plan,
                    depth + 1,
                )?;
                Some(child)
            } else {
                None
            };
            return self.atom(index, last, child);
        }
        let row = self.cell(index, 9, "0", 0, 0, 0, 0, 0)?;
        let left = Self::edge(row, "a")?;
        let right = Self::edge(row, "b")?;
        if !Self::absent(row, "c") {
            return Err(denied("static recipe has an extra edge"));
        }
        self.static_recipe(left, earlier, plan, depth + 1)?;
        self.static_recipe(right, std::slice::from_ref(last), plan, depth + 1)
    }

    fn static_dimensions(
        index: usize,
        plan: &Plan<'_>,
        visited: &mut BTreeSet<usize>,
    ) -> Result<(u32, u32), String> {
        if visited.len() >= 32 || !visited.insert(index) {
            return Err(denied("recursive static Program dimensions"));
        }
        let source = plan
            .programs
            .get(index)
            .ok_or_else(|| denied("missing static dependency"))?;
        let mut depth = 1u32;
        let mut leaves = 0u32;
        for operation in rows(source, "operations")? {
            leaves = leaves
                .checked_add(1)
                .ok_or_else(|| denied("static recipe leaf overflow"))?;
            if operation["kind"] == "static-program" {
                let (child_depth, child_leaves) = Self::static_dimensions(
                    number(operation, "static_program_index")? as usize,
                    plan,
                    visited,
                )?;
                depth = depth.max(
                    child_depth
                        .checked_add(1)
                        .ok_or_else(|| denied("static dependency depth overflow"))?,
                );
                leaves = leaves
                    .checked_add(child_leaves)
                    .ok_or_else(|| denied("static dependency leaf overflow"))?;
            }
        }
        visited.remove(&index);
        Ok((depth, leaves))
    }

    fn static_program(
        &mut self,
        index: usize,
        source_index: usize,
        plan: &Plan<'_>,
        depth: usize,
    ) -> Result<Shape, String> {
        if depth >= 32 || !plan.program_indices.contains(&source_index) {
            return Err(denied("unchecked static Program child"));
        }
        let source = plan
            .programs
            .get(source_index)
            .ok_or_else(|| denied("missing checked static Program"))?;
        let (program_depth, leaves) =
            Self::static_dimensions(source_index, plan, &mut BTreeSet::new())?;
        let input = number(source, "input_signature")?;
        let output = number(source, "output_signature")?;
        let row = self.cell(
            index,
            4,
            "0",
            number(source, "entry")?,
            input,
            output,
            program_depth,
            leaves,
        )?;
        if !Self::absent(row, "a") || !Self::absent(row, "b") {
            return Err(denied("static Program has unchecked captures"));
        }
        let recipe = Self::edge(row, "c")?;
        self.static_recipe(recipe, rows(source, "operations")?, plan, depth + 1)?;
        Ok(Shape {
            input,
            output,
            depth: program_depth,
            leaves,
            recipe,
        })
    }

    fn node(
        &mut self,
        index: usize,
        source: &Value,
        plan: &Plan<'_>,
        depth: usize,
    ) -> Result<Shape, String> {
        if depth >= 32 {
            return Err(denied("constructed Program DAG depth exceeds bound"));
        }
        let kind = text(source, "kind")?;
        let alias = number(source, "alias")? as usize;
        if kind == "reference" {
            let previous = *self
                .aliases
                .get(alias)
                .ok_or_else(|| denied("forward retained Program alias"))?;
            if index != previous {
                return Err(denied("source Program sharing differs from retained graph"));
            }
            let row = self
                .cells
                .get(index)
                .ok_or_else(|| denied("shared Program edge truncated"))?;
            return Ok(Shape {
                input: number(row, "y")?,
                output: number(row, "z")?,
                depth: number(row, "w")?,
                leaves: number(row, "n")?,
                recipe: Self::edge(row, "c")?,
            });
        }
        if alias != self.aliases.len() {
            return Err(denied("source and graph aliases differ"));
        }
        self.aliases.push(index);
        if kind != "static-program" && !self.constructed.insert(index) {
            return Err(denied(
                "distinct source constructors share a returned Program cell",
            ));
        }
        match kind {
            "static-program" => self.static_program(
                index,
                number(source, "program_index")? as usize,
                plan,
                depth + 1,
            ),
            "quote-i64" => {
                let site = plan.source_site(source, kind)?;
                let value = match text(&source["operand"], "kind")? {
                    "root-i64" => {
                        let position = number(&source["operand"], "position")? as usize;
                        text(
                            plan.inputs
                                .get(position)
                                .ok_or_else(|| denied("missing quote root operand"))?,
                            "value",
                        )?
                    }
                    "fixed-i64" => text(&source["operand"], "value")?,
                    _ => return Err(denied("unsupported quoted scalar")),
                };
                let value = if source["operand"]["kind"] == "root-i64" {
                    canonical_i64(value)?
                } else {
                    signed(value)?
                };
                let input = number(site, "quote_input_signature")?;
                let output = number(site, "quote_output_signature")?;
                let row = self.cell(index, 4, "0", 1, input, output, 1, 1)?;
                if !Self::absent(row, "b") {
                    return Err(denied("I64 quote has an extra capture"));
                }
                let capture = Self::edge(row, "a")?;
                let recipe = Self::edge(row, "c")?;
                let scalar = self.cell(capture, 1, &value, 0, 0, 0, 0, 0)?;
                if ["a", "b", "c"]
                    .iter()
                    .any(|field| !Self::absent(scalar, field))
                {
                    return Err(denied("I64 capture has unchecked edges"));
                }
                let atom = self.cell(recipe, 8, &value, 1, 0, 0, 0, 0)?;
                if ["a", "b", "c"]
                    .iter()
                    .any(|field| !Self::absent(atom, field))
                {
                    return Err(denied("I64 quote atom has unchecked edges"));
                }
                Ok(Shape {
                    input,
                    output,
                    depth: 1,
                    leaves: 1,
                    recipe,
                })
            }
            "quote-program" => {
                let site = plan.source_site(source, kind)?;
                let cells = self.cells;
                let row = cells
                    .get(index)
                    .ok_or_else(|| denied("missing quoted Program"))?;
                let capture = Self::edge(row, "a")?;
                let recipe = Self::edge(row, "c")?;
                let child = self.node(capture, &source["operand"], plan, depth + 1)?;
                let program_depth = child
                    .depth
                    .checked_add(1)
                    .ok_or_else(|| denied("quoted Program depth overflow"))?;
                let leaves = child
                    .leaves
                    .checked_add(1)
                    .ok_or_else(|| denied("quoted Program leaf overflow"))?;
                let input = number(site, "quote_input_signature")?;
                let output = number(site, "quote_output_signature")?;
                let row = self.cell(index, 4, "0", 1, input, output, program_depth, leaves)?;
                if !Self::absent(row, "b") {
                    return Err(denied("Program quote has extra capture"));
                }
                let atom = self.cell(recipe, 8, "0", 3, 0, 0, 0, 0)?;
                if Self::edge(atom, "a")? != capture
                    || !Self::absent(atom, "b")
                    || !Self::absent(atom, "c")
                {
                    return Err(denied("quoted Program recipe differs from captured child"));
                }
                Ok(Shape {
                    input,
                    output,
                    depth: program_depth,
                    leaves,
                    recipe,
                })
            }
            "compose" => {
                let site = plan.source_site(source, kind)?;
                let cells = self.cells;
                let row = cells
                    .get(index)
                    .ok_or_else(|| denied("missing composed Program"))?;
                let left = Self::edge(row, "a")?;
                let right = Self::edge(row, "b")?;
                let recipe = Self::edge(row, "c")?;
                let left_shape = self.node(left, &source["left"], plan, depth + 1)?;
                let right_shape = self.node(right, &source["right"], plan, depth + 1)?;
                if left_shape.output != right_shape.input {
                    return Err(denied("incompatible composed Program interfaces"));
                }
                let program_depth = left_shape
                    .depth
                    .max(right_shape.depth)
                    .checked_add(1)
                    .ok_or_else(|| denied("composition depth overflow"))?;
                let leaves = left_shape
                    .leaves
                    .checked_add(right_shape.leaves)
                    .ok_or_else(|| denied("composition leaves overflow"))?;
                let row = self.cell(
                    index,
                    4,
                    "0",
                    2,
                    left_shape.input,
                    right_shape.output,
                    program_depth,
                    leaves,
                )?;
                if Self::edge(row, "a")? != left
                    || Self::edge(row, "b")? != right
                    || site["effect_mask"] != 0
                {
                    return Err(denied("composed source or retained children differ"));
                }
                self.concat(recipe, left_shape.recipe, right_shape.recipe)?;
                Ok(Shape {
                    input: left_shape.input,
                    output: right_shape.output,
                    depth: program_depth,
                    leaves,
                    recipe,
                })
            }
            _ => Err(denied("unsupported retained Program constructor")),
        }
    }

    fn capture_matches(&self, root: usize, reported: &Value) -> bool {
        let Some(summary) = reported.as_array() else {
            return false;
        };
        let mut global = [0usize; 32];
        let mut mapped = [None; 32];
        if root >= self.cells.len() {
            return false;
        }
        global[0] = root;
        mapped[root] = Some(0);
        let (mut local, mut pending) = (0, 1);
        while local < pending {
            let Some(actual) = self.cells.get(global[local]) else {
                return false;
            };
            let Some(claimed) = summary.get(local) else {
                return false;
            };
            if claimed.as_object().is_none_or(|fields| fields.len() != 11)
                || ["kind", "payload", "x", "y", "z", "w", "n", "textBytes"]
                    .iter()
                    .any(|key| claimed.get(*key) != actual.get(*key))
            {
                return false;
            }
            for field in ["a", "b", "c"] {
                let Some(edge) = actual.get(field) else {
                    return false;
                };
                let Some(rebased) = claimed.get(field) else {
                    return false;
                };
                if edge.is_null() {
                    if !rebased.is_null() {
                        return false;
                    }
                } else {
                    let Some(child) = edge.as_u64().and_then(|value| usize::try_from(value).ok())
                    else {
                        return false;
                    };
                    let Some(place) = mapped.get_mut(child) else {
                        return false;
                    };
                    let offset = match place {
                        Some(offset) => *offset,
                        None => {
                            if pending == global.len() {
                                return false;
                            }
                            *place = Some(pending);
                            global[pending] = child;
                            pending += 1;
                            pending - 1
                        }
                    };
                    if rebased.as_u64() != Some(offset as u64) {
                        return false;
                    }
                }
            }
            local += 1;
        }
        summary.len() == pending
    }

    fn validate(&mut self, plan: &Plan<'_>, saved: &Value) -> Result<(), String> {
        if saved.get("lexical_owner").is_some()
            || saved["capture_status"] != "checked-selected-origin"
        {
            return Err(denied(
                "saved Program lacks checked selected capture ownership",
            ));
        }
        self.node(0, plan.result, plan, 0)?;
        if self.seen[..self.cells.len()].iter().any(|seen| !seen) {
            return Err(denied(
                "retained Program contains extra unchecked graph cells",
            ));
        }
        let captures = rows(saved, "capture_values")?;
        let root = &self.cells[0];
        if root["x"] == 1
            && root["a"]
                .as_u64()
                .and_then(|index| self.cells.get(index as usize))
                .is_some_and(|captured| captured["kind"] == 1)
        {
            let value = &self.cells[Self::edge(root, "a")?]["payload"];
            if captures != [json!({"type":"I64","value":value})] {
                return Err(denied(
                    "saved I64 quote capture differs from retained owner",
                ));
            }
        } else {
            let children = ["a", "b"]
                .iter()
                .filter_map(|field| root[*field].as_u64().map(|child| (*field, child as usize)))
                .collect::<Vec<_>>();
            if captures.len() != children.len()
                || children
                    .iter()
                    .zip(captures)
                    .any(|((field, index), capture)| {
                        capture["field"] != format!("cell_{field}")
                            || !self.capture_matches(*index, &capture["value"])
                    })
            {
                return Err(denied(
                    "saved public capture graph differs from retained owner",
                ));
            }
        }
        Ok(())
    }
}

pub(in crate::core::live_slot) fn anonymous_target_identity(
    origin: &Value,
    contracts: &[ProgramContract],
    position: usize,
    descriptor: &str,
    saved: &Value,
    graph: &Value,
    inputs: &[Value],
) -> Result<Option<Value>, String> {
    let Some(plan) = Plan::new(origin, contracts, position, descriptor, inputs)? else {
        return Ok(None);
    };
    Graph::new(graph)?.validate(&plan, saved)?;
    Ok(Some(plan.identity()?))
}

fn recipe_digest(graph: &Value) -> Result<String, String> {
    let cells = graph
        .as_array()
        .ok_or_else(|| denied("retained graph is not an array"))?;
    let recipe = Graph::edge(
        cells
            .first()
            .ok_or_else(|| denied("missing retained root"))?,
        "c",
    )?;
    let mut global = [0usize; 32];
    let mut mapped = [None; 32];
    global[0] = recipe;
    *mapped
        .get_mut(recipe)
        .ok_or_else(|| denied("recipe root is truncated"))? = Some(0);
    let (mut local, mut pending) = (0, 1);
    let mut normalized = Vec::new();
    while local < pending {
        let mut row = cells
            .get(global[local])
            .ok_or_else(|| denied("truncated recipe cell"))?
            .clone();
        for field in ["a", "b", "c"] {
            if row[field].is_null() {
                continue;
            }
            let child = Graph::edge(&row, field)?;
            let Some(place) = mapped.get_mut(child) else {
                return Err(denied("recipe edge is outside graph"));
            };
            let position = match place {
                Some(position) => *position,
                None => {
                    if pending == global.len() {
                        return Err(denied("recipe graph exceeds 32 cells"));
                    }
                    *place = Some(pending);
                    global[pending] = child;
                    pending += 1;
                    pending - 1
                }
            };
            row[field] = json!(position);
        }
        normalized.push(row);
        local += 1;
    }
    Ok(crate::workflow::intrinsic::sha256(
        &serde_json::to_vec(&normalized)
            .map_err(|_| denied("recipe cannot be canonically encoded"))?,
    ))
}

#[expect(
    clippy::too_many_arguments,
    reason = "Owner: noble-maintainers; each source-bound donor coordinate is an independent identity the manifest binds; bundling them would hide which coordinate each check compares."
)]
pub(in crate::core::live_slot) fn checked_code_donor_manifest(
    owner: &str,
    handle: u32,
    caller_id: &str,
    caller: &Candidate,
    core_id: &str,
    core: &Candidate,
    position: usize,
    descriptor: &str,
    identity: &Value,
    graph: &Value,
    inputs: &[Value],
) -> Result<Option<Value>, String> {
    let Some(origin) = caller.selected_origin.as_ref() else {
        return Ok(None);
    };
    let Some(plan) = Plan::new(
        &origin.checked_metadata,
        &caller.programs,
        position,
        descriptor,
        inputs,
    )?
    else {
        return Ok(None);
    };
    let mut observed = Graph::new(graph)?;
    observed.node(0, plan.result, &plan, 0)?;
    if observed.seen[..observed.cells.len()]
        .iter()
        .any(|seen| !seen)
    {
        return Err(denied("code donor graph contains extra unchecked cells"));
    }
    if core.span.generation != 1
        || core.selected_origin.is_none()
        || handle < 4096
        || identity != &plan.identity()?
    {
        return Err(denied(
            "selected returned DAG lacks checked identity or native core owner",
        ));
    }
    let end = caller
        .span
        .start
        .checked_add(caller.span.slots)
        .ok_or_else(|| denied("selected donor code span overflows"))?;
    if plan
        .entries
        .iter()
        .any(|entry| *entry < caller.span.start || *entry >= end)
    {
        return Err(denied(
            "selected donor code entry is outside installed caller span",
        ));
    }
    let graph_bytes = serde_json::to_vec(graph)
        .map_err(|_| denied("retained graph cannot be canonically encoded"))?;
    let manifest = json!({
        "schema":"noble-live-slot-composite-code-donors/v1",
        "owner":{"token":owner,"handle":handle},
        "identity":{"definition_id":identity["definition_id"],
            "program_value_id":identity["program_value_id"],
            "captures":identity["captures"],"interface":identity["interface"],
            "lexical_recursive_owner":null},
        "source":{"caller_id":caller_id,"source_sha256":caller.source_sha256,
            "selected_source_sha256":origin.checked_metadata["selected_source_sha256"],
            "definition_identity":origin.definition_identity,
            "source_generation":origin.source_generation,
            "constructed_sites":plan.constructed_sites,"static_sites":plan.static_sites,
            "program_indices":plan.program_indices},
        "donors":{
            "native":{"module_id":core_id,"artifact_sha256":core.artifact_sha256,
                "source_sha256":core.source_sha256,
                "span":{"start":core.span.start,"length":core.span.slots,
                    "generation":core.span.generation},
                "entries":if plan.dip { vec![1,2,3] } else { vec![1,2] }},
            "static":{"module_id":caller_id,"artifact_sha256":caller.artifact_sha256,
                "source_sha256":caller.source_sha256,
                "span":{"start":caller.span.start,"length":caller.span.slots,
                    "generation":caller.span.generation},
                "entries":plan.entries}},
        "graph_sha256":crate::workflow::intrinsic::sha256(&graph_bytes),
        "recipe_sha256":recipe_digest(graph)?
    });
    Ok(Some(json!({"descriptor":manifest,
        "manifest_sha256":digest(MANIFEST_DOMAIN,&manifest)?})))
}
