// Independent preparation for the ACORR controls: the counterexample call
// sources, the sealed wrapper renderings and the wasm-tools/text mutations are
// derived here, never through the admission code path. The gate hashes this
// file and every input it prepares; it never imports admission internals.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';

const digest = bytes => createHash('sha256').update(bytes).digest('hex');
export const textSha256 = text => digest(Buffer.from(text));

// The sealed v1 invocation-wrapper renderings (documented in the ACORR design
// and guard.rs). The gate writes these texts itself so a consumer rendering
// that drifts from the documented template is observed as a byte mismatch.
export function wrapperSource(template, literal) {
  return template === 'EqI64Literal'
    ? `[ swap dup ${literal} = [ swap run inr ] [ drop drop 1 inl ] if ]`
    : `[ swap dup ${literal} = [ drop drop 1 inl ] [ swap run inr ] if ]`;
}

// The certified call: the host-selected argument, the consumer-selected source
// text and, when a guard is bound, the wrapper applied to the subject program.
export function callSource(argument, source, wrapper) {
  return wrapper === null
    ? `${argument} ${source} run\n`
    : `${argument} ${source} ${wrapper} run\n`;
}

// Print/parse round-trip through the selected wasm-tools: the same independent
// tools the consumer invokes, but a different code path from admission.
export function watToWasm(wasmTools, environment, watPath, wasmPath) {
  const result = spawnSync(wasmTools, ['parse', watPath, '-o', wasmPath], {
    encoding: 'utf8', env: environment, timeout: 120000, maxBuffer: 64 * 1024 * 1024,
  });
  if (result.error || result.status !== 0) {
    throw Error(`wasm-tools parse failed: ${result.error?.message ?? result.status}\n${result.stderr}`);
  }
  return digest(readFileSync(wasmPath));
}

// A byte-level edit of a prepared pin document: only the named field changes.
export function editPins(path, changes) {
  const document = JSON.parse(readFileSync(path, 'utf8'));
  const merged = { ...document, ...changes };
  if (changes.helpers) merged.helpers = { ...document.helpers, ...changes.helpers };
  if (changes.cli) merged.cli = { ...document.cli, ...changes.cli };
  if (changes.lean) merged.lean = { ...document.lean, ...changes.lean };
  if (changes.tools) merged.tools = { ...document.tools, ...changes.tools };
  return `${JSON.stringify(merged, null, 2)}\n`;
}

// A WAT transform must actually change the text: a no-op mutation would make a
// control vacuous.
export function transformWat(text, transform, label) {
  const mutated = transform(text);
  if (mutated === text) throw Error(`${label}: the WAT mutation changed nothing`);
  return mutated;
}
