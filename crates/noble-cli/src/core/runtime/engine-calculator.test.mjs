import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { CoreEngine } from './host.mjs';

const runtime = path.dirname(fileURLToPath(import.meta.url));
const selection = JSON.parse(fs.readFileSync(path.join(runtime, 'config.json'), 'utf8'));
const abi = JSON.parse(fs.readFileSync(path.join(runtime, 'abi.json'), 'utf8'));
const assembler = process.env.NOBLE_CALCULATOR_ASSEMBLER ?? process.env.CARGO_BIN_EXE_noble;

// Boundary compaction renames cells, so no handle may persist outside Wasm.
test('calculator refuses host-held operand frames and host-supplied handles',
  { skip: !assembler && 'requires a built noble calculator assembler' }, () => {
    const engine = new CoreEngine(selection, abi,
      { calculator: true, calculator_shell_state: true, calculator_assembler: assembler });
    try {
      assert.throws(() => engine.park(), /calculator does not park operand frames/);
      assert.equal(engine.parked, null);
      assert.throws(() => engine.project(1), /calculator does not project host-supplied handles/);
    } finally {
      engine.close();
    }
  });

// Two distinct exported functions, so a test can tell table entries apart.
const tableFunction = () => new WebAssembly.Instance(new WebAssembly.Module(Buffer.from([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
  0x01, 0x04, 0x01, 0x60, 0x00, 0x00,
  0x03, 0x02, 0x01, 0x00,
  0x07, 0x05, 0x01, 0x01, 0x66, 0x00, 0x00,
  0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b,
]))).exports.f;

// A failed submission may rewrite any arena byte (including cells beyond the
// first 16 pages), table entry, ABI global, published signature or engine
// counter, and may poison the instance. Restore returns all of them.
test('calculator restore returns the whole committed arena byte for byte',
  { skip: !assembler && 'requires a built noble calculator assembler' }, () => {
    const engine = new CoreEngine(selection, abi,
      { calculator: true, calculator_shell_state: true, calculator_assembler: assembler });
    try {
      // The arena is at its 64-page maximum from the start: runtime growth
      // never resizes it, so every restore meets the committed size.
      assert.equal(engine.memory.buffer.byteLength, 64 * 65536);
      const memory = new Uint8Array(engine.memory.buffer);
      const [committedEntry, failedEntry] = [tableFunction(), tableFunction()];
      memory.fill(1, 65536, 65536 + 48);
      memory.fill(2, 4194304 - 48);
      engine.table.set(5, committedEntry);
      engine.shared.generation.value = 3;
      engine.shared.allocated_total.value = 7n;
      engine.signatures = ['[I64]'];
      const bytes = memory.slice();
      const globals = abi.globals.map(global => engine.shared[global.name].value);
      const committed = engine.calculatorCommit();
      assert.equal(committed.outcome, 'committed');

      memory.fill(0xab, 1048576);
      memory[0] = 0xcd;
      engine.table.set(5, failedEntry);
      engine.table.set(6, failedEntry);
      for (const global of abi.globals) {
        engine.shared[global.name].value = global.type === 'i64' ? 99n : 99;
      }
      engine.signatures = ['[I64]', '[Bool]'];
      engine.compilations = 9;
      engine.save('module-9.noble', 'failed line');
      engine.pending = { record: {} };
      engine.poisoned = true;
      assert.notDeepEqual(engine.calculatorArena(), committed.arena);
      assert.throws(() => engine.calculatorCommit(), /idle, unpoisoned/);

      const restored = engine.calculatorRestore();
      assert.equal(restored.outcome, 'restored');
      assert.deepEqual(restored.arena, committed.arena);
      assert.ok(Buffer.from(engine.memory.buffer).equals(Buffer.from(bytes)));
      assert.equal(engine.table.get(5), committedEntry);
      assert.equal(engine.table.get(6), null);
      assert.deepEqual(abi.globals.map(global => engine.shared[global.name].value), globals);
      assert.deepEqual(engine.signatures, ['[I64]']);
      assert.equal(engine.compilations, 0);
      assert.equal(engine.pending, null);
      assert.equal(engine.poisoned, false);
      assert.equal(fs.existsSync(path.join(engine.directory, 'module-9.noble')), false);
      // The commit survives its restore, so a later failure restores again.
      engine.poisoned = true;
      assert.deepEqual(engine.calculatorRestore().arena, committed.arena);
    } finally {
      engine.close();
    }
  });

test('calculator restore requires a commit',
  { skip: !assembler && 'requires a built noble calculator assembler' }, () => {
    const engine = new CoreEngine(selection, abi,
      { calculator: true, calculator_shell_state: true, calculator_assembler: assembler });
    try {
      assert.equal(engine.calculatorRestorable(), false);
      assert.throws(() => engine.calculatorRestore(), /no committed calculator arena/);
    } finally {
      engine.close();
    }
  });
