import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { CoreEngine } from './host.mjs';

const runtime = path.dirname(fileURLToPath(import.meta.url));
const wasmRuntime = path.resolve(runtime, '../../../../noble-wasm/runtime');
const selection = JSON.parse(fs.readFileSync(path.join(runtime, 'config.json'), 'utf8'));
const abi = JSON.parse(fs.readFileSync(path.join(runtime, 'abi.json'), 'utf8'));
const declaredAbi = fs.readFileSync(path.join(runtime, 'declared-abi.json'), 'utf8');
const assembler = process.env.NOBLE_CALCULATOR_ASSEMBLER ?? process.env.CARGO_BIN_EXE_noble;

const wat = Buffer.from('(module)');
const source = Buffer.from('unit');
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');

test('signed declared text cannot own the live Core bridge', () => {
  // The signed source epoch and the in-process live bridge are disjoint
  // profiles. The declared-ABI guard refuses each combination before any
  // table entry, source epoch or observation store exists, so the later
  // `signed source epoch cannot own the live Core bridge` guard is defense in
  // depth: no constructor argument reaches it.
  assert.throws(() => new CoreEngine(selection, abi, {
    declared_modules: true, text_byte_cursor: true, signed_self_recursion: true,
    declared_extension: declaredAbi, in_process_live: true,
  }), /text byte cursor requires a session or compile/);
  assert.throws(() => new CoreEngine(selection, abi, {
    declared_modules: true, signed_self_recursion: true,
    declared_extension: declaredAbi, in_process_live: true,
  }), /signed recursion requires the declared module host boundary/);
  const signed = new CoreEngine(selection, abi, { declared_modules: true,
    text_byte_cursor: true, signed_self_recursion: true, declared_extension: declaredAbi });
  try {
    assert.equal(signed.profile, 'Signed-Declared-Text-v1');
    assert.equal(signed.inProcessLive, false);
  } finally {
    signed.close();
  }
});

test('calculator-owned compilation keeps provenance but not disposable WAT and Wasm',
  { skip: !assembler && 'requires a built noble calculator assembler' }, () => {
    const explicit = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-calculator-audit-'));
    let owned;
    let audited;
    try {
      const options = { calculator: true, calculator_shell_state: true, calculator_assembler: assembler };
      owned = new CoreEngine(selection, abi, options);
      const ownedReady = owned.prepare(wat, source, 1, 0);
      assert.equal(ownedReady.outcome, 'ready');
      assert.equal(ownedReady.module.wat_sha256, sha256(wat));
      assert.equal(fs.readFileSync(path.join(owned.directory, 'module-1.noble'), 'utf8'), 'unit');
      assert.ok(fs.existsSync(path.join(owned.directory, 'module-1-calculator-assemble.json')));
      assert.ok(!fs.existsSync(path.join(owned.directory, 'module-1.wat')));
      assert.ok(!fs.existsSync(path.join(owned.directory, 'module-1.wasm')));

      audited = new CoreEngine(selection, abi, { ...options, artifacts: explicit });
      const auditReady = audited.prepare(wat, source, 1, 0);
      assert.equal(auditReady.outcome, 'ready');
      assert.deepEqual(fs.readFileSync(path.join(audited.directory, 'module-1.wat')), wat);
      const wasm = fs.readFileSync(path.join(audited.directory, 'module-1.wasm'));
      assert.ok(wasm.length > 0);
      assert.equal(auditReady.module.wasm_sha256, sha256(wasm));
      assert.equal(ownedReady.module.wasm_sha256, auditReady.module.wasm_sha256);
      assert.equal(ownedReady.module.assembler_sha256, auditReady.module.assembler_sha256);
      assert.equal(ownedReady.module.assembly_toolchain, auditReady.module.assembly_toolchain);

      const ownedDirectory = owned.directory;
      owned.close();
      owned = null;
      audited.close();
      audited = null;
      assert.ok(!fs.existsSync(ownedDirectory));
      assert.ok(fs.existsSync(path.join(explicit, 'engine', 'module-1.wasm')));
    } finally {
      if (owned) owned.close();
      if (audited) audited.close();
      fs.rmSync(explicit, { recursive: true, force: true });
    }
  });

test('calculator worker reuses assembled modules only for byte-identical WAT',
  { skip: !assembler && 'requires a built noble calculator assembler' }, () => {
    const base = { calculator: true, calculator_shell_state: true, calculator_assembler: assembler,
      calculator_trace: true };
    const reusing = new CoreEngine(selection, abi, { ...base, calculator_module_cache: true });
    const plain = new CoreEngine(selection, abi, base);
    try {
      const prepare = (engine, bytes, submission) => {
        const ready = engine.prepare(bytes, source, submission, 0);
        engine.pending = null;
        return ready;
      };
      const first = prepare(reusing, wat, 1);
      const again = prepare(reusing, Buffer.from(wat), 2);
      const other = prepare(reusing, Buffer.from('(module (memory 1))'), 3);
      assert.deepEqual([first, again, other].map(ready => ready.timing.worker_cache),
        ['miss', 'hit', 'miss']);
      assert.equal(again.module.wasm_sha256, first.module.wasm_sha256);
      assert.notEqual(other.module.wasm_sha256, first.module.wasm_sha256);
      const record = JSON.parse(fs.readFileSync(
        path.join(reusing.directory, 'module-2-calculator-assemble.json'), 'utf8'));
      assert.equal(record.reused_assembly, true);
      assert.deepEqual([prepare(plain, wat, 1), prepare(plain, wat, 2)]
        .map(ready => ready.timing.worker_cache), ['miss', 'miss']);
      // Every calculator module must carry its host-validated generation.
      assert.throws(() => reusing.prepare(wat, source, 4), /invalid submit generation/);
    } finally {
      reusing.close();
      plain.close();
    }
  });

function assembleWat(watText) {
  const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-wat-test-'));
  try {
    const watFile = path.join(scratch, 'module.wat');
    const wasmFile = path.join(scratch, 'module.wasm');
    fs.writeFileSync(watFile, watText);
    const parsed = spawnSync(selection.tools.wasm_tools.path, ['parse', watFile, '-o', wasmFile],
      { encoding: 'utf8' });
    assert.equal(parsed.status, 0, parsed.stderr);
    return new WebAssembly.Module(fs.readFileSync(wasmFile));
  } finally {
    fs.rmSync(scratch, { recursive: true, force: true });
  }
}

function coreStatusModuleWat({ submitBody, extraImports = '' }) {
  return `(module
  (import "noble" "memory" (memory 16 16))
  (import "noble" "table" (table 16384 16384 funcref))
  (import "noble" "failure" (global $failure (mut i32)))
  (import "noble" "quota_reason" (global $quota_reason (mut i32)))
  (import "noble" "sp" (global $sp (mut i32)))
  ${extraImports}
  (func (export "begin")
    (param i32 i32 i32 i32 i32 i32) (result i32)
    (global.set $failure (i32.const 0))
    (global.set $quota_reason (i32.const 0))
    (i32.const 0))
  (func (export "submit") (result i32)
    ${submitBody})
  (func (export "metric") (param $key i32) (result i64)
    (if (i32.eq (local.get $key) (i32.const 0))
      (then (return (i64.extend_i32_u (global.get $failure)))))
    (if (i32.eq (local.get $key) (i32.const 1))
      (then (return (i64.extend_i32_u (global.get $quota_reason)))))
    (if (i32.eq (local.get $key) (i32.const 12))
      (then (return (i64.extend_i32_u (global.get $sp)))))
    (i64.const 0))
  (func (export "stack_length") (result i32) (i32.const 0))
  (func (export "stack_kind") (param i32) (result i32) (i32.const 0))
  (func (export "stack_value") (param i32) (result i64) (i64.const 0))
)`;
}

function fragmentGlobalsWat({ cellLimit = 4096, allocationLimit = 196608 } = {}) {
  return `
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
}

test('abi.json defines quota_reason 1..9 and distinct F1..F3 failure_class mappings', () => {
  assert.deepEqual(abi.quota_reason, {
    1: 'allocation',
    2: 'recipe leaves',
    3: 'program graph depth',
    4: 'operand bytes',
    5: 'continuation bytes',
    6: 'execution work',
    7: 'reflection/type-observation work',
    8: 'strategy exhaustion',
    9: 'memory growth',
  });
  assert.deepEqual(abi.failure_class, {
    1: 'F1-logical-quota',
    2: 'F1-logical-quota',
    3: 'F1-logical-quota',
    4: 'F1-logical-quota',
    5: 'F1-logical-quota',
    6: 'F1-logical-quota',
    7: 'F1-logical-quota',
    8: 'F2-strategy-exhaustion',
    9: 'F3-memory-growth',
  });
});

test('CoreEngine.execute emits distinct F1-F4 failure_class and null for normal/non-OOM traps', () => {
  const runWat = (watText, { beforeExecute = null } = {}) => {
    const engine = new CoreEngine(selection, abi, {});
    try {
      const ready = engine.prepare(Buffer.from(watText), source, 1);
      assert.equal(ready.outcome, 'ready');
      if (beforeExecute) beforeExecute(engine);
      return engine.execute();
    } finally {
      engine.close();
    }
  };

  const normal = runWat(coreStatusModuleWat({ submitBody: '(i32.const 0)' }));
  assert.equal(normal.outcome, 'normal');
  assert.equal(normal.quota_reason, null);
  assert.equal(normal.failure_class, null);

  const trap = runWat(coreStatusModuleWat({ submitBody: '(unreachable) (i32.const 0)' }));
  assert.equal(trap.outcome, 'trap');
  assert.equal(trap.quota_reason, null);
  assert.equal(trap.failure_class, null);

  const f1 = runWat(coreStatusModuleWat({
    submitBody: '(global.set $failure (i32.const 2)) (global.set $quota_reason (i32.const 1)) (i32.const 2)',
  }));
  assert.equal(f1.outcome, 'runtime-exhausted');
  assert.equal(f1.quota_reason, 'allocation');
  assert.equal(f1.failure_class, 'F1-logical-quota');

  const f1Work = runWat(coreStatusModuleWat({
    submitBody: '(global.set $failure (i32.const 2)) (global.set $quota_reason (i32.const 6)) (i32.const 2)',
  }));
  assert.equal(f1Work.outcome, 'runtime-exhausted');
  assert.equal(f1Work.quota_reason, 'execution work');
  assert.equal(f1Work.failure_class, 'F1-logical-quota');

  const f2 = runWat(coreStatusModuleWat({
    submitBody: '(global.set $failure (i32.const 2)) (global.set $quota_reason (i32.const 8)) (i32.const 2)',
  }));
  assert.equal(f2.outcome, 'runtime-exhausted');
  assert.equal(f2.quota_reason, 'strategy exhaustion');
  assert.equal(f2.failure_class, 'F2-strategy-exhaustion');

  const f3 = runWat(coreStatusModuleWat({
    submitBody: '(global.set $failure (i32.const 2)) (global.set $quota_reason (i32.const 9)) (i32.const 2)',
  }));
  assert.equal(f3.outcome, 'runtime-exhausted');
  assert.equal(f3.quota_reason, 'memory growth');
  assert.equal(f3.failure_class, 'F3-memory-growth');

  const f4 = runWat(coreStatusModuleWat({
    extraImports: '(import "noble" "test_emit" (func $test_emit (param i32 i32) (result i32)))',
    submitBody: '(drop (call $test_emit (i32.const 0) (i32.const 0))) (i32.const 0)',
  }), {
    beforeExecute: engine => {
      engine.trace = {
        length: 0,
        slice: () => [],
        push: () => { throw new RangeError('Array buffer allocation failed'); },
      };
    },
  });
  assert.equal(f4.outcome, 'trap');
  assert.equal(f4.failure_class, 'F4-engine-oom');
});

test('preamble helpers classify F1-F4 consistently across Core and Slot hosts', async () => {
  const preamble = fs.readFileSync(path.join(runtime, 'preamble.mjs'), 'utf8');
  const mod = await import(`data:text/javascript;base64,${Buffer.from(
    `${preamble}\nexport { isEngineOomError, classifyFailure };\n`,
  ).toString('base64')}`);

  assert.equal(mod.isEngineOomError(new RangeError('Array buffer allocation failed')), true);
  assert.equal(mod.isEngineOomError(new RangeError('WebAssembly.Memory(): could not allocate memory')), true);
  assert.equal(mod.isEngineOomError(Object.assign(new Error('oom'), { code: 'ERR_MEMORY_ALLOCATION_FAILED' })), true);
  assert.equal(mod.isEngineOomError(new RangeError('Maximum call stack size exceeded')), false);
  assert.equal(mod.isEngineOomError(new WebAssembly.RuntimeError('unreachable')), false);

  assert.equal(mod.classifyFailure(abi, { status: 0, quotaCode: 1 }), null);
  assert.equal(mod.classifyFailure(abi, { status: 6, quotaCode: 0 }), null);
  assert.equal(mod.classifyFailure(abi, { status: 6, quotaCode: 0, oom: true }), 'F4-engine-oom');
  assert.equal(mod.classifyFailure(abi, { status: 2, quotaCode: 1 }), 'F1-logical-quota');
  assert.equal(mod.classifyFailure(abi, { status: 2, quotaCode: 8 }), 'F2-strategy-exhaustion');
  assert.equal(mod.classifyFailure(abi, { status: 2, quotaCode: 9 }), 'F3-memory-growth');
  assert.equal(mod.classifyFailure(abi, { status: 2, quotaCode: 0, extraExhausted: 'typed-input-nodes' }),
    'F1-logical-quota');
});

test('live-accounting.wat and accounting.wat emit distinct quota codes 1, 8, and 9', () => {
  const readWat = name => fs.readFileSync(path.join(wasmRuntime, name), 'utf8');
  const driver = `
    (func (export "alloc_one") (result i32)
      (call $new (i32.const 1) (i64.const 0) (i32.const 0) (i32.const 0) (i32.const 0)
        (i32.const 0) (i32.const 0) (i32.const 0) (i32.const 0) (i32.const 0)))
    (func (export "failure") (result i32) (global.get $failure))
    (func (export "quota_reason") (result i32) (global.get $quota_reason))
  `;

  const liveFullPool = new WebAssembly.Instance(assembleWat(`(module
    (type $entry (func (param i32)))
    (memory (export "memory") 20 20)
    ${fragmentGlobalsWat({ allocationLimit: 196608 })}
    ${readWat('live-accounting.wat')}
    ${readWat('live-linear-storage.wat')}
    ${driver}
  )`), Object.create(null)).exports;
  for (let i = 0; i < 4096; i += 1) assert.notEqual(liveFullPool.alloc_one(), 0);
  assert.equal(liveFullPool.alloc_one(), 0);
  assert.equal(liveFullPool.failure(), 1);
  assert.equal(liveFullPool.quota_reason(), 8);

  const liveByteQuota = new WebAssembly.Instance(assembleWat(`(module
    (type $entry (func (param i32)))
    (memory (export "memory") 20 20)
    ${fragmentGlobalsWat({ allocationLimit: 96 })}
    ${readWat('live-accounting.wat')}
    ${readWat('live-linear-storage.wat')}
    ${driver}
  )`), Object.create(null)).exports;
  assert.notEqual(liveByteQuota.alloc_one(), 0);
  assert.notEqual(liveByteQuota.alloc_one(), 0);
  assert.equal(liveByteQuota.alloc_one(), 0);
  assert.equal(liveByteQuota.failure(), 1);
  assert.equal(liveByteQuota.quota_reason(), 1);

  const liveLowPages = new WebAssembly.Instance(assembleWat(`(module
    (type $entry (func (param i32)))
    (memory (export "memory") 2 2)
    ${fragmentGlobalsWat({ allocationLimit: 196608 })}
    ${readWat('live-accounting.wat')}
    ${readWat('live-linear-storage.wat')}
    ${driver}
  )`), Object.create(null)).exports;
  assert.equal(liveLowPages.alloc_one(), 0);
  assert.equal(liveLowPages.failure(), 1);
  assert.equal(liveLowPages.quota_reason(), 9);

  const linearGrowthFail = new WebAssembly.Instance(assembleWat(`(module
    (type $entry (func (param i32)))
    (memory (export "memory") 1 2)
    ${fragmentGlobalsWat({ cellLimit: 4096, allocationLimit: 196608 })}
    ${readWat('linear-address.wat')}
    ${readWat('accounting.wat')}
    ${readWat('linear-storage.wat')}
    ${driver}
  )`), Object.create(null)).exports;
  for (let i = 0; i < 1365; i += 1) assert.notEqual(linearGrowthFail.alloc_one(), 0);
  assert.equal(linearGrowthFail.alloc_one(), 0);
  assert.equal(linearGrowthFail.failure(), 1);
  assert.equal(linearGrowthFail.quota_reason(), 9);

  const linearCellQuota = new WebAssembly.Instance(assembleWat(`(module
    (type $entry (func (param i32)))
    (memory (export "memory") 1 4)
    ${fragmentGlobalsWat({ cellLimit: 2, allocationLimit: 196608 })}
    ${readWat('linear-address.wat')}
    ${readWat('accounting.wat')}
    ${readWat('linear-storage.wat')}
    ${driver}
  )`), Object.create(null)).exports;
  assert.notEqual(linearCellQuota.alloc_one(), 0);
  assert.notEqual(linearCellQuota.alloc_one(), 0);
  assert.equal(linearCellQuota.alloc_one(), 0);
  assert.equal(linearCellQuota.failure(), 1);
  assert.equal(linearCellQuota.quota_reason(), 1);
});
