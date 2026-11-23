// Strategy-fragment harness modules for the failure-class gate. These are not shipped
// profiles: they assemble the *shipped* accounting/storage fragments behind a
// one-operation driver so the failure classes that the shipped profiles cannot reach
// (a fixed pool with a lowered memory maximum for F3, a pool filled past 4096 live
// slots for F2) are exercised on the real fragment code. The receipt labels them as
// harness modules, never as the shipped profile.
import fs from 'node:fs';
import path from 'node:path';

import { root, sha256 } from './core.mjs';

const runtime = path.join(root, 'crates/noble-wasm/runtime');
const read = name => fs.readFileSync(path.join(runtime, name), 'utf8');

const globals = ({ cellLimit = 4096, allocationLimit = 196608 } = {}) => `
  (global $failure (mut i32) (i32.const 0))
  (global $quota_reason (mut i32) (i32.const 0))
  (global $phase (mut i32) (i32.const 0))
  (global $heap_cursor (mut i32) (i32.const 0))
  (global $heap_baseline (mut i32) (i32.const 0))
  (global $cell_limit i32 (i32.const ${cellLimit}))
  (global $allocation_limit (mut i32) (i32.const ${allocationLimit}))
  (global $recipe_limit (mut i32) (i32.const 256))
  (global $depth_limit (mut i32) (i32.const 256))
  (global $operand_limit (mut i32) (i32.const 2048))
  (global $continuation_limit (mut i32) (i32.const 4096))
  (global $step_limit (mut i32) (i32.const 100000))
  (global $allocated_total (mut i64) (i64.const 0))
  (global $released_total (mut i64) (i64.const 0))
  (global $allocation_peak (mut i32) (i32.const 0))
  (global $operand_peak (mut i32) (i32.const 0))
  (global $continuation_peak (mut i32) (i32.const 0))
  (global $steps (mut i32) (i32.const 0))
  (global $quote_invocations (mut i32) (i32.const 0))
  (global $sp (mut i32) (i32.const 0))
  (global $cp (mut i32) (i32.const 0))
  (global $observed_program (mut i32) (i32.const 0))
  (global $recipe_count (mut i32) (i32.const 0))
  (global $reflection_steps (mut i32) (i32.const 0))
  (global $rp (mut i32) (i32.const 0))
`;

const DRIVER = `
  ;; One driver allocation: the same $reserve/$new path the shipped source ABI uses.
  (func (export "alloc_one") (result i32)
    (call $new (i32.const 1) (i64.const 0) (i32.const 0) (i32.const 0) (i32.const 0)
      (i32.const 0) (i32.const 0) (i32.const 0) (i32.const 0) (i32.const 0)))
  (func (export "live_count") (result i32) (call $storage_live_count))
  (func (export "heap_cursor") (result i32) (global.get $heap_cursor))
  (func (export "failure") (result i32) (global.get $failure))
  (func (export "quota_reason") (result i32) (global.get $quota_reason))
  (func (export "reset") (result i32)
    (global.set $failure (i32.const 0))
    (global.set $quota_reason (i32.const 0))
    (global.set $phase (i32.const 0))
    (global.set $heap_cursor (i32.const 0))
    (global.set $heap_baseline (i32.const 0))
    (global.set $allocated_total (i64.const 0))
    (global.set $released_total (i64.const 0))
    (global.set $allocation_peak (i32.const 0))
    (global.set $steps (i32.const 0))
    (i32.const 0))
`;

// Fixed-pool linear storage (A1/A2/A5 fragments) with an explicit memory maximum.
export function linearFragment({ maxPages = 4, cellLimit = 4096, allocationLimit = 196608 } = {}) {
  return `(module
  (type $entry (func (param i32)))
  (memory (export "memory") 1 ${maxPages})
${globals({ cellLimit, allocationLimit })}
${read('linear-address.wat')}
${read('accounting.wat')}
${read('linear-storage.wat')}
${DRIVER}
)
`;
}

// Live-slot storage (A4 fragments): free list, deferred RC and the 4,096-slot pool.
export function liveFragment() {
  return `(module
  (type $entry (func (param i32)))
  (memory (export "memory") 20 20)
${globals()}
${read('live-accounting.wat')}
${read('live-linear-storage.wat')}
${DRIVER}
)
`;
}

export function writeFragment(directory, name, text) {
  const file = path.join(directory, `${name}.wat`);
  fs.writeFileSync(file, text);
  return { file, sha256: sha256(fs.readFileSync(file)) };
}
