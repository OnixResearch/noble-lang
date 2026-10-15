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
