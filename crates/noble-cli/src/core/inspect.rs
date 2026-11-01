#![allow(
    tigerstyle::missing_const_fn,
    tigerstyle::no_unwrap,
    reason = "Owner: noble-maintainers; the inspection report walks bounded JSON observations and refuses every malformed shape explicitly, so checked unwraps and flat helpers are the readable form."
)]
//! A bounded, inert inspection report over an **already observed** checked Program (issue #76).
//!
//! The report is presentation only: it reads the stack observation the REPL already captured when a
//! line was submitted, and never installs, runs or re-checks anything. `guest_requests` and
//! `protected_operations` are always zero because this module cannot reach a runtime at all.
//!
//! Four properties are made explicit rather than inferred:
//!
//! - **Selection** is by stack index and then by recipe-node path, and every way a path can fail to
//!   select a Program is its own outcome: no observation, out of range, a stack entry that is not a
//!   Program, a node that is not a quotation.
//! - **Generation scope.** The current namespace generation is reported beside the observation's own
//!   `source_epoch` and `source_store_id`. A saved observation keeps its own epoch; it is never
//!   presented as current merely because the generation advanced.
//! - **Dependency identity** is the exact session-local definition id the observation carries, never
//!   re-resolved through today's same-named definition, and its provenance says plainly when no
//!   retained metadata is carried by this observation.
//! - **Evidence** is `not-checked`, not failure and not success: no evidence is bound to this
//!   observation, and the four fields a bound subject would need are named as empty lists.
//!
//! Traversal is bounded in depth, recipe nodes, witnesses and dependencies. Exceeding a bound is
//! reported in `truncated` and the report still returns: a truncated traversal is never presented
//! as complete.

use serde_json::Value;

/// Report schema emitted by `:inspect`.
pub(crate) const SCHEMA: &str = "noble-live-inspection/v1";

/// Deepest quotation nesting an inspection descends to.
const MAX_DEPTH: usize = 8;
/// Most recipe nodes one report carries.
const MAX_NODES: usize = 512;
/// Most per-node interface witnesses, dependency entries and effects one report carries.
const MAX_WITNESSES: usize = 128;

/// Why a path did not select a Program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Outcome {
    Selected,
    NoObservation,
    PathMalformed,
    OutOfRange,
    EntryIsNotProgram,
    NodeOutOfRange,
    NodeIsNotQuotation,
    TooDeep,
}

impl Outcome {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Selected => "selected",
            Self::NoObservation => "no-observation",
            Self::PathMalformed => "path-malformed",
            Self::OutOfRange => "out-of-range",
            Self::EntryIsNotProgram => "stack-entry-not-a-program",
            Self::NodeOutOfRange => "node-out-of-range",
            Self::NodeIsNotQuotation => "node-not-a-quotation",
            Self::TooDeep => "quotation-depth-bound",
        }
    }
}

/// Parsed inspection path: a stack index and then recipe-node indices.
fn parse(path: &str) -> Option<(usize, Vec<usize>)> {
    if path.is_empty() {
        return None;
    }
    let mut segments = path.split('.');
    let root = segments.next()?.parse::<usize>().ok()?;
    let mut nodes = Vec::new();
    for segment in segments {
        nodes.push(segment.parse::<usize>().ok()?);
    }
    Some((root, nodes))
}

fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    value.get(name).unwrap_or(&Value::Null)
}

/// One selected Program, borrowable straight out of the observation.
struct Selection<'a> {
    observed: &'a Value,
    interface: &'a Value,
    recipe: &'a Value,
    witnesses: &'a Value,
}

/// Walk the node path through nested quotations. The first failure wins and names its own outcome.
/// Every reference returned borrows from `stack`, so no observation is copied or kept alive.
fn select<'a>(stack: &'a Value, path: &str) -> (Outcome, Option<Selection<'a>>) {
    if stack.as_array().is_some_and(Vec::is_empty) {
        return (Outcome::NoObservation, None);
    }
    let Some((root, nodes)) = parse(path) else {
        return (Outcome::PathMalformed, None);
    };
    let Some(entry) = stack.get(root) else {
        return (Outcome::OutOfRange, None);
    };
    if entry.get("type").and_then(Value::as_str) != Some("Program") {
        return (Outcome::EntryIsNotProgram, None);
    }
    if nodes.len() > MAX_DEPTH {
        return (Outcome::TooDeep, None);
    }
    let mut recipe = field(entry, "recipe");
    let mut interface = field(entry, "interface");
    let mut witnesses = field(entry, "witnesses");
    for node in nodes {
        let Some(child) = recipe.get(node) else {
            return (Outcome::NodeOutOfRange, None);
        };
        let Some(quotation) = child.get("quotation") else {
            return (Outcome::NodeIsNotQuotation, None);
        };
        recipe = quotation;
        interface = field(child, "interface");
        witnesses = field(child, "witnesses");
    }
    (
        Outcome::Selected,
        Some(Selection {
            observed: entry,
            interface,
            recipe,
            witnesses,
        }),
    )
}

/// Entries of a JSON array, empty for anything else.
fn entries(value: &Value) -> &[Value] {
    value.as_array().map_or(&[], Vec::as_slice)
}

/// A dependency every recipe entry names, with the exact identity the observation carries.
fn dependencies(recipe: &Value) -> (Vec<Value>, Option<Value>) {
    let all: Vec<(usize, &str)> = entries(recipe)
        .iter()
        .enumerate()
        .filter_map(|(node, entry)| {
            let identity = entry.get("invoke").and_then(Value::as_str)?;
            Some((node, identity.strip_prefix("definition:")?))
        })
        .filter(|(_, identity)| !identity.is_empty())
        .collect();
    let record = (all.len() > MAX_WITNESSES).then(|| {
        serde_json::json!({
            "kind": "dependencies",
            "kept": MAX_WITNESSES,
            "total": all.len(),
        })
    });
    let named = all.iter()
        .take(MAX_WITNESSES)
        .map(|(node, identity)| {
            serde_json::json!({
                "node": node,
                "identity": { "kind": "session-local-definition-id", "value": identity },
                "provenance": {
                    "status": "unavailable-origin",
                    "reason": "this observation carries no retained source metadata for that identity",
                },
            })
        })
        .collect();
    (named, record)
}

/// The ordered effects an interface declares followed by those its witnesses name, so a Program
/// whose interface ceiling is wider than any single witness still reports that ceiling.
fn effects(interface: &Value, witnesses: &Value) -> Vec<Value> {
    let mut ordered: Vec<Value> = Vec::new();
    for effect in entries(field(interface, "effects")) {
        if !ordered.iter().any(|seen| seen == effect) {
            ordered.push(effect.clone());
        }
    }
    for entry in entries(witnesses).iter().take(MAX_WITNESSES) {
        for effect in entries(field(entry, "effects")) {
            if !ordered.iter().any(|seen| seen == effect) {
                ordered.push(effect.clone());
            }
        }
    }
    ordered
}

/// Bound an array, recording the bound that fired.
fn bounded(value: &Value, limit: usize, kind: &str) -> (Vec<Value>, Option<Value>) {
    let all = entries(value);
    let record = (all.len() > limit)
        .then(|| serde_json::json!({ "kind": kind, "kept": limit, "total": all.len() }));
    (all.iter().take(limit).cloned().collect(), record)
}

/// Build the bounded inspection report for one path over one observation.
pub(crate) fn report(stack: &Value, path: &str, generation: u64, engine: &str) -> Value {
    let mut truncated: Vec<Value> = Vec::new();
    let (outcome, selected) = select(stack, path);
    let (root, node_path) = parse(path).unwrap_or((usize::MAX, Vec::new()));
    let stack_index = if root == usize::MAX {
        Value::Null
    } else {
        serde_json::json!(root)
    };
    let mut document = serde_json::json!({
        "schema": SCHEMA,
        "stage": "inspection",
        "outcome": outcome.as_str(),
        "generation": generation,
        "engine": engine,
        "guest_requests": 0,
        "protected_operations": 0,
        "selection": { "path": path, "stack_index": stack_index, "node_path": node_path },
        "observed": Value::Null,
        "program": Value::Null,
        "evidence": {
            "status": "not-checked",
            "reason": "no evidence is bound to this observation; this is neither a proof failure nor a proof success",
            "subjects": [],
            "claims": [],
            "context": [],
            "assumptions": [],
        },
        "truncated": truncated.clone(),
    });
    let Some(program) = selected else {
        document["truncated"] = serde_json::json!(truncated);
        return document;
    };
    let (recipe, recipe_record) = bounded(program.recipe, MAX_NODES, "recipe-nodes");
    let (witnesses, witness_record) = bounded(program.witnesses, MAX_WITNESSES, "witnesses");
    let (named, dependency_record) = dependencies(program.recipe);
    for record in [recipe_record, witness_record, dependency_record]
        .into_iter()
        .flatten()
    {
        truncated.push(record);
    }
    let total = entries(program.recipe).len();
    document["observed"] = serde_json::json!({
        "stack_handle": field(program.observed, "handle"),
        "source_epoch": field(program.observed, "source_epoch"),
        "source_store_id": field(program.observed, "source_store_id"),
        "generation_scope": "observation-not-current-namespace",
        "dependency_identity_scope": "session-local, not portable content identity or authority",
    });
    document["program"] = serde_json::json!({
        "interface": program.interface,
        "effects": effects(program.interface, program.witnesses),
        "nodes": {
            "total": total,
            "kept": recipe.len(),
            "depth_bound": MAX_DEPTH,
            "node_bound": MAX_NODES,
        },
        "recipe": recipe,
        "witnesses": witnesses,
        "dependencies": named,
    });
    document["truncated"] = serde_json::json!(truncated);
    document
}

#[cfg(test)]
mod tests {
    use super::{report, MAX_NODES, MAX_WITNESSES};

    fn program(recipe: serde_json::Value, witnesses: serde_json::Value) -> serde_json::Value {
        serde_json::json!([{
            "handle": 12,
            "source_epoch": 3,
            "source_store_id": "store-1",
            "type": "Program",
            "interface": {
                "stack_in": { "values": ["I64"], "depth": 1 },
                "stack_out": { "values": [], "depth": 0 },
                "effects": [],
            },
            "recipe": recipe,
            "witnesses": witnesses,
        }])
    }

    fn literal(value: &str) -> serde_json::Value {
        serde_json::json!({ "literal": { "type": "I64", "value": value } })
    }

    #[test]
    fn a_selected_program_keeps_its_observation_epoch_and_names_dependencies() {
        let stack = program(
            serde_json::json!([literal("20"), { "invoke": "definition:7" }]),
            serde_json::json!([]),
        );
        let inspected = report(&stack, "0", 9, "v8");
        assert_eq!(inspected["outcome"], "selected");
        assert_eq!(inspected["generation"], 9);
        assert_eq!(
            inspected["observed"]["source_epoch"], 3,
            "observation epoch"
        );
        assert_eq!(inspected["observed"]["stack_handle"], 12);
        assert_eq!(
            inspected["program"]["dependencies"][0]["identity"]["value"],
            "7"
        );
        assert_eq!(
            inspected["program"]["dependencies"][0]["provenance"]["status"],
            "unavailable-origin"
        );
        assert_eq!(inspected["evidence"]["status"], "not-checked");
        assert_eq!(inspected["guest_requests"], 0);
    }

    #[test]
    fn every_failed_selection_has_its_own_outcome() {
        let stack = program(serde_json::json!([literal("1")]), serde_json::json!([]));
        assert_eq!(report(&stack, "", 1, "v8")["outcome"], "path-malformed");
        assert_eq!(report(&stack, "x", 1, "v8")["outcome"], "path-malformed");
        assert_eq!(report(&stack, "4", 1, "v8")["outcome"], "out-of-range");
        assert_eq!(
            report(&stack, "0.7", 1, "v8")["outcome"],
            "node-out-of-range"
        );
        assert_eq!(
            report(&stack, "0.0", 1, "v8")["outcome"],
            "node-not-a-quotation"
        );
        assert_eq!(
            report(&serde_json::json!([]), "0", 1, "v8")["outcome"],
            "no-observation"
        );
        let pair = serde_json::json!([{ "type": "Pair", "value": [] }]);
        assert_eq!(
            report(&pair, "0", 1, "v8")["outcome"],
            "stack-entry-not-a-program"
        );
    }

    #[test]
    fn a_large_recipe_is_truncated_and_says_so() {
        let recipe: Vec<serde_json::Value> = (0..(MAX_NODES + 5))
            .map(|at| literal(at.to_string().as_str()))
            .collect();
        let witnesses: Vec<serde_json::Value> = (0..(MAX_WITNESSES + 1))
            .map(|_| serde_json::json!({ "node": 0, "effects": [] }))
            .collect();
        let stack = program(
            serde_json::Value::Array(recipe),
            serde_json::Value::Array(witnesses),
        );
        let inspected = report(&stack, "0", 1, "v8");
        assert_eq!(
            inspected["program"]["recipe"].as_array().map(Vec::len),
            Some(MAX_NODES)
        );
        assert_eq!(inspected["program"]["nodes"]["total"], MAX_NODES + 5);
        let kinds: Vec<&str> = inspected["truncated"]
            .as_array()
            .expect("a truncation list")
            .iter()
            .filter_map(|entry| entry["kind"].as_str())
            .collect();
        assert!(kinds.contains(&"recipe-nodes"), "{kinds:?}");
        assert!(kinds.contains(&"witnesses"), "{kinds:?}");
    }

    #[test]
    fn a_nested_quotation_is_reachable_and_keeps_its_own_interface() {
        let stack = program(
            serde_json::json!([{
                "quotation": [serde_json::json!({ "literal": { "type": "Bool", "value": true } })],
                "interface": {
                    "stack_in": { "values": [], "depth": 0 },
                    "stack_out": { "values": [], "depth": 0 },
                    "effects": ["test.emit"],
                },
                "witnesses": [],
            }]),
            serde_json::json!([]),
        );
        let inspected = report(&stack, "0.0", 1, "v8");
        assert_eq!(inspected["outcome"], "selected");
        assert_eq!(inspected["selection"]["node_path"], serde_json::json!([0]));
        assert_eq!(
            inspected["program"]["effects"],
            serde_json::json!(["test.emit"])
        );
        assert_eq!(
            inspected["program"]["recipe"],
            serde_json::json!([{ "literal": { "type": "Bool", "value": true } }])
        );
    }

    #[test]
    fn an_over_deep_path_is_refused_rather_than_walked() {
        let nested = serde_json::json!([{
            "handle": 1,
            "type": "Program",
            "interface": serde_json::Value::Null,
            "recipe": [],
            "witnesses": [],
        }]);
        let stack = program(nested, serde_json::json!([]));
        let mut path = std::string::String::from("0");
        for _ in 0..10 {
            path.push_str(".0");
        }
        assert_eq!(
            report(&stack, &path, 1, "v8")["outcome"],
            "quotation-depth-bound"
        );
        let empty = program(serde_json::json!([]), serde_json::json!([]));
        assert_eq!(report(&empty, "0", 1, "v8")["outcome"], "selected");
    }
}
