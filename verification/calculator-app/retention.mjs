import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

export const DEFAULT_RETAINED = 65_536;
export const OPTED_RETAINED = 262144;
export const MAX_SOURCE = 65_536;
export const MAX_COMMAND_BYTES = 262_144;
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');

export function modulePaths(languageRoot, numericDir, here) {
  return [
    path.join(languageRoot, 'crates/noble-contracts/src/fixtures/result.noble'),
    path.join(languageRoot, 'crates/noble-contracts/src/fixtures/calc-errors.noble'),
    // This ordering matches MA1's declared full-library registration graph.
    ...['bitstep', 'bitdecode', 'bitfull', 'bittrim', 'bitadd', 'bitcompare',
      'bitborrow', 'bitsub', 'bitmul', 'bitdiv', 'bitpair', 'bitsum',
      'bitorder', 'bitsigned', 'bitaddcarry', 'bitaddsigned', 'bitrelation',
      'bitquotient', 'numeric_budget_limits', 'numeric_budget_preflight_depth',
      'numeric_budget_preflight_late', 'numeric_budget_preflight',
      'numeric_budget', 'bigint', 'rat', 'decimal']
      .map(name => path.join(numericDir, `${name}.noble`)),
    ...['calc_chars', 'calc_token', 'calc_diagnostic', 'calc_code',
      'calc_lex', 'calc_name', 'calc_parse_state', 'calc_parse', 'calc_env',
      'calc_bind', 'calc_eval', 'calc_session'].map(name => path.join(here, `${name}.noble`)),
  ];
}

// The only accepted opt-in authority is MA1's full, source-bound compiled
// guest verification, its independent selected CLI build and its raw guest
// transcript. Check their exact committed recipes/modules before even probing
// the CLI flag; an arbitrary argv claim is not a compiler receipt.
export function selectedCompilerReceipt(binary, receiptPath, numericDir, languageRoot, sourceAudit) {
  if (!receiptPath) throw Error('committed MA1 compiled CLI/recipe receipt is required');
  assert.equal(sourceAudit?.modules?.length, 40,
    'selected compiler requires the exact 40-module calculator source audit');
  const numericRoot = fs.realpathSync(path.resolve(numericDir, '../..'));
  assert.equal(fs.realpathSync(languageRoot), numericRoot,
    'selected language root differs from committed numeric source root');
  assert.equal(numericRoot, fs.realpathSync(path.resolve(import.meta.dirname, '../..')),
    'selected numeric source root differs from committed calculator application root');
  const executable = fs.realpathSync(binary);
  const verificationBytes = fs.readFileSync(receiptPath);
  const verification = JSON.parse(verificationBytes);
  const buildPath = path.join(path.dirname(path.dirname(path.dirname(executable))), 'build.json');
  const buildBytes = fs.readFileSync(buildPath);
  const build = JSON.parse(buildBytes);
  const guestBytes = fs.readFileSync(path.join(path.dirname(receiptPath), 'receipt.json'));
  const guest = JSON.parse(guestBytes);
  const binarySha256 = sha256(fs.readFileSync(executable));
  assert.equal(verification.schema, 'noble-ma1-exact-numeric-gate/v1');
  assert.equal(verification.result, 'passed', 'MA1 full compiled numeric gate is required');
  assert.equal(verification.scope, 'full-library');
  assert.equal(verification.vendor_recipe?.exact_nine_lock_recipe, true,
    'MA1 guest success without the exact committed nine-lock vendor recipe is not acceptance');
  assert.equal(fs.realpathSync(verification.source_root), numericRoot);
  assert.equal(verification.binary_sha256, binarySha256);
  assert.equal(verification.source_bound_cli_build_sha256, sha256(buildBytes));
  assert.equal(verification.guest_receipt_sha256, sha256(guestBytes));
  assert.equal(build.schema, 'noble-result-cli-build/v1');
  assert.equal(build.result, 'built');
  assert.deepEqual(build.preSources, build.sources);
  assert.deepEqual(build.postSources, build.sources);
  assert.equal(build.sourceRevision, build.postSourceRevision);
  assert.match(build.sourceRevision, /^[0-9a-f]{40}$/);
  assert.equal(fs.realpathSync(build.binary.path), executable);
  assert.equal(build.binary.sha256, binarySha256);
  assert.deepEqual(build.tools.vendor.recipe, verification.vendor_recipe,
    'selected CLI build and MA1 guest disagree about the vendor recipe');
  const git = '/run/current-system/sw/bin/git';
  const head = spawnSync(git, ['rev-parse', 'HEAD'], {
    cwd: numericRoot, encoding: 'utf8', timeout: 30_000,
  });
  assert.equal(head.status, 0, 'selected numeric source HEAD is unavailable');
  assert.equal(head.stdout.trim(), build.sourceRevision,
    'compiled selected numeric source is not the committed HEAD');
  const selected = JSON.parse(fs.readFileSync(path.join(numericRoot, 'policy/tool-selection.json')));
  const rustBin = path.join(selected.tool_paths.quality_rust.output, 'bin');
  for (const tool of ['cargo', 'rustc']) {
    const expected = path.join(rustBin, tool);
    assert.equal(fs.realpathSync(build.tools[tool].path), fs.realpathSync(expected));
    assert.equal(build.tools[tool].sha256, sha256(fs.readFileSync(expected)));
  }
  const selectedPaths = modulePaths(numericRoot, numericDir, path.join(numericRoot, 'verification/calculator-app'))
    .slice(0, 28);
  const moduleName = file => path.basename(file, '.noble').replace('-', '_');
  const expected = new Map(selectedPaths.map(file => [moduleName(file), file]));
  assert.equal(expected.size, 28, 'selected numeric closure is not 28 modules');
  assert.equal(verification.module_sources?.length, expected.size);
  assert.equal(verification.retained_source_accounting?.limit_bytes, OPTED_RETAINED);
  assert.equal(verification.retained_source_accounting?.per_submission_limit_bytes, MAX_SOURCE);
  assert.equal(verification.retained_source_accounting?.outside_definition_history_bytes, 0);
  let numericBytes = 0;
  const observed = new Set();
  const committed = file => {
    const relative = path.relative(numericRoot, file);
    assert.ok(relative && !relative.startsWith('..') && !path.isAbsolute(relative),
      `source escaped selected root: ${file}`);
    const original = fs.readFileSync(file);
    assert.equal(build.sources[relative]?.sha256, sha256(original),
      `selected CLI build omitted or changed ${relative}`);
    const blob = spawnSync(git, ['show', `${build.sourceRevision}:${relative}`], {
      cwd: numericRoot, timeout: 30_000, maxBuffer: MAX_SOURCE + 16_384,
    });
    assert.equal(blob.status, 0, `selected source is not a committed blob: ${relative}`);
    assert.equal(sha256(blob.stdout), sha256(original),
      `committed source differs from selected CLI build: ${relative}`);
    return { bytes: original.length, sha256: sha256(original) };
  };
  for (const item of verification.module_sources) {
    const file = expected.get(item.name);
    assert.ok(file && !observed.has(item.name), `numeric module absent or duplicated: ${item.name}`);
    observed.add(item.name);
    const source = committed(file);
    assert.equal(item.bytes, source.bytes);
    assert.equal(item.sha256, source.sha256);
    const applicationLink = sourceAudit.modules.find(module => module.name === item.name);
    assert.ok(applicationLink, `calculator source audit omitted numeric module: ${item.name}`);
    assert.equal(fs.realpathSync(applicationLink.file), fs.realpathSync(file));
    assert.equal(applicationLink?.bytes, source.bytes);
    assert.equal(applicationLink?.source_sha256, source.sha256,
      `calculator linked source differs from committed numeric guest: ${item.name}`);
    numericBytes += source.bytes;
    const registered = guest.frames?.find(frame => frame.name === `register-${item.name}`);
    assert.equal(registered?.source_sha256, source.sha256,
      `numeric guest registered different original bytes: ${item.name}`);
    assert.equal(registered?.outcome, 'linked');
  }
  assert.equal(numericBytes,
    verification.retained_source_accounting.total_original_source_bytes);
  for (const relative of ['verification/calculator-exact-library/gate.py',
    'verification/calculator-exact-library/guest_session.py',
    'verification/calculator-exact-library/oracle.py',
    'verification/result-library/collect-build.mjs', 'specs/CALCULATOR.md',
    'verification/m6/pins.json', 'verification/selected-vendor.mjs',
    'nix/reviewed-vendor.nix',
    'policy/tool-selection.json', 'policy/tool-selection.ncl',
    'policy/tool-selection-files.ncl', 'policy/tool-selection-tools.ncl']) {
    const source = committed(path.join(numericRoot, relative));
    if (relative.endsWith('/gate.py')) assert.equal(source.sha256, verification.driver_sha256);
    if (relative.endsWith('/oracle.py')) assert.equal(source.sha256, verification.oracle_sha256);
  }
  assert.equal(guest.schema, 'noble-ma1-numeric-guest-local/v1');
  assert.equal(fs.realpathSync(guest.source_root), numericRoot);
  assert.equal(fs.realpathSync(guest.binary), executable);
  assert.equal(guest.binary_sha256, binarySha256);
  assert.equal(guest.binary_sha256_after, binarySha256);
  assert.equal(guest.retained_source_bytes, OPTED_RETAINED);
  assert.equal(guest.exit_code, 0);
  assert.equal(guest.unmatched_stdout_bytes, 0);
  assert.deepEqual(guest.argv?.slice(0, 8), [
    executable, 'session', '--framed', '--declared-modules', '--text-byte-cursor',
    '--signed-self-recursion', '--retained-source-bytes', String(OPTED_RETAINED),
  ]);
  return { selected_binary_sha256: binarySha256,
    selected_compiled_receipt_sha256: sha256(verificationBytes),
    selected_cli_build_sha256: sha256(buildBytes),
    selected_numeric_guest_sha256: sha256(guestBytes),
    committed_numeric_source_revision: build.sourceRevision,
    numeric_original_source_bytes: numericBytes,
    numeric_module_sources: verification.module_sources };
}

// Static initial audit. The separate runtime ledger tracks module commits and
// checks every expression; it also models outside `def` commits and alias
// replay for any future ingress. Both retain original source plus four bytes,
// as checked in the donor's source preparation and rebind paths.
export function auditSources(paths, retainedBytes = DEFAULT_RETAINED, outsideDefinitions = []) {
  if (![DEFAULT_RETAINED, OPTED_RETAINED].includes(retainedBytes))
    throw Error('unsupported calculator retained-source bound');
  let moduleBytes = 0, outsideHistoryBytes = 0;
  const declared = new Set();
  const modules = paths.map(file => {
    const source = fs.readFileSync(file);
    if (source.length > MAX_SOURCE) throw Error(`source exceeds per-source bound: ${file}`);
    const text = source.toString('utf8');
    const declaration = text.match(/^\s*module\s+([A-Za-z_]\w*)@1\s*\[/);
    const name = declaration?.[1];
    if (!name || name !== path.basename(file, '.noble').replace('-', '_') ||
        declared.has(name))
      throw Error(`module declaration identity differs from source path: ${file}`);
    const executable = text.replace(/"(?:\\.|[^"\\])*"|#[^\n]*/g, match => ' '.repeat(match.length));
    const dependsOn = [...new Set([...executable.matchAll(/\b([A-Za-z_]\w*)@1\./g)]
      .map(match => match[1]))].sort();
    for (const dependency of dependsOn)
      if (dependency !== name && !declared.has(dependency))
        throw Error(`module dependency unavailable before ${name}: ${dependency}`);
    declared.add(name);
    moduleBytes += source.length;
    return { file, name, depends_on: dependsOn,
      source, bytes: source.length, source_sha256: sha256(source) };
  });
  for (const source of outsideDefinitions) {
    if (!Buffer.isBuffer(source) || source.length > MAX_SOURCE)
      throw Error('invalid outside definition source');
    outsideHistoryBytes += 4 + source.length;
  }
  const retainedTotalBytes = moduleBytes + outsideHistoryBytes;
  if (!Number.isSafeInteger(retainedTotalBytes) || retainedTotalBytes > retainedBytes)
    throw Error(`retained source preflight refused: ${retainedTotalBytes} > ${retainedBytes}`);
  return { default_retained_source_bytes: DEFAULT_RETAINED,
    selected_retained_source_bytes: retainedBytes, per_source_bytes: MAX_SOURCE,
    module_bytes: moduleBytes, outside_history_bytes: outsideHistoryBytes,
    outside_history_prefix_bytes_per_definition: 4, retained_total_bytes: retainedTotalBytes,
    modules };
}

// Mirrors ModuleSession's retained namespace: module source bytes plus every
// committed outside `def` source and every transitive alias-rebind replay,
// each framed by a four-byte history header. Expression submissions (including
// calculator var/function/import operations) do not enter source.history.
// Plans are checked before submission and applied only after checked commit.
export class RetainedNamespace {
  #bound;
  #moduleBytes = 0;
  #historyBytes = 0;
  #definitions = [];
  #pending = null;

  constructor(bound) {
    if (![DEFAULT_RETAINED, OPTED_RETAINED].includes(bound))
      throw Error('unsupported retained namespace bound');
    this.#bound = bound;
  }

  get moduleBytes() { return this.#moduleBytes; }
  get outsideHistoryBytes() { return this.#historyBytes; }
  get retainedTotalBytes() { return this.#moduleBytes + this.#historyBytes; }
  get definitionCount() { return this.#definitions.length; }

  #source(source) {
    if (!Buffer.isBuffer(source) || source.length > MAX_SOURCE)
      throw Error('retained namespace source exceeds per-source bound');
  }

  #plan(kind, sources) {
    if (this.#pending) throw Error('retained namespace has an uncommitted plan');
    let extra = 0;
    for (const source of sources) {
      this.#source(source);
      extra += source.length + (kind === 'module' ? 0 : 4);
    }
    if (!Number.isSafeInteger(extra) || !Number.isSafeInteger(this.retainedTotalBytes + extra) ||
        this.retainedTotalBytes + extra > this.#bound)
      throw Error(`retained source preflight refused: ${this.retainedTotalBytes + extra} > ${this.#bound}`);
    const plan = Object.freeze({ kind,
      sources: Object.freeze(sources.map(source => Buffer.from(source))) });
    this.#pending = plan;
    return plan;
  }

  planModule(source) { return this.#plan('module', [source]); }
  planDefinition(source) { return this.#plan('definition', [source]); }
  planReplay(indices) {
    if (!Array.isArray(indices) || indices.length > this.#definitions.length ||
        new Set(indices).size !== indices.length || indices.some(index =>
      !Number.isSafeInteger(index) || index < 0 || index >= this.#definitions.length))
      throw Error('rebind references an unknown retained definition');
    return this.#plan('replay', indices.map(index => this.#definitions[index]));
  }

  checkExpression(source) {
    if (this.#pending) throw Error('retained namespace has an uncommitted plan');
    this.#source(source);
    if (this.retainedTotalBytes > this.#bound)
      throw Error('retained source preflight refused before expression');
  }

  commit(plan) {
    if (plan !== this.#pending) throw Error('retained namespace commit lacks checked plan');
    this.#pending = null;
    if (plan.kind === 'module') this.#moduleBytes += plan.sources[0].length;
    else for (const source of plan.sources) {
      this.#historyBytes += 4 + source.length;
      this.#definitions.push(Buffer.from(source));
    }
  }

  discard(plan) {
    if (plan !== this.#pending) throw Error('retained namespace discard lacks checked plan');
    this.#pending = null;
  }
}

function frame(source) {
  return Buffer.concat([Buffer.from(`${source.length}\n`), source]);
}

// This test executes only when an explicitly requested opt-in reaches a
// compiled selected binary. It cannot turn a refused/unknown CLI flag into a
// successful calculator run. All probes are source-checked declared modules.
export function verifyRetainedSourceFlag(binary, cwd, env) {
  const module = (name, padding) => Buffer.from(
    `module ${name}@1 [ signature value forall<S:stack> [ S -- S I64 ! pure ] export value def value [ 1 ] ]\n#${'x'.repeat(padding)}`);
  const first = module('retention_probe_a', 33_000);
  const second = module('retention_probe_b', 33_000);
  const oversized = module('retention_probe_oversized', MAX_SOURCE);
  assert.ok(first.length <= MAX_SOURCE && second.length <= MAX_SOURCE &&
    first.length + second.length > DEFAULT_RETAINED &&
    first.length + second.length <= OPTED_RETAINED && oversized.length > MAX_SOURCE);
  const base = ['session', '--framed', '--declared-modules',
    '--text-byte-cursor', '--signed-self-recursion'];
  function run(sources, opted) {
    const argv = [...base, ...(opted ? ['--retained-source-bytes', String(OPTED_RETAINED)] : [])];
    const observed = spawnSync(binary, argv, {
      cwd, env, input: Buffer.concat(sources.map(frame)),
      timeout: 60_000, maxBuffer: 8 * 1024 * 1024,
    });
    if (observed.error || observed.signal) throw Error(`retained-source CLI probe stopped: ${observed.error ?? observed.signal}`);
    const lines = observed.stdout.toString('utf8').trim().split('\n').filter(Boolean);
    if (lines.length !== sources.length) throw Error(`retained-source CLI probe reports ${lines.length}/${sources.length}`);
    const reports = lines.map(line => JSON.parse(line));
    for (let i = 0; i < reports.length; i++) {
      if (reports[i].host_requests !== 0 || reports[i].guest_requests !== 0 ||
          reports[i].acquired_authority !== false || reports[i].outcome === 'trap')
        throw Error('retained-source CLI probe crossed a guest authority/runtime boundary');
      if (reports[i].outcome === 'linked' &&
          reports[i].module?.source_sha256 !== sha256(sources[i]))
        throw Error('retained-source CLI probe lost original source byte identity');
    }
    return { status: observed.status, reports };
  }
  const old = run([first, second], false);
  const opted = run([first, second], true);
  const individual = run([oversized], true);
  if (old.reports[0].outcome !== 'linked' || old.reports[1].outcome === 'linked' ||
      !/retained.*byte limit exceeded/.test(JSON.stringify(old.reports[1].diagnostic)) ||
      opted.status !== 0 || opted.reports.some(report => report.outcome !== 'linked') ||
      individual.reports[0].outcome === 'linked' ||
      !/source byte limit exceeded/.test(JSON.stringify(individual.reports[0].diagnostic)))
    throw Error('selected compiler lacks tested retained-source opt-in/default/per-source behavior');
  return { default_refused_above_bytes: DEFAULT_RETAINED,
    opted_linked_with_bytes: first.length + second.length,
    per_source_refused_above_bytes: MAX_SOURCE,
    default_probe_source_sha256: [sha256(first), sha256(second)],
    oversized_probe_source_sha256: sha256(oversized) };
}
