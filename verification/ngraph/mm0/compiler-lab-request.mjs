#!/usr/bin/env node
// Narrow Noble consumer of the qualified compiler-lab #220 MM0 backend
// (`compiler-lab validate mm0-certificate`) for issue #127.
//
// The frozen request is built from retained, digest-pinned Noble inputs only.
// Every endpoint and fold claim is derived here from the original graph, the
// ngraph-cert/0 derivation and the rewritten graph; a producer-supplied hash or
// theorem text is never the source of an expected value. The positive request,
// its proposer-free replay, and refusal submissions that change only presented
// files (or only the caller's resource limits) are submitted; each report is
// compared with Noble's own expectations, path-scrubbed and retained.
//
// Usage: node compiler-lab-request.mjs <compiler-lab checkout> <compiler-lab binary>
//   <tools root from setup-mm0-certificate.py> <new directory outside this repository>
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { homedir, hostname } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const lane = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(lane, '../../..');
const usage = 'usage: node compiler-lab-request.mjs <compiler-lab checkout> <compiler-lab binary> ' +
  '<tools root> <new directory outside this repository>';
const [checkoutArg, binaryArg, toolsArg, outArg] = process.argv.slice(2);
assert.ok(checkoutArg && binaryArg && toolsArg && outArg, usage);
const checkout = path.resolve(checkoutArg);
const binary = path.resolve(binaryArg);
const toolsLink = path.resolve(toolsArg);
const out = path.resolve(outArg);
assert.ok(out !== root && !out.startsWith(`${root}${path.sep}`), 'output must be outside the repository');
assert.ok(!existsSync(out), 'output directory must be new');
mkdirSync(out, { recursive: true, mode: 0o700 });

const sha = (body) => createHash('sha256').update(body).digest('hex');
const fileSha = (file) => sha(readFileSync(file));
const git = (cwd, args) => {
  const run = spawnSync('git', args, { cwd, encoding: 'utf8' });
  assert.equal(run.status, 0, `git ${args.join(' ')} failed in ${cwd}`);
  return run.stdout;
};

// ---- Source and backend identity -------------------------------------------------------------
assert.equal(git(root, ['status', '--porcelain']), '', 'commit the Noble consumer before producing a receipt');
const nobleCommit = git(root, ['rev-parse', 'HEAD']).trim();
const policy = JSON.parse(readFileSync(path.join(root, 'policy/tool-selection.json'))).qualified_mm0;
assert.ok(policy, 'policy/tool-selection.json selects no qualified MM0 backend');
assert.equal(git(checkout, ['rev-parse', 'HEAD']).trim(), policy.compiler_lab_commit,
  'compiler-lab checkout is not the selected commit');
assert.equal(git(checkout, ['status', '--porcelain', '--untracked-files=no']), '',
  'compiler-lab checkout has modified tracked files');
const tools = realpathSync(toolsLink);
assert.equal(tools, policy.tools_closure, 'tools root is not the selected checker closure');
assert.equal(fileSha(path.join(tools, 'bin/mm0-c')), policy.checker_sha256, 'qualified checker digest differs');
assert.equal(fileSha(path.join(tools, 'libexec/mm0-c-no-parser')), policy.negative_checker_sha256,
  'negative NO_PARSER control digest differs');
assert.equal(fileSha(path.join(tools, 'share/compiler-lab/pins.json')), policy.pins_sha256,
  'compiler-lab pins differ');
const baseFiles = ['peano.mm0', 'peano_hex.mm0'];
for (const name of baseFiles) {
  assert.equal(fileSha(path.join(tools, 'share/mm0/examples', name)), policy.base_theory_sha256[name],
    `pinned base theory ${name} differs`);
}

// ---- Noble's caller-owned inputs --------------------------------------------------------------
const profile = JSON.parse(readFileSync(path.join(lane, 'profile.json')));
for (const [file, expected] of Object.entries(profile.inputs)) {
  assert.equal(fileSha(path.join(lane, file)), expected, `retained Noble input changed: ${file}`);
}
const specFile = path.join(lane, 'retained/spec.mm0');
const proofFile = path.join(lane, 'retained/proof.mmb');
assert.equal(fileSha(specFile), profile.spec_sha256, 'retained specification is not the approved one');
assert.equal(fileSha(proofFile), profile.retained_proof_sha256, 'retained proof is not the approved one');
assert.equal(profile.selected_source_rev, policy.mm0_source_rev, 'MM0 revision differs from the backend');

// Split MM0 text into whitespace-normalised statements: `;` outside a math
// string or `--` comment ends one. Several statements may share a line.
function statements(text) {
  const result = [];
  let current = '';
  for (let i = 0; i < text.length; i += 1) {
    const c = text[i];
    if (c === '-' && text[i + 1] === '-') {
      const end = text.indexOf('\n', i);
      i = end < 0 ? text.length : end;
      current += ' ';
      continue;
    }
    if (c === '$') {
      const end = text.indexOf('$', i + 1);
      assert.ok(end > i, 'unterminated MM0 math string');
      current += text.slice(i, end + 1);
      i = end;
      continue;
    }
    current += c;
    if (c === ';') {
      result.push(current.replace(/\s+/g, ' ').trim());
      current = '';
    }
  }
  assert.equal(current.trim(), '', 'trailing MM0 text without a terminator');
  return result;
}
const nameOf = (statement) => /^(?:pure |strict |provable |free )*(?:axiom|theorem|def|term) (\w+)/.exec(statement)?.[1];
const theory = statements(readFileSync(path.join(lane, 'theory.mm0'), 'utf8'));
const spec = statements(readFileSync(specFile, 'utf8'));
const base = baseFiles.flatMap((name) =>
  statements(readFileSync(path.join(tools, 'share/mm0/examples', name), 'utf8')));

// Approved assumptions: exactly the axioms of the pinned base theory. The
// caller theory declares none. The original lane's line-anchored inventory
// missed the 16 hex-digit axioms `h2n0`..`h2nf` that share a line with a
// theorem; they are the definitional digit values `xK = K`, listed here.
const hexDigitAxioms = [...'0123456789abcdef'].map((digit) => `h2n${digit}`);
const approvedNames = [...profile.approved_axioms, ...hexDigitAxioms].sort();
assert.equal(theory.filter((s) => s.startsWith('axiom ')).length, 0, 'caller theory declares an axiom');
const axioms = base.filter((s) => s.startsWith('axiom '));
assert.deepEqual(spec.filter((s) => s.startsWith('axiom ')), axioms,
  'approved specification axioms differ from the pinned base theory');
assert.deepEqual(axioms.map(nameOf).sort(), approvedNames, 'axiom inventory differs from the approved names');
for (const digit of hexDigitAxioms) {
  const value = parseInt(digit.slice(3), 16);
  assert.ok(axioms.includes(`axiom ${digit}: $ x${digit.slice(3)} = ${value} $;`),
    `${digit} is not the digit value axiom`);
}

// Caller definitions and the wrapping interpretation, evaluated independently.
const definitions = theory.filter((s) => s.startsWith('def '));
const defBody = (name) => {
  const statement = definitions.find((s) => nameOf(s) === name);
  assert.ok(statement, `caller definition ${name} missing`);
  return statement;
};
const hexValue = (statement) => {
  const body = /= \$ (.*) \$;$/.exec(statement)?.[1];
  const digits = body?.split(' :x ') ?? [];
  assert.ok(digits.length > 0 && digits.every((d) => /^x[0-9a-f]$/.test(d)), `not a hex numeral: ${statement}`);
  // `:x` is infixl `a * 16 + x` in peano_hex.mm0.
  return digits.reduce((acc, d) => acc * 16n + BigInt(parseInt(d.slice(1), 16)), 0n);
};
const MOD = 1n << 64n;
assert.equal(hexValue(defBody('modulus')), MOD, 'modulus is not 2^64');
assert.equal(defBody('uadd'), 'def uadd (a b: nat): nat = $ (a + b) % modulus $;', 'wrapping add changed');
assert.equal(defBody('umul'), 'def umul (a b: nat): nat = $ (a * b) % modulus $;', 'wrapping mul changed');
assert.ok(spec.includes('def hex (a: nat) (x: hex): nat = $ a * 16 + x $;') &&
  spec.includes('infixl hex: $:x$ prec 120;'), 'hex numeral anchor changed');
// Numeral names: decimal tokens the base theory defines as successors, then
// nullary caller hex definitions (`wmax`, `wmin`).
const numerals = new Map();
assert.ok(spec.includes('term d0: nat;') && spec.includes('prefix d0: $0$ prec max;'), 'zero numeral changed');
numerals.set(0n, '0');
for (let n = 1; spec.includes(`prefix d${n}: $${n}$ prec max;`); n += 1) {
  assert.ok(spec.includes(`def d${n}: nat = $suc ${n - 1}$;`), `numeral ${n} is not a successor`);
  numerals.set(BigInt(n), `${n}`);
}
for (const statement of definitions) {
  if (!/^def \w+: nat = \$ x[0-9a-f]( :x x[0-9a-f])* \$;$/.test(statement)) continue;
  const value = hexValue(statement);
  if (value !== MOD && !numerals.has(value)) numerals.set(value, nameOf(statement));
}
const numeral = (bits) => {
  assert.ok(numerals.has(bits), `no frozen numeral for ${bits}`);
  return numerals.get(bits);
};

// ---- Subject decoding and claim derivation ---------------------------------------------------
const signedMin = -(1n << 63n);
const signedMax = (1n << 63n) - 1n;
const bitsOf = (value) => ((value % MOD) + MOD) % MOD;
function graph(text) {
  const lines = text.split('\n');
  assert.equal(lines.pop(), '', 'graph lacks the final newline');
  assert.equal(lines[0], 'ngraph-ir/0 ngraph-i64/0', 'graph revision');
  assert.ok(/^subject authored [0-9a-f]{64} closure -$/.test(lines[1]), 'graph subject line');
  let i = 2;
  let slots = 0;
  while (lines[i] === 'slot input i64') { slots += 1; i += 1; }
  const nodes = [];
  for (; i < lines.length && lines[i].startsWith('%'); i += 1) {
    const id = nodes.length;
    let m = /^%(\d+) = input (\d+)$/.exec(lines[i]);
    if (m && Number(m[1]) === id && Number(m[2]) < slots) { nodes.push({ kind: 'input', slot: Number(m[2]) }); continue; }
    m = /^%(\d+) = const i64 (-?\d+)$/.exec(lines[i]);
    if (m && Number(m[1]) === id) {
      const value = BigInt(m[2]);
      assert.ok(value >= signedMin && value <= signedMax, 'non-I64 constant');
      nodes.push({ kind: 'const', bits: bitsOf(value) });
      continue;
    }
    m = /^%(\d+) = i64\.(add|mul) %(\d+) %(\d+)$/.exec(lines[i]);
    assert.ok(m && Number(m[1]) === id && Number(m[3]) < id && Number(m[4]) < id, `unsupported node: ${lines[i]}`);
    nodes.push({ kind: m[2], operands: [Number(m[3]), Number(m[4])] });
  }
  const outputs = lines.slice(i).map((line) => {
    const m = /^out %(\d+)$/.exec(line);
    assert.ok(m && Number(m[1]) < nodes.length, `invalid output line: ${line}`);
    return Number(m[1]);
  });
  assert.ok(slots > 0 && outputs.length > 0, 'graph needs input slots and outputs');
  return { subject: lines[1], slots, nodes, outputs };
}
function certificate(text) {
  const parts = text.split(' ');
  assert.equal(parts.slice(0, 2).join(' '), 'ngraph-cert/0 ngraph-sem/0', 'certificate revision');
  const numbers = parts.slice(2).map((part) => { assert.ok(/^\d+$/.test(part)); return BigInt(part); });
  const [count, ...rest] = numbers;
  assert.equal(count, 1n, 'the profile admits exactly one step');
  assert.equal(rest.length, 7, 'one step carries seven fields');
  // cert.rs `encode`: rule node operand0 operand1 first_bits second_bits claimed; rule 0 = FoldAddConstants.
  const [rule, node, left, right, first, second, claimed] = rest;
  assert.equal(rule, 0n, 'not a FoldAddConstants step');
  for (const bits of [first, second, claimed]) assert.ok(bits < MOD, 'certificate bit width exceeded');
  return { node: Number(node), operands: [Number(left), Number(right)], first, second, claimed };
}
const binders = (slots) => [...Array(slots).keys()].map((k) => String.fromCharCode(97 + k));
function mm0Term(g, id, names) {
  const node = g.nodes[id];
  if (node.kind === 'input') return names[node.slot];
  if (node.kind === 'const') return numeral(node.bits);
  const arg = (operand) => {
    const term = mm0Term(g, operand, names);
    return term.includes(' ') ? `(${term})` : term;
  };
  return `${node.kind === 'add' ? 'uadd' : 'umul'} ${arg(node.operands[0])} ${arg(node.operands[1])}`;
}
// Compiler-lab's report spelling of a reconstructed output, for cross-checking.
function reportTerm(g, id) {
  const node = g.nodes[id];
  if (node.kind === 'input') return `input${node.slot}`;
  if (node.kind === 'const') return node.bits.toString();
  return `(${node.kind} ${reportTerm(g, node.operands[0])} ${reportTerm(g, node.operands[1])})`;
}
const subjectNames = ['small', 'boundary'];
const derived = {};
for (const name of subjectNames) {
  const read = (file) => readFileSync(path.join(lane, 'subjects', name, file), 'utf8');
  const before = graph(read('before.graph'));
  const after = graph(read('after.graph'));
  const step = certificate(read('certificate.wire'));
  // Replay the fold on the original graph and require the exact rewritten graph.
  const target = before.nodes[step.node];
  assert.ok(target?.kind === 'add' && target.operands.join() === step.operands.join(), 'step names no add node');
  const [l, r] = step.operands.map((id) => before.nodes[id]);
  assert.ok(l.kind === 'const' && r.kind === 'const' && l.bits === step.first && r.bits === step.second,
    'certificate operand bits differ from the consumed constants');
  assert.equal(step.claimed, (step.first + step.second) % MOD, 'claimed value is not the wrapping sum');
  const replayed = before.nodes.map((node, id) => (id === step.node ? { kind: 'const', bits: step.claimed } : node));
  assert.deepEqual({ ...after, nodes: after.nodes }, { ...before, nodes: replayed },
    'rewritten graph is not the replayed fold');
  const names = binders(before.slots);
  const side = (g) => `${g.outputs.map((id) => mm0Term(g, id, names)).join(' : ')} : 0`;
  const params = `(${names.join(' ')}: nat)`;
  derived[name] = {
    endpoint: `theorem endpoint_${name} ${params}: $ ${side(before)} = ${side(after)} $;`,
    fold: `theorem ufold_${name}: $ uadd ${numeral(step.first)} ${numeral(step.second)} = ${numeral(step.claimed)} $;`,
    original_outputs: before.outputs.map((id) => reportTerm(before, id)),
    rewritten_outputs: after.outputs.map((id) => reportTerm(after, id)),
    step,
  };
}
// Noble's caller theory must state exactly the derived claims.
const theorem = (name) => {
  const statement = theory.find((s) => nameOf(s) === name && s.startsWith('theorem '));
  assert.ok(statement, `caller theorem ${name} missing`);
  return statement;
};
for (const name of subjectNames) {
  assert.equal(theorem(`endpoint_${name}`), derived[name].endpoint, `theory endpoint_${name} differs from the derived claim`);
  assert.equal(theorem(`ufold_${name}`), derived[name].fold, `theory ufold_${name} differs from the derived claim`);
}
const lemmas = profile.required_theorems.filter((name) => name.startsWith('fold_'));
assert.deepEqual(theory.filter((s) => s.startsWith('theorem ')).map(nameOf).sort(),
  [...profile.required_theorems].sort(), 'caller theorem inventory differs from the required theorems');

// ---- Frozen request ----------------------------------------------------------------------------
// Key order follows compiler-lab's serde field order, so the compact JSON digest
// below is the same value the backend reports as `binding.frozen_sha256`.
const frozen = {
  checker: { profile: policy.checker_profile, build: 'cc -O2 main.c -o mm0-c', flags: ['-O2'], defines: [],
    parser_enabled: true },
  obligation_profile: policy.obligation_profile,
  specification: { theory_sha256: profile.inputs['theory.mm0'], imports: ['peano_hex.mm0', 'peano.mm0'],
    flattened_sha256: profile.spec_sha256 },
  approved_axioms: spec.filter((s) => s.startsWith('axiom ')),
  definitions,
  interpretation: { modulus: 'modulus', add: 'uadd', mul: 'umul' },
  obligations: [
    ...subjectNames.map((subject) => ({ role: 'endpoint-equivalence', declaration: derived[subject].endpoint, subject })),
    ...subjectNames.map((subject) => ({ role: 'fold-fact', declaration: derived[subject].fold, subject })),
    ...lemmas.map((name) => ({ role: 'lemma', declaration: theorem(name) })),
  ],
  subjects: subjectNames.map((name) => ({
    name,
    representation: 'ngraph-ir/0 ngraph-i64/0',
    certificate_format: 'ngraph-cert/0 ngraph-sem/0',
    before_sha256: profile.inputs[`subjects/${name}/before.graph`],
    after_sha256: profile.inputs[`subjects/${name}/after.graph`],
    certificate_sha256: profile.inputs[`subjects/${name}/certificate.wire`],
  })),
};
const frozenSha = sha(JSON.stringify(frozen));
const lanePath = (relative) => path.join(lane, relative);
const presented = {
  theory: lanePath('theory.mm0'),
  specification: specFile,
  proof: proofFile,
  subjects: Object.fromEntries(subjectNames.map((name) => [name, {
    before: lanePath(`subjects/${name}/before.graph`),
    after: lanePath(`subjects/${name}/after.graph`),
    certificate: lanePath(`subjects/${name}/certificate.wire`),
  }])),
  generator: lanePath('retained/production.json'),
};

// ---- Submissions -------------------------------------------------------------------------------
const requests = path.join(out, 'requests');
const changed = path.join(out, 'presented');
mkdirSync(requests);
mkdirSync(changed);
const writeChanged = (name, body) => {
  const file = path.join(changed, name);
  writeFileSync(file, body);
  return file;
};
const mutate = (file, from, to) => {
  const text = readFileSync(file, 'utf8');
  assert.ok(text.split(from).length === 2 && !text.includes(to), `mutation is not unique: ${from}`);
  return text.replace(from, to);
};
const submission = (name, change = () => {}) => {
  const copy = structuredClone(presented);
  change(copy);
  const file = path.join(requests, `${name}.json`);
  writeFileSync(file, `${JSON.stringify({ schema: 1, kind: 'compiler-lab-mm0-certificate-request', frozen, presented: copy }, null, 2)}\n`);
  return file;
};
const small = presented.subjects.small;
const proofBytes = readFileSync(proofFile);
const malformed = Buffer.from(proofBytes);
malformed.fill(0, 0, 16);
const controls = path.join(lane, 'retained/controls');
// Expected outcomes follow compiler-lab's documented first-refusing-stage order.
const refusals = [
  ['malformed-proof', (p) => { p.proof = writeChanged('malformed.mmb', malformed); },
    { stage: 'logical-check', checker_stage: 'mmb-header' }],
  ['truncated-proof', (p) => { p.proof = writeChanged('truncated.mmb', proofBytes.subarray(0, Math.floor(proofBytes.length / 2))); },
    { stage: 'logical-check', checker_stage: 'mmb-proof' }],
  ['unrelated-endpoint', (p) => {
    p.subjects.small.after = writeChanged('unrelated-after.graph', mutate(small.after, '%4 = i64.mul %0 %3', '%4 = i64.mul %0 %1'));
  }, { stage: 'claim-reconstruction' }],
  ['reordered-outputs', (p) => {
    p.subjects.small.after = writeChanged('reordered-after.graph', mutate(small.after, 'out %1\nout %2', 'out %2\nout %1'));
  }, { stage: 'claim-reconstruction' }],
  ['changed-shared-use', (p) => {
    p.subjects.small.before = writeChanged('shared-before.graph', mutate(small.before, '%4 = i64.mul %0 %3', '%4 = i64.mul %0 %2'));
  }, { stage: 'claim-reconstruction' }],
  ['changed-consumed-bits', (p) => {
    p.subjects.small.before = writeChanged('bits-before.graph', mutate(small.before, '%2 = const i64 5', '%2 = const i64 6'));
  }, { stage: 'claim-reconstruction' }],
  ['changed-certificate-bits', (p) => {
    p.subjects.small.certificate = writeChanged('bits-certificate.wire', mutate(small.certificate, '3 5 8', '3 5 9'));
  }, { stage: 'claim-reconstruction' }],
  ['proof-of-another-claim', (p) => {
    p.specification = path.join(controls, 'different.mm0');
    p.proof = path.join(controls, 'different.mmb');
  }, { stage: 'assumption-policy', also: ['obligations'] }],
  ['unapproved-axiom', (p) => {
    p.specification = path.join(controls, 'forged.mm0');
    p.proof = path.join(controls, 'forged.mmb');
  }, { stage: 'assumption-policy' }],
  ['sorry-proof', (p) => {
    p.specification = path.join(controls, 'sorry.mm0');
    p.proof = path.join(controls, 'sorry.mmb');
  }, { stage: 'assumption-policy', checker_stage: 'sorry' }],
  ['extra-axiom', (p) => {
    p.specification = writeChanged('extra-axiom.mm0',
      `${readFileSync(specFile, 'utf8')}\naxiom oracle_any (p: wff): $ p $;\n`);
  }, { stage: 'assumption-policy' }],
  ['changed-axiom-formula', (p) => {
    p.specification = writeChanged('changed-axiom.mm0', mutate(specFile,
      'axiom peano1 (a: nat): $ suc a != 0 $;', 'axiom peano1 (a: nat): $ suc a = 0 $;'));
  }, { stage: 'assumption-policy' }],
  ['changed-arithmetic-definition', (p) => {
    p.specification = writeChanged('changed-arithmetic.mm0', mutate(specFile,
      'def uadd (a b: nat): nat = $ (a + b) % modulus $;', 'def uadd (a b: nat): nat = $ (a * b) % modulus $;'));
  }, { stage: 'definitions' }],
  ['changed-model-theory', (p) => {
    const from = 'def umul (a b: nat): nat = $ (a * b) % modulus $;';
    const to = 'def umul (a b: nat): nat = $ (a * b) % wmin $;';
    p.theory = writeChanged('changed-model-theory.mm0', mutate(p.theory, from, to));
    p.specification = writeChanged('changed-model-spec.mm0', mutate(specFile, from, to));
  }, { stage: 'definitions' }],
  ['stale-subject', (p) => {
    for (const kind of ['before', 'after']) {
      p.subjects.small[kind] = writeChanged(`stale-${kind}.graph`,
        mutate(small[kind], 'subject authored 9551', 'subject authored 8551'));
    }
  }, { stage: 'subject-identity' }],
];
// Resource exhaustion through the caller's limits; the presented files are unchanged.
const limited = [
  ['exhausted-wall-limit', ['--timeout-ms', '1'], { outcome: 'timeout' }],
  ['exhausted-storage-limit', ['--max-storage-mib', '1'], { outcome: 'storage-limit' }],
];

const runs = path.join(out, 'runs');
mkdirSync(runs);
const env = { ...process.env, PATH: `${process.env.PATH}:/run/current-system/sw/bin` };
function admit(name, args) {
  const runDir = path.join(runs, name);
  const started = process.hrtime.bigint();
  const run = spawnSync(binary, ['validate', 'mm0-certificate', '--tools-root', toolsLink, ...args, '--out', runDir],
    { encoding: 'utf8', env, maxBuffer: 64 * 1024 * 1024, timeout: 900_000, killSignal: 'SIGKILL' });
  const wallMs = Number(process.hrtime.bigint() - started) / 1e6;
  const reportFile = path.join(runDir, 'report.json');
  assert.ok(existsSync(reportFile), `${name}: no report (exit ${run.status}) ${run.stderr}`);
  const raw = readFileSync(reportFile, 'utf8');
  return { runDir, exit: run.status, wallMs, raw, report: JSON.parse(raw) };
}
const home = homedir();
const host = hostname();
const scrubPairs = [
  [out, '<run-directory>'], [root, '<noble-checkout>'], [toolsLink, '<tools-root>'],
  [checkout, '<compiler-lab-checkout>'], [home, '<home>'],
].sort((a, b) => b[0].length - a[0].length);
function scrub(text) {
  let result = text;
  for (const [from, to] of scrubPairs) result = result.split(from).join(to);
  assert.ok(!result.includes('/home/') && !result.includes(host), 'scrubbed report still names the host');
  JSON.parse(result);
  return result;
}
const retained = path.join(out, 'retain');
const retainedReports = path.join(retained, 'reports');
mkdirSync(retainedReports, { recursive: true });
const rows = {};
function record(name, admission, expected) {
  const r = admission.report;
  assert.equal(r.binding.frozen_sha256, frozenSha, `${name}: backend frozen digest differs from Noble's`);
  // A run whose limits stop it before closure authentication reports no tools.
  if (r.tools) {
    assert.equal(r.tools.root, policy.tools_closure, `${name}: tools closure differs`);
    assert.equal(r.tools.closure_identity_sha256, policy.closure_identity_sha256, `${name}: closure identity differs`);
    assert.equal(r.tools.pins_sha256, policy.pins_sha256, `${name}: pins differ`);
    assert.equal(r.tools.expression_sha256, policy.expression_sha256, `${name}: expression differs`);
  }
  assert.equal(r.logical_checking?.checker?.executable?.sha256 ?? policy.checker_sha256, policy.checker_sha256,
    `${name}: checker digest differs`);
  assert.equal(r.producer_invoked, false, `${name}: backend ran the proof producer`);
  const scrubbed = scrub(admission.raw);
  writeFileSync(path.join(retainedReports, `${name}.json`), scrubbed);
  const a = r.admission;
  const run = r.logical_checking?.run;
  const row = {
    expected, exit_code: admission.exit,
    observed: { outcome: a.outcome, stage: a.stage, checker_stage: a.checker_stage, refused_stages: a.refused_stages },
    controls: { held: r.controls?.held ?? null, failed: r.controls?.failed ?? null, passed: r.controls?.passed ?? null },
    checker: run ? { exit_code: run.process?.exit_code ?? null, elapsed_ms: run.process?.elapsed_ms ?? null,
      peak_rss_kib: run.time?.peak_rss_kib ?? null, verdict: run.verdict ?? null,
      specification_sha256: run.specification_sha256, proof_sha256: run.proof_sha256 } : null,
    admission_wall_ms: Math.round(admission.wallMs),
    report_sha256: sha(scrubbed),
  };
  rows[name] = row;
  return row;
}
const controlOutcome = (r, control) => r.controls.results.find((c) => c.name === control);

// Positive submission and its proposer-free replay.
const positive = admit('positive', ['--request', submission('positive')]);
assert.ok(positive.report.tools, 'positive submission authenticated no tools');
const pos = record('positive', positive, { outcome: 'accepted' });
assert.equal(positive.exit, 0, 'positive submission not accepted');
assert.equal(pos.observed.outcome, 'accepted');
assert.deepEqual([pos.controls.held, pos.controls.failed, pos.controls.passed], [23, 0, true], 'controls not all held');
assert.equal(pos.checker.proof_sha256, profile.retained_proof_sha256);
assert.equal(pos.checker.specification_sha256, profile.spec_sha256);
const claims = positive.report.claim_descriptor.reconstructed_claims;
for (const name of subjectNames) {
  const endpoint = claims.find((c) => c.obligation === `endpoint_${name}`);
  assert.ok(endpoint?.matched, `endpoint_${name} not matched`);
  assert.deepEqual(endpoint.original_outputs, derived[name].original_outputs, `${name}: backend original outputs differ`);
  assert.deepEqual(endpoint.rewritten_outputs, derived[name].rewritten_outputs, `${name}: backend rewritten outputs differ`);
  assert.ok(claims.find((c) => c.obligation === `ufold_${name}`)?.matched, `ufold_${name} not matched`);
}
for (const [control, outcome] of [['exhausted-address-space', 'resource-exhausted'], ['exhausted-wall-clock', 'timeout']]) {
  const result = controlOutcome(positive.report, control);
  assert.ok(result?.passed && result.observed.admission === outcome, `${control} did not end in ${outcome}`);
  pos[control] = { observed: result.observed.admission, passed: result.passed };
}
const replay = admit('replay', ['--replay', positive.runDir]);
const rep = record('replay', replay, { outcome: 'accepted', producer_invoked: false });
assert.equal(replay.exit, 0, 'replay not accepted');
assert.equal(replay.report.replay.retained_admission.outcome, 'accepted');
assert.equal(replay.report.replay.fresh_checker_verdict.verdict, 'accepted');
assert.equal(replay.report.replay.retained_checker_verdict.verdict, 'accepted');
assert.equal(replay.report.replay.producer_invoked, false);
rep.replay = { retained: 'accepted', fresh: 'accepted', retained_frozen_sha256: replay.report.replay.retained_binding.frozen_sha256 };
assert.equal(rep.replay.retained_frozen_sha256, frozenSha);

// Refusals: each must not accept and must refuse at Noble's expected stage.
for (const [name, change, expected] of refusals) {
  const result = admit(name, ['--request', submission(name, change)]);
  const row = record(name, result, { outcome: 'refused', ...expected });
  assert.notEqual(result.exit, 0, `${name} was accepted`);
  assert.equal(row.observed.outcome, 'refused', `${name}: outcome ${row.observed.outcome}`);
  assert.equal(row.observed.stage, expected.stage, `${name}: refused at ${row.observed.stage}`);
  if (expected.checker_stage) assert.equal(row.observed.checker_stage, expected.checker_stage, `${name}: checker stage`);
  for (const stage of expected.also ?? []) assert.ok(row.observed.refused_stages.includes(stage), `${name}: ${stage} not refused`);
}
for (const [name, flags, expected] of limited) {
  const result = admit(name, ['--request', submission(name), ...flags]);
  const row = record(name, result, expected);
  row.caller_limits = flags;
  assert.notEqual(result.exit, 0, `${name} was accepted`);
  assert.equal(row.observed.outcome, expected.outcome, `${name}: outcome ${row.observed.outcome}`);
}

const receipt = {
  schema: 'noble-mm0-compiler-lab-admission/v1',
  issue: 127,
  noble_source_commit: nobleCommit,
  adapter_sha256: fileSha(fileURLToPath(import.meta.url)),
  backend: {
    name: policy.backend, compiler_lab_commit: policy.compiler_lab_commit,
    compiler_lab_binary_sha256: fileSha(binary), checker_profile: policy.checker_profile,
    obligation_profile: policy.obligation_profile, system: policy.system, tools_closure: policy.tools_closure,
    closure_identity_sha256: policy.closure_identity_sha256, checker_sha256: policy.checker_sha256,
    negative_checker_sha256: policy.negative_checker_sha256, pins_sha256: policy.pins_sha256,
    expression_sha256: policy.expression_sha256, base_theory_sha256: policy.base_theory_sha256,
  },
  noble_frozen_sha256: frozenSha,
  derived_claims: Object.fromEntries(subjectNames.map((name) => [name, {
    endpoint: derived[name].endpoint, fold_fact: derived[name].fold,
    original_outputs: derived[name].original_outputs, rewritten_outputs: derived[name].rewritten_outputs,
    step: Object.fromEntries(Object.entries(derived[name].step).map(([k, v]) =>
      [k, typeof v === 'bigint' ? v.toString() : v])),
  }])),
  approved_axioms: frozen.approved_axioms.length,
  artifacts: { specification_bytes: readFileSync(specFile).length, proof_bytes: proofBytes.length,
    specification_sha256: profile.spec_sha256, proof_sha256: profile.retained_proof_sha256 },
  producer_invoked: false,
  rows,
};
writeFileSync(path.join(retained, 'request.json'),
  scrub(readFileSync(path.join(requests, 'positive.json'), 'utf8')));
writeFileSync(path.join(retained, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({ frozen_sha256: frozenSha, accepted: ['positive', 'replay'],
  refused: Object.keys(rows).filter((k) => !['positive', 'replay'].includes(k)), retain: retained }));
