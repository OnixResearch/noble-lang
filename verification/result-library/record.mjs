// Source-bound command recording for the DX-04 acceptance gate. A recorded run is
// evidence only after the caller checks its actual production observations.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

export const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
export const sha256 = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
export const fileHash = file => sha256(fs.readFileSync(file));

function safeName(name) {
  assert.ok(typeof name === 'string' && name.length > 0
    && !path.posix.isAbsolute(name) && !name.split('/').includes('..')
    && !name.split('/').includes('.') && !name.includes('\\') && !name.includes('\0'),
  `unsafe artifact name: ${name}`);
  return name;
}

export class Recorder {
  constructor(destination) {
    this.output = path.resolve(destination);
    assert.ok(this.output !== root && !this.output.startsWith(`${root}${path.sep}`),
      'acceptance output must be outside the repository');
    assert.ok(!fs.existsSync(this.output), 'acceptance output must be fresh');
    assert.equal(fs.realpathSync(path.dirname(this.output)), path.dirname(this.output),
      'output parent may not be a symlink');
    fs.mkdirSync(this.output);
    this.watched = new Map();
    this.receipt = {
      schema: 'noble-result-library-acceptance/v1', profile: 'Result-Library-Draft',
      result: 'running', source_root: root, output_directory: this.output,
      plan: null, revision_inputs: null, source_revision: null,
      peer_build: null,
      tools: {}, retained: {}, commands: [], cases: [], peer: null, failures: [], integrity_failures: [],
      assumptions: [
        'Selected pinned Noble/kernel binaries, source compiler, managed-memory Wasm, Node/V8 and wasm-tools are trusted execution mechanisms; byte hashes alone are not correctness proofs.',
        'Test-host bindings and authorization are host-supplied independently of guest library source.',
        'Finite DX-04 observations do not prove universal compiler, host or engine refinement, portable package identity or resource-positive ownership.',
      ],
    };
  }

  relative(file) {
    const relative = path.relative(this.output, file).split(path.sep).join('/');
    safeName(relative);
    return relative;
  }

  watch(file) {
    const digest = fileHash(file);
    const before = this.watched.get(file);
    assert.ok(before === undefined || before === digest, `watched file changed: ${file}`);
    this.watched.set(file, digest);
    return digest;
  }

  retain(name, bytes, mode = 0o400) {
    const file = path.join(this.output, safeName(name));
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, bytes, { flag: 'wx', mode });
    this.receipt.retained[name] = { sha256: this.watch(file), bytes: fs.statSync(file).size };
    return file;
  }

  source(name) {
    const source = path.join(root, safeName(name));
    assert.ok(fs.lstatSync(source).isFile(), `source must be a plain file: ${name}`);
    const bytes = fs.readFileSync(source);
    const digest = this.watch(source);
    assert.equal(sha256(bytes), digest, `source changed while snapshotting: ${name}`);
    const snapshot = `plan/sources/${name}`;
    this.retain(snapshot, bytes);
    return { sha256: digest, bytes: bytes.length, snapshot };
  }

  freeze(name, executable) {
    safeName(name);
    const original = fs.realpathSync(path.resolve(executable));
    assert.ok(fs.statSync(original).isFile(), `selected tool is not a file: ${name}`);
    const frozen = this.retain(`executables/${name}`, fs.readFileSync(original), 0o500);
    const sha256 = this.watch(original);
    assert.equal(sha256, fileHash(frozen), `${name}: copied executable differs`);
    this.receipt.tools[name] = { original, frozen: this.relative(frozen), sha256 };
    return frozen;
  }

  command(label, executable, args, input = Buffer.alloc(0), timeout = 180_000) {
    safeName(label);
    assert.ok(Array.isArray(args) && args.every(value => typeof value === 'string'),
      `${label}: argv must be exact strings`);
    assert.ok(Number.isSafeInteger(timeout) && timeout > 0, `${label}: bounded timeout required`);
    assert.ok(Object.values(this.receipt.tools).some(tool =>
      path.join(this.output, tool.frozen) === executable), `${label}: executable was not frozen`);
    const id = this.receipt.commands.length + 1;
    const stem = `commands/${String(id).padStart(4, '0')}-${label}`;
    const stdin = this.retain(`${stem}.stdin`, input);
    const run = spawnSync(executable, args, {
      cwd: root, input, encoding: null, timeout, killSignal: 'SIGKILL', maxBuffer: 32 * 1024 * 1024,
      env: { PATH: '/nonexistent', LANG: 'C', LC_ALL: 'C', RUST_BACKTRACE: '1' },
    });
    const stdout = this.retain(`${stem}.stdout`, run.stdout ?? Buffer.alloc(0));
    const stderr = this.retain(`${stem}.stderr`, run.stderr ?? Buffer.alloc(0));
    const entry = {
      id, label, executable: this.relative(executable), executable_sha256: fileHash(executable),
      argv: args, cwd: root, stdin: this.relative(stdin), stdout: this.relative(stdout),
      stderr: this.relative(stderr), timeout_ms: timeout, status: run.status,
      signal: run.signal, error: run.error ? String(run.error) : null,
    };
    this.receipt.commands.push(entry);
    return { ...entry, bytes: run.stdout ?? Buffer.alloc(0) };
  }

  framed(sourceFiles) {
    return Buffer.concat(sourceFiles.flatMap(file => {
      const bytes = fs.readFileSync(file);
      this.watch(file);
      return [Buffer.from(`${bytes.length}\n`), bytes];
    }));
  }

  emissionPath(label) {
    safeName(label);
    const directory = path.join(this.output, 'emission', label);
    fs.mkdirSync(path.dirname(directory), { recursive: true });
    assert.ok(!fs.existsSync(directory), `${label}: output directory must be fresh`);
    return directory;
  }

  emitted(label, directory, sources) {
    assert.ok(fs.lstatSync(directory).isDirectory(), `${label}: no plain CLI emission directory`);
    const files = [];
    const visit = folder => {
      for (const entry of fs.readdirSync(folder, { withFileTypes: true })
        .sort((left, right) => left.name.localeCompare(right.name))) {
        const file = path.join(folder, entry.name);
        if (entry.isDirectory()) visit(file);
        else {
          assert.ok(entry.isFile() && fs.lstatSync(file).isFile(),
            `${label}: emission cannot contain a symlink or special file`);
          files.push({ file: this.relative(file), sha256: this.watch(file), bytes: fs.statSync(file).size });
        }
      }
    };
    visit(directory);
    for (let i = 0; i < sources.length; i++) {
      const emitted = path.join(directory, `${i + 1}.noble`);
      assert.ok(fs.existsSync(emitted), `${label}: CLI did not retain source frame ${i + 1}`);
      assert.deepEqual(fs.readFileSync(emitted), fs.readFileSync(sources[i]),
        `${label}: emitted source frame differs from frozen input bytes`);
    }
    return files;
  }

  reports(run, expected) {
    const lines = run.bytes.toString('utf8').split('\n').filter(line => line.length > 0);
    assert.equal(lines.length, expected, `${run.label}: wrong number of source reports`);
    return lines.map((line, index) => {
      const report = JSON.parse(line);
      assert.equal(report.schema, 'noble-core-report/v1',
        `${run.label}/${index}: not a production core report`);
      assert.equal(report.profile, 'Declared-Modules-v1',
        `${run.label}/${index}: not a declared-module execution`);
      return report;
    });
  }

  quiet(report, label) {
    assert.equal(report.guest_requests, 0, `${label}: guest executed during static preparation`);
    assert.equal(report.host_requests, 0, `${label}: host was consulted during static preparation`);
    assert.equal(report.protected_operations, 0, `${label}: protected operation during preparation`);
    assert.equal(report.candidate_prepare_requests, 0, `${label}: candidate ran during preparation`);
    assert.equal(report.ambient_fallback_calls, 0, `${label}: ambient host fallback used`);
    assert.equal(report.acquired_authority, false, `${label}: preparation granted authority`);
  }

  selectedTrace(report, label, { module, version, adapterIdentity, marker, selected }) {
    assert.equal(typeof selected, 'boolean', `${label}: selected-arm assertion must be boolean`);
    assert.ok(typeof module === 'string' && Number.isSafeInteger(version)
      && typeof adapterIdentity === 'string' && typeof marker === 'string',
    `${label}: exact bound callback identity and marker are required`);
    const count = Number(selected);
    assert.ok(Array.isArray(report.request_trace), `${label}: missing actual guest request trace`);
    assert.equal(report.request_trace.length, count, `${label}: wrong callback request count`);
    assert.equal(report.guest_requests, count, `${label}: wrong guest request count`);
    assert.equal(report.host_requests, count, `${label}: wrong host request count`);
    assert.deepEqual(report.effect_requests, selected ? ['test.emit'] : [],
      `${label}: callback effect request disagrees with selected arm`);
    assert.equal(report.effect_requests_unrecorded, 0, `${label}: effect trace truncated`);
    assert.equal(report.request_trace_complete, true, `${label}: request trace incomplete`);
    assert.equal(report.protected_operations, count, `${label}: unauthorized host operation`);
    assert.equal(report.ambient_fallback_calls, 0, `${label}: ambient fallback request`);
    assert.equal(report.acquired_authority, false, `${label}: guest acquired host authority`);
    assert.equal(report.native_trap, null, `${label}: host operation trapped`);
    assert.ok(Array.isArray(report.adapter_invocations), `${label}: no adapter accounting`);
    const invocations = report.adapter_invocations.filter(item =>
      item.adapter_identity === adapterIdentity);
    assert.equal(invocations.length, 1, `${label}: expected one immutable host binding`);
    assert.ok(Number.isSafeInteger(invocations[0].slot) && invocations[0].slot >= 0,
      `${label}: binding has no retained nonnegative slot`);
    assert.equal(invocations[0].count, count, `${label}: callback adapter invocation mismatch`);
    for (const item of report.adapter_invocations) if (item !== invocations[0])
      assert.equal(item.count, 0, `${label}: an unrelated adapter was invoked`);
    if (!selected) return null;
    const [call] = report.request_trace;
    assert.equal(call.module, module, `${label}: wrong request source module`);
    assert.equal(call.version, version, `${label}: wrong request source version`);
    assert.equal(call.adapter_identity, adapterIdentity, `${label}: wrong bound adapter`);
    assert.equal(call.slot, invocations[0].slot, `${label}: wrong bound adapter slot`);
    assert.equal(call.effect, 'test.emit', `${label}: wrong callback effect`);
    assert.equal(call.text, marker, `${label}: wrong selected callback marker`);
    assert.equal(call.decision, 'allow', `${label}: selected callback was denied`);
    return call;
  }

  session(label, cli, binding, sources, optimization) {
    assert.ok(['off', 'on'].includes(optimization), `${label}: unreviewed optimization`);
    this.watch(binding);
    const directory = this.emissionPath(label);
    const args = ['session', '--framed', '--declared-modules', '--bindings',
      binding, '--opt', optimization, '--emit', directory];
    const run = this.command(label, cli, args, this.framed(sources));
    const reports = this.reports(run, sources.length);
    const artifacts = this.emitted(label, directory, sources);
    return { command: run.id, status: run.status, reports, artifacts };
  }

  compile(label, cli, binding, modules, source, inputTypes = []) {
    assert.ok(Array.isArray(modules) && modules.length > 0,
      `${label}: source-bound module dependencies required`);
    assert.ok(Array.isArray(inputTypes) && inputTypes.every(type => typeof type === 'string'),
      `${label}: input types must be exact source spellings`);
    this.watch(binding);
    this.watch(source);
    for (const module of modules) this.watch(module);
    const args = ['compile', source, '--declared-modules', '--bindings', binding,
      ...modules.flatMap(file => ['--module', file]),
      ...inputTypes.flatMap(type => ['--input-type', type])];
    const run = this.command(label, cli, args);
    if (run.status !== 0) {
      const [report] = this.reports(run, 1);
      return { command: run.id, status: run.status, report };
    }
    return { command: run.id, status: run.status,
      compilation_output: { stdout: this.receipt.commands.at(-1).stdout,
        sha256: sha256(run.bytes) } };
  }

  validateWasm(label, wasmTools, artifacts) {
    const modules = artifacts.filter(item => item.file.endsWith('.wasm'));
    assert.ok(modules.length > 0, `${label}: CLI emitted no compiled Wasm`);
    return modules.map((item, index) => {
      const file = path.join(this.output, item.file);
      assert.equal(fileHash(file), item.sha256, `${label}: compiled Wasm changed`);
      const validated = this.command(`${label}-wasm-validate-${index}`,
        wasmTools, ['validate', file]);
      assert.equal(validated.status, 0, `${label}: selected wasm-tools rejected compiled Wasm`);
      const inspected = this.command(`${label}-wasm-print-${index}`,
        wasmTools, ['print', file]);
      assert.equal(inspected.status, 0, `${label}: selected wasm-tools could not inspect compiled Wasm`);
      return { file: item.file, sha256: item.sha256,
        validation_command: validated.id, inspection_command: inspected.id,
        wat_file: this.receipt.commands.at(-1).stdout };
    });
  }

  seal(summary) {
    if (!this.receipt.plan || !this.receipt.source_revision || !this.receipt.revision_inputs
      || !this.receipt.tools.noble || !this.receipt.tools['kernel-peer']
      || !this.receipt.peer_build || this.receipt.peer_build.status !== 0
      || !this.receipt.peer || this.receipt.commands.length === 0
      || this.receipt.cases.length === 0) {
      this.receipt.failures.push({ case_id: 'gate',
        error: 'missing planned workload, source revision, real commands, cases or independent peer result' });
    }
    for (const [file, sha256] of this.watched) {
      try { assert.equal(fileHash(file), sha256, `${file}: changed during acceptance`); }
      catch (error) { this.receipt.integrity_failures.push({ file, error: String(error) }); }
    }
    this.receipt.summary = { ...summary,
      failures: this.receipt.failures.length,
      integrity_failures: this.receipt.integrity_failures.length };
    this.receipt.result = this.receipt.failures.length === 0
      && this.receipt.integrity_failures.length === 0 ? 'passed' : 'failed';
    const report = path.join(this.output, 'report.json');
    fs.writeFileSync(report, JSON.stringify(this.receipt, null, 2) + '\n', { flag: 'wx', mode: 0o400 });
    return { report, result: this.receipt.result, summary: this.receipt.summary };
  }
}
