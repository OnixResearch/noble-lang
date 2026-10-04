#!/usr/bin/env bun
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const proofRoot = join(root, 'proofs/mc1');
const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json')));
const lean = selection.tool_paths.lean.output;
const lake = join(lean, 'bin/lake');
const noble = resolve(process.argv[2] ?? join(root, 'target/debug/noble'));
const base = join(homedir(), '.cache/noble-proof-tmp');
mkdirSync(base, { recursive: true });
const temporary = mkdtempSync(join(base, 'intrinsic-'));
const fixture = join(root, 'verification/mc1/increment.noble-contract');
const typeSource = join(proofRoot, 'IntrinsicTypeWitness.lean');
const proofSource = readFileSync(join(root, 'verification/mc1/intrinsic-feasibility-proof.lean'), 'utf8');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const lakePath = spawnSync(lake, ['env', 'printenv', 'LEAN_PATH'], {
  cwd: proofRoot, env: { ...process.env, LEAN_PATH: temporary },
  encoding: 'utf8', timeout: 30000,
});
assert.equal(lakePath.status, 0, lakePath.stderr);

function run(binary, args, options = {}) {
  const result = spawnSync(binary, args, { cwd: options.cwd ?? proofRoot,
    env: { ...process.env, TMPDIR: base, LEAN_PATH: lakePath.stdout.trim(), ...options.env },
    encoding: 'utf8', timeout: 120000, maxBuffer: 8 * 1024 * 1024 });
  if (result.error) throw result.error;
  return { exit: result.status, output: `${result.stdout ?? ''}${result.stderr ?? ''}` };
}
function requireSuccess(name, result) {
  if (result.exit !== 0) throw Error(`${name}: exit ${result.exit}:\n${result.output}`);
  console.log(`PASS ${name}: Lean exit 0`);
}
function reject(name, result) {
  if (result.exit === 0 || !/error:|error\(/.test(result.output))
    throw Error(`${name}: expected Lean error, got exit ${result.exit}:\n${result.output}`);
  console.log(`REJECT ${name}: Lean exit ${result.exit}: ${result.output.split('\n').find(line => /error:|error\(/.test(line))}`);
}
function leanFile(name, content, compile = false) {
  const source = join(temporary, `${name}.lean`);
  writeFileSync(source, content);
  const args = [];
  if (compile) args.push('-o', join(temporary, `${name}.olean`));
  args.push(source);
  return run(join(lean, 'bin/lean'), args, { cwd: temporary });
}
function obligation(sourcePath) {
  const result = run(noble, ['explain-proof', sourcePath]);
  requireSuccess(`MC1 exporter ${sourcePath}`, result);
  const report = JSON.parse(result.output);
  assert.equal(report.ordinary_typing?.outcome, 'accepted');
  assert.equal(report.claim_kind, 'partial-correctness');
  assert.equal(report.subject?.semantic_revision, 0);
  assert.equal(report.subject?.accepted_candidate?.includes('Invocation { def: Definition(4)'), true);
  assert.equal(report.subject?.requires?.source, 'true');
  assert.equal(report.subject?.ensures?.source, '(eq (out y) (add (in x) 1))');
  assert.deepEqual(report.subject?.inputs, [{ name: 'x', type: 'I64' }]);
  assert.deepEqual(report.subject?.outputs, [{ name: 'y', type: 'I64' }]);
  assert.equal(report.subject?.ghost_parameters?.length, 0);
  return report;
}

// This is a feasibility check of the current independently accepted MC1
// `.noble-contract` equivalent, NOT acceptance of the new `.noble` module path.
const audit = `
open Lean Elab Command
elab "check_intrinsic_axioms" : command => do
  let env ← getEnv
  for name in [\`\`IntrinsicFeasibility.increment,
      \`\`NobleContracts.Intrinsic.polymorphicReflexivity_witness,
      \`\`NobleContracts.Intrinsic.encode_hasType,
      \`\`NobleContracts.Intrinsic.encode_injective] do
    match env.find? name with
    | some (.thmInfo _) => pure ()
    | _ => throwError "missing audited theorem {name}"
    let axioms ← liftCoreM (collectAxioms name)
    for ax in axioms do
      unless [\`\`propext, \`\`Classical.choice, \`\`Quot.sound].contains ax do
        throwError "disallowed intrinsic axiom {ax} in {name}"
    logInfo m!"INTRINSIC-AXIOMS {name}: {axioms}"
check_intrinsic_axioms
`;
try {
  assert.equal(selection.lean.toolchain, readFileSync(join(proofRoot, 'lean-toolchain'), 'utf8').trim());
  const cases = JSON.parse(readFileSync(join(root, 'specs/conformance/contract-cases.json')));
  const sourceModule = (Array.isArray(cases) ? cases : cases.cases)
    .find(item => item.id === 'CONTRACT-18')?.input?.source;
  assert.match(sourceModule, /^module arithmetic@1 \[ def increment \[ 1 \+ \] contract 1 increment-law \[/);
  assert.match(sourceModule, /requires \[ true \] ensures \[ \(eq \(out y\) \(add \(in x\) 1\)\) \]/);
  // This pins the files actually checked here, rather than replaying the
  // older MC1 evidence's Model hash (its historical source must not change).
  const pinned = {
    'NobleContracts.Model': '368cad7d6d86db4b182bd6b9e2bd0d3a354559c04579e3e27b789bb69313a7c3',
    'NobleContracts.Rules': '1d5802ddec13fcfcbba70e6c75f46ab051ea569e6f30fe92d61e3b5dec8d877b',
    'NobleContracts.Expression': '4638c9b146da69b5d0a545734fd5bdc690288f86d1c3a2a40a235007a70561db',
    'NobleContracts.Obligation': '13dddb8d47346ba648eb2dd372031ccd2f2e090f3312f1797a1ca567ab9cd616',
  };
  for (const module of ['Model', 'Rules', 'Expression', 'Obligation']) {
    const path = join(proofRoot, `NobleContracts/${module}.lean`);
    assert.equal(hash(readFileSync(path)), pinned[`NobleContracts.${module}`], `${module} model changed`);
  }
  requireSuccess('rebuilt current reviewed MC1 library', run(lake, ['build', 'NobleContracts']));
  const typeCheck = leanFile('IntrinsicTypeWitness', readFileSync(typeSource, 'utf8'), true);
  requireSuccess('predicative code, model correspondence and polymorphic Eq', typeCheck);
  // The source fixture is MC1's already implemented analog of CONTRACT-18.
  const generated = obligation(fixture);
  assert.equal(generated.source, '(contract 1 increment\n  (input (x I64))\n  (output (y I64))\n  (program [ 1 + ])\n  (requires true)\n  (ensures (eq (out y) (add (in x) 1))))\n');
  requireSuccess('regenerated typed MC1 claim', leanFile('MC1Obligation', generated.generated_statement, true));
  const positive = `import Lean\nimport IntrinsicTypeWitness\n${proofSource}\n${audit}`;
  const checked = leanFile('IncrementPositive', positive);
  requireSuccess('compositional PC → exact MC1 claim, strict axioms', checked);
  for (const line of checked.output.split('\n').filter(line => line.includes('INTRINSIC-AXIOMS')))
    console.log(line);
  const missingBridge = positive.replace('(fun _ middle _ exactMiddle =>\n      exactMiddle.trans (List.append_assoc tail [.i64 x] [.i64 1]))',
    '(fun _ middle _ exactMiddle => exactMiddle)');
  assert.notEqual(missingBridge, positive);
  reject('missing append-association bridge', leanFile('MissingBridge', missingBridge));
  const swapped = positive.replace('PC ([.lit (.i64 1)] ++ [.word 4])',
    'PC ([.word 4] ++ [.lit (.i64 1)])');
  assert.notEqual(swapped, positive);
  reject('swapped operation order', leanFile('SwappedOperations', swapped));
  const alteredSource = readFileSync(fixture, 'utf8').replace('[ 1 + ]', '[ 2 + ]');
  assert.notEqual(alteredSource, readFileSync(fixture, 'utf8'));
  const alteredPath = join(temporary, 'changed-subject.noble-contract');
  writeFileSync(alteredPath, alteredSource);
  const altered = obligation(alteredPath);
  requireSuccess('regenerated altered subject', leanFile('MC1ObligationAltered',
    altered.generated_statement.replace('namespace MC1Obligation', 'namespace MC1ObligationAltered')
      .replace('end MC1Obligation', 'end MC1ObligationAltered'), true));
  const alteredProof = positive.replaceAll('MC1Obligation.', 'MC1ObligationAltered.')
    .replace('import MC1Obligation', 'import MC1ObligationAltered');
  reject('altered accepted subject [ 2 + ]', leanFile('AlteredSubject', alteredProof));
  const forged = positive.replace('theorem increment : MC1Obligation.claim :=\n  exportedClaim_consequence',
    'axiom forged : MC1Obligation.claim\ntheorem increment : MC1Obligation.claim := forged\n\n/-\n  exportedClaim_consequence')
    .replace('\nend IntrinsicFeasibility\n', '\n-/\nend IntrinsicFeasibility\n');
  assert.notEqual(forged, positive);
  const forgedResult = leanFile('ForgedPremise', forged);
  reject('forged premise', forgedResult);
  assert.match(forgedResult.output, /disallowed intrinsic axiom IntrinsicFeasibility.forged/);
  reject('fake Type0 constructor', leanFile('FakeType0',
    'import IntrinsicTypeWitness\ndef fake : NobleContracts.Intrinsic.PureTyCode := .program\n'));
  reject('unrestricted Type binder', leanFile('UnrestrictedType',
    'import IntrinsicTypeWitness\nopen NobleContracts.Intrinsic\ntheorem wrong : ∀ (A : Type) (x : A), x = x := polymorphicReflexivity_witness\n'));
  console.log(`PASS strict feasibility: Lean ${selection.lean.toolchain}; semantic revision 0; gate-local source digest sha256:${hash(JSON.stringify(pinned))}; temporary objects isolated`);
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
