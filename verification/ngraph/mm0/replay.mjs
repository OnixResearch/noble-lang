#!/usr/bin/env node
// Consumer side of the bounded MM0 experiment. No proof generation or Noble
// rewrite proposer is invoked here. The expected claim and selected spec come
// from the caller-owned, precommitted profile and retained Noble subjects.
// Usage: node replay.mjs <selected-mm0-c> <production-directory> <outside-repo-report-directory>
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const lane = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(lane, '../../..');
const [checkerArg, generatedArg, reportArg] = process.argv.slice(2);
assert.ok(checkerArg && generatedArg && reportArg,
  'usage: node replay.mjs <selected-mm0-c> <production-directory> <outside-repo-report-directory>');
const checker = path.resolve(checkerArg);
const generated = path.resolve(generatedArg);
const report = path.resolve(reportArg);
assert.ok(!report.startsWith(`${root}${path.sep}`) && report !== root, 'report must be outside repo');
mkdirSync(report, { recursive: true, mode: 0o700 });
const policy = JSON.parse(readFileSync(path.join(root, 'policy/tool-selection.json'))).experimental_mm0;
const profile = JSON.parse(readFileSync(path.join(lane, 'profile.json')));
const production = JSON.parse(readFileSync(path.join(generated, 'production.json')));
const sha = (body) => createHash('sha256').update(body).digest('hex');
const fileSha = (file) => sha(readFileSync(file));
class Refusal extends Error {
  constructor(stage, reason) { super(reason); this.stage = stage; }
}
function requireAt(stage, condition, reason) {
  if (!condition) throw new Refusal(stage, reason);
}
function timedChecker(name, spec, proof, addressBytes = profile.build_profile.checker_memory_bytes,
  wallMs = profile.build_profile.checker_wall_ms) {
  const label = name.replace(/[^a-zA-Z0-9_-]/g, '_');
  const time = path.join(report, `${label}.time`);
  const start = process.hrtime.bigint();
  const run = spawnSync('/usr/bin/time', ['-f', '%e %M', '-o', time,
    'prlimit', `--as=${addressBytes}`, `--cpu=${profile.build_profile.checker_cpu_seconds}`,
    '--', checker, proof], { input: spec, encoding: 'utf8', maxBuffer: 4 * 1024 * 1024,
    timeout: wallMs, killSignal: 'SIGKILL' });
  const metrics = statSync(time, { throwIfNoEntry: false }) ? readFileSync(time, 'utf8').trim().split(' ') : [];
  return { status: run.status, signal: run.signal, error: run.error?.message ?? null,
    stdout: run.stdout ?? '', stderr: run.stderr ?? '',
    wall_ms: Number(process.hrtime.bigint() - start) / 1e6,
    checker_elapsed_s: metrics.length === 2 ? Number(metrics[0]) : null,
    checker_peak_rss_kib: metrics.length === 2 ? Number(metrics[1]) : null,
    address_space_bytes: addressBytes, wall_limit_ms: wallMs };
}
const u64 = (1n << 64n) - 1n;
const max = (1n << 63n) - 1n;
const min = -(1n << 63n);
const signed = (value) => { const word = value & u64; return word > max ? word - (1n << 64n) : word; };
function graph(body) {
  const lines = body.toString('utf8').trimEnd().split('\n');
  const header = lines.slice(0, 3);
  requireAt('graph-admission', header[0] === 'ngraph-ir/0 ngraph-i64/0' &&
    /^subject authored [a-f0-9]{64} closure -$/.test(header[1]) &&
    header[2] === 'slot input i64' && lines.length === 11,
  'unexpected graph revision, subject, input, or topology');
  const input = /^%0 = input 0$/;
  const constant = /^%(\d) = const i64 (-?\d+)$/;
  const binary = /^%(\d) = i64\.(add|mul) %(\d) %(\d)$/;
  const parsed = [];
  for (let id = 0; id < 5; id += 1) {
    const line = lines[id + 3];
    if (id === 0 && input.test(line)) { parsed.push({ kind: 'input' }); continue; }
    const c = constant.exec(line);
    if (c && Number(c[1]) === id) {
      const value = BigInt(c[2]);
      requireAt('graph-admission', value >= min && value <= max, 'non-I64 constant');
      parsed.push({ kind: 'const', value });
      continue;
    }
    const b = binary.exec(line);
    requireAt('graph-admission', !!b && Number(b[1]) === id &&
      Number(b[3]) < id && Number(b[4]) < id, 'unexpected or invalid node');
    parsed.push({ kind: b[2], left: Number(b[3]), right: Number(b[4]) });
  }
  const outputs = lines.slice(8).map((line) => /^out %(\d)$/.exec(line));
  requireAt('graph-admission', outputs.length === 3 &&
    outputs.every((out) => out && Number(out[1]) <= 4), 'invalid ordered outputs');
  return { subject: header[1].split(' ')[2], nodes: parsed, outputs: outputs.map((out) => Number(out[1])) };
}
function certificate(body) {
  const parts = body.toString('utf8').trimEnd().split(' ');
  requireAt('certificate-claim', parts.length === 10 && parts[0] === 'ngraph-cert/0' &&
    parts[1] === 'ngraph-sem/0' && parts.slice(2, 7).join(' ') === '1 0 3 1 2' &&
    parts.slice(7).every((item) => /^\d+$/.test(item)), 'certificate shape or identity changed');
  const values = parts.slice(7).map(BigInt);
  requireAt('certificate-claim', values.every((value) => value >= 0n && value <= u64),
    'certificate bit width exceeded');
  return values;
}
function project(name, before, after, wire) {
  const lhs = graph(before);
  const rhs = graph(after);
  const [first, second, sum] = certificate(wire);
  const expected = name === 'small' ? [3n, 5n, 8n] : [max, 1n, 1n << 63n];
  requireAt('certificate-claim', expected.every((value, i) => value === [first, second, sum][i]),
    'retained source constant bits differ from selected theorem');
  requireAt('certificate-claim', ((first + second) & u64) === sum,
    'constant fold is not a 64-bit wrapping addition');
  const left = signed(first);
  const right = signed(second);
  const result = signed(sum);
  const isConst = (node, value) => node.kind === 'const' && node.value === value;
  const isOp = (node, kind, a, b) => node.kind === kind && node.left === a && node.right === b;
  requireAt('claim-projection', lhs.subject === rhs.subject &&
    lhs.nodes[0].kind === 'input' && rhs.nodes[0].kind === 'input' &&
    isConst(lhs.nodes[1], left) && isConst(rhs.nodes[1], left) &&
    isConst(lhs.nodes[2], right) && isConst(rhs.nodes[2], right) &&
    isOp(lhs.nodes[3], 'add', 1, 2) && isConst(rhs.nodes[3], result) &&
    isOp(lhs.nodes[4], 'mul', 0, 3) && isOp(rhs.nodes[4], 'mul', 0, 3) &&
    lhs.outputs.join(',') === '1,2,4' && rhs.outputs.join(',') === '1,2,4',
  'the changed graph projects a different full ordered endpoint claim');
  const a = name === 'small' ? '3' : 'wmax';
  const b = name === 'small' ? '5' : '1';
  const c = name === 'small' ? '8' : 'wmin';
  const proposition = `${a} : ${b} : umul a (uadd ${a} ${b}) : 0 = ` +
    `${a} : ${b} : umul a ${c} : 0`;
  const theory = readFileSync(path.join(lane, 'theory.mm0'), 'utf8').replace(/\s+/g, ' ');
  requireAt('claim-projection', theory.includes(`theorem endpoint_${name} (a: nat): $ ${proposition} $;`),
    'the exact projected full-endpoint theorem is absent');
  return { subject: lhs.subject, first_bits: first.toString(), second_bits: second.toString(),
    folded_bits: sum.toString(), ordered_outputs: lhs.outputs, input_slots: 1,
    theorem: `endpoint_${name}`, proposition };
}
function selectedSpec(spec) {
  requireAt('spec-digest', sha(spec) === profile.spec_sha256,
    'not the caller-approved complete specification and axiom inventory');
  const axioms = [...spec.toString('utf8').matchAll(/^axiom\s+(\w+)/gm)].map((item) => item[1]);
  requireAt('assumption-inventory', axioms.join(',') === profile.approved_axioms.join(','),
    'changed axiom inventory');
  for (const theorem of profile.required_theorems) {
    requireAt('claim-projection', spec.toString('utf8').includes(`theorem ${theorem}`),
      `selected final theorem/lemma ${theorem} missing`);
  }
}
function evaluate(name, before, after, wire, spec, proof) {
  const claim = project(name, before, after, wire);
  for (const [kind, body] of [['before.graph', before], ['after.graph', after], ['certificate.wire', wire]]) {
    requireAt('subject-digest', sha(body) === profile.inputs[`subjects/${name}/${kind}`],
      `${name}/${kind} is not the exact retained Noble subject`);
  }
  selectedSpec(spec);
  const proofFile = path.join(report, `presented-${name}.mmb`);
  writeFileSync(proofFile, proof);
  const run = timedChecker(`replay-${name}`, spec, proofFile);
  requireAt('checker-refused', run.status === 0, `MM0 checker refused: ${run.stderr} ${run.error}`);
  requireAt('proof-identity', sha(proof) === production.artifact.proof_sha256,
    'not the retained MM0 proof bytes');
  return { stage: 'accepted', claim, checker: run,
    subject_shas: ['before.graph', 'after.graph', 'certificate.wire'].map((kind) =>
      profile.inputs[`subjects/${name}/${kind}`]) };
}
function outcome(action) {
  try { return action(); } catch (error) {
    if (error instanceof Refusal) return { stage: error.stage, reason: error.message };
    throw error;
  }
}
function assertRefusal(name, result, stage) {
  assert.equal(result.stage, stage, `${name} refused at wrong stage: ${JSON.stringify(result)}`);
  return result;
}
assert.equal(policy.parser_enabled, true);
assert.equal(profile.selected_source_rev, policy.source_rev);
assert.equal(profile.selected_source_archive_sha256, policy.git_archive_sha256);
assert.equal(fileSha(checker), profile.checker_sha256, 'selected parser-enabled checker mismatch');
assert.equal(fileSha(path.join(generated, 'spec.mm0')), profile.spec_sha256, 'production spec mismatch');
assert.equal(fileSha(path.join(generated, 'proof.mmb')), production.artifact.proof_sha256,
  'production proof differs from receipt');
assert.equal(production.artifact.proof_sha256, profile.retained_proof_sha256,
  'proof differs from caller-retained production artifact');
assert.equal(production.clean_source, true);
assert.equal(production.artifact.spec_sha256, profile.spec_sha256);
for (const [file, expected] of Object.entries(profile.inputs)) {
  assert.equal(fileSha(path.join(lane, file)), expected, `retained input changed: ${file}`);
}
const spec = readFileSync(path.join(generated, 'spec.mm0'));
const proof = readFileSync(path.join(generated, 'proof.mmb'));
selectedSpec(spec);
const subject = {};
for (const name of ['small', 'boundary']) {
  subject[name] = Object.fromEntries(['before.graph', 'after.graph', 'certificate.wire'].map(
    (kind) => [kind, readFileSync(path.join(lane, 'subjects', name, kind))]));
}
const s = subject.small;
const b = subject.boundary;
const rows = {};
for (const name of ['small', 'boundary']) {
  const files = subject[name];
  rows[`positive-${name}`] = evaluate(name, files['before.graph'], files['after.graph'],
    files['certificate.wire'], spec, proof);
}
const text = (body) => body.toString('utf8');
const mutate = (body, from, to) => {
  const string = text(body);
  assert.ok(string.includes(from) && !string.includes(to), `bad unique mutation ${from}`);
  return Buffer.from(string.replace(from, to));
};
const changed = (key, before, after, wire, expected) => {
  rows[key] = assertRefusal(key, outcome(() => evaluate('small', before, after, wire, spec, proof)), expected);
};
changed('unrelated-valid-endpoint', s['before.graph'],
  mutate(s['after.graph'], '%4 = i64.mul %0 %3', '%4 = i64.mul %0 %1'),
  s['certificate.wire'], 'claim-projection');
changed('reordered-output', s['before.graph'],
  mutate(s['after.graph'], 'out %1\nout %2', 'out %2\nout %1'),
  s['certificate.wire'], 'claim-projection');
changed('changed-shared-use',
  mutate(s['before.graph'], '%4 = i64.mul %0 %3', '%4 = i64.mul %0 %2'),
  s['after.graph'], s['certificate.wire'], 'claim-projection');
changed('changed-consumed-bits',
  mutate(s['before.graph'], '%2 = const i64 5', '%2 = const i64 6'),
  s['after.graph'], s['certificate.wire'], 'claim-projection');
changed('changed-certificate-bits', s['before.graph'], s['after.graph'],
  mutate(s['certificate.wire'], '3 5 8', '3 5 9'), 'certificate-claim');
changed('stale-subject-identity',
  mutate(s['before.graph'], 'subject authored 9551', 'subject authored 8551'),
  mutate(s['after.graph'], 'subject authored 9551', 'subject authored 8551'),
  s['certificate.wire'], 'subject-digest');
const mutatedSpec = (name, body) => {
  rows[name] = assertRefusal(name,
    outcome(() => evaluate('small', s['before.graph'], s['after.graph'], s['certificate.wire'], body, proof)),
    'spec-digest');
  rows[name].raw_checker = timedChecker(`raw-${name}`, body, path.join(generated, 'proof.mmb'));
};
mutatedSpec('changed-arithmetic-definition', mutate(spec,
  'def uadd (a b: nat): nat = $ (a + b) % modulus $;',
  'def uadd (a b: nat): nat = $ (a * b) % modulus $;'));
mutatedSpec('changed-axiom', mutate(spec, 'axiom peano1', 'axiom peano1_changed'));
mutatedSpec('additional-unapproved-axiom', Buffer.concat([spec,
  Buffer.from('\naxiom oracle_any (p: wff): $ p $;\n')]));
for (const name of ['different', 'forged', 'sorry']) {
  const control = path.join(generated, 'controls', name);
  const otherSpec = readFileSync(`${control}.mm0`);
  const otherProof = `${control}.mmb`;
  assert.equal(fileSha(otherProof), production.control_proofs[name].proof_sha256,
    `foreign-${name} proof differs from production receipt`);
  const raw = timedChecker(`raw-${name}`, otherSpec, otherProof);
  rows[`foreign-${name}`] = { stage: name === 'sorry' ? 'checker-refused' : 'spec-digest',
    control_spec_sha256: sha(otherSpec), control_proof_sha256: fileSha(otherProof), raw_checker: raw };
  assert.equal(raw.status === 0, name !== 'sorry', `${name} raw control outcome not as expected`);
  const result = outcome(() => evaluate('small', s['before.graph'], s['after.graph'],
    s['certificate.wire'], otherSpec, readFileSync(otherProof)));
  assertRefusal(`foreign-${name}`, result, 'spec-digest');
  rows[`foreign-${name}`].adapter = result;
}
for (const [name, corrupt] of [
  ['truncated-proof', proof.subarray(0, Math.floor(proof.length / 2))],
  ['malformed-proof', Buffer.from(proof)]]) {
  if (name === 'malformed-proof') corrupt.fill(0, 0, 16);
  const file = path.join(report, `${name}.mmb`);
  writeFileSync(file, corrupt);
  const raw = timedChecker(`raw-${name}`, spec, file);
  rows[name] = { stage: 'checker-refused', input_proof_sha256: sha(corrupt), raw_checker: raw };
  assert.notEqual(raw.status, 0, `${name} unexpectedly verified`);
}
const exhausted = timedChecker('exhausted-address-space', spec,
  path.join(generated, 'proof.mmb'), 16 * 1024 * 1024);
rows['exhausted-address-space'] = { stage: exhausted.status === 0 ? 'accepted' : 'resource-limit',
  raw_checker: exhausted };
assert.notEqual(exhausted.status, 0, 'checker accepted with exhausted 16MiB address-space limit');
const deadline = timedChecker('exhausted-wall-clock', spec,
  path.join(generated, 'proof.mmb'), profile.build_profile.checker_memory_bytes, 1);
rows['exhausted-wall-clock'] = { stage: deadline.status === 0 ? 'accepted' : 'resource-limit',
  raw_checker: deadline };
assert.notEqual(deadline.status, 0, 'checker accepted under exhausted 1ms wall limit');
const receipt = { schema: 'noble-mm0-replay/v1', source_commit: production.source_commit,
  retained_spec_sha256: profile.spec_sha256, retained_proof_sha256: sha(proof),
  checker_binary_sha256: fileSha(checker), producer_invoked: false,
  qualified_compiler_lab_backend: false, rows };
writeFileSync(path.join(report, 'replay.json'), `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({ positive: Object.keys(rows).filter((key) => key.startsWith('positive-')),
  refusals: Object.keys(rows).filter((key) => !key.startsWith('positive-')),
  receipt: path.join(report, 'replay.json') }));
