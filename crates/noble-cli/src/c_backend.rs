//! Nonselected C11 source-only build. Native ELF is not admitted or executed here.
//! Every instruction is lowered from a source-prepared, independently checked
//! Core submission. The emitted C executes typed instructions and immutable
//! captured Program values; it never parses Noble source or recipe text.

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use noble_kernel::contracts::{Behavior, Definition};
use noble_kernel::types::Ty;
use noble_kernel::untrusted::{Candidate, Lit, Node, NodeId};
use serde::de::{Error as _, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;
use serde_json::{json, Value};

const SOURCE_MAX: usize = 4096;
const CONTROL_MAX: usize = 4096;
const C_MAX: usize = 65536;
const PROGRAM_MAX: usize = 256;
const FRAME_MAX: usize = 512;
const RECIPE_MAX: usize = 256;
const PROGRAM_DEPTH_MAX: usize = 256;
const EMITTER_SOURCE: &str = include_str!("c_backend.rs");
const USAGE: &str = "usage: noble build SOURCE --target c --out NEW_DIR --contract CONTRACT_JSON --policy POLICY_JSON (C source only; no native execution)";
const RUNTIME_HEADER: &str = r#"/* Noble nonselected C11 source; requires separately admitted native execution. */
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
_Static_assert(CHAR_BIT == 8 && sizeof(uint64_t) == 8 && UINT64_MAX == UINT64_C(18446744073709551615), "unsupported I64 target");
enum { TAG_I64 = 1, TAG_BOOL = 2, TAG_PROGRAM = 3, TAG_UNIT = 4, TAG_TEXT = 5 };
typedef struct { unsigned tag; uint64_t bits; } Value;
typedef struct { unsigned op; uint64_t bits; } Instruction;
typedef struct { const Instruction *code; size_t count; unsigned valid; size_t leaves, depth; } StaticProgram;
typedef struct { const unsigned char *bytes; size_t length; unsigned valid; } StaticText;
enum { OP_I64 = 1, OP_BOOL, OP_DUP, OP_DROP, OP_SWAP, OP_ADD, OP_SUB, OP_MUL, OP_EQ, OP_STATIC, OP_QUOTE, OP_COMPOSE, OP_RUN, OP_UNIT, OP_IF, OP_DIP, OP_TEXT };
typedef struct { unsigned kind; uint64_t left, right; Value capture; size_t leaves, depth; } Program;
typedef struct { size_t leaves, depth; } Metrics;
typedef struct { uint64_t program; size_t pc; unsigned phase; Value restore; } Frame;
"#;
const RUNTIME: &str = r#"
static Value stack[STACK_SLOTS];
static size_t height;
static Program programs[PROGRAM_SLOTS];
static size_t program_count;
static Frame frames[FRAME_SLOTS];
static size_t frame_count;
static uint64_t steps;
static char output[OUTPUT_BYTES];
static size_t length;
static void fail(const char *message) {
    const char *outcome = strstr(message, "exhausted") ? "exhausted" : "trap";
    fprintf(stderr, "{\"stage\":\"native-runtime\",\"outcome\":\"%s\",\"diagnostic\":\"%s\"}\n", outcome, message);
    exit(2);
}
static void tick(void) { if (steps == STEP_LIMIT) fail("steps exhausted"); ++steps; }
static void append(const char *text) {
    size_t n = strlen(text);
    if (n >= OUTPUT_BYTES - length) fail("output bytes exhausted");
    memcpy(output + length, text, n);
    length += n;
    output[length] = '\0';
}
static void append_byte(unsigned char byte) {
    if (length >= OUTPUT_BYTES - 1) fail("output bytes exhausted");
    output[length++] = (char)byte;
    output[length] = '\0';
}
static void push(unsigned tag, uint64_t bits) {
    if (height == STACK_SLOTS) fail("stack slots exhausted");
    stack[height].tag = tag;
    stack[height].bits = bits;
    ++height;
}
static void push_value(Value value) { push(value.tag, value.bits); }
static Value pop(void) {
    if (height == 0) fail("stack underflow");
    --height;
    return stack[height];
}
static Value pop_program(void) {
    Value value = pop();
    if (value.tag != TAG_PROGRAM || value.bits >= STATIC_PROGRAMS + program_count) fail("invalid Program tag");
    return value;
}
static uint64_t pop_i64(void) {
    Value value = pop();
    if (value.tag != TAG_I64) fail("invalid I64 tag");
    return value.bits;
}
static unsigned pop_bool(void) {
    Value value = pop();
    if (value.tag != TAG_BOOL || value.bits > 1) fail("invalid Bool tag");
    return (unsigned)value.bits;
}
static Metrics metrics(uint64_t id) {
    if (id < STATIC_PROGRAMS) {
        const StaticProgram *program = &static_programs[id];
        if (!program->valid) fail("invalid static Program");
        return (Metrics){program->leaves, program->depth};
    }
    size_t index = (size_t)(id - STATIC_PROGRAMS);
    if (index >= program_count) fail("invalid Program reference");
    return (Metrics){programs[index].leaves, programs[index].depth};
}
static uint64_t new_program(unsigned kind, uint64_t left, uint64_t right, Value capture,
                            size_t leaves, size_t depth) {
    if (leaves > RECIPE_LIMIT) fail("recipe leaves exhausted");
    if (depth > DEPTH_LIMIT) fail("program depth exhausted");
    if (program_count == PROGRAM_SLOTS) fail("program slots exhausted");
    size_t index = program_count++;
    programs[index] = (Program){kind, left, right, capture, leaves, depth};
    return STATIC_PROGRAMS + index;
}
static void enter(uint64_t program) {
    if (program >= STATIC_PROGRAMS + program_count) fail("invalid Program reference");
    if (frame_count == FRAME_SLOTS) fail("program frames exhausted");
    frames[frame_count++] = (Frame){program, 0, 0, {0, 0}};
}
static void arith(unsigned operation) {
    uint64_t right = pop_i64();
    uint64_t left = pop_i64();
    if (operation == 0) push(TAG_I64, left + right);
    else if (operation == 1) push(TAG_I64, left - right);
    else if (operation == 2) push(TAG_I64, left * right);
    else push(TAG_BOOL, left == right);
}
static void execute(Instruction instruction) {
    switch (instruction.op) {
    case OP_I64: push(TAG_I64, instruction.bits); break;
    case OP_BOOL: if (instruction.bits > 1) fail("invalid Bool"); push(TAG_BOOL, instruction.bits); break;
    case OP_UNIT: push(TAG_UNIT, 0); break;
    case OP_TEXT:
        if (instruction.bits >= STATIC_TEXTS || !text_literals[instruction.bits].valid) fail("invalid Text reference");
        push(TAG_TEXT, instruction.bits);
        break;
    case OP_DUP:
        if (!height) fail("stack underflow");
        push_value(stack[height - 1]);
        break;
    case OP_DROP: (void)pop(); break;
    case OP_SWAP:
        if (height < 2) fail("stack underflow");
        { Value temporary = stack[height - 1]; stack[height - 1] = stack[height - 2]; stack[height - 2] = temporary; }
        break;
    case OP_ADD: case OP_SUB: case OP_MUL: case OP_EQ: arith(instruction.op - OP_ADD); break;
    case OP_STATIC:
        if (instruction.bits >= STATIC_PROGRAMS || !static_programs[instruction.bits].valid) fail("invalid static Program");
        push(TAG_PROGRAM, instruction.bits);
        break;
    case OP_QUOTE: {
        Value capture = pop();
        if (capture.tag != TAG_I64 && capture.tag != TAG_BOOL && capture.tag != TAG_UNIT &&
            capture.tag != TAG_TEXT && capture.tag != TAG_PROGRAM) fail("invalid capture");
        size_t leaves = 1, depth = 1;
        if (capture.tag == TAG_PROGRAM) {
            Metrics subject = metrics(capture.bits);
            if (subject.leaves >= RECIPE_LIMIT) fail("recipe leaves exhausted");
            if (subject.depth >= DEPTH_LIMIT) fail("program depth exhausted");
            leaves += subject.leaves;
            depth += subject.depth;
        }
        uint64_t program = new_program(1, 0, 0, capture, leaves, depth);
        push(TAG_PROGRAM, program);
        break;
    }
    case OP_COMPOSE: {
        Value right = pop_program();
        Value left = pop_program();
        Metrics lhs = metrics(left.bits);
        Metrics rhs = metrics(right.bits);
        if (rhs.leaves > RECIPE_LIMIT || lhs.leaves > RECIPE_LIMIT - rhs.leaves)
            fail("recipe leaves exhausted");
        size_t depth = lhs.depth > rhs.depth ? lhs.depth : rhs.depth;
        if (depth >= DEPTH_LIMIT) fail("program depth exhausted");
        uint64_t program = new_program(2, left.bits, right.bits, (Value){0, 0},
                                       lhs.leaves + rhs.leaves, depth + 1);
        push(TAG_PROGRAM, program);
        break;
    }
    case OP_RUN: {
        Value value = pop_program();
        enter(value.bits);
        break;
    }
    case OP_IF: {
        Value otherwise = pop_program();
        Value then_branch = pop_program();
        unsigned condition = pop_bool();
        enter(condition ? then_branch.bits : otherwise.bits);
        break;
    }
    case OP_DIP: {
        Value program = pop_program();
        Value saved = pop();
        if (saved.tag != TAG_I64 && saved.tag != TAG_BOOL &&
            saved.tag != TAG_UNIT && saved.tag != TAG_TEXT && saved.tag != TAG_PROGRAM) fail("invalid dip value");
        if (saved.tag == TAG_PROGRAM) (void)metrics(saved.bits);
        if (frame_count > FRAME_SLOTS - 2) fail("program frames exhausted");
        frames[frame_count++] = (Frame){UINT64_MAX, 0, 0, saved};
        enter(program.bits);
        break;
    }
    default: fail("invalid instruction");
    }
}
static void run_program(void) {
    enter(0);
    while (frame_count) {
        /* Charge every transition, including empty bodies and compose phases. */
        tick();
        Frame *frame = &frames[frame_count - 1];
        if (frame->program == UINT64_MAX) {
            Value saved = frame->restore;
            --frame_count;
            push_value(saved);
            continue;
        }
        if (frame->program < STATIC_PROGRAMS) {
            const StaticProgram *body = &static_programs[frame->program];
            if (!body->valid) fail("invalid static Program");
            if (frame->pc == body->count) { --frame_count; continue; }
            if (frame->pc > body->count) fail("invalid program counter");
            Instruction instruction = body->code[frame->pc++];
            execute(instruction);
            continue;
        }
        size_t index = (size_t)(frame->program - STATIC_PROGRAMS);
        if (index >= program_count) fail("invalid Program reference");
        const Program *program = &programs[index];
        if (program->kind == 1) {
            if (frame->phase++ == 0) push_value(program->capture);
            else --frame_count;
        } else if (program->kind == 2) {
            if (frame->phase == 0) { frame->phase = 1; enter(program->left); }
            else if (frame->phase == 1) { frame->phase = 2; enter(program->right); }
            else --frame_count;
        } else fail("invalid Program kind");
    }
}
static void decimal(uint64_t bits) {
    char digits[32];
    uint64_t magnitude = bits;
    if ((bits & UINT64_C(9223372036854775808)) != 0) {
        append("-");
        magnitude = (~bits) + UINT64_C(1);
    }
    int n = snprintf(digits, sizeof digits, "%llu", (unsigned long long)magnitude);
    if (n < 0 || (size_t)n >= sizeof digits) fail("decimal encoding");
    append(digits);
}
static void emit_text(uint64_t id) {
    static const char hex[] = "0123456789abcdef";
    if (id >= STATIC_TEXTS || !text_literals[id].valid) fail("invalid Text reference");
    const StaticText *text = &text_literals[id];
    append("{\"type\":\"Text\",\"value\":\"");
    for (size_t i = 0; i < text->length; ++i) {
        unsigned char byte = text->bytes[i];
        if (byte == '"' || byte == '\\') {
            append_byte('\\');
            append_byte(byte);
        } else if (byte < 0x20) {
            append("\\u00");
            append_byte((unsigned char)hex[byte >> 4]);
            append_byte((unsigned char)hex[byte & 15]);
        } else append_byte(byte);
    }
    append("\"}");
}
static void emit_stack(void) {
    append("{\"stage\":\"native-runtime\",\"outcome\":\"normal\",\"stack\":[");
    for (size_t i = 0; i < height; ++i) {
        if (i) append(",");
        if (stack[i].tag == TAG_I64) {
            append("{\"type\":\"I64\",\"value\":\"");
            decimal(stack[i].bits);
            append("\"}");
        } else if (stack[i].tag == TAG_BOOL) {
            append(stack[i].bits ? "{\"type\":\"Bool\",\"value\":true}" : "{\"type\":\"Bool\",\"value\":false}");
        } else if (stack[i].tag == TAG_UNIT) {
            append("{\"type\":\"Unit\"}");
        } else if (stack[i].tag == TAG_TEXT) {
            emit_text(stack[i].bits);
        } else fail("invalid output tag");
    }
    append("]}");
    if (fputs(output, stdout) == EOF || fputc('\n', stdout) == EOF || fflush(stdout) == EOF) fail("output stream");
}
int main(void) {
    run_program();
"#;

fn read_bounded(path: &Path, max: usize) -> Result<Vec<u8>, String> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    if !metadata.file_type().is_file() || metadata.len() > max as u64 {
        return Err(format!("{} must be a bounded regular file", path.display()));
    }
    let mut bytes = Vec::new();
    file.take((max + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    if bytes.len() > max {
        return Err(format!("{} exceeds {max} bytes", path.display()));
    }
    Ok(bytes)
}

struct Options {
    source: PathBuf,
    contract: PathBuf,
    policy: PathBuf,
    out: PathBuf,
}

fn options(arguments: &[std::ffi::OsString]) -> Result<Options, String> {
    let mut source = None;
    let mut contract = None;
    let mut policy = None;
    let mut out = None;
    let mut target = false;
    let mut index = 1;
    while index < arguments.len() {
        let arg = arguments[index].to_str().ok_or(USAGE)?;
        if arg == "--native" {
            return Err("native C build unavailable: no selected sandbox, clock/loader barrier or source-to-ELF admission".into());
        }
        if arg == "--target" || arg == "--out" || arg == "--contract" || arg == "--policy" {
            let value = arguments.get(index + 1).ok_or(USAGE)?;
            if value.to_string_lossy().starts_with("--") {
                return Err(USAGE.into());
            }
            match arg {
                "--target" if !target && value == "c" => target = true,
                "--out" if out.is_none() => out = Some(PathBuf::from(value)),
                "--contract" if contract.is_none() => contract = Some(PathBuf::from(value)),
                "--policy" if policy.is_none() => policy = Some(PathBuf::from(value)),
                _ => return Err(USAGE.into()),
            }
            index += 2;
        } else if !arg.starts_with('-') && source.is_none() {
            source = Some(PathBuf::from(&arguments[index]));
            index += 1;
        } else {
            return Err(USAGE.into());
        }
    }
    if !target {
        return Err(USAGE.into());
    }
    Ok(Options {
        source: source.ok_or(USAGE)?,
        contract: contract.ok_or(USAGE)?,
        policy: policy.ok_or(USAGE)?,
        out: out.ok_or(USAGE)?,
    })
}

struct Limits {
    slots: usize,
    steps: u64,
    output: usize,
}

struct UniqueValue(Value);

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct UniqueVisitor;
        impl<'de> Visitor<'de> for UniqueVisitor {
            type Value = UniqueValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a JSON value without duplicate object members")
            }

            fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Bool(value)))
            }

            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Number(value.into())))
            }

            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Number(value.into())))
            }

            fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(value)
                    .map(Value::Number)
                    .map(UniqueValue)
                    .ok_or_else(|| E::custom("invalid JSON number"))
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::String(value.to_owned())))
            }

            fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::String(value)))
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Null))
            }

            fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Null))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(UniqueValue(value)) = sequence.next_element()? {
                    values.push(value);
                }
                Ok(UniqueValue(Value::Array(values)))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some((key, UniqueValue(value))) = map.next_entry::<String, UniqueValue>()? {
                    if values.contains_key(&key) {
                        return Err(A::Error::custom("duplicate JSON member"));
                    }
                    values.insert(key, value);
                }
                Ok(UniqueValue(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(UniqueVisitor)
    }
}

fn unique_json(bytes: &[u8]) -> Result<Value, String> {
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let UniqueValue(value) = UniqueValue::deserialize(&mut decoder)
        .map_err(|error| format!("invalid or duplicate control JSON: {error}"))?;
    decoder.end().map_err(|error| format!("invalid control JSON: {error}"))?;
    Ok(value)
}

fn controls(
    contract: &[u8],
    policy: &[u8],
) -> Result<(noble_contracts::source::CoreInterface, Limits), String> {
    let contract = unique_json(contract)?;
    let policy = unique_json(policy)?;
    let fields = contract.as_object().ok_or("contract must be an object")?;
    if fields.len() != 2
        || !fields.contains_key("expected_interface")
        || fields
            .get("resolved_host_contracts")
            .and_then(Value::as_object)
            .is_none_or(|v| !v.is_empty())
    {
        return Err("C slice requires an exact interface and empty host contracts".into());
    }
    let interface = contract["expected_interface"]
        .as_str()
        .ok_or("expected_interface must be an ordered Core stack interface")?;
    let expected = noble_contracts::source::parse_core_interface(interface)
        .map_err(|error| format!("invalid Core interface: {:?}", error.diagnostic()))?;
    if !expected.stack_in.is_empty() || expected.stack_out.len() != 1 {
        return Err("C slice requires no runtime input and one output".into());
    }
    if !matches!(expected.stack_out.as_slice(), [Ty::I64 | Ty::Bool | Ty::Unit | Ty::Text]) {
        return Err("checked aggregate, Syntax or Program C output is unsupported".into());
    }
    let fields = policy.as_object().ok_or("policy must be an object")?;
    if fields.len() != 4
        || policy["profile"] != "Core-Bootstrap"
        || policy["target"] != "x86_64-unknown-linux-gnu"
        || policy["allowed_effects"] != json!([])
    {
        return Err("unsupported C scope, effects, target or policy field".into());
    }
    let limits = policy["limits"]
        .as_object()
        .ok_or("missing finite C limits")?;
    if limits.len() != 3 {
        return Err("C limits require exactly stack_slots, steps, output_bytes".into());
    }
    let slots = limits
        .get("stack_slots")
        .and_then(Value::as_u64)
        .ok_or("invalid stack_slots")?;
    let steps = limits
        .get("steps")
        .and_then(Value::as_u64)
        .ok_or("invalid steps")?;
    let output = limits
        .get("output_bytes")
        .and_then(Value::as_u64)
        .ok_or("invalid output_bytes")?;
    if !(1..=256).contains(&slots)
        || !(1..=4096).contains(&steps)
        || !(128..=16384).contains(&output)
        || output < 128 + slots * 64
    {
        return Err("C limits exceed the supported bounded slice".into());
    }
    Ok((
        expected,
        Limits {
            slots: slots as usize,
            steps,
            output: output as usize,
        },
    ))
}

struct LoweredProgram {
    code: String,
    recipe: Value,
    leaves: usize,
    depth: usize,
}

fn bind_texts(body: &noble_kernel::execution::Body) -> Result<Vec<Option<&str>>, String> {
    let mut texts = vec![None; body.candidate.nodes.len()];
    for text in &body.texts {
        let slot = texts.get_mut(text.node.0 as usize).ok_or("Text metadata node out of bounds")?;
        if !matches!(
            body.candidate.nodes.get(text.node.0 as usize),
            Some(Node::Literal { lit: Lit::Text, .. })
        ) || slot.is_some()
        {
            return Err("Text metadata must bind exactly one Text literal".into());
        }
        *slot = Some(std::str::from_utf8(&text.bytes).map_err(|_| "invalid Text UTF-8")?);
    }
    if body.candidate.nodes.iter().enumerate().any(|(index, node)| {
        matches!(node, Node::Literal { lit: Lit::Text, .. }) != texts[index].is_some()
    }) {
        return Err("missing or extraneous checked Text metadata".into());
    }
    Ok(texts)
}

fn lower_body(
    candidate: &Candidate,
    body: &[NodeId],
    kinds: &[Behavior],
    texts: &[Option<&str>],
    visited: &mut [bool],
    programs: &mut [Option<LoweredProgram>],
    slot: usize,
) -> Result<(usize, usize), String> {
    let mut code = String::new();
    let mut recipe = Vec::new();
    let mut leaves = 0usize;
    let mut depth = 1usize;
    for id in body {
        let index = id.0 as usize;
        let seen = visited.get_mut(index).ok_or("invalid checked node reference")?;
        if *seen {
            return Err("repeated or cyclic C node reference".into());
        }
        *seen = true;
        let (op, bits, atom) = match candidate.nodes.get(index).ok_or("missing checked node")? {
            Node::Literal { lit: Lit::I64(value), .. } =>
                ("OP_I64", *value as u64, json!(["i64", value.to_string()])),
            Node::Literal { lit: Lit::Bool(value), .. } =>
                ("OP_BOOL", u64::from(*value), json!(["bool", value])),
            Node::Literal { lit: Lit::Unit, .. } =>
                ("OP_UNIT", 0, json!(["unit"])),
            Node::Literal { lit: Lit::Text, .. } => {
                let text = texts.get(index).and_then(|value| *value)
                    .ok_or("missing checked Text payload")?;
                ("OP_TEXT", index as u64, json!(["text", text]))
            }
            Node::Quotation { body, .. } => {
                let (child_leaves, child_depth) = lower_body(
                    candidate, body, kinds, texts, visited, programs, index + 1,
                )?;
                leaves = leaves.checked_add(child_leaves).ok_or("recipe leaves exhausted")?;
                depth = depth.max(child_depth.checked_add(1).ok_or("program depth exhausted")?);
                ("OP_STATIC", (index + 1) as u64, json!(["quotation", index + 1]))
            }
            Node::Invocation { def: Definition(word), .. } => {
                let kind = kinds.get(*word as usize).ok_or("missing checked operation contract")?;
                let op = match (*word, kind) {
                    (0, Behavior::Dup) => "OP_DUP",
                    (1, Behavior::Drop) => "OP_DROP",
                    (2, Behavior::Swap) => "OP_SWAP",
                    (3, Behavior::Dip) => "OP_DIP",
                    (4, Behavior::Arith) => "OP_ADD",
                    (5, Behavior::Arith) => "OP_SUB",
                    (6, Behavior::Arith) => "OP_MUL",
                    (7, Behavior::Equals) => "OP_EQ",
                    (8, Behavior::Quote) => "OP_QUOTE",
                    (9, Behavior::Compose) => "OP_COMPOSE",
                    (10, Behavior::Run) => "OP_RUN",
                    (12, Behavior::Unit) => "OP_UNIT",
                    (18, Behavior::If) => "OP_IF",
                    _ => return Err(format!("unsupported checked C operation {word}")),
                };
                (op, 0, json!(["word", word]))
            }
            Node::SlotInvoke { .. } | Node::Literal { .. } =>
                return Err("unsupported checked C value, effect or live operation".into()),
        };
        code.push_str(&format!("    {{{op}, UINT64_C({bits})}},\n"));
        recipe.push(atom);
        leaves = leaves.checked_add(1).ok_or("recipe leaves exhausted")?;
        if leaves > RECIPE_MAX || depth > PROGRAM_DEPTH_MAX {
            return Err("static C program recipe exhausted".into());
        }
        if code.len() > C_MAX {
            return Err("C instruction output exhausted".into());
        }
    }
    programs[slot] = Some(LoweredProgram {
        code,
        recipe: json!(recipe),
        leaves,
        depth,
    });
    Ok((leaves, depth))
}

fn emit(
    source: &[u8],
    expected: &noble_contracts::source::CoreInterface,
    limits: &Limits,
) -> Result<(String, Value, String, String), String> {
    let frontend = noble_contracts::source::Session::without_test_hosts();
    let prepared = frontend
        .prepare(
            source,
            &[],
            noble_contracts::Limits {
                bytes: SOURCE_MAX as u32,
                nodes: 1024,
                depth: 16,
                work: 100_000,
            },
        )
        .map_err(|error| format!("source {:?}: {:?}", error.stage(), error.diagnostic()))?;
    let submission = prepared
        .submission()
        .ok_or("C build requires an expression")?;
    if !submission.definitions.is_empty()
        || submission.request.expected.stack_in != expected.stack_in
        || submission.request.expected.stack_out != expected.stack_out
        || !submission
            .request
            .expected
            .allowed_effects
            .as_slice()
            .is_empty()
    {
        return Err("unsupported C interface or resolved definitions".into());
    }
    let candidate = &submission.body.candidate;
    // Source preparation already checks every concrete body. Recheck the
    // restricted root against the independent kernel, without also compiling
    // and allocating an unused Wasm artifact.
    let checked = match noble_kernel::acceptance::check(
        &submission.environment,
        &submission.request,
        candidate,
    ) {
        noble_kernel::untrusted::Outcome::Accepted(checked) => checked,
        outcome => return Err(format!("independent kernel acceptance: {outcome:?}")),
    };
    if checked.interface.stack_in != expected.stack_in
        || checked.interface.stack_out != expected.stack_out
        || !checked.interface.effects.as_slice().is_empty()
        || !checked.live_sites.is_empty()
    {
        return Err("checked C body differs from its expected interface or order".into());
    }
    let texts = bind_texts(&submission.body)?;
    let mut visited = vec![false; candidate.nodes.len()];
    let mut programs = Vec::with_capacity(candidate.nodes.len() + 1);
    programs.resize_with(candidate.nodes.len() + 1, || None);
    lower_body(
        candidate, &candidate.body, &submission.environment.kinds, &texts,
        &mut visited, &mut programs, 0,
    )?;
    if visited.iter().any(|seen| !seen)
        || checked.derivations.len() != visited.len()
        || checked.derivations.iter().any(|row| !visited.get(row.node.0 as usize).copied().unwrap_or(false))
    {
        return Err("checked C body omits or duplicates an accepted node".into());
    }
    let mut c = format!(
        "#define STACK_SLOTS {}\n#define STEP_LIMIT UINT64_C({})\n#define OUTPUT_BYTES {}\n#define PROGRAM_SLOTS {}\n#define FRAME_SLOTS {}\n#define RECIPE_LIMIT {}\n#define DEPTH_LIMIT {}\n#define STATIC_PROGRAMS {}\n#define STATIC_TEXTS {}\n",
        limits.slots, limits.steps, limits.output,
        PROGRAM_MAX, FRAME_MAX, RECIPE_MAX, PROGRAM_DEPTH_MAX, programs.len(), texts.len().max(1)
    );
    c.push_str(RUNTIME_HEADER);
    for (index, text) in texts.iter().enumerate() {
        if let Some(text) = text {
            write!(c, "static const unsigned char text_{index}[] = {{").map_err(|_| "C output failure")?;
            for byte in text.as_bytes() {
                write!(c, "0x{byte:02x},").map_err(|_| "C output failure")?;
            }
            c.push_str("0};\n");
        }
    }
    c.push_str("static const StaticText text_literals[STATIC_TEXTS] = {\n");
    for (index, text) in texts.iter().enumerate() {
        if let Some(text) = text {
            writeln!(c, "    {{text_{index}, {}, 1}},", text.len()).map_err(|_| "C output failure")?;
        } else {
            c.push_str("    {NULL, 0, 0},\n");
        }
    }
    if texts.is_empty() {
        c.push_str("    {NULL, 0, 0},\n");
    }
    c.push_str("};\n");
    for (index, program) in programs.iter().enumerate() {
        if let Some(program) = program {
            // An empty quotation is an actual zero-instruction program.
            if !program.code.is_empty() {
                c.push_str(&format!("static const Instruction instructions_{index}[] = {{\n{}}};\n", program.code));
            }
        }
    }
    c.push_str("static const StaticProgram static_programs[STATIC_PROGRAMS] = {\n");
    for (index, program) in programs.iter().enumerate() {
        match program {
            Some(program) if !program.code.is_empty() =>
                c.push_str(&format!("    {{instructions_{index}, {}, 1, {}, {}}},\n",
                    program.code.lines().count(), program.leaves, program.depth)),
            Some(program) => c.push_str(&format!("    {{NULL, 0, 1, {}, {}}},\n",
                program.leaves, program.depth)),
            None => c.push_str("    {NULL, 0, 0, 0, 0},\n"),
        }
    }
    c.push_str("};\n");
    c.push_str(RUNTIME);
    c.push_str(match expected.stack_out.as_slice() {
        [Ty::I64] => "    if (height != 1 || stack[0].tag != TAG_I64) fail(\"output interface\");\n",
        [Ty::Bool] => "    if (height != 1 || stack[0].tag != TAG_BOOL) fail(\"output interface\");\n",
        [Ty::Unit] => "    if (height != 1 || stack[0].tag != TAG_UNIT) fail(\"output interface\");\n",
        [Ty::Text] => "    if (height != 1 || stack[0].tag != TAG_TEXT) fail(\"output interface\");\n",
        _ => return Err("unsupported C result type".into()),
    });
    c.push_str("    emit_stack();\n    return 0;\n}\n");
    if c.len() > C_MAX {
        return Err("C source output exhausted".into());
    }
    let interface = match expected.stack_out.as_slice() {
        [Ty::I64] => "-- I64",
        [Ty::Bool] => "-- Bool",
        [Ty::Unit] => "-- Unit",
        [Ty::Text] => "-- Text",
        _ => return Err("unsupported C result type".into()),
    };
    let recipes = programs.into_iter().map(|program| program.map(|item| item.recipe))
        .collect::<Vec<_>>();
    Ok((
        c,
        json!({"interface":interface, "programs":recipes}),
        format!("{candidate:?}"),
        format!("{:?}", submission.environment),
    ))
}

fn build(arguments: &[std::ffi::OsString]) -> Result<Value, String> {
    let options = options(arguments)?;
    let source = read_bounded(&options.source, SOURCE_MAX)?;
    let contract = read_bounded(&options.contract, CONTROL_MAX)?;
    let policy = read_bounded(&options.policy, CONTROL_MAX)?;
    let (expected, limits) = controls(&contract, &policy)?;
    let (c, recipe, candidate, environment) = emit(&source, &expected, &limits)?;
    let digest = crate::workflow::intrinsic::sha256;
    let semantic_recipe = serde_json::to_vec(&recipe).map_err(|error| error.to_string())?;
    let mut subject = b"noble-c11-source-only/v1\0".to_vec();
    subject.extend_from_slice(&noble_kernel::untrusted::CANDIDATE_FORMAT.to_le_bytes());
    subject.extend_from_slice(&noble_kernel::untrusted::SEMANTIC_REVISION.to_le_bytes());
    for bytes in [
        &source[..],
        &contract[..],
        &policy[..],
        candidate.as_bytes(),
        environment.as_bytes(),
        EMITTER_SOURCE.as_bytes(),
        c.as_bytes(),
    ] {
        subject.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        subject.extend_from_slice(bytes);
    }
    let manifest = json!({"profile":"C-Backend-Draft", "status":"nonselected-source-only",
        "candidate_format":noble_kernel::untrusted::CANDIDATE_FORMAT,
        "semantic_revision":noble_kernel::untrusted::SEMANTIC_REVISION,
        "source_sha256":digest(&source), "contract_sha256":digest(&contract),
        "policy_sha256":digest(&policy), "c_sha256":digest(c.as_bytes()),
        "checked_candidate_sha256":digest(candidate.as_bytes()),
        "resolved_environment_sha256":digest(environment.as_bytes()),
        "emitter_source_sha256":digest(EMITTER_SOURCE.as_bytes()),
        "semantic_recipe_sha256":digest(&semantic_recipe),
        "build_key":digest(&subject), "native_artifact":null,
        "runtime_limits":{"stack_slots":limits.slots, "steps":limits.steps,
            "output_bytes":limits.output, "program_slots":PROGRAM_MAX,
            "frame_slots":FRAME_MAX, "recipe_leaves":RECIPE_MAX,
            "program_depth":PROGRAM_DEPTH_MAX},
        "source_to_elf_admitted":false, "native_execution":"blocked-no-pinned-sandbox"});
    std::fs::create_dir(&options.out).map_err(|error| format!("new output directory: {error}"))?;
    let write_new = |name: &str, bytes: &[u8]| -> Result<(), String> {
        let path = options.out.join(name);
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .and_then(|mut file| file.write_all(bytes))
            .map_err(|error| format!("{}: {error}", path.display()))
    };
    write_new("source.noble", &source)?;
    write_new("contract.json", &contract)?;
    write_new("policy.json", &policy)?;
    write_new("artifact.c", c.as_bytes())?;
    write_new(
        "manifest.json",
        serde_json::to_string_pretty(&manifest)
            .map_err(|error| error.to_string())?
            .as_bytes(),
    )?;
    Ok(manifest)
}

pub fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    match build(arguments) {
        Ok(manifest) => {
            println!("{manifest}");
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("C source build refused: {error}");
            std::process::ExitCode::from(2)
        }
    }
}
