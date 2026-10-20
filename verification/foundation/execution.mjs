import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { CoreEngine } from '../../crates/noble-cli/src/core/runtime/host.mjs';
import { selectedVendor } from '../selected-vendor.mjs';

export const digest = bytes => createHash('sha256').update(bytes).digest('hex');
export const fileHash = path => digest(readFileSync(path));
const json = value => JSON.stringify(value, null, 2) + '\n';
const requireThat = (condition, message) => { if (!condition) throw Error(message); };

// Both differential lanes build the checkout, without ambient Cargo wrappers,
// linker flags or unreviewed registry inputs. The output directory is private.
export function buildCli({ root, out, selection, childBase, run }) {
  const vendor = selectedVendor(selection.component_sync);
  const cargoHome = join(out, 'cargo-home');
  mkdirSync(cargoHome);
  writeFileSync(join(cargoHome, 'config.toml'),
    '[source.crates-io]\nreplace-with = "reviewed-vendor"\n[source.reviewed-vendor]\n' +
    `directory = ${JSON.stringify(vendor.directory)}\n[net]\noffline = true\n`);
  const env = { ...childBase, TMPDIR: out, CARGO_HOME: cargoHome,
    PATH: `${selection.component_sync.linker_bin}:${childBase.PATH}`,
    RUSTC_WRAPPER: '', RUSTC_WORKSPACE_WRAPPER: '', CARGO_BUILD_RUSTC_WRAPPER: '',
    RUSTFLAGS: '', CARGO_ENCODED_RUSTFLAGS: '', CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS: '',
    CARGO_TARGET_DIR: join(root, 'target') };
  const cargo = join(selection.tool_paths.quality_rust.output, 'bin/cargo');
  const built = run(cargo, ['build', '--locked', '--offline', '-j', '4', '-p', 'noble-cli'],
    { cwd: root, env, allowFailure: true });
  writeFileSync(join(out, 'cargo-build.stdout'), built.stdout);
  writeFileSync(join(out, 'cargo-build.stderr'), built.stderr);
  requireThat(built.status === 0, `CLI build failed: ${built.stderr}`);
  return { cli: join(root, 'target/debug/noble'), env, vendor };
}

export function observation(report) {
  return { outcome: report.outcome, stage: report.stage, diagnostic: report.diagnostic ?? null,
    guest_requests: report.guest_requests, protected_operations: report.protected_operations,
    trace: report.request_trace, metrics: report.metrics, report,
    ...(report.outcome === 'normal' ? { wasm_sha256: report.module.wasm_sha256,
      stack: report.stack.map(v => [v.type, String(v.value)]) } : {}) };
}
export function matchesScalar(expected, observed, decode = x => x) {
  return observed.outcome === 'normal' && observed.guest_requests === 0 &&
    observed.protected_operations === 0 && Array.isArray(observed.trace) &&
    JSON.stringify(observed.trace) === JSON.stringify(expected.trace) && Array.isArray(observed.stack) &&
    JSON.stringify(observed.stack.map(decode)) === JSON.stringify(expected.stack);
}
function checkBytes(report, source, wasm) {
  requireThat(report.module.source_sha256 === digest(Buffer.from(source)) &&
    report.module.wasm_sha256 === fileHash(wasm), 'emitted/loaded bytes do not bind the source report');
}

// Original arithmetic comparator route: retain the CLI's real run command.
export function createCliObserver({ root, out, cli, env, run }) {
  let serial = 0;
  return (c, mode) => {
    const tag = `${c.index}-${mode}-${++serial}`;
    const directory = join(out, `case-${tag}`), file = join(out, `case-${tag}.noble`);
    writeFileSync(file, c.source);
    const result = run(cli, ['run', file, '--opt', mode, '--emit', directory], { cwd: root, env, allowFailure: true });
    writeFileSync(join(out, `case-${tag}.stdout`), result.stdout);
    writeFileSync(join(out, `case-${tag}.stderr`), result.stderr);
    const report = JSON.parse(result.stdout);
    if (report.outcome === 'normal') {
      requireThat(report.module.optimized === (mode === 'on'), 'optimization flag mismatch');
      checkBytes(report, c.source, join(directory, 'engine', mode === 'on' ? 'module-1-optimized.wasm' : 'module-1.wasm'));
    }
    return observation(report);
  };
}

// EMI first rechecks the whole source through the real frontend/kernel. Only
// eligible, admitted subjects reach the same production managed-Wasm loader.
// Explicit engine limits avoid relying on an implicit runtime default.
export function createEmiExecutor({ root, out, cli, env, run, runtimeConfig, abi, limits }) {
  const admissions = join(out, 'subjects');
  mkdirSync(admissions);
  let serial = 0;
  const admit = (source, label) => {
    requireThat(/^[A-Za-z0-9_-]+$/.test(label), 'invalid subject label');
    const directory = join(admissions, label);
    mkdirSync(directory);
    const file = join(directory, 'source.noble');
    writeFileSync(file, source);
    const argv = ['compile', file];
    const result = run(cli, argv, { cwd: root, env, allowFailure: true });
    writeFileSync(join(directory, 'compile.stdout'), result.stdout);
    writeFileSync(join(directory, 'compile.stderr'), result.stderr);
    const record = { label, directory, source, source_sha256: digest(Buffer.from(source)),
      admitted: result.status === 0, exit: result.status, argv,
      stdout_sha256: digest(result.stdout), stderr_sha256: digest(result.stderr) };
    writeFileSync(join(directory, 'admission.json'), json(record));
    return { ...record, wat: result.status === 0 ? result.stdout : null };
  };
  const execute = (admission, mode, { mutateWat = null } = {}) => {
    requireThat(admission.admitted, 'refused subject cannot execute');
    const directory = join(admission.directory, `${mode}-${++serial}`);
    mkdirSync(directory);
    const engine = new CoreEngine(runtimeConfig, abi, { optimized: mode === 'on', artifacts: directory });
    const wat = mutateWat ? mutateWat(admission.wat) : admission.wat;
    engine.prepare(Buffer.from(wat), Buffer.from(admission.source));
    const report = engine.execute({ limits });
    writeFileSync(join(directory, 'observation.json'), json(report));
    const wasm = join(directory, 'engine', mode === 'on' ? 'module-1-optimized.wasm' : 'module-1.wasm');
    checkBytes(report, admission.source, wasm);
    requireThat(report.module.optimized === (mode === 'on'), 'optimization flag mismatch');
    return { ...observation(report), directory, wat_sha256: digest(wat), wasm_path: wasm,
      limits: { ...limits }, fault_injected: mutateWat !== null };
  };
  return { admit, execute };
}
