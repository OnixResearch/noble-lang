#!/usr/bin/env bun
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const binary = resolve(process.argv[2] ?? process.env.NOBLE_BIN ?? join(root, 'target/debug/noble'));
const destination = resolve(process.argv[3] ?? join(root, 'verification/mc1/acceptance.json'));
const temporary = await mkdtemp(join(tmpdir(), 'noble-mc1-gate-'));
const environment = { ...process.env, NOBLE_CONTRACT_LIBRARY: join(root, 'proofs/mc1') };
const evidence = { schema: 'noble-mc1-cli-acceptance/v1', binary, cases: [] };
const fixtures = join(root, 'verification/mc1');
const hash = text => createHash('sha256').update(text).digest('hex');
const constant = name => ['const', name.split('.'), []];
const selectedRoot = ['MC1Proof', 'proof'];

function command(executable, args, env = environment, timeout = 190000) {
  return new Promise((accept, reject) => {
    const child = spawn(executable, args, { cwd: root, env, stdio: ['ignore', 'pipe', 'pipe'] });
    const stdout = [], stderr = [];
    let size = 0, excessive = false;
    const timer = setTimeout(() => { excessive = true; child.kill('SIGKILL'); }, timeout);
    for (const [stream, chunks] of [[child.stdout, stdout], [child.stderr, stderr]]) {
      stream.on('data', chunk => {
        size += chunk.length;
        if (size > 33554432) { excessive = true; child.kill('SIGKILL'); }
        else chunks.push(chunk);
      });
    }
    child.once('error', error => { clearTimeout(timer); reject(error); });
    child.once('close', (exit, signal) => {
      clearTimeout(timer);
      if (excessive) reject(new Error(`gate process bound exceeded: ${executable}`));
      else accept({ exit, signal, stdout: Buffer.concat(stdout).toString(), stderr: Buffer.concat(stderr).toString() });
    });
  });
}

async function source(name, content) {
  const path = join(temporary, name);
  await writeFile(path, content);
  return path;
}

async function check(name, args, expected, options = {}) {
  const started = Date.now();
  const row = { name, expected, args, passed: false };
  evidence.cases.push(row);
  try {
    const result = await command(binary, args, { ...environment, ...options.env });
    row.elapsed_ms = Date.now() - started;
    row.exit = result.exit;
    row.stderr = result.stderr;
    const report = JSON.parse(result.stdout);
    row.report = report;
    assert.equal(report.outcome, expected.outcome);
    assert.equal(result.exit, expected.exit);
    if (options.ordinary) assert.equal(report.ordinary_typing.outcome, 'accepted');
    if (options.code) assert.ok(report.diagnostics.some(item => item.code === options.code));
    if (['proved', 'disproved'].includes(report.outcome)) {
      assert.equal(report.independent_recheck, true);
      assert.equal(report.implementation_refinement.status, 'not-checked-by-this-command');
      assert.equal(report.backend_correspondence.status, 'not-claimed');
      assert.ok(report.proof_declarations);
      assert.ok(report.assumptions.accepted_transitive_axioms.every(name =>
        ['propext', 'Classical.choice', 'Quot.sound'].includes(name)));
    } else assert.equal(report.independent_recheck, false);
    row.passed = true;
    console.log(`PASS ${name}: ${report.outcome}`);
    return report;
  } catch (error) {
    row.failure = String(error);
    console.error(`FAIL ${name}: ${error}`);
    return row.report;
  }
}

async function parallel(items, action) {
  let index = 0;
  await Promise.all([0, 1].map(async () => {
    while (index < items.length) {
      const item = items[index++];
      await action(item);
    }
  }));
}

// An untrusted module initializer controls its exporter and exits successfully.
// Only the resulting JSON reaches the authoritative consumer. This deliberately
// bypasses the helpful producer exporter to exercise the real hostile boundary.
function injectedWire(payload) {
  return `import Lean\nimport MC1Obligation\ninitialize do\n  if ← System.FilePath.pathExists "/out/MC1Proof.json" then\n    IO.FS.writeFile "/out/MC1Proof.json" ${JSON.stringify(payload)}\n    IO.Process.exit 0\nnamespace MC1Proof\ntheorem proof : True := True.intro\nend MC1Proof\n`;
}

function changedWire(wire, change) {
  const result = structuredClone(wire);
  change(result, result.declarations.find(item => JSON.stringify(item.name) === JSON.stringify(selectedRoot)));
  return JSON.stringify(result);
}

try {
  const positiveNames = ['increment', 'composed', 'family', 'structural', 'syntax', 'wrap'];
  await parallel(positiveNames, async name => {
    await check(name, ['verify', join(fixtures, `${name}.noble-contract`), '--proof', join(fixtures, `${name}-proof.lean`)],
      { outcome: 'proved', exit: 0 }, { ordinary: true });
  });
  await check('signed-overflow-refutation', ['verify', join(fixtures, 'monotonic.noble-contract'), '--refutation', join(fixtures, 'monotonic-refutation.lean')],
    { outcome: 'disproved', exit: 1 }, { ordinary: true });

  const increment = await readFile(join(fixtures, 'increment.noble-contract'), 'utf8');
  const incrementPath = join(fixtures, 'increment.noble-contract');
  const incrementProof = join(fixtures, 'increment-proof.lean');
  const unknown = await source('unknown-operator.noble-contract', increment.replace('(ensures ', '(ensures (future ').replace(/\)\)\s*$/, ')))\n'));
  const invalid = await source('invalid-predicate.noble-contract', increment.replace('(eq (out y) (add (in x) 1))', '(lt true 1)'));
  const malformed = await source('malformed.noble-contract', '(contract 1');
  await parallel([
    ['unknown-without-evidence', ['verify', incrementPath], { outcome: 'unknown', exit: 3 }, { ordinary: true }],
    ['explanation-does-not-run-tools', ['explain-proof', incrementPath], { outcome: 'not-run', exit: 0 }, { ordinary: true, env: { NOBLE_LEAN: '/nonexistent/never-run' } }],
    ['malformed-source', ['verify', malformed], { outcome: 'error', exit: 2 }, {}],
    ['typed-predicate-error-retains-acceptance', ['verify', invalid], { outcome: 'error', exit: 2 }, { ordinary: true }],
    ['unsupported-predicate-retains-acceptance', ['verify', unknown], { outcome: 'unsupported', exit: 4 }, { ordinary: true }],
    ['deadline-outcome', ['verify', incrementPath, '--proof', incrementProof, '--timeout-ms', '1'], { outcome: 'timeout', exit: 124 }, {}],
    ['missing-sandbox-fails-closed', ['verify', incrementPath, '--proof', incrementProof], { outcome: 'unsupported', exit: 4 }, { env: { NOBLE_BWRAP: '/nonexistent/missing-sandbox' } }],
  ], async ([name, args, expected, options]) => { await check(name, args, expected, options); });

  // Replay exact old declaration data against a different current statement.
  const original = evidence.cases.find(item => item.name === 'increment').report;
  assert.equal(original.outcome, 'proved', 'positive evidence is required for stale-statement controls');
  const wire = JSON.parse(original.proof_declarations);
  const replay = await source('replay-original.lean', injectedWire(original.proof_declarations));
  await check('untrusted-exporter-valid-wire', ['verify', incrementPath, '--proof', replay], { outcome: 'proved', exit: 0 });
  await parallel([
    ['changed-body', increment.replace('[ 1 + ]', '[ 2 + ]')],
    ['changed-postcondition', increment.replace('(add (in x) 1)', '(add (in x) 2)')],
    ['changed-precondition', increment.replace('(requires true)', '(requires (lt (in x) 0))')],
    ['changed-arithmetic', increment.replace('(add (in x) 1)', '(mul (in x) 1)')],
  ], async ([name, text]) => {
    assert.notEqual(text, increment, `${name} must change the subject`);
    const path = await source(`${name}.noble-contract`, text);
    await check(name, ['verify', path, '--proof', replay], { outcome: 'error', exit: 2 }, { ordinary: true });
  });

  const rawCases = [
    ['reordered-proof-dependencies', changedWire(wire, value => value.declarations.reverse()), 'proved', 0],
    ['malformed-proof-json', '{', 'error', 2],
    ['duplicate-envelope-field', original.proof_declarations.replace('{', '{"format":"noble-mc1-proof/v1",'), 'error', 2],
    ['duplicate-declaration', changedWire(wire, (value, declaration) => value.declarations.push(declaration)), 'error', 2],
    ['missing-dependency', changedWire(wire, (_, declaration) => { declaration.value = constant('Missing.proof'); }), 'error', 2],
    ['cyclic-dependency', changedWire(wire, (_, declaration) => { declaration.value = constant('MC1Proof.proof'); }), 'error', 2],
    ['unreachable-extra-declaration', changedWire(wire, value => value.declarations.push({ kind: 'theorem', name: ['Unused'], levels: [], type: constant('True'), value: constant('True.intro') })), 'error', 2],
    ['forbidden-axiom-declaration', changedWire(wire, (_, declaration) => { declaration.kind = 'axiom'; }), 'error', 2],
    ['free-variable-expression', changedWire(wire, (_, declaration) => { declaration.value = ['fvar', ['escape']]; }), 'error', 2],
    ['wrong-theorem-type', changedWire(wire, (value, declaration) => {
      declaration.type = constant('True'); declaration.value = constant('True.intro'); value.declarations = [declaration];
    }), 'error', 2],
    ['trusted-name-override', JSON.stringify({ format: 'noble-mc1-proof/v1', root: selectedRoot, declarations: [
      { kind: 'definition', name: ['MC1Obligation', 'claim'], levels: [], type: ['sort', ['zero']], value: constant('True') },
      { kind: 'theorem', name: selectedRoot, levels: [], type: constant('MC1Obligation.claim'), value: constant('True.intro') },
    ] }), 'error', 2],
    ['erased-forbidden-axiom', changedWire(wire, (_, declaration) => {
      const sorry = ['app', ['app', ['const', ['sorryAx'], [['zero']]], constant('True')], constant('Bool.true')];
      declaration.value = ['let', ['erased'], constant('True'), sorry, declaration.value, true];
    }), 'error', 2],
  ];
  await parallel(rawCases, async ([name, payload, outcome, exit]) => {
    const proof = await source(`${name}.lean`, injectedWire(payload));
    await check(name, ['verify', incrementPath, '--proof', proof], { outcome, exit },
      name === 'erased-forbidden-axiom' ? { code: 'forbidden-axiom' } : {});
  });

  const originalProof = await readFile(incrementProof, 'utf8');
  const forged = await source('forged-producer-status.lean', `import Lean\nimport MC1Obligation\nrun_cmd Lean.Elab.Command.liftIO <| IO.println "NOBLE-MC1-ACCEPT"\nnamespace MC1Proof\ntheorem proof : True := True.intro\nend MC1Proof\n`);
  const admitted = await source('admitted-proof.lean', 'import MC1Obligation\nnamespace MC1Proof\ntheorem proof : MC1Obligation.claim := by sorry\nend MC1Proof\n');
  const secretPath = await source('host-secret', randomUUID());
  const isolation = await source('isolation.lean', originalProof.replace('import MC1Obligation', `import Lean\nimport MC1Obligation\nrun_cmd do\n  let leaked ← Lean.Elab.Command.liftIO do\n    try\n      let _ ← IO.FS.readFile ${JSON.stringify(secretPath)}\n      pure true\n    catch _ => pure false\n  if leaked then throwError "host secret was readable"\n  let writable ← Lean.Elab.Command.liftIO do\n    try\n      IO.FS.writeFile "/obligation/MC1Obligation.lean" "changed"\n      pure true\n    catch _ => pure false\n  if writable then throwError "consumer obligation was writable"\n  let devices ← Lean.Elab.Command.liftIO <| IO.FS.readFile "/proc/net/dev"\n  for line in devices.splitOn "\\n" do\n    if line.contains ':' && !(line.trimAscii.toString.startsWith "lo:") then\n      throwError "host network interface visible"\n`));
  await parallel([
    ['forged-producer-status', forged, 'error', 2],
    ['admitted-proof', admitted, 'error', 2],
    ['host-files-obligation-and-network-isolated', isolation, 'proved', 0],
  ], async ([name, proof, outcome, exit]) => { await check(name, ['verify', incrementPath, '--proof', proof], { outcome, exit }); });

  const childTag = `MC1TimeoutChild_${randomUUID().replaceAll('-', '')}`;
  const childSource = 'def main : IO Unit := IO.sleep 120000\n';
  const hanging = await source('timeout-with-descendant.lean', `import Lean\nimport MC1Obligation\nrun_cmd Lean.Elab.Command.liftIO do\n  IO.FS.writeFile "/tmp/${childTag}.lean" ${JSON.stringify(childSource)}\n  let some executable ← IO.getEnv "NOBLE_LEAN_EXECUTABLE" | throw <| IO.userError "missing executable"\n  let _ ← IO.Process.spawn { cmd := executable, args := #["--run", "/tmp/${childTag}.lean"] }\n  IO.sleep 120000\n`);
  let observedChild = false;
  let monitoring = true;
  const monitor = (async () => {
    while (monitoring) {
      const result = await command('ps', ['-eo', 'args'], process.env, 10000);
      observedChild ||= result.stdout.includes(childTag);
      await new Promise(done => setTimeout(done, 500));
    }
  })();
  await check('untrusted-worker-timeout', ['verify', incrementPath, '--proof', hanging, '--timeout-ms', '45000'], { outcome: 'timeout', exit: 124 });
  monitoring = false;
  await monitor;
  const processes = await command('ps', ['-eo', 'args'], process.env, 10000);
  const cleanup = { name: 'timeout-kills-observed-descendant', observed_child: observedChild, remaining_child: processes.stdout.includes(childTag) };
  cleanup.passed = cleanup.observed_child && !cleanup.remaining_child;
  evidence.cases.push(cleanup);
  console.log(`${cleanup.passed ? 'PASS' : 'FAIL'} ${cleanup.name}`);

  evidence.passed = evidence.cases.every(item => item.passed);
  evidence.outcomes = [...new Set(evidence.cases.map(item => item.report?.outcome).filter(Boolean))].sort();
  assert.deepEqual(evidence.outcomes, ['disproved', 'error', 'not-run', 'proved', 'timeout', 'unknown', 'unsupported']);
} catch (error) {
  evidence.passed = false;
  evidence.failure = String(error);
  console.error(error);
} finally {
  evidence.binary_sha256 = hash(await readFile(binary));
  const consumerPaths = ['crates/noble-cli/src/consumer/decoding.lean', 'crates/noble-cli/src/consumer/replay.lean'];
  const consumerParts = await Promise.all(consumerPaths.map(file => readFile(join(root, file))));
  evidence.consumer_sha256 = hash(Buffer.concat(consumerParts));
  evidence.consumer_source_files = Object.fromEntries(consumerPaths.map((file, index) => [file, hash(consumerParts[index])]));
  evidence.gate_sha256 = hash(await readFile(fileURLToPath(import.meta.url)));
  await mkdir(dirname(destination), { recursive: true });
  await writeFile(destination, JSON.stringify(evidence, null, 2) + '\n');
  await rm(temporary, { recursive: true, force: true });
}
console.log(`${evidence.passed ? 'PASS' : 'FAIL'} MC1 CLI acceptance: ${evidence.cases.filter(item => item.passed).length}/${evidence.cases.length}; ${destination}`);
process.exitCode = evidence.passed ? 0 : 1;
