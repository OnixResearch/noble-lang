// Separate real-child control: deliberately not part of ambient `cargo test`.
// Usage: node tools/live-mmap-smoke.mjs --noble ABS --cc NIX_STORE_ABS
//   --cc-version EXACT_FIRST_LINE --cc-target EXACT_TRIPLE
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import readline from 'node:readline';
import { fileURLToPath } from 'node:url';

const DEADLINE_MS = 20_000;
const SOURCE = 'def addone [ 1 + ]';
const HELPER_SOURCE = fileURLToPath(new URL('../crates/noble-cli/tests/mmap_mutate.c', import.meta.url));

function options() {
  const selected = {};
  for (let at = 2; at < process.argv.length; at += 2) {
    const flag = process.argv[at];
    assert.ok(['--noble', '--cc', '--cc-version', '--cc-target'].includes(flag)
      && process.argv[at + 1] && selected[flag] === undefined, `invalid or duplicate option: ${flag}`);
    selected[flag] = process.argv[at + 1];
  }
  assert.equal(Object.keys(selected).length, 4, 'all four explicit selections are required');
  for (const flag of ['--noble', '--cc']) {
    assert.ok(path.isAbsolute(selected[flag]) && fs.statSync(selected[flag]).isFile(), `${flag} must name an absolute file`);
  }
  assert.ok(fs.realpathSync(selected['--cc']).startsWith('/nix/store/'), 'selected compiler must resolve to the pinned Nix store');
  return selected;
}

function run(file, args) {
  const result = spawnSync(file, args, { encoding: 'utf8', timeout: DEADLINE_MS, maxBuffer: 65_536 });
  assert.ifError(result.error);
  assert.equal(result.status, 0, `${file} ${args.join(' ')}: ${result.stderr}`);
  return result.stdout.trim();
}

function bounded(promise, label) {
  let timer;
  const deadline = new Promise((_, reject) => {
    timer = setTimeout(() => reject(Error(`${label} exceeded ${DEADLINE_MS}ms`)), DEADLINE_MS);
  });
  return Promise.race([promise, deadline]).finally(() => clearTimeout(timer));
}

async function main() {
  const selected = options();
  const version = run(selected['--cc'], ['--version']).split('\n')[0].trim();
  const target = run(selected['--cc'], ['-dumpmachine']);
  assert.equal(version, selected['--cc-version'], 'compiler version differs from the explicit selection');
  assert.equal(target, selected['--cc-target'], 'compiler target differs from the explicit selection');
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-live-mmap-'));
  let child;
  try {
    const source = path.join(directory, 'math.noble');
    const replacement = path.join(directory, 'math.noble.next');
    const helper = path.join(directory, 'mmap-mutate');
    fs.writeFileSync(source, SOURCE, { flag: 'wx', mode: 0o600 });
    run(selected['--cc'], ['-std=c11', '-D_GNU_SOURCE', '-Wall', '-Wextra', '-Werror', HELPER_SOURCE, '-o', helper]);
    child = spawn(selected['--noble'], ['live', 'repl', '--source', source,
      '--engine', 'v8', '--verify-source-before-execute'], { stdio: ['pipe', 'pipe', 'pipe'] });
    const lines = readline.createInterface({ input: child.stdout })[Symbol.asyncIterator]();
    let stderr = '';
    child.stderr.setEncoding('utf8');
    child.stderr.on('data', chunk => {
      stderr += chunk;
      if (stderr.length > 65_536) child.kill();
    });
    const closed = new Promise(resolve => child.once('close', (code, signal) => resolve({ code, signal })));
    const report = async () => {
      const { value, done } = await bounded(lines.next(), 'live report');
      assert.equal(done, false, `live child closed without report: ${stderr}`);
      assert.ok(Buffer.byteLength(value, 'utf8') <= 65_536, 'live report exceeds byte limit');
      return JSON.parse(value);
    };
    const submit = async text => {
      assert.ok(Buffer.byteLength(text, 'utf8') < 65_536, 'source request exceeds byte limit');
      await bounded(new Promise((resolve, reject) => child.stdin.write(`${text}\n`, error =>
        error ? reject(error) : resolve())), 'live input');
      return report();
    };
    const initial = await report();
    assert.equal(initial.schema, 'noble-live-report/v1');
    assert.equal(initial.outcome, 'reload-committed');
    assert.deepEqual(initial.stack, []);
    const generation = initial.generation;
    const acknowledged = initial.source_sha256;
    assert.equal(acknowledged, createHash('sha256').update(SOURCE).digest('hex'));

    run(helper, [source]); // Actual MAP_SHARED write + msync, without a selected-directory rename.
    assert.equal(fs.readFileSync(source, 'utf8'), 'def addone [ 7 + ]');
    const refused = await submit('20 addone');
    assert.equal(refused.schema, 'noble-live-report/v1');
    assert.equal(refused.stage, 'admission');
    assert.equal(refused.outcome, 'source-change-refused');
    assert.equal(refused.generation, generation);
    assert.equal(refused.selected_file_sha256, acknowledged);
    assert.notEqual(refused.observed_source_sha256, acknowledged);
    assert.deepEqual(refused.stack, []);
    assert.equal(refused.guest_requests, 0);
    assert.equal(refused.protected_operations, 0);

    fs.writeFileSync(replacement, SOURCE, { flag: 'wx', mode: 0o600 });
    fs.renameSync(replacement, source);
    const resumed = await submit('20 addone');
    assert.equal(resumed.outcome, 'normal');
    assert.equal(resumed.stack.length, 1);
    assert.equal(resumed.stack[0].type, 'I64');
    assert.equal(resumed.stack[0].value, '21');
    const inspection = await submit(':generation');
    assert.equal(inspection.outcome, 'generation-observed');
    assert.equal(inspection.generation, generation);
    child.stdin.end();
    assert.deepEqual(await bounded(closed, 'live child exit'), { code: 0, signal: null });
    console.log(JSON.stringify({ scope: 'separate-live-mmap-real-child-smoke',
      compiler: fs.realpathSync(selected['--cc']), compiler_version: version,
      compiler_target: target, helper_source_sha256: createHash('sha256')
        .update(fs.readFileSync(HELPER_SOURCE)).digest('hex'),
      generation, selected_file_sha256: acknowledged,
      mapped_source_sha256: refused.observed_source_sha256,
      refused_outcome: refused.outcome, resumed_outcome: resumed.outcome,
      guest_requests_on_refusal: refused.guest_requests,
      canonical_live_case_receipt: false }));
  } finally {
    if (child && child.exitCode === null) child.kill();
    fs.rmSync(directory, { recursive: true, force: true });
  }
}

main().catch(error => {
  console.error(`live mmap smoke failed: ${error.stack ?? error}`);
  process.exitCode = 1;
});
