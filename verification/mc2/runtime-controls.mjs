#!/usr/bin/env node
// Run only with the selected immutable Node from the runtime configuration.
// Both engines independently prepare byte-identical compiler-produced Wasm;
// neither outcome is a static preparation failure relabelled as fuel exhaustion.
import { readFileSync, mkdtempSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CoreEngine } from '../../crates/noble-cli/src/core/runtime/host.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const [watPath, sourcePath, directory] = process.argv.slice(2);
if (!watPath || !sourcePath || !directory || process.argv.length !== 5) {
  throw new Error('usage: SELECTED_NODE runtime-controls.mjs WAT SOURCE ARTIFACT_DIRECTORY');
}
const configuration = JSON.parse(readFileSync(join(root, 'crates/noble-cli/src/core/runtime/config.json')));
const abi = JSON.parse(readFileSync(join(root, 'crates/noble-cli/src/core/runtime/abi.json')));
const wat = readFileSync(watPath), source = readFileSync(sourcePath);
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const receipt = { schema: 'noble-mc2-runtime-fuel/v1', input: {
  source_path: sourcePath, source_sha256: digest(source), wat_path: watPath, wat_sha256: digest(wat),
}, baseline: null, exhausted: null, limits: { baseline: abi.limits_maximum, exhausted: { steps: 0 } } };
const baseline = new CoreEngine(configuration, abi, { artifacts: mkdtempSync(join(directory, 'baseline-')) });
try {
  receipt.preparation = baseline.prepare(wat, source);
  const before = baseline.compilations;
  receipt.baseline = baseline.execute();
  if (baseline.compilations !== before) throw new Error('baseline execution triggered compilation');
} finally { baseline.close(); }
const exhausted = new CoreEngine(configuration, abi, { artifacts: mkdtempSync(join(directory, 'exhausted-')) });
try {
  receipt.exhausted_preparation = exhausted.prepare(wat, source);
  if (receipt.exhausted_preparation.module.wasm_sha256 !== receipt.preparation.module.wasm_sha256) {
    throw new Error('exhausted module differs from reviewed preparation');
  }
  const before = exhausted.compilations;
  receipt.exhausted = exhausted.execute({ limits: { steps: 0 } });
  if (exhausted.compilations !== before) throw new Error('exhausted execution triggered compilation');
} finally { exhausted.close(); }
console.log(JSON.stringify(receipt));
