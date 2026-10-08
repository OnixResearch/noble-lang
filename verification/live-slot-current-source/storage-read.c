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
