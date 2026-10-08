/* Independent C11 one-state oracle for the live Wasm storage-read path.
 * Not an implementation of native dispatch, not installed code, and not an
 * accepted proof. A future source-bound gate must compile this exact file,
 * execute the selected installed Wasm helpers on identical imported bytes,
 * compare all results/traps, and reject mutated binaries before using it as
 * differential evidence. Unsigned arithmetic mirrors Wasm i32 modulo 2^32. */
#include <stddef.h>
#include <stdint.h>

static uint32_t load32(const uint8_t *memory, size_t at) {
    return (uint32_t)memory[at] | ((uint32_t)memory[at + 1] << 8)
        | ((uint32_t)memory[at + 2] << 16) | ((uint32_t)memory[at + 3] << 24);
}

static uint64_t load64(const uint8_t *memory, size_t at) {
    return (uint64_t)memory[at] | ((uint64_t)memory[at + 1] << 8)
        | ((uint64_t)memory[at + 2] << 16) | ((uint64_t)memory[at + 3] << 24)
        | ((uint64_t)memory[at + 4] << 32) | ((uint64_t)memory[at + 5] << 40)
        | ((uint64_t)memory[at + 6] << 48) | ((uint64_t)memory[at + 7] << 56);
}

static void store32(uint8_t *slot, uint32_t value) {
    slot[0] = (uint8_t)value;
    slot[1] = (uint8_t)(value >> 8);
    slot[2] = (uint8_t)(value >> 16);
    slot[3] = (uint8_t)(value >> 24);
}

static void store64(uint8_t *slot, uint64_t value) {
    slot[0] = (uint8_t)value;
    slot[1] = (uint8_t)(value >> 8);
    slot[2] = (uint8_t)(value >> 16);
    slot[3] = (uint8_t)(value >> 24);
    slot[4] = (uint8_t)(value >> 32);
    slot[5] = (uint8_t)(value >> 40);
    slot[6] = (uint8_t)(value >> 48);
    slot[7] = (uint8_t)(value >> 56);
}

/* Returns -1 for an invalid initial memory/cursor, 0 for dead, 1 for live. */
int noble_lslot_storage_is_live(const uint8_t *memory, size_t length,
                               uint32_t heap_cursor, uint32_t handle) {
    if (!memory || length != 20u * 65536u || heap_cursor > 4096u) return -1;
    uint32_t slot = handle & 4095u;
    if (handle < 4096u || slot >= heap_cursor) return 0;
    size_t at = 65536u + (size_t)slot * 48u;
    uint32_t id = load32(memory, at + 4u);
    uint32_t kind = load32(memory, at) & 255u;
    return id == handle && (uint32_t)(kind - 1u) <= 16u;
}

/* 0 means a returned child (including null); -1 denotes the Wasm trap.
 * The installed helper accepts but does not dereference its parent argument. */
int noble_lslot_linear_child(const uint8_t *memory, size_t length,
                             uint32_t heap_cursor, uint32_t child,
                             uint32_t parent, uint32_t *result) {
    (void)parent;
    if (!result || (child && noble_lslot_storage_is_live(
            memory, length, heap_cursor, child) != 1)) return -1;
    *result = child;
    return 0;
}

/* Exact i32 loads for a/b/c and x/y/z; only a/b/c validate the loaded
 * non-null child handle through linear_child. This has not been compiled or
 * compared against an executing installed Wasm binary. */
int noble_lslot_read_field32(const uint8_t *memory, size_t length,
                             uint32_t heap_cursor, uint32_t handle,
                             uint32_t offset, uint32_t *result) {
    if (!result || (offset != 16u && offset != 20u && offset != 24u
                    && offset != 28u && offset != 32u && offset != 36u)
        || noble_lslot_storage_is_live(memory, length, heap_cursor, handle) != 1) return -1;
    size_t at = 65536u + (size_t)(handle & 4095u) * 48u;
    uint32_t value = load32(memory, at + offset);
    if (offset <= 24u && noble_lslot_linear_child(
            memory, length, heap_cursor, value, handle, &value) != 0) return -1;
    *result = value;
    return 0;
}

/* Independent, bounded model of one direct quote_entry -> a -> unbox ->
 * push_tag step, not a selected C backend, executed differential, or checked
 * Wasm/C/Lean refinement.
 * Output pointers designate mutually disjoint caller-owned storage, also
 * disjoint from imported memory.
 * On success slot[0..3] is tag 1, slot[4..7] retains memory[4..7],
 * slot[8..15] is the signed I64 payload's little-endian bit pattern; sp is 1
 * and failure remains 0. No heap/sidecar refcount transitions occur.
 * Returns -1 without writing outputs for null pointers, invalid bounds,
 * nonzero failure/sp, quota refusal, or a mismatched/dead handle, kind or code.
 * Imports, tables, continuations and other entry paths are not inputs to this
 * oracle; a caller must rule them out, not infer their validity from success. */
int noble_lslot_scalar_quote(const uint8_t *memory, size_t length,
                             uint32_t heap_cursor, uint32_t environment,
                             uint32_t failure, uint32_t sp,
                             uint32_t operand_limit, uint32_t operand_peak,
                             uint32_t quote_invocations, uint8_t slot[16],
                             uint64_t *payload, uint32_t *next_operand_peak,
                             uint32_t *next_quote_invocations) {
    if (!memory || !slot || !payload || !next_operand_peak
        || !next_quote_invocations || length != 20u * 65536u
        || heap_cursor == 0u || heap_cursor > 4096u
        || failure != 0u || sp != 0u
        || operand_limit < 16u || operand_limit > 2048u
        || operand_peak > operand_limit) return -1;

    /* read_field32 independently validates both the environment and its
     * non-null child via storage_is_live, including the child's epoch header. */
    uint32_t child;
    if (noble_lslot_read_field32(memory, length, heap_cursor,
                                 environment, 16u, &child) != 0
        || child == 0u) return -1;
    size_t environment_at = 65536u + (size_t)(environment & 4095u) * 48u;
    size_t child_at = 65536u + (size_t)(child & 4095u) * 48u;
    if ((load32(memory, environment_at) & 255u) != 4u
        || load32(memory, environment_at + 28u) != 1u
        || (load32(memory, child_at) & 255u) != 1u) return -1;

    uint64_t bits = load64(memory, child_at + 8u);
    store32(slot, 1u);
    slot[4] = memory[4];
    slot[5] = memory[5];
    slot[6] = memory[6];
    slot[7] = memory[7];
    store64(slot + 8, bits);
    *payload = bits;
    *next_operand_peak = operand_peak > 16u ? operand_peak : 16u;
    *next_quote_invocations = quote_invocations + 1u;
    return 0;
}
