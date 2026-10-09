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
const AGGREGATE_MAX: usize = 256;
const LIST_MAX: usize = 255;
const SHAPE_MAX: usize = 512;
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
enum { TAG_I64 = 1, TAG_BOOL = 2, TAG_PROGRAM = 3, TAG_UNIT = 4, TAG_TEXT = 5,
       TAG_PAIR = 6, TAG_SUM_LEFT = 7, TAG_SUM_RIGHT = 8, TAG_LIST = 9 };
enum { AGG_LIST_NIL = 10, AGG_LIST_CONS = 11 };
typedef struct { unsigned tag; uint64_t bits; unsigned shape; } Value;
typedef struct { unsigned op; uint64_t bits; unsigned shape, inputs, input_shape[3]; } Instruction;
typedef struct { const Instruction *code; size_t count; unsigned valid, shape; size_t leaves, depth; } StaticProgram;
typedef struct { const unsigned char *bytes; size_t length; unsigned valid; } StaticText;
enum { OP_I64 = 1, OP_BOOL, OP_DUP, OP_DROP, OP_SWAP, OP_ADD, OP_SUB, OP_MUL, OP_EQ, OP_STATIC, OP_QUOTE, OP_COMPOSE, OP_RUN, OP_UNIT, OP_IF, OP_DIP, OP_TEXT,
       OP_PAIR, OP_UNPAIR, OP_INL, OP_INR, OP_CASE, OP_NIL, OP_CONS, OP_LIST_CASE };
typedef struct { unsigned kind, shape; uint64_t left, right; Value capture; size_t leaves, depth; } Program;
typedef struct { unsigned kind, shape; size_t length; Value left, right; } Aggregate;
typedef struct { size_t leaves, depth; } Metrics;
typedef struct { Value value; size_t depth; } DataVisit;
typedef struct { uint64_t program; size_t pc; unsigned phase; Value restore; } Frame;
"#;
const RUNTIME: &str = r#"
static Value stack[STACK_SLOTS];
static size_t height;
static Program programs[PROGRAM_SLOTS];
static size_t program_count;
static Aggregate aggregates[AGGREGATE_SLOTS];
static size_t aggregate_count;
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
static void append_bytes(const char *text, size_t n) {
    if (n >= OUTPUT_BYTES - length) fail("output bytes exhausted");
    memcpy(output + length, text, n);
    length += n;
    output[length] = '\0';
}
#define APPEND_LITERAL(text) append_bytes(text, sizeof(text) - 1)
static void append_byte(unsigned char byte) {
    if (length >= OUTPUT_BYTES - 1) fail("output bytes exhausted");
    output[length++] = (char)byte;
    output[length] = '\0';
}
static int shape_kind(unsigned tag, unsigned shape) {
    if (shape == 0 || shape > SHAPES) return 0;
    unsigned expected = shape_tags[shape];
    return expected == tag || (expected == TAG_SUM_LEFT && tag == TAG_SUM_RIGHT);
}
static void push(unsigned tag, uint64_t bits, unsigned shape) {
    if (height == STACK_SLOTS) fail("stack slots exhausted");
    if (!shape_kind(tag, shape)) fail("invalid value shape");
    stack[height++] = (Value){tag, bits, shape};
}
static void push_value(Value value) { push(value.tag, value.bits, value.shape); }
static Value pop(void) {
    if (height == 0) fail("stack underflow");
    --height;
    return stack[height];
}
static Value pop_program(void) {
    Value value = pop();
    if (value.tag != TAG_PROGRAM || value.shape == 0 || value.shape > SHAPES ||
        value.bits >= STATIC_PROGRAMS + program_count) fail("invalid Program tag");
    if (value.bits < STATIC_PROGRAMS) {
        if (!static_programs[value.bits].valid || static_programs[value.bits].shape != value.shape)
            fail("invalid static Program shape");
    } else if (programs[value.bits - STATIC_PROGRAMS].shape != value.shape)
        fail("invalid dynamic Program shape");
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
static const Aggregate *aggregate(Value value, unsigned kind) {
    if (value.tag != kind || value.shape == 0 || value.shape > SHAPES ||
        value.bits >= aggregate_count) fail("invalid aggregate tag");
    const Aggregate *item = &aggregates[value.bits];
    if (item->kind != kind || item->shape != value.shape ||
        item->length != (kind == TAG_PAIR ? 2u : 1u))
        fail("invalid aggregate reference");
    return item;
}
static const Aggregate *list(Value value) {
    if (value.tag != TAG_LIST || value.shape == 0 || value.shape > SHAPES ||
        value.bits >= aggregate_count) fail("invalid List tag");
    const Aggregate *item = &aggregates[value.bits];
    if (item->shape != value.shape || item->length > LIST_LIMIT)
        fail("invalid List shape or length");
    if (item->kind == AGG_LIST_NIL) {
        if (item->length != 0) fail("invalid nil length");
    } else if (item->kind == AGG_LIST_CONS) {
        Value tail = item->right;
        if (item->length == 0 || tail.tag != TAG_LIST || tail.shape != value.shape ||
            tail.bits >= value.bits) fail("invalid List tail");
        const Aggregate *previous = &aggregates[tail.bits];
        if ((previous->kind != AGG_LIST_NIL && previous->kind != AGG_LIST_CONS) ||
            previous->shape != value.shape || previous->length != item->length - 1)
            fail("invalid List predecessor");
    } else fail("invalid List cell");
    return item;
}
static void check_value(Value value) {
    if (!shape_kind(value.tag, value.shape)) fail("invalid value shape or kind");
    switch (value.tag) {
    case TAG_I64: break;
    case TAG_UNIT: if (value.bits != 0) fail("invalid Unit"); break;
    case TAG_BOOL: if (value.bits > 1) fail("invalid Bool"); break;
    case TAG_TEXT:
        if (value.bits >= STATIC_TEXTS || !text_literals[value.bits].valid) fail("invalid Text reference");
        break;
    case TAG_PROGRAM:
        if (value.bits >= STATIC_PROGRAMS + program_count ||
            (value.bits < STATIC_PROGRAMS &&
             (!static_programs[value.bits].valid || static_programs[value.bits].shape != value.shape)) ||
            (value.bits >= STATIC_PROGRAMS &&
             programs[value.bits - STATIC_PROGRAMS].shape != value.shape))
            fail("invalid Program reference");
        break;
    case TAG_PAIR: case TAG_SUM_LEFT: case TAG_SUM_RIGHT:
        (void)aggregate(value, value.tag);
        break;
    case TAG_LIST: (void)list(value); break;
    default: fail("invalid value tag");
    }
}
static void new_aggregate(unsigned kind, unsigned shape, Value left, Value right) {
    if (aggregate_count == AGGREGATE_SLOTS) fail("aggregate slots exhausted");
    if (shape == 0 || shape > SHAPES) fail("invalid aggregate shape");
    if (kind != TAG_PAIR && kind != TAG_SUM_LEFT && kind != TAG_SUM_RIGHT)
        fail("invalid aggregate kind");
    size_t index = aggregate_count;
    aggregates[index] = (Aggregate){kind, shape, kind == TAG_PAIR ? 2u : 1u, left, right};
    ++aggregate_count;
    push(kind, index, shape);
}
static void new_list(unsigned kind, unsigned shape, Value head, Value tail, size_t size) {
    if (aggregate_count == AGGREGATE_SLOTS) fail("aggregate slots exhausted");
    if (shape == 0 || shape > SHAPES) fail("invalid List shape");
    if (kind == AGG_LIST_NIL) {
        if (size != 0) fail("invalid nil constructor");
    } else if (kind != AGG_LIST_CONS || size == 0 ||
               size > LIST_LIMIT || tail.shape != shape)
        fail("invalid cons constructor");
    size_t index = aggregate_count;
    aggregates[index] = (Aggregate){kind, shape, size, head, tail};
    ++aggregate_count;
    push(TAG_LIST, index, shape);
}
static Metrics metrics(uint64_t id) {
    if (id < STATIC_PROGRAMS) {
        const StaticProgram *program = &static_programs[id];
        if (!program->valid) fail("invalid static Program");
        return (Metrics){program->leaves, program->depth};
    }
    size_t index = (size_t)(id - STATIC_PROGRAMS);
    if (index >= program_count) fail("invalid Program reference");
    if (programs[index].kind != 1 && programs[index].kind != 2)
        fail("invalid Program kind");
    return (Metrics){programs[index].leaves, programs[index].depth};
}
static Metrics measure_data(Value root) {
    DataVisit pending[RECIPE_LIMIT];
    size_t count = 1, leaves = 0, depth = 1;
    pending[0] = (DataVisit){root, 1};
    while (count) {
        tick();
        DataVisit visit = pending[--count];
        Value value = visit.value;
        check_value(value);
        if (leaves == RECIPE_LIMIT) fail("recipe leaves exhausted");
        ++leaves;
        size_t level = visit.depth;
        if (level > DEPTH_LIMIT) fail("program depth exhausted");
        if (value.tag == TAG_PROGRAM) {
            Metrics subject = metrics(value.bits);
            if (subject.leaves > RECIPE_LIMIT - leaves) fail("recipe leaves exhausted");
            leaves += subject.leaves;
            if (subject.depth > DEPTH_LIMIT - level) fail("program depth exhausted");
            level += subject.depth;
        }
        if (level > DEPTH_LIMIT) fail("program depth exhausted");
        if (level > depth) depth = level;
        if (value.tag == TAG_PAIR || value.tag == TAG_SUM_LEFT || value.tag == TAG_SUM_RIGHT) {
            const Aggregate *item = aggregate(value, value.tag);
            if (level == DEPTH_LIMIT) fail("program depth exhausted");
            if (value.tag == TAG_PAIR) {
                if (count == RECIPE_LIMIT) fail("program depth exhausted");
                pending[count++] = (DataVisit){item->right, level + 1};
            }
            if (count == RECIPE_LIMIT) fail("program depth exhausted");
            pending[count++] = (DataVisit){item->left, level + 1};
        } else if (value.tag == TAG_LIST) {
            const Aggregate *item = list(value);
            if (item->kind == AGG_LIST_CONS) {
                if (level == DEPTH_LIMIT || count > RECIPE_LIMIT - 2)
                    fail("program depth exhausted");
                pending[count++] = (DataVisit){item->right, level + 1};
                pending[count++] = (DataVisit){item->left, level + 1};
            }
        }
    }
    return (Metrics){leaves, depth};
}
static uint64_t new_program(unsigned kind, unsigned shape, uint64_t left, uint64_t right, Value capture,
                            size_t leaves, size_t depth) {
    if (kind != 1 && kind != 2) fail("invalid Program constructor");
    if (shape == 0 || shape > SHAPES) fail("invalid Program shape");
    if (leaves > RECIPE_LIMIT) fail("recipe leaves exhausted");
    if (depth > DEPTH_LIMIT) fail("program depth exhausted");
    if (program_count == PROGRAM_SLOTS) fail("program slots exhausted");
    size_t index = program_count++;
    programs[index] = (Program){kind, shape, left, right, capture, leaves, depth};
    return STATIC_PROGRAMS + index;
}
static void enter(uint64_t program) {
    if (program >= STATIC_PROGRAMS + program_count) fail("invalid Program reference");
    if (frame_count == FRAME_SLOTS) fail("program frames exhausted");
    frames[frame_count++] = (Frame){program, 0, 0, {0, 0, 0}};
}
static void arith(unsigned operation, unsigned shape) {
    uint64_t right = pop_i64();
    uint64_t left = pop_i64();
    if (operation == 0) push(TAG_I64, left + right, shape);
    else if (operation == 1) push(TAG_I64, left - right, shape);
    else if (operation == 2) push(TAG_I64, left * right, shape);
    else push(TAG_BOOL, left == right, shape);
}
static void execute(Instruction instruction) {
    if (instruction.inputs > 3 || height < instruction.inputs)
        fail("input stack interface");
    for (size_t i = 0; i < instruction.inputs; ++i) {
        check_value(stack[height - instruction.inputs + i]);
        if (instruction.input_shape[i] == 0 ||
            stack[height - instruction.inputs + i].shape != instruction.input_shape[i])
            fail("input value shape");
    }
    switch (instruction.op) {
    case OP_I64: push(TAG_I64, instruction.bits, instruction.shape); break;
    case OP_BOOL: if (instruction.bits > 1) fail("invalid Bool"); push(TAG_BOOL, instruction.bits, instruction.shape); break;
    case OP_UNIT: push(TAG_UNIT, 0, instruction.shape); break;
    case OP_TEXT:
        if (instruction.bits >= STATIC_TEXTS || !text_literals[instruction.bits].valid) fail("invalid Text reference");
        push(TAG_TEXT, instruction.bits, instruction.shape);
        break;
    case OP_PAIR: {
        if (height < 2) fail("stack underflow");
        if (aggregate_count == AGGREGATE_SLOTS) fail("aggregate slots exhausted");
        Value right = pop();
        Value left = pop();
        new_aggregate(TAG_PAIR, instruction.shape, left, right);
        break;
    }
    case OP_UNPAIR: {
        if (!height) fail("stack underflow");
        if (height == STACK_SLOTS) fail("stack slots exhausted");
        Value value = pop();
        const Aggregate *pair = aggregate(value, TAG_PAIR);
        push_value(pair->left);
        push_value(pair->right);
        break;
    }
    case OP_INL: case OP_INR: {
        if (!height) fail("stack underflow");
        if (aggregate_count == AGGREGATE_SLOTS) fail("aggregate slots exhausted");
        Value payload = pop();
        new_aggregate(instruction.op == OP_INL ? TAG_SUM_LEFT : TAG_SUM_RIGHT,
                      instruction.shape, payload, (Value){0, 0, 0});
        break;
    }
    case OP_CASE: {
        if (height < 3) fail("stack underflow");
        Value right = pop_program();
        Value left = pop_program();
        Value sum = pop();
        if (sum.tag != TAG_SUM_LEFT && sum.tag != TAG_SUM_RIGHT) fail("invalid Sum tag");
        const Aggregate *branch = aggregate(sum, sum.tag);
        push_value(branch->left);
        enter(sum.tag == TAG_SUM_LEFT ? left.bits : right.bits);
        break;
    }
    case OP_NIL:
        new_list(AGG_LIST_NIL, instruction.shape,
                 (Value){0, 0, 0}, (Value){0, 0, 0}, 0);
        break;
    case OP_CONS: {
        if (height < 2) fail("stack underflow");
        if (aggregate_count == AGGREGATE_SLOTS) fail("aggregate slots exhausted");
        const Aggregate *previous = list(stack[height - 1]);
        if (previous->length == LIST_LIMIT) fail("list length exhausted");
        size_t size = previous->length + 1;
        Value tail = pop();
        Value head = pop();
        new_list(AGG_LIST_CONS, instruction.shape, head, tail, size);
        break;
    }
    case OP_LIST_CASE: {
        if (height < 3) fail("stack underflow");
        Value cons_branch = pop_program();
        Value nil_branch = pop_program();
        Value subject = pop();
        const Aggregate *item = list(subject);
        if (item->kind == AGG_LIST_NIL) {
            enter(nil_branch.bits);
        } else {
            push_value(item->left);
            push_value(item->right);
            enter(cons_branch.bits);
        }
        break;
    }
    case OP_DUP:
        if (!height) fail("stack underflow");
        push_value(stack[height - 1]);
        break;
    case OP_DROP: (void)pop(); break;
    case OP_SWAP:
        if (height < 2) fail("stack underflow");
        { Value temporary = stack[height - 1]; stack[height - 1] = stack[height - 2]; stack[height - 2] = temporary; }
        break;
    case OP_ADD: case OP_SUB: case OP_MUL: case OP_EQ:
        arith(instruction.op - OP_ADD, instruction.shape);
        break;
    case OP_STATIC:
        if (instruction.bits >= STATIC_PROGRAMS || !static_programs[instruction.bits].valid ||
            static_programs[instruction.bits].shape != instruction.shape)
            fail("invalid static Program");
        push(TAG_PROGRAM, instruction.bits, instruction.shape);
        break;
    case OP_QUOTE: {
        Value capture = pop();
        Metrics data = measure_data(capture);
        uint64_t program = new_program(1, instruction.shape, 0, 0, capture, data.leaves, data.depth);
        push(TAG_PROGRAM, program, instruction.shape);
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
        uint64_t program = new_program(2, instruction.shape, left.bits, right.bits, (Value){0, 0, 0},
                                       lhs.leaves + rhs.leaves, depth + 1);
        push(TAG_PROGRAM, program, instruction.shape);
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
        check_value(saved);
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
        APPEND_LITERAL("-");
        magnitude = (~bits) + UINT64_C(1);
    }
    int n = snprintf(digits, sizeof digits, "%llu", (unsigned long long)magnitude);
    if (n < 0 || (size_t)n >= sizeof digits) fail("decimal encoding");
    append_bytes(digits, (size_t)n);
}
static void emit_text(uint64_t id) {
    static const char hex[] = "0123456789abcdef";
    if (id >= STATIC_TEXTS || !text_literals[id].valid) fail("invalid Text reference");
    const StaticText *text = &text_literals[id];
    APPEND_LITERAL("{\"type\":\"Text\",\"value\":\"");
    for (size_t i = 0; i < text->length; ++i) {
        unsigned char byte = text->bytes[i];
        if (byte == '"' || byte == '\\') {
            append_byte('\\');
            append_byte(byte);
        } else if (byte < 0x20) {
            APPEND_LITERAL("\\u00");
            append_byte((unsigned char)hex[byte >> 4]);
            append_byte((unsigned char)hex[byte & 15]);
        } else append_byte(byte);
    }
    APPEND_LITERAL("\"}");
}
static void emit_stack(void) {
    APPEND_LITERAL("{\"stage\":\"native-runtime\",\"outcome\":\"normal\",\"stack\":[");
    for (size_t i = 0; i < height; ++i) {
        if (i) APPEND_LITERAL(",");
        if (stack[i].tag == TAG_I64) {
            APPEND_LITERAL("{\"type\":\"I64\",\"value\":\"");
            decimal(stack[i].bits);
            APPEND_LITERAL("\"}");
        } else if (stack[i].tag == TAG_BOOL) {
            if (stack[i].bits == 0) APPEND_LITERAL("{\"type\":\"Bool\",\"value\":false}");
            else if (stack[i].bits == 1) APPEND_LITERAL("{\"type\":\"Bool\",\"value\":true}");
            else fail("invalid Bool output");
        } else if (stack[i].tag == TAG_UNIT) {
            APPEND_LITERAL("{\"type\":\"Unit\"}");
        } else if (stack[i].tag == TAG_TEXT) {
            emit_text(stack[i].bits);
        } else fail("invalid output tag");
    }
    APPEND_LITERAL("]}");
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
    list_length: usize,
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

            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(UniqueValue(value)) = sequence.next_element()? {
                    values.push(value);
                }
                Ok(UniqueValue(Value::Array(values)))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some((key, UniqueValue(value))) =
                    map.next_entry::<String, UniqueValue>()?
                {
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
    decoder
        .end()
        .map_err(|error| format!("invalid control JSON: {error}"))?;
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
    if !matches!(
        expected.stack_out.as_slice(),
        [Ty::I64 | Ty::Bool | Ty::Unit | Ty::Text]
    ) {
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
    if limits.len() != 3 && (limits.len() != 4 || !limits.contains_key("list_length")) {
        return Err(
            "C limits require stack_slots, steps, output_bytes and optional list_length".into(),
        );
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
    let list_length = match limits.get("list_length") {
        Some(value) => value.as_u64().ok_or("invalid list_length")?,
        None => LIST_MAX as u64,
    };
    if !(1..=256).contains(&slots)
        || !(1..=4096).contains(&steps)
        || !(128..=16384).contains(&output)
        || output < 128 + slots * 64
        || list_length > LIST_MAX as u64
    {
        return Err("C limits exceed the supported bounded slice".into());
    }
    Ok((
        expected,
        Limits {
            slots: slots as usize,
            steps,
            output: output as usize,
            list_length: list_length as usize,
        },
    ))
}

struct LoweredProgram {
    code: String,
    recipe: Value,
    shape: usize,
    leaves: usize,
    depth: usize,
}

fn shape_id<'a>(shapes: &mut Vec<&'a Ty>, ty: &'a Ty) -> Result<usize, String> {
    if let Some(index) = shapes.iter().position(|known| *known == ty) {
        return Ok(index + 1);
    }
    if shapes.len() == SHAPE_MAX {
        return Err("checked C type shapes exhausted".into());
    }
    shapes.push(ty);
    Ok(shapes.len())
}

fn bind_texts(body: &noble_kernel::execution::Body) -> Result<Vec<Option<&str>>, String> {
    let mut texts = vec![None; body.candidate.nodes.len()];
    for text in &body.texts {
        let slot = texts
            .get_mut(text.node.0 as usize)
            .ok_or("Text metadata node out of bounds")?;
        if !matches!(
            body.candidate.nodes.get(text.node.0 as usize),
            Some(Node::Literal { lit: Lit::Text, .. })
        ) || slot.is_some()
        {
            return Err("Text metadata must bind exactly one Text literal".into());
        }
        *slot = Some(std::str::from_utf8(&text.bytes).map_err(|_| "invalid Text UTF-8")?);
    }
    if body
        .candidate
        .nodes
        .iter()
        .enumerate()
        .any(|(index, node)| {
            matches!(node, Node::Literal { lit: Lit::Text, .. }) != texts[index].is_some()
        })
    {
        return Err("missing or extraneous checked Text metadata".into());
    }
    Ok(texts)
}

fn lower_body<'a>(
    candidate: &Candidate,
    body: &[NodeId],
    kinds: &[Behavior],
    texts: &[Option<&str>],
    interfaces: &[Option<&'a noble_kernel::untrusted::Interface>],
    shapes: &mut Vec<&'a Ty>,
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
        let seen = visited
            .get_mut(index)
            .ok_or("invalid checked node reference")?;
        if *seen {
            return Err("repeated or cyclic C node reference".into());
        }
        *seen = true;
        let interface = interfaces
            .get(index)
            .and_then(|item| *item)
            .ok_or("missing checked C node derivation")?;
        if !interface.effects.is_empty() {
            return Err("effectful C node is unsupported".into());
        }
        let node = candidate.nodes.get(index).ok_or("missing checked node")?;
        let (op, bits, atom) = match node {
            Node::Literal {
                lit: Lit::I64(value),
                ..
            } => ("OP_I64", *value as u64, json!(["i64", value.to_string()])),
            Node::Literal {
                lit: Lit::Bool(value),
                ..
            } => ("OP_BOOL", u64::from(*value), json!(["bool", value])),
            Node::Literal { lit: Lit::Unit, .. } => ("OP_UNIT", 0, json!(["unit"])),
            Node::Literal { lit: Lit::Text, .. } => {
                let text = texts
                    .get(index)
                    .and_then(|value| *value)
                    .ok_or("missing checked Text payload")?;
                ("OP_TEXT", index as u64, json!(["text", text]))
            }
            Node::Quotation { body, .. } => {
                let (child_leaves, child_depth) = lower_body(
                    candidate,
                    body,
                    kinds,
                    texts,
                    interfaces,
                    shapes,
                    visited,
                    programs,
                    index + 1,
                )?;
                leaves = leaves
                    .checked_add(child_leaves)
                    .ok_or("recipe leaves exhausted")?;
                depth = depth.max(
                    child_depth
                        .checked_add(1)
                        .ok_or("program depth exhausted")?,
                );
                (
                    "OP_STATIC",
                    (index + 1) as u64,
                    json!(["quotation", index + 1]),
                )
            }
            Node::Invocation {
                def: Definition(word),
                ..
            } => {
                let kind = kinds
                    .get(*word as usize)
                    .ok_or("missing checked operation contract")?;
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
                    (13, Behavior::Pair) => "OP_PAIR",
                    (14, Behavior::Unpair) => "OP_UNPAIR",
                    (15, Behavior::Inl) => "OP_INL",
                    (16, Behavior::Inr) => "OP_INR",
                    (17, Behavior::Case) => "OP_CASE",
                    (18, Behavior::If) => "OP_IF",
                    (19, Behavior::Nil) => "OP_NIL",
                    (20, Behavior::Cons) => "OP_CONS",
                    (21, Behavior::ListCase) => "OP_LIST_CASE",
                    _ => return Err(format!("unsupported checked C operation {word}")),
                };
                (op, 0, json!(["word", word]))
            }
            Node::SlotInvoke { .. } | Node::SignedSelf { .. } => {
                return Err("unsupported checked C value, effect or live operation".into())
            }
        };
        let inputs = match op {
            "OP_DUP" | "OP_DROP" | "OP_QUOTE" | "OP_RUN" | "OP_UNPAIR" | "OP_INL" | "OP_INR" => 1,
            "OP_SWAP" | "OP_ADD" | "OP_SUB" | "OP_MUL" | "OP_EQ" | "OP_COMPOSE" | "OP_DIP"
            | "OP_PAIR" | "OP_CONS" => 2,
            "OP_IF" | "OP_CASE" | "OP_LIST_CASE" => 3,
            _ => 0,
        };
        let first = interface
            .stack_in
            .len()
            .checked_sub(inputs)
            .ok_or("incomplete checked C input interface")?;
        let mut input_shapes = [0usize; 3];
        for (entry, ty) in input_shapes
            .iter_mut()
            .zip(interface.stack_in[first..].iter())
        {
            *entry = shape_id(shapes, ty)?;
        }
        let produced = matches!(
            op,
            "OP_I64"
                | "OP_BOOL"
                | "OP_UNIT"
                | "OP_TEXT"
                | "OP_STATIC"
                | "OP_ADD"
                | "OP_SUB"
                | "OP_MUL"
                | "OP_EQ"
                | "OP_QUOTE"
                | "OP_COMPOSE"
                | "OP_PAIR"
                | "OP_INL"
                | "OP_INR"
                | "OP_NIL"
                | "OP_CONS"
        );
        let result_shape = if produced {
            let ty = interface
                .stack_out
                .last()
                .ok_or("missing checked C result")?;
            let right_kind = match op {
                "OP_I64" | "OP_ADD" | "OP_SUB" | "OP_MUL" => matches!(ty, Ty::I64),
                "OP_BOOL" | "OP_EQ" => matches!(ty, Ty::Bool),
                "OP_UNIT" => matches!(ty, Ty::Unit),
                "OP_TEXT" => matches!(ty, Ty::Text),
                "OP_STATIC" | "OP_QUOTE" | "OP_COMPOSE" => matches!(ty, Ty::Program(..)),
                "OP_PAIR" => matches!(ty, Ty::Pair(..)),
                "OP_INL" | "OP_INR" => matches!(ty, Ty::Sum(..)),
                "OP_NIL" | "OP_CONS" => matches!(ty, Ty::List(..)),
                _ => false,
            };
            if !right_kind {
                return Err("unsupported checked C result shape".into());
            }
            shape_id(shapes, ty)?
        } else {
            0
        };
        if matches!(node, Node::Quotation { .. }) {
            programs[index + 1]
                .as_mut()
                .ok_or("missing C quotation program")?
                .shape = result_shape;
        }
        code.push_str("    {");
        write!(code, "{op}, UINT64_C({bits}), {result_shape}, {inputs}, ")
            .map_err(|_| "C instruction output failure")?;
        code.push('{');
        write!(
            code,
            "{}, {}, {}",
            input_shapes[0], input_shapes[1], input_shapes[2]
        )
        .map_err(|_| "C instruction output failure")?;
        code.push_str("}},\n");
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
        shape: 0,
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
    let mut interfaces = vec![None; candidate.nodes.len()];
    for row in &checked.derivations {
        let entry = interfaces
            .get_mut(row.node.0 as usize)
            .ok_or("checked C derivation node out of bounds")?;
        if entry.replace(&row.interface).is_some() {
            return Err("duplicate checked C node derivation".into());
        }
    }
    let mut shapes = Vec::new();
    let mut programs = Vec::with_capacity(candidate.nodes.len() + 1);
    programs.resize_with(candidate.nodes.len() + 1, || None);
    lower_body(
        candidate,
        &candidate.body,
        &submission.environment.kinds,
        &texts,
        &interfaces,
        &mut shapes,
        &mut visited,
        &mut programs,
        0,
    )?;
    if visited.iter().any(|seen| !seen)
        || checked.derivations.len() != visited.len()
        || checked
            .derivations
            .iter()
            .any(|row| !visited.get(row.node.0 as usize).copied().unwrap_or(false))
    {
        return Err("checked C body omits or duplicates an accepted node".into());
    }
    let mut c = format!(
        "#define STACK_SLOTS {}\n#define STEP_LIMIT UINT64_C({})\n#define OUTPUT_BYTES {}\n#define PROGRAM_SLOTS {}\n#define AGGREGATE_SLOTS {}\n#define LIST_LIMIT {}\n#define SHAPES {}\n#define FRAME_SLOTS {}\n#define RECIPE_LIMIT {}\n#define DEPTH_LIMIT {}\n#define STATIC_PROGRAMS {}\n#define STATIC_TEXTS {}\n",
        limits.slots, limits.steps, limits.output,
        PROGRAM_MAX, AGGREGATE_MAX, limits.list_length, shapes.len(), FRAME_MAX,
        RECIPE_MAX, PROGRAM_DEPTH_MAX,
        programs.len(), texts.len().max(1)
    );
    c.push_str(RUNTIME_HEADER);
    c.push_str("static const unsigned char shape_tags[SHAPES + 1] = {0,\n");
    for ty in &shapes {
        let tag = match ty {
            Ty::I64 => "TAG_I64",
            Ty::Bool => "TAG_BOOL",
            Ty::Unit => "TAG_UNIT",
            Ty::Text => "TAG_TEXT",
            Ty::Program(_, _, effects) if effects.is_empty() => "TAG_PROGRAM",
            Ty::Pair(..) => "TAG_PAIR",
            Ty::Sum(..) => "TAG_SUM_LEFT",
            Ty::List(..) => "TAG_LIST",
            _ => return Err("unsupported checked C value shape".into()),
        };
        writeln!(c, "    {tag},").map_err(|_| "C shape output failure")?;
    }
    c.push_str("};\n");
    for (index, text) in texts.iter().enumerate() {
        if let Some(text) = text {
            write!(c, "static const unsigned char text_{index}[] = {{")
                .map_err(|_| "C output failure")?;
            for byte in text.as_bytes() {
                write!(c, "0x{byte:02x},").map_err(|_| "C output failure")?;
            }
            c.push_str("0};\n");
        }
    }
    c.push_str("static const StaticText text_literals[STATIC_TEXTS] = {\n");
    for (index, text) in texts.iter().enumerate() {
        if let Some(text) = text {
            writeln!(c, "    {{text_{index}, {}, 1}},", text.len())
                .map_err(|_| "C output failure")?;
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
                c.push_str(&format!(
                    "static const Instruction instructions_{index}[] = {{\n{}}};\n",
                    program.code
                ));
            }
        }
    }
    c.push_str("static const StaticProgram static_programs[STATIC_PROGRAMS] = {\n");
    for (index, program) in programs.iter().enumerate() {
        match program {
            Some(program) if !program.code.is_empty() => c.push_str(&format!(
                "    {{instructions_{index}, {}, 1, {}, {}, {}}},\n",
                program.code.lines().count(),
                program.shape,
                program.leaves,
                program.depth
            )),
            Some(program) => c.push_str(&format!(
                "    {{NULL, 0, 1, {}, {}, {}}},\n",
                program.shape, program.leaves, program.depth
            )),
            None => c.push_str("    {NULL, 0, 0, 0, 0, 0},\n"),
        }
    }
    c.push_str("};\n");
    c.push_str(RUNTIME);
    let output_shape = shapes
        .iter()
        .position(|ty| *ty == &expected.stack_out[0])
        .ok_or("checked C output shape was not produced")?
        + 1;
    let output_tag = match expected.stack_out.as_slice() {
        [Ty::I64] => "TAG_I64",
        [Ty::Bool] => "TAG_BOOL",
        [Ty::Unit] => "TAG_UNIT",
        [Ty::Text] => "TAG_TEXT",
        _ => return Err("unsupported C result type".into()),
    };
    c.push_str(&format!(
        "    if (height != 1 || stack[0].tag != {output_tag} || stack[0].shape != {output_shape}) fail(\"output interface\");\n",
    ));
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
    let recipes = programs
        .into_iter()
        .map(|program| program.map(|item| item.recipe))
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
            "aggregate_slots":AGGREGATE_MAX, "list_length":limits.list_length,
            "list_length_cap":LIST_MAX,
            "type_shapes":SHAPE_MAX, "frame_slots":FRAME_MAX,
            "recipe_leaves":RECIPE_MAX,
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
