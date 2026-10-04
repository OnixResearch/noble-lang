import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

export const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
export const sha256 = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
export const fileHash = file => sha256(fs.readFileSync(file));

function relativeName(name) {
  assert.ok(typeof name === 'string' && name && !path.posix.isAbsolute(name)
    && !name.split('/').some(piece => piece === '.' || piece === '..' || piece === '')
    && !name.includes('\\') && !name.includes('\0'), `unsafe retained name: ${name}`);
  return name;
}

export class Recorder {
  constructor(destination) {
    this.output = path.resolve(destination);
    assert.ok(this.output !== root && !this.output.startsWith(`${root}${path.sep}`),
      'fresh external output directory required');
    assert.ok(!fs.existsSync(this.output), 'do not reuse an existing receipt directory');
    assert.equal(fs.realpathSync(path.dirname(this.output)), path.dirname(this.output),
      'output parent may not be a symlink');
    fs.mkdirSync(this.output);
    this.watched = new Map();
    this.receipt = {
      schema: 'noble-intrinsic-proofs-acceptance/v1', profile: 'Intrinsic-Proofs-Draft',
      result: 'running', source_root: root, output_directory: this.output,
      plan: null, source_revision: null, tools: {}, build: [], retained: {}, commands: [],
      cases: [], review: null, blocked_release_cases: ['CONTRACT-20', 'CONTRACT-23'],
      failures: [], integrity_failures: [],
      assumptions: [
        'Pinned source compiler, selected Rust/Lean/Node/wasm-tools and isolated proof consumer remain trusted; hashes bind bytes rather than semantic correctness.',
        'These finite checks cannot prove Noble source-to-Lean translation, Rust implementation refinement, general compiler-to-Wasm correspondence or host authorization.',
        'Candidate-authored module contracts cannot discharge the owner-frozen law required by VC-OWNER-01/CONTRACT-16.',
        'The separately compiled NamedV1 Lean overlay does not establish the accepted Noble source-to-named-recipe correspondence; named-call MC1 host proof admission remains fail-closed.',
      ],
    };
  }

  relative(file) { return relativeName(path.relative(this.output, file).split(path.sep).join('/')); }

  watch(file) {
    const digest = fileHash(file);
    const prior = this.watched.get(file);
    assert.ok(prior === undefined || prior === digest, `source changed during gate: ${file}`);
    this.watched.set(file, digest);
    return digest;
  }

  retain(name, bytes, mode = 0o400) {
    const file = path.join(this.output, relativeName(name));
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, bytes, { flag: 'wx', mode });
    this.receipt.retained[name] = { sha256: this.watch(file), bytes: fs.statSync(file).size };
    return file;
  }

  source(name) {
    const file = path.join(root, relativeName(name));
    assert.ok(fs.lstatSync(file).isFile(), `not plain source: ${name}`);
    const bytes = fs.readFileSync(file);
    const digest = this.watch(file);
    assert.equal(sha256(bytes), digest, `changed source while reading: ${name}`);
    const snapshot = this.retain(`plan/sources/${name}`, bytes);
    return { sha256: digest, bytes: bytes.length, snapshot: this.relative(snapshot) };
  }

  freeze(name, original) {
    relativeName(name);
    const real = fs.realpathSync(path.resolve(original));
    assert.ok(fs.statSync(real).isFile(), `not a selected executable: ${name}`);
    const frozen = this.retain(`executables/${name}`, fs.readFileSync(real), 0o500);
    assert.equal(fileHash(frozen), this.watch(real), `selected tool changed while freezing: ${name}`);
    this.receipt.tools[name] = { original: real, frozen: this.relative(frozen), sha256: fileHash(frozen) };
    return frozen;
  }

  command(label, executable, args, { input = Buffer.alloc(0), env, cwd = root,
    timeout = 180_000 } = {}) {
    relativeName(label);
    assert.ok(Array.isArray(args) && args.every(arg => typeof arg === 'string'), `${label}: argv`);
    assert.ok(Buffer.isBuffer(input), `${label}: raw stdin required`);
    assert.ok(Number.isSafeInteger(timeout) && timeout > 0, `${label}: bounded deadline`);
    const id = this.receipt.commands.length + 1;
    const stem = `commands/${String(id).padStart(4, '0')}-${label}`;
    const stdin = this.retain(`${stem}.stdin`, input);
    const result = spawnSync(executable, args, { cwd, input, encoding: null,
      timeout, killSignal: 'SIGKILL', maxBuffer: 32 * 1024 * 1024, env });
    const stdout = this.retain(`${stem}.stdout`, result.stdout ?? Buffer.alloc(0));
    const stderr = this.retain(`${stem}.stderr`, result.stderr ?? Buffer.alloc(0));
    const row = { id, label, executable, executable_sha256: this.watch(executable),
      argv: args, cwd, environment: env ?? null, stdin: this.relative(stdin),
      stdout: this.relative(stdout), stderr: this.relative(stderr), timeout_ms: timeout,
      status: result.status, signal: result.signal, error: result.error ? String(result.error) : null };
    this.receipt.commands.push(row);
    assert.equal(row.error, null, `${label}: could not launch executable`);
    assert.equal(row.signal, null, `${label}: killed or timed out`);
    assert.notEqual(row.status, null, `${label}: no exit status`);
    return { ...row, bytes: result.stdout ?? Buffer.alloc(0), stderrBytes: result.stderr ?? Buffer.alloc(0) };
  }

  framed(sourceFiles) {
    return Buffer.concat(sourceFiles.flatMap(file => {
      const bytes = fs.readFileSync(file);
      this.watch(file);
      return [Buffer.from(`${bytes.length}\n`), bytes];
    }));
  }

  emission(label, directory, sourceFiles) {
    assert.ok(fs.existsSync(directory) && fs.lstatSync(directory).isDirectory(),
      `${label}: no production CLI emission`);
    const files = [];
    const visit = parent => {
      for (const item of fs.readdirSync(parent, { withFileTypes: true })
        .sort((left, right) => left.name.localeCompare(right.name))) {
        const file = path.join(parent, item.name);
        if (item.isDirectory()) visit(file);
        else {
          assert.ok(item.isFile() && fs.lstatSync(file).isFile(), `${label}: non-file artifact`);
          files.push({ file: this.relative(file), sha256: this.watch(file),
            bytes: fs.statSync(file).size });
        }
      }
    };
    visit(directory);
    for (const [index, source] of sourceFiles.entries()) {
      const emitted = path.join(directory, `${index + 1}.noble`);
      assert.ok(fs.existsSync(emitted), `${label}: source frame not emitted`);
      assert.deepEqual(fs.readFileSync(emitted), fs.readFileSync(source),
        `${label}: actual submitted bytes differ from frozen source`);
    }
    return files;
  }

  wasm(label, wasmTools, artifacts) {
    const binaries = artifacts.filter(item => item.file.endsWith('.wasm'));
    assert.ok(binaries.length > 0, `${label}: no actual compiled Wasm`);
    return binaries.map((item, index) => {
      const file = path.join(this.output, item.file);
      assert.equal(fileHash(file), item.sha256, `${label}: Wasm changed`);
      const validation = this.command(`${label}-validate-${index}`, wasmTools,
        ['validate', file], { env: { PATH: '/nonexistent', LANG: 'C', LC_ALL: 'C' } });
      assert.equal(validation.status, 0, `${label}: selected wasm-tools rejected emitted Wasm`);
      const inspection = this.command(`${label}-print-${index}`, wasmTools,
        ['print', file], { env: { PATH: '/nonexistent', LANG: 'C', LC_ALL: 'C' } });
      assert.equal(inspection.status, 0, `${label}: selected wasm-tools cannot print emitted Wasm`);
      return { ...item, validate_command: validation.id, print_command: inspection.id };
    });
  }

  seal(expectedCases) {
    if (!this.receipt.plan || !this.receipt.source_revision)
      this.receipt.failures.push({ case_id: 'gate', error: 'missing source-bound plan or revision' });
    try {
      assert.deepEqual(this.receipt.cases.map(row => row.id).sort(), expectedCases.slice().sort(),
        'missing or duplicate executed conformance case');
      assert.ok(this.receipt.cases.every(row => row.passed === true),
        'unexecuted or failed selected conformance case');
      for (const row of this.receipt.cases) {
        const planned = this.receipt.case_ledger?.[row.id];
        assert.ok(planned, `${row.id}: missing independently frozen case ledger`);
        assert.deepEqual((row.positive ?? []).map(item => item.name).sort(),
          planned.required_positive_baselines.slice().sort(),
          `${row.id}: missing or duplicate real positive baseline`);
        assert.deepEqual((row.hostiles ?? []).map(item => item.name).sort(),
          planned.required_hostile_variants.slice().sort(),
          `${row.id}: missing or duplicate hostile outcome`);
        assert.ok([...row.positive, ...row.hostiles].every(item =>
          Number.isSafeInteger(item.command) && item.command > 0),
        `${row.id}: observations are not bound to actual commands`);
      }
      assert.ok(this.receipt.review && this.receipt.commands.length > 0,
        'no actual commands or separated assurance review');
    } catch (error) {
      this.receipt.failures.push({ case_id: 'gate', error: String(error) });
    }
    for (const [file, digest] of this.watched) try {
      assert.equal(fileHash(file), digest, `${file}: changed during acceptance`);
    } catch (error) { this.receipt.integrity_failures.push({ file, error: String(error) }); }
    this.receipt.summary = { cases: this.receipt.cases.length, commands: this.receipt.commands.length,
      failures: this.receipt.failures.length, integrity_failures: this.receipt.integrity_failures.length };
    this.receipt.result = this.receipt.failures.length === 0
      && this.receipt.integrity_failures.length === 0 ? 'passed' : 'failed';
    const file = path.join(this.output, 'report.json');
    fs.writeFileSync(file, JSON.stringify(this.receipt, null, 2) + '\n', { flag: 'wx', mode: 0o400 });
    return { report: file, result: this.receipt.result, summary: this.receipt.summary };
  }
}
