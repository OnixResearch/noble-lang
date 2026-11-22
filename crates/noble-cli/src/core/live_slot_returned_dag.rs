//! Independently bind one source-classified returned Program to its complete
//! retained VM graph and the selected installed code closure. The hashes below
//! are local experimental identities, not portable ProgramValueIds.

use super::super::{Candidate, ProgramContract};
use super::{denied, number, rows, text};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

const DEFINITION_DOMAIN: &[u8] = b"noble returned pure-i64 DAG definition experimental v4\0";
const VALUE_DOMAIN: &[u8] = b"noble returned pure-i64 DAG value experimental v4\0";
const MANIFEST_DOMAIN: &[u8] = b"noble live-slot composite code donors v1\0";
const TEMPLATE_DOMAIN: &[u8] = b"noble returned pure-i64 template projection experimental v1\0";

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

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; the replay records each newly visited Program origin by pushing its pointer onto the shared alias table, so it needs the growable vector."
)]
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

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; replay pushes and pops values on the replayed operand stack, so it needs the growable vector."
)]
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

#[expect(
    tigerstyle::allocation_in_loop,
    reason = "Owner: noble-maintainers; Vec::new allocates no storage; each returned Program is replayed against a fresh empty alias table."
)]
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
    position: usize,
    named: usize,
    interface: Value,
    template: Value,
    captures: Vec<Value>,
    captured_positions: BTreeSet<usize>,
    capture_positions: Vec<usize>,
    classes: Vec<(String, u32)>,
    fixed_nodes: usize,
    aliases: usize,
    semantic_aliases: Vec<Value>,
    semantic_keys: Vec<String>,
    static_indices: BTreeSet<usize>,
    static_keys: BTreeMap<usize, String>,
    static_visiting: BTreeSet<usize>,
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
            position,
            named,
            interface: json!({"input":input_types,"output":output_types,"effects":[]}),
            template: Value::Null,
            captures: Vec::new(),
            captured_positions: BTreeSet::new(),
            capture_positions: Vec::new(),
            classes: Vec::new(),
            fixed_nodes: 0,
            aliases: 0,
            semantic_aliases: Vec::new(),
            semantic_keys: Vec::new(),
            static_indices: BTreeSet::new(),
            static_keys: BTreeMap::new(),
            static_visiting: BTreeSet::new(),
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
        plan.template = plan.template_node(result, 0)?.0;
        plan.classes.sort_by(|left, right| left.0.cmp(&right.0));
        bounded_semantics(&plan.template, 1, &mut 0)?;
        plan.check_template_metadata(origin)?;
        Ok(Some(plan))
    }

    /// Require the independent compiler projection to equal this replay's
    /// derivation exactly: ordered captures, sorted logical binding classes,
    /// occurrence counts and a checked-absent recursive owner. A missing,
    /// forged, truncated, stale or ambiguous projection is refused, so no
    /// identity-bearing target is minted from it.
    fn check_template_metadata(&self, origin: &Value) -> Result<(), String> {
        let templates = rows(origin, "templates")?;
        let outputs = rows(origin, "outputs")?;
        if templates.len() != 1
            || outputs.len() != 1
            || number(&outputs[0], "stack_position")? as usize != self.position
        {
            return Err(denied(
                "compiler target metadata does not describe the checked returned output",
            ));
        }
        let template = &templates[0];
        if template.as_object().is_none_or(|fields| fields.len() != 3) {
            return Err(denied(
                "compiler target template metadata is absent or malformed",
            ));
        }
        let captures = template["captures"]
            .as_array()
            .ok_or_else(|| denied("compiler target captures are absent"))?;
        if captures.len() != self.capture_positions.len()
            || captures
                .iter()
                .zip(&self.capture_positions)
                .any(|(declared, expected)| declared.as_u64() != Some(*expected as u64))
        {
            return Err(denied(
                "compiler target captures differ from independently replayed root bindings",
            ));
        }
        let classes = template["classes"]
            .as_array()
            .ok_or_else(|| denied("compiler target binding classes are absent"))?;
        if classes.len() != self.classes.len() {
            return Err(denied(
                "compiler target binding classes differ from independently replayed bindings",
            ));
        }
        let mut previous: Option<&str> = None;
        for (declared, (binding, occurrences)) in classes.iter().zip(&self.classes) {
            if declared.as_object().is_none_or(|fields| fields.len() != 2) {
                return Err(denied("compiler target binding class is malformed"));
            }
            let declared_binding = text(declared, "binding")?;
            if declared_binding != binding
                || previous.is_some_and(|prior| prior >= declared_binding)
            {
                return Err(denied(
                    "compiler target binding classes are forged, stale or ambiguous",
                ));
            }
            previous = Some(declared_binding);
            if number(declared, "occurrences")? != *occurrences {
                return Err(denied(
                    "compiler target binding occurrence count differs from the retained graph",
                ));
            }
        }
        let owner = template["owner"]
            .as_object()
            .ok_or_else(|| denied("compiler target recursive owner is absent"))?;
        if owner.len() != 1 || owner["kind"] != "absent" {
            return Err(denied(
                "returned target requires an unclassified lexical recursive owner",
            ));
        }
        Ok(())
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

    /// Canonical normalized recipe spelling of one static quotation body. The
    /// class follows the normalized recipe content, not the compiled program
    /// index, so two equal quotation bodies share one logical binding.
    fn static_program_key(
        programs: &[Value],
        index: usize,
        visiting: &mut BTreeSet<usize>,
        memo: &mut BTreeMap<usize, String>,
        depth: usize,
    ) -> Result<String, String> {
        if let Some(key) = memo.get(&index) {
            return Ok(key.clone());
        }
        if depth >= 32 || !visiting.insert(index) {
            return Err(denied("recursive or unbounded static quotation binding"));
        }
        let program = programs
            .get(index)
            .ok_or_else(|| denied("static quotation is outside checked program map"))?;
        let operations = rows(program, "operations")?;
        if program["source_artifact"] != "definition"
            || program["effect_mask"] != 0
            || operations.len() > 128
        {
            return Err(denied(
                "static quotation binding is foreign, effectful or over bound",
            ));
        }
        let mut key = format!(
            "p{}:{}[",
            number(program, "input_signature")?,
            number(program, "output_signature")?
        );
        for (at, operation) in operations.iter().enumerate() {
            if at > 0 {
                key.push(';');
            }
            match text(operation, "kind")? {
                "i64" => {
                    key.push('#');
                    key.push_str(&signed(text(operation, "value")?)?);
                }
                "static-program" => key.push_str(&Self::static_program_key(
                    programs,
                    number(operation, "static_program_index")? as usize,
                    visiting,
                    memo,
                    depth + 1,
                )?),
                "quote-i64" | "quote-program" => key.push_str("quote"),
                kind @ ("add" | "sub" | "mul" | "dup" | "drop" | "swap" | "dip" | "run"
                | "compose") => key.push_str(kind),
                _ => {
                    return Err(denied(
                        "static quotation has an unclassified binding operation",
                    ))
                }
            }
        }
        key.push(']');
        visiting.remove(&index);
        memo.insert(index, key.clone());
        Ok(key)
    }

    /// Canonical structural spelling of a fixed I64 source recipe. It is the
    /// same spelling the independent compiler projection uses, and it never
    /// contains a source span or NodeId.
    fn recipe_key(recipe: &Value, depth: usize, nodes: &mut usize) -> Result<String, String> {
        *nodes = nodes
            .checked_add(1)
            .ok_or_else(|| denied("fixed expression size overflow"))?;
        if depth > 32 || *nodes > 128 {
            return Err(denied("returned fixed expression exceeds bounded source"));
        }
        match text(recipe, "kind")? {
            "literal" => Ok(format!("#{}", signed(text(recipe, "value")?)?)),
            kind @ ("add" | "sub" | "mul") => Ok(format!(
                "{kind}({},{})",
                Self::recipe_key(&recipe["left"], depth + 1, nodes)?,
                Self::recipe_key(&recipe["right"], depth + 1, nodes)?
            )),
            _ => Err(denied("unsupported fixed source recipe")),
        }
    }

    /// Intern one constructed occurrence's logical binding class. Physical VM
    /// sharing never changes a class: a reference returns the first occurrence's
    /// key instead of adding an occurrence.
    fn intern_class(&mut self, key: &str) -> Result<(), String> {
        if let Some(class) = self.classes.iter_mut().find(|class| class.0 == key) {
            class.1 = class
                .1
                .checked_add(1)
                .ok_or_else(|| denied("returned binding occurrence overflow"))?;
            return Ok(());
        }
        if self.classes.len() >= 32 {
            return Err(denied("returned binding classes exceed graph bound"));
        }
        self.classes.push((key.to_owned(), 1));
        Ok(())
    }

    fn template_node(&mut self, source: &Value, depth: usize) -> Result<(Value, String), String> {
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
            let key = self
                .semantic_keys
                .get(alias)
                .ok_or_else(|| denied("returned Program reference has no binding class"))?;
            return Ok((prior.clone(), key.clone()));
        }
        if self.aliases >= 32 {
            return Err(denied("returned source DAG exceeds cell bound"));
        }
        if alias != self.aliases {
            return Err(denied("returned source alias order differs from DFS"));
        }
        self.aliases += 1;
        self.semantic_aliases.push(Value::Null);
        self.semantic_keys.push(String::new());
        let (normalized, key) = match kind {
            "static-program" => {
                let site = self.source_site(source, kind)?;
                let index = number(source, "program_index")? as usize;
                if index == self.named {
                    return Err(denied(
                        "returned quotation requires an unclassified lexical owner",
                    ));
                }
                self.static_indices.insert(index);
                self.static_sites
                    .push(json!({"program_index":index,"node":number(site,"node")?,
                    "table_entry":number(site,"table_entry")?}));
                self.static_graph(index)?;
                let body = self.static_body(index, &mut BTreeSet::new())?;
                let binding = Self::static_program_key(
                    self.programs,
                    index,
                    &mut self.static_visiting,
                    &mut self.static_keys,
                    0,
                )?;
                (json!({"kind":kind,"program":body}), binding)
            }
            "quote-i64" => {
                self.charge_graph(3)?;
                let site = self.source_site(source, kind)?;
                self.constructed_sites
                    .push(json!({"kind":kind,"node":number(site,"node")?,
                    "table_entry":number(site,"table_entry")?}));
                let operand = &source["operand"];
                let (operand, binding) = match text(operand, "kind")? {
                    "root-i64" => {
                        let index = number(operand, "position")? as usize;
                        let input = self
                            .inputs
                            .get(index)
                            .ok_or_else(|| denied("quoted root binder does not exist"))?;
                        if self.captured_positions.insert(index) {
                            self.capture_positions.push(index);
                            self.captures.push(json!({"type":"I64",
                                "value":canonical_i64(text(input,"value")?)?}));
                        }
                        (
                            json!({"kind":"root-i64","position":index}),
                            format!("r{index}"),
                        )
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
                        let binding = Self::recipe_key(&recipe, 0, &mut 0)?;
                        (
                            json!({"kind":"fixed-i64","value":value,"recipe":recipe}),
                            binding,
                        )
                    }
                    _ => return Err(denied("foreign Program quote I64 operand")),
                };
                (
                    json!({"kind":kind,"operand":operand}),
                    format!("q({binding})"),
                )
            }
            "quote-program" => {
                self.charge_graph(2)?;
                let site = self.source_site(source, kind)?;
                self.constructed_sites
                    .push(json!({"kind":kind,"node":number(site,"node")?,
                    "table_entry":number(site,"table_entry")?}));
                let (operand, binding) = self.template_node(&source["operand"], depth + 1)?;
                (
                    json!({"kind":kind,"operand":operand}),
                    format!("q({binding})"),
                )
            }
            "compose" => {
                self.charge_graph(2)?;
                let site = self.source_site(source, kind)?;
                self.constructed_sites
                    .push(json!({"kind":kind,"node":number(site,"node")?,
                    "table_entry":number(site,"table_entry")?}));
                let (left, left_key) = self.template_node(&source["left"], depth + 1)?;
                let (right, right_key) = self.template_node(&source["right"], depth + 1)?;
                (
                    json!({"kind":kind,"left":left,"right":right}),
                    format!("c({left_key},{right_key})"),
                )
            }
            _ => return Err(denied("unsupported constructed returned Program")),
        };
        bounded_semantics(&normalized, 1, &mut 0)?;
        self.intern_class(&key)?;
        self.semantic_aliases[alias] = normalized.clone();
        self.semantic_keys[alias] = key.clone();
        Ok((normalized, key))
    }

    fn identity(&self) -> Result<Value, String> {
        let owner = json!({"kind":"absent"});
        let descriptor = json!({"schema":"noble-returned-pure-i64-dag/experimental-v4",
            "interface":self.interface,"template":self.template,
            "lexical_recursive_owner":owner});
        let definition_id = format!(
            "anonymous-template-experimental-v4-sha256:{}",
            digest(DEFINITION_DOMAIN, &descriptor)?
        );
        let program_value_id = format!(
            "anonymous-program-experimental-v4-sha256:{}",
            digest(
                VALUE_DOMAIN,
                &json!({"definition_id":definition_id,"captures":self.captures})
            )?
        );
        let classes = self
            .classes
            .iter()
            .map(|(binding, occurrences)| json!({"binding":binding,"occurrences":occurrences}))
            .collect::<Vec<_>>();
        Ok(
            json!({"definition_id":definition_id,"program_value_id":program_value_id,
            "captures":self.captures,"interface":self.interface,"effects":[],
            "dependencies":self.dependencies,
            "lexical_recursive_owner":owner,"template_classes":classes,
            "artifact_sha256":null}),
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

/// The closed Noble quotation a verified returned target instantiates: its
/// template with every quoted root input replaced by the captured value. It is
/// the subject a selected-target claim must bind (LSLOT-05); `None` when the
/// target is outside the verified template subset.
pub(in crate::core::live_slot) fn anonymous_target_subject(
    origin: &Value,
    contracts: &[ProgramContract],
    position: usize,
    descriptor: &str,
    saved: &Value,
    graph: &Value,
    inputs: &[Value],
) -> Result<Option<String>, String> {
    let Some(plan) = Plan::new(origin, contracts, position, descriptor, inputs)? else {
        return Ok(None);
    };
    Graph::new(graph)?.validate(&plan, saved)?;
    let mut subject = String::from("[");
    plan.subject_node(&plan.template, 0, &mut subject)?;
    subject.push_str(" ]");
    Ok(Some(subject))
}

impl Plan<'_> {
    /// Append the code of one template node: a composition is its left code
    /// followed by its right code, a quoted I64 pushes that value, a quoted
    /// Program pushes that Program, and a static Program is its checked body.
    #[expect(
        tigerstyle::borrowed_argument_types,
        reason = "Owner: noble-maintainers; the renderer appends to the caller's growing subject text, which requires String growth rather than a &str."
    )]
    fn subject_node(&self, node: &Value, depth: usize, out: &mut String) -> Result<(), String> {
        if depth > 32 {
            return Err(denied("returned template is too deep to render"));
        }
        match text(node, "kind")? {
            "compose" => {
                self.subject_node(&node["left"], depth + 1, out)?;
                self.subject_node(&node["right"], depth + 1, out)
            }
            "quote-i64" => {
                let operand = &node["operand"];
                let value = match text(operand, "kind")? {
                    "root-i64" => {
                        let index = number(operand, "position")? as usize;
                        let input = self
                            .inputs
                            .get(index)
                            .ok_or_else(|| denied("quoted root binder does not exist"))?;
                        canonical_i64(text(input, "value")?)?
                    }
                    "fixed-i64" => text(operand, "value")?.to_owned(),
                    _ => return Err(denied("foreign Program quote I64 operand")),
                };
                out.push(' ');
                out.push_str(&value);
                Ok(())
            }
            "quote-program" => {
                out.push_str(" [");
                self.subject_node(&node["operand"], depth + 1, out)?;
                out.push_str(" ]");
                Ok(())
            }
            "static-program" => subject_body(&node["program"], depth + 1, out),
            _ => Err(denied("unsupported returned template node")),
        }
    }
}

/// Append the words of one checked static body.
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; the renderer appends to the caller's growing subject text, which requires String growth rather than a &str."
)]
fn subject_body(body: &Value, depth: usize, out: &mut String) -> Result<(), String> {
    if depth > 32 {
        return Err(denied("returned template is too deep to render"));
    }
    for operation in rows(body, "operations")? {
        let word = match text(operation, "kind")? {
            "i64" => text(operation, "value")?,
            "static-program" => {
                out.push_str(" [");
                subject_body(&operation["program"], depth + 1, out)?;
                out.push_str(" ]");
                continue;
            }
            "add" => "+",
            "sub" => "-",
            "mul" => "*",
            "quote-i64" | "quote-program" => "quote",
            kind @ ("dup" | "drop" | "swap" | "dip" | "run" | "compose") => kind,
            _ => {
                return Err(denied(
                    "unsupported static operation in a selected-target subject",
                ))
            }
        };
        out.push(' ');
        out.push_str(word);
    }
    Ok(())
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
    let template_projection = json!({
        "interface":plan.interface,
        "template":plan.template,
        "lexical_recursive_owner":identity["lexical_recursive_owner"],
        "captures":plan.captures,
        "template_classes":identity["template_classes"],
    });
    let manifest = json!({
        "schema":"noble-live-slot-composite-code-donors/v1",
        "owner":{"token":owner,"handle":handle},
        "identity":{"definition_id":identity["definition_id"],
            "program_value_id":identity["program_value_id"],
            "captures":identity["captures"],"interface":identity["interface"],
            "lexical_recursive_owner":identity["lexical_recursive_owner"],
            "template_classes":identity["template_classes"]},
        "source":{"caller_id":caller_id,"source_sha256":caller.source_sha256,
            "selected_source_sha256":origin.checked_metadata["selected_source_sha256"],
            "definition_identity":origin.definition_identity,
            "source_generation":origin.source_generation,
            "constructed_sites":plan.constructed_sites,"static_sites":plan.static_sites,
            "program_indices":plan.program_indices,
            "template_captures":plan.capture_positions,
            "template_classes":identity["template_classes"]},
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
        "template_sha256":digest(TEMPLATE_DOMAIN, &template_projection)?,
        "graph_sha256":crate::workflow::intrinsic::sha256(&graph_bytes),
        "recipe_sha256":recipe_digest(graph)?
    });
    Ok(Some(json!({"descriptor":manifest,
        "manifest_sha256":digest(MANIFEST_DOMAIN,&manifest)?})))
}

#[cfg(test)]
mod tests {
    use super::*;
    use noble_kernel::types::Ty;

    const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
        bytes: 65_536,
        nodes: 16_384,
        depth: 64,
        work: 2_000_000,
    };

    /// One independently prepared selected builder plus the exact origin
    /// metadata, contract map, output descriptor and runtime root inputs that
    /// identity-bearing admission consumes.
    struct Selected {
        origin: Value,
        contracts: Vec<ProgramContract>,
        descriptor: String,
        inputs: Vec<Value>,
    }

    fn selected(source: &[u8], roots: &[Ty], values: &[&str]) -> Result<Selected, String> {
        let environment = noble_kernel::contracts::environment()
            .map_err(|error| format!("environment: {error:?}"))?
            .enable_live_slots();
        let mut session = noble_contracts::source::Session::new_live_slots(environment)
            .map_err(|error| error.diagnostic().message.clone())?;
        let definition = session
            .prepare(source, &[], LIMITS)
            .map_err(|error| error.diagnostic().message.clone())?;
        session
            .commit(definition)
            .map_err(|error| error.diagnostic().message.clone())?;
        let call = session
            .prepare(b"builder", roots, LIMITS)
            .map_err(|error| error.diagnostic().message.clone())?;
        let target = session
            .checked_selected_target(&call, "builder")
            .map_err(|error| format!("selected source: {error:?}"))?;
        let prepared = noble_wasm::source::Compiler::new_live_slots()
            .prepare_checked_selected(&target)
            .map_err(|_| "selected compiler refused checked source".to_owned())?;
        let bound = prepared
            .selected_target()
            .ok_or("missing checked selection")?;
        let binding = json!({
            "definition_index":bound.definition_id.index(),
            "definition_identity":bound.definition_id.identity().to_string(),
            "source_generation":bound.source_generation.to_string(),
            "named_program_index":bound.named_program_index,
            "selected_source_sha256":crate::workflow::intrinsic::sha256(target.source())
        });
        let replay = super::super::super::lexical_replay_json(&target)?;
        let origin = super::super::super::selected_origin_json(
            prepared.selected_origin().ok_or("missing checked origin")?,
            &binding,
            &replay,
        )?;
        let contracts = prepared
            .program_metadata()
            .iter()
            .map(|program| {
                Ok(super::super::super::ProgramContract {
                    input: super::super::super::canonical_stack(&program.stack_in, 0)?,
                    output: super::super::super::canonical_stack(&program.stack_out, 0)?,
                    effects: super::super::super::effects(&program.effects)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let named = prepared
            .named_targets()
            .first()
            .ok_or("missing checked named target")?;
        let position = number(&origin["outputs"][0], "stack_position")? as usize;
        let descriptor = super::super::super::canonical_stack(&named.stack_out, 0)?
            .into_iter()
            .nth(position)
            .ok_or("named target has no returned descriptor")?;
        if values.len() != roots.len() {
            return Err("fixture root value count differs from its interface".into());
        }
        Ok(Selected {
            origin,
            contracts,
            descriptor,
            inputs: values
                .iter()
                .map(|value| json!({"kind":"i64","value":value}))
                .collect(),
        })
    }

    impl Selected {
        /// The checked returned-Program stack position of this fixture.
        fn position(&self) -> Result<usize, String> {
            Ok(number(&self.origin["outputs"][0], "stack_position")? as usize)
        }

        fn plan(&self) -> Result<Plan<'_>, String> {
            Plan::new(
                &self.origin,
                &self.contracts,
                self.position()?,
                &self.descriptor,
                &self.inputs,
            )?
            .ok_or_else(|| "checked fixture became opaque".to_owned())
        }
    }

    fn class(plan: &Plan<'_>, binding: &str) -> Option<u32> {
        plan.classes
            .iter()
            .find(|class| class.0 == binding)
            .map(|class| class.1)
    }

    #[test]
    fn returned_target_binds_verified_logical_alias_classes() -> Result<(), String> {
        let aliased = selected(
            b"def builder [ drop dup quote swap quote compose [ + + + ] compose ]",
            &[Ty::I64, Ty::I64],
            &["2", "2"],
        )?;
        let renamed = selected(
            b"def builder [ swap swap drop dup quote swap quote compose [ + + + ] compose ]",
            &[Ty::I64, Ty::I64],
            &["2", "2"],
        )?;
        let independent = selected(
            b"def builder [ quote swap quote compose [ + + + ] compose ]",
            &[Ty::I64, Ty::I64],
            &["2", "2"],
        )?;
        let same = aliased.plan()?;
        let renamed = renamed.plan()?;
        let distinct = independent.plan()?;
        // The same immutable root binding quoted twice is one logical class,
        // independent of the two distinct quote executions (two VM cells).
        if same.capture_positions != [0] || class(&same, "q(r0)") != Some(2) {
            return Err(format!(
                "same-binding duplication lost its logical alias class: {:?} {:?}",
                same.capture_positions, same.classes
            ));
        }
        // Two independent bindings with equal value bits stay separate slots.
        if distinct.capture_positions != [1, 0]
            || class(&distinct, "q(r1)") != Some(1)
            || class(&distinct, "q(r0)") != Some(1)
        {
            return Err(format!(
                "independent equal-valued bindings were merged: {:?} {:?}",
                distinct.capture_positions, distinct.classes
            ));
        }
        let same_identity = same.identity()?;
        let renamed_identity = renamed.identity()?;
        let distinct_identity = distinct.identity()?;
        if same_identity["definition_id"] != renamed_identity["definition_id"]
            || same_identity["program_value_id"] != renamed_identity["program_value_id"]
        {
            return Err("renaming recycled one logical binding into a new template".into());
        }
        if same_identity["definition_id"] == distinct_identity["definition_id"]
            || same_identity["program_value_id"] == distinct_identity["program_value_id"]
        {
            return Err("independent equal-valued bindings shared a target identity".into());
        }
        if same_identity["lexical_recursive_owner"] != json!({"kind":"absent"}) {
            return Err("supported target lacks a verified absent recursive owner".into());
        }
        // Physical VM sharing may change the occurrence count but never the
        // logical binding, the captures or the target identity.
        let shared_fixture = selected(
            b"def builder [ quote [ + ] compose let q q q compose [ 0 + ] compose ]",
            &[Ty::I64],
            &["2"],
        )?;
        let shared = shared_fixture.plan()?;
        let fresh_fixture = selected(
            b"def builder [ dup quote [ + ] compose swap quote [ + ] compose compose [ 0 + ] compose ]",
            &[Ty::I64],
            &["2"],
        )?;
        let fresh = fresh_fixture.plan()?;
        let shared_inner = shared
            .classes
            .iter()
            .filter(|class| class.0.starts_with("c(q(r0)"))
            .map(|class| class.1)
            .sum::<u32>();
        let fresh_inner = fresh
            .classes
            .iter()
            .filter(|class| class.0.starts_with("c(q(r0)"))
            .map(|class| class.1)
            .sum::<u32>();
        if shared_inner != 1 || fresh_inner != 2 {
            return Err(format!(
                "VM sharing was not observed distinctly: {shared_inner} {fresh_inner}"
            ));
        }
        let shared_identity = shared.identity()?;
        let fresh_identity = fresh.identity()?;
        if shared_identity["definition_id"] != fresh_identity["definition_id"]
            || shared_identity["program_value_id"] != fresh_identity["program_value_id"]
        {
            return Err("physical Program sharing changed the target identity".into());
        }
        if shared.capture_positions != fresh.capture_positions {
            return Err("physical Program sharing changed the ordered captures".into());
        }
        Ok(())
    }

    #[test]
    fn returned_target_distinguishes_fixed_computed_and_root_operands() -> Result<(), String> {
        struct Projection {
            identity: Value,
            classes: Vec<(String, u32)>,
        }
        fn projection(source: &[u8], roots: &[Ty], values: &[&str]) -> Result<Projection, String> {
            let fixture = selected(source, roots, values)?;
            let plan = fixture.plan()?;
            Ok(Projection {
                identity: plan.identity()?,
                classes: plan.classes,
            })
        }
        let varying2 = projection(b"def builder [ quote [ + ] compose ]", &[Ty::I64], &["2"])?;
        let varying3 = projection(b"def builder [ quote [ + ] compose ]", &[Ty::I64], &["3"])?;
        if varying2.identity["definition_id"] != varying3.identity["definition_id"]
            || varying2.identity["program_value_id"] == varying3.identity["program_value_id"]
        {
            return Err("varying root input 2->3 did not retain D while changing P".into());
        }
        let fixed2 = projection(b"def builder [ 2 quote [ + ] compose ]", &[Ty::I64], &["3"])?;
        let fixed3 = projection(b"def builder [ 3 quote [ + ] compose ]", &[Ty::I64], &["3"])?;
        if fixed2.identity["definition_id"] == fixed3.identity["definition_id"]
            || fixed2.identity["program_value_id"] == fixed3.identity["program_value_id"]
        {
            return Err("fixed literal 2->3 did not change the target identity".into());
        }
        if !fixed2.identity["captures"]
            .as_array()
            .is_some_and(|captures| captures.is_empty())
        {
            return Err("a fixed literal became a runtime capture slot".into());
        }
        let computed2 = projection(
            b"def builder [ 1 1 + quote [ + ] compose ]",
            &[Ty::I64],
            &["3"],
        )?;
        let computed3 = projection(
            b"def builder [ 1 2 + quote [ + ] compose ]",
            &[Ty::I64],
            &["3"],
        )?;
        if computed2.identity["definition_id"] == computed3.identity["definition_id"]
            || computed2.identity["program_value_id"] == computed3.identity["program_value_id"]
        {
            return Err("checked computed 2->3 did not change the target identity".into());
        }
        if fixed2.identity["definition_id"] == computed2.identity["definition_id"] {
            return Err("literal 2 and computed 1+1 shared one template".into());
        }
        let quote_class = |classes: &[(String, u32)], binding: &str| -> Option<u32> {
            classes
                .iter()
                .find(|class| class.0 == binding)
                .map(|class| class.1)
        };
        if quote_class(&fixed2.classes, "q(#2)") != Some(1)
            || quote_class(&computed2.classes, "q(add(#1,#1))") != Some(1)
            || quote_class(&varying2.classes, "q(r0)") != Some(1)
            || quote_class(&fixed2.classes, "q(add(#1,#1))").is_some()
        {
            return Err(format!(
                "binding classes lost fixed/computed/root provenance: {:?} {:?} {:?}",
                fixed2.classes, computed2.classes, varying2.classes
            ));
        }
        Ok(())
    }

    #[test]
    fn returned_target_refuses_absent_forged_truncated_and_ambiguous_template_metadata(
    ) -> Result<(), String> {
        let base = selected(b"def builder [ quote [ + ] compose ]", &[Ty::I64], &["2"])?;
        Plan::new(
            &base.origin,
            &base.contracts,
            0,
            &base.descriptor,
            &base.inputs,
        )?
        .ok_or("baseline target became opaque")?;
        let refused = |origin: &Value| -> bool {
            !matches!(
                Plan::new(origin, &base.contracts, 0, &base.descriptor, &base.inputs),
                Ok(Some(_))
            )
        };
        let mut absent = base.origin.clone();
        absent
            .as_object_mut()
            .ok_or("origin is not a structure")?
            .remove("templates");
        if !refused(&absent) {
            return Err("absent compiler template metadata acquired an identity".into());
        }
        let mut truncated = base.origin.clone();
        truncated["templates"][0]["captures"] = json!([]);
        if !refused(&truncated) {
            return Err("truncated compiler captures acquired an identity".into());
        }
        let mut forged_capture = base.origin.clone();
        forged_capture["templates"][0]["captures"] = json!([1]);
        if !refused(&forged_capture) {
            return Err("forged compiler capture position acquired an identity".into());
        }
        let mut forged_binding = base.origin.clone();
        forged_binding["templates"][0]["classes"][0]["binding"] = json!("q(#2)");
        if !refused(&forged_binding) {
            return Err("forged logical binding class acquired an identity".into());
        }
        let mut forged_occurrences = base.origin.clone();
        forged_occurrences["templates"][0]["classes"][0]["occurrences"] = json!(2);
        if !refused(&forged_occurrences) {
            return Err("forged occurrence count acquired an identity".into());
        }
        let mut recursive = base.origin.clone();
        recursive["templates"][0]["owner"] = json!({"kind":"self-named"});
        if !refused(&recursive) {
            return Err("unclassified lexical recursive owner acquired an identity".into());
        }
        let mut ambiguous = base.origin.clone();
        let mut duplicated = ambiguous["templates"][0]["classes"]
            .as_array()
            .ok_or("missing classes")?
            .clone();
        duplicated.push(duplicated[0].clone());
        ambiguous["templates"][0]["classes"] = json!(duplicated);
        if !refused(&ambiguous) {
            return Err("a duplicated logical binding class acquired an identity".into());
        }
        // A well-formed projection from another checked selection is stale
        // here: its binding classes describe different source bindings.
        let other = selected(
            b"def builder [ drop dup quote swap quote compose [ + + + ] compose ]",
            &[Ty::I64, Ty::I64],
            &["2", "2"],
        )?;
        let mut stale = base.origin.clone();
        stale["templates"] = other.origin["templates"].clone();
        if !refused(&stale) {
            return Err("stale cross-selection template metadata acquired an identity".into());
        }
        Ok(())
    }
}
