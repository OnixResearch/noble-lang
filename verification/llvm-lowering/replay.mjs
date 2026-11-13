// Fresh-process binding replay for issue #107's retained graph-to-LLVM bundle.
// This verifies byte and stage bindings; it does not replace the Rust validator,
// the Lean kernel proof, compiler-lab, or a native artifact check.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, isAbsolute, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const lane = dirname(fileURLToPath(import.meta.url));
const root = resolve(lane, '../..');
const bundle = JSON.parse(readFileSync(resolve(lane, 'bundle.json'), 'utf8'));
const identityDomain = 'noble.ngraph.retained-text.v1';
const profile = 'ngraph-llvm-i64/0';
const triple = 'x86_64-unknown-linux-gnu';
const dataLayout = 'e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128';
const mask64 = (1n << 64n) - 1n;

function testDigest(chunks) {
  let state = 0n;
  for (const chunk of chunks) {
    for (const byte of chunk) state = (state * 31n + BigInt(byte)) & mask64;
  }
  const digest = Buffer.alloc(32);
  digest.writeBigUInt64LE(state);
  return digest.toString('hex');
}

function encodedIdentity(domain, values) {
  const chunks = [Buffer.from(domain, 'utf8'), Buffer.from([0])];
  for (const value of values) {
    const bytes = Buffer.from(value, 'utf8');
    const length = Buffer.alloc(8);
    length.writeBigUInt64LE(BigInt(bytes.length));
    chunks.push(length, bytes);
  }
  return testDigest(chunks);
}

function retainedIdentity(path, text) {
  return encodedIdentity(identityDomain, [path, text]);
}

function moduleIdentity(text) {
  return encodedIdentity('noble.ngraph.llvm-module.v0', [profile, triple, dataLayout, text]);
}

function laneText(path, expectedIdentity) {
  assert.equal(typeof path, 'string');
  assert.equal(isAbsolute(path), false, `retained lane path must be relative: ${path}`);
  assert.equal(path.split(/[\\/]/).includes('..'), false, `retained lane path escapes: ${path}`);
  const fullPath = resolve(lane, path);
  assert.equal(fullPath.startsWith(`${lane}${sep}`), true, `retained lane path escapes: ${path}`);
  const text = readFileSync(fullPath, 'utf8');
  if (expectedIdentity !== null) {
    assert.equal(retainedIdentity(path, text), expectedIdentity, `retained text binding differs: ${path}`);
  }
  return text;
}

function rootText(path, expectedIdentity) {
  assert.equal(typeof path, 'string');
  assert.equal(isAbsolute(path), false, `repository path must be relative: ${path}`);
  assert.equal(path.split(/[\\/]/).includes('..'), false, `repository path escapes: ${path}`);
  const fullPath = resolve(root, path);
  assert.equal(fullPath.startsWith(`${root}${sep}`), true, `repository path escapes: ${path}`);
  const text = readFileSync(fullPath, 'utf8');
  assert.equal(retainedIdentity(path, text), expectedIdentity, `retained proof binding differs: ${path}`);
  return text;
}

function parseGraph(text) {
  const lines = text.trimEnd().split('\n');
  const firstNode = lines.findIndex(line => /^%\d+ = /.test(line));
  assert.notEqual(firstNode, -1, 'retained graph has a node table');
  const afterNodes = lines.findIndex((line, index) => index > firstNode && !/^%\d+ = /.test(line));
  const end = afterNodes === -1 ? lines.length : afterNodes;
  const nodes = lines.slice(firstNode, end).map(line => {
    const match = /^(\d+) = (.*)$/.exec(line.replace(/^%/, ''));
    assert.ok(match, `malformed retained graph node: ${line}`);
    return { index: Number(match[1]), body: match[2] };
  });
  for (let index = 0; index < nodes.length; index += 1) {
    assert.equal(nodes[index].index, index, 'graph nodes are in canonical index order');
  }
  return { lines, firstNode, end, nodes };
}

function checkCertificate(certificateText, original, rewritten) {
  const certificate = JSON.parse(certificateText);
  assert.equal(certificate.schema, 'ngraph-certificate-pair/v1');
  assert.equal(certificate.revision, 'ngraph-cert/0');
  assert.equal(certificate.left, original.graph_identity);
  assert.equal(certificate.right, rewritten.graph_identity);
  assert.ok(Array.isArray(certificate.steps) && certificate.steps.length > 0);

  const before = parseGraph(original.graphText);
  const after = parseGraph(rewritten.graphText);
  assert.deepEqual(before.lines.slice(0, before.firstNode), after.lines.slice(0, after.firstNode));
  assert.deepEqual(before.lines.slice(before.end), after.lines.slice(after.end));
  assert.equal(before.nodes.length, after.nodes.length, 'the in-place rewrite preserves node identities');

  const expected = new Map(before.nodes.map(({ index, body }) => [index, body]));
  for (const step of certificate.steps) {
    assert.ok(Number.isSafeInteger(step.node) && step.node >= 0 && step.node < before.nodes.length);
    assert.equal(step.operands.length, 2);
    const [left, right] = step.operands;
    const node = before.nodes[step.node].body;
    if (step.rule === 'fold-add-constants') {
      assert.equal(node, `i64.add %${left} %${right}`);
      const leftMatch = /^const i64 (-?\d+)$/.exec(before.nodes[left].body);
      const rightMatch = /^const i64 (-?\d+)$/.exec(before.nodes[right].body);
      assert.ok(leftMatch && rightMatch, 'fold premises are exact graph constants');
      const leftBits = BigInt.asUintN(64, BigInt(leftMatch[1]));
      const rightBits = BigInt.asUintN(64, BigInt(rightMatch[1]));
      assert.equal(`0x${leftBits.toString(16).padStart(16, '0').toUpperCase()}`, step.first_bits);
      assert.equal(`0x${rightBits.toString(16).padStart(16, '0').toUpperCase()}`, step.second_bits);
      const claimed = (leftBits + rightBits) & mask64;
      assert.equal(`0x${claimed.toString(16).padStart(16, '0').toUpperCase()}`, step.claimed);
      expected.set(step.node, `const i64 ${BigInt.asIntN(64, claimed).toString()}`);
    } else if (step.rule === 'neutral-mul-right') {
      assert.equal(node, `i64.mul %${left} %${right}`);
      assert.equal(before.nodes[right].body, 'const i64 1');
      assert.equal(step.first_bits, '0x0000000000000001');
      assert.equal(step.second_bits, '0x0000000000000001');
      assert.equal(step.claimed, '0x0000000000000000');
    } else {
      assert.fail(`unsupported retained certificate rule: ${step.rule}`);
    }
  }
  for (const { index, body } of after.nodes) {
    assert.equal(body, expected.get(index), `rewritten graph node ${index} is replayed by the certificate`);
  }
  return certificate;
}

assert.equal(bundle.schema, 'noble-llvm-lowering-bundle/v1');
assert.equal(bundle.profile, profile);
assert.equal(bundle.identity_scheme, 'test');
assert.equal(bundle.revisions.ir, 'ngraph-ir/0');
assert.equal(bundle.revisions.semantic, 'ngraph-sem/0');
assert.equal(bundle.revisions.facts, 'ngraph-facts/0');
assert.equal(bundle.revisions.lowering, profile);
assert.equal(bundle.revisions.llvm_parser, profile);
assert.equal(bundle.revisions.llvm_validator, profile);
assert.equal(bundle.revisions.rewrite_certificate, 'ngraph-cert/0');
assert.equal(bundle.revisions.production_backend, 'ngraph-layered/0');
assert.deepEqual(bundle.assumptions, [
  'I64 addition, subtraction and multiplication wrap modulo 2^64; flagged LLVM forms are refused.',
  'The ABI is exactly two i64 noundef arguments and one returned i64; memory, calls, globals and externals are outside the profile.',
  'The input domain is every unsigned 64-bit pair and the observation is the returned i64 value.',
  'The Lean theorem proves the parsed-instruction model; Rust parser-to-model correspondence and native-code correspondence are not proved here.',
  'Recorded concrete executions are spot checks and do not replace the universal theorem.',
]);
const sourcePaths = [
  'crates/noble-ngraph/src/llvm.rs',
  'crates/noble-ngraph/src/ir.rs',
  'crates/noble-ngraph/src/check.rs',
  'crates/noble-ngraph/src/format/graph.rs',
  'crates/noble-ngraph/src/sem.rs',
  'crates/noble-ngraph/src/eval.rs',
  'crates/noble-ngraph/src/cert.rs',
  'crates/noble-ngraph/tests/cert/differential.rs',
  'crates/noble-ngraph/tests/cert/llvm.rs',
  'crates/noble-ngraph/tests/cert/llvm/artifact_tests.rs',
  'crates/noble-ngraph/tests/cert/llvm/bundle.rs',
  'crates/noble-ngraph/tests/cert/llvm/controls.rs',
  'crates/noble-ngraph/tests/cert/llvm/corpus.rs',
  'crates/noble-ngraph/tests/cert/llvm/fixtures.rs',
  'crates/noble-ngraph/tests/cert/llvm/model.rs',
  'crates/noble-ngraph/tests/cert/llvm/profile_tests.rs',
  'proofs/foundation/NobleFoundation/LlvmLowering.lean',
  'verification/llvm-lowering/replay.mjs',
];
assert.deepEqual(bundle.source_bindings.map(binding => binding.path), sourcePaths);
for (const binding of bundle.source_bindings) rootText(binding.path, binding.identity);
assert.equal(bundle.target.triple, triple);
assert.equal(bundle.target.data_layout, dataLayout);
assert.equal(bundle.target.calling_convention, 'ccc');
assert.deepEqual(bundle.target.parameters, ['i64 noundef %a0', 'i64 noundef %a1']);
assert.equal(bundle.target.result, 'i64');
assert.deepEqual(bundle.target.attributes, []);
assert.deepEqual(bundle.target.flags, []);

const requestText = laneText(bundle.pair.request, bundle.bindings.request_identity);
const request = JSON.parse(requestText);
assert.equal(request.schema, 1);
assert.equal(request.kind, 'compiler-lab-llvm-pair-request');
assert.equal(request.source_module.path, bundle.pair.source_module);
assert.equal(request.target_module.path, bundle.pair.target_module);
assert.equal(bundle.pair.relation, request.relation);
assert.equal(bundle.pair.direction, request.direction);
assert.equal(request.source_module.entry, 'noble_i64');
assert.equal(request.target_module.entry, 'noble_i64');
assert.equal(request.relation, 'equivalence');
assert.equal(request.direction, 'both');
assert.deepEqual(request.arguments, ['i64', 'i64']);
assert.equal(request.result, 'i64');
assert.equal(request.calling_convention, 'ccc');
assert.deepEqual(request.attributes, []);
assert.equal(request.input_domain, 'all-u64-pairs');
assert.equal(request.poison_policy, 'exclude');
assert.deepEqual(request.permitted_externals, []);

const corpusText = rootText(bundle.bindings.lean_corpus, bundle.bindings.lean_corpus_identity);
assert.match(corpusText, /theorem (?:foldable|wrapping)_sound/);
const names = new Set();
for (const artifact of bundle.artifacts) {
  assert.equal(names.has(artifact.name), false, `artifact name is unique: ${artifact.name}`);
  names.add(artifact.name);
  assert.equal(artifact.validator, 'accepted');
  assert.match(artifact.graph_identity, /^[0-9a-f]{64}$/);
  artifact.graphText = laneText(artifact.graph, artifact.graph_text_identity);
  assert.match(artifact.graphText, /^ngraph-ir\/0 ngraph-i64\/0\n/);
  artifact.moduleText = laneText(artifact.module, null);
  assert.equal(moduleIdentity(artifact.moduleText), artifact.module_identity, `module identity: ${artifact.name}`);
  assert.match(artifact.moduleText, /^; ngraph-llvm-i64\/0 module:/);
  assert.match(artifact.moduleText, /target triple = "x86_64-unknown-linux-gnu"/);
  assert.match(artifact.moduleText, /target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"/);
  assert.match(artifact.moduleText, /define i64 @noble_i64\(i64 noundef %a0, i64 noundef %a1\) \{/);
  assert.equal(artifact.executions.length, 2);
  for (const execution of artifact.executions) {
    assert.equal(execution.module, execution.reference, `finite concrete agreement: ${artifact.name}`);
    assert.match(corpusText, new RegExp(`theorem ${execution.theorem}\\b`));
  }
}
assert.deepEqual([...names].sort(), ['foldable', 'foldable-rewritten', 'negative', 'shared', 'subtractive', 'wrapping'].sort());

const artifactByName = new Map(bundle.artifacts.map(artifact => [artifact.name, artifact]));
const original = artifactByName.get(bundle.pair.original);
const rewritten = artifactByName.get(bundle.pair.rewritten);
assert.ok(original && rewritten);
const certificateText = laneText(bundle.pair.certificate, bundle.bindings.certificate_identity);
const certificate = checkCertificate(certificateText, original, rewritten);
assert.ok(certificate.steps.length > 0, 'graph rewrite evidence is retained');
assert.equal(laneText(bundle.pair.source_module, null), original.moduleText);
assert.equal(laneText(bundle.pair.target_module, null), rewritten.moduleText);

const expectedControls = [
  'operand-swapped', 'constant-changed', 'negative-zero', 'return-changed', 'flag-nsw', 'flag-nuw',
  'opcode-unsupported', 'instruction-call', 'attribute-dropped', 'convention-named', 'triple-changed',
  'datalayout-changed', 'artifact-substituted', 'return-duplicated', 'text-after-brace',
  'instruction-missing', 'destination-changed',
];
assert.deepEqual(bundle.controls.map(control => control.class).sort(), expectedControls.sort());
for (const control of bundle.controls) {
  assert.ok(artifactByName.has(control.artifact), `control source exists: ${control.artifact}`);
  assert.ok(['parse', 'verify'].includes(control.stage));
  assert.equal(control.model, control.stage === 'verify');
  assert.match(control.refusal, /^L-/);
  laneText(control.path, control.text_identity);
}

const stages = new Map(bundle.stages.map(stage => [stage.stage, stage]));
assert.deepEqual([...stages.keys()].sort(), [
  'concrete-agreement', 'graph-rewrite', 'graph-to-llvm-lowering', 'llvm-optimisation',
  'native-artifact', 'semantic-validation',
].sort());
for (const stage of stages.values()) assert.ok(stage.evidence.length > 0);
assert.equal(stages.get('graph-rewrite').status, 'accepted');
assert.equal(stages.get('graph-to-llvm-lowering').status, 'validated');
assert.equal(stages.get('semantic-validation').status, 'proved');
assert.equal(stages.get('concrete-agreement').status, 'checked');
assert.equal(stages.get('llvm-optimisation').status, 'not-run');
assert.equal(stages.get('native-artifact').status, 'not-run');

console.log(`replayed ${bundle.artifacts.length} graph/module bindings, ${certificate.steps.length} rewrite step, ${bundle.controls.length} retained controls, ${stages.size} stage records`);
console.log('semantic validation and concrete agreement remain separate; LLVM optimisation and native artifact are not run');
