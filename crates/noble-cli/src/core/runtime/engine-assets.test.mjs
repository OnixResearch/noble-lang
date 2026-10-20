import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { CoreEngine } from './host.mjs';

const runtime = path.dirname(fileURLToPath(import.meta.url));
const selection = JSON.parse(fs.readFileSync(path.join(runtime, 'config.json'), 'utf8'));
const abi = JSON.parse(fs.readFileSync(path.join(runtime, 'abi.json'), 'utf8'));
const assembler = process.env.NOBLE_CALCULATOR_ASSEMBLER ?? process.env.CARGO_BIN_EXE_noble;

const wat = Buffer.from('(module)');
const source = Buffer.from('unit');
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');

test('calculator-owned compilation keeps provenance but not disposable WAT and Wasm',
  { skip: !assembler && 'requires a built noble calculator assembler' }, () => {
    const explicit = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-calculator-audit-'));
    let owned;
    let audited;
    try {
      const options = { calculator: true, calculator_shell_state: true, calculator_assembler: assembler };
      owned = new CoreEngine(selection, abi, options);
      const ownedReady = owned.prepare(wat, source);
      assert.equal(ownedReady.outcome, 'ready');
      assert.equal(ownedReady.module.wat_sha256, sha256(wat));
      assert.equal(fs.readFileSync(path.join(owned.directory, 'module-1.noble'), 'utf8'), 'unit');
      assert.ok(fs.existsSync(path.join(owned.directory, 'module-1-calculator-assemble.json')));
      assert.ok(!fs.existsSync(path.join(owned.directory, 'module-1.wat')));
      assert.ok(!fs.existsSync(path.join(owned.directory, 'module-1.wasm')));

      audited = new CoreEngine(selection, abi, { ...options, artifacts: explicit });
      const auditReady = audited.prepare(wat, source);
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
