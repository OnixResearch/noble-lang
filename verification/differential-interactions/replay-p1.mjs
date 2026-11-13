// Fresh-process replay of the exact retained P1 original and minimized source bytes.
// Usage: SELECTED_NODE verification/differential-interactions/replay-p1.mjs \
//   --witness MUTATION_WITNESS_JSON --output NEW_DIR --target-dir NEW_OR_EXISTING_TARGET
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { userInfo } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { p1Eligible, p1Expected } from './p1-cases.mjs';
import { buildCli, createCliObserver, fileHash } from '../foundation/execution.mjs';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json')));
const leanBin = join(selection.tool_paths.lean.output, 'bin');
const rustBin = join(selection.tool_paths.quality_rust.output, 'bin');
const node = join(selection.tool_paths.node.output, 'bin/node');
const git = realpathSync('/run/current-system/sw/bin/git');
const { homedir: home, username } = userInfo();
const childBase = Object.freeze({ HOME: home, USER: username, LOGNAME: username, LANG: 'C.UTF-8', CI: '1',
  GIT_TERMINAL_PROMPT: '0', PATH: [leanBin, rustBin, dirname(git),
    `/etc/profiles/per-user/${username}/bin`, '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':') });
const args = process.argv.slice(2);
const option = name => { const at = args.indexOf(name); return at < 0 ? undefined : args[at + 1]; };
const witnessPath = option('--witness');
const outputPath = option('--output');
const targetDir = option('--target-dir');
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const requireThat = (condition, message) => { if (!condition) throw Error(message); };
const run = (command, argv, options = {}) => {
  const raw = spawnSync(command, argv, { maxBuffer: 64 * 1024 * 1024, timeout: 3600000,
    env: childBase, ...options, encoding: 'utf8' });
  const result = { ...raw, stdout: raw.stdout ?? '', stderr: raw.stderr ?? '' };
  if (result.error || (result.status !== 0 && !options.allowFailure)) {
    throw Error(`${command} ${argv.join(' ')} failed: ${result.error?.message ?? result.status}\n${result.stderr}\n${result.stdout}`);
  }
  return options.allowFailure ? result : result.stdout.trim();
};
const revive = (value, key = '') => Array.isArray(value) ? value.map(item => revive(item))
  : typeof value === 'object' && value !== null
    ? Object.fromEntries(Object.entries(value).map(([name, item]) => [name, revive(item, name)]))
    : key === 'push' || key === 'quoted' ? BigInt(value) : value;
const mismatchClass = (expected, observed, model) => {
  if (observed.outcome !== 'normal') return `unexpected-${observed.outcome}`;
  if (JSON.stringify(expected.stack) === JSON.stringify(observed.stack)) return null;
  const prefixLength = model.prefix.length;
  if (prefixLength > 0 && JSON.stringify(expected.stack.slice(0, prefixLength)) !==
      JSON.stringify(observed.stack.slice(0, prefixLength))) return 'ordered-prefix-output-mismatch';
  if (expected.stack.length !== observed.stack.length) return 'ordered-output-shape-mismatch';
  return 'ordered-arithmetic-output-mismatch';
};

try {
  requireThat(witnessPath && outputPath && targetDir && args.length === 6,
    'usage: replay-p1.mjs --witness MUTATION_WITNESS_JSON --output NEW_DIR --target-dir TARGET');
  requireThat(realpathSync(process.execPath) === realpathSync(node), `use selected Node: ${node}`);
  mkdirSync(outputPath);
  const witness = JSON.parse(readFileSync(witnessPath, 'utf8'));
  requireThat(witness.outcome === 'killed' && witness.original && witness.minimized,
    'P1 witness is not a retained semantic kill');
  const { cli, env } = buildCli({ root, out: resolve(outputPath), selection, childBase, run, targetDir });
  const observe = createCliObserver({ root, out: resolve(outputPath), cli, env, run });
  const variants = [witness.original, witness.minimized].map((entry, index) => {
    const model = revive(entry.model);
    const source = entry.source;
    requireThat(digest(Buffer.from(source)) === entry.source_sha256, 'retained P1 source bytes changed');
    requireThat(model.interaction?.cell === witness.target_cell && p1Eligible(model),
      'retained P1 source lost its selected interaction');
    const expected = p1Expected(model);
    requireThat(JSON.stringify(expected) === JSON.stringify(entry.expected),
      'fresh P1 expectation differs from the retained model-bound expectation');
    const mode = entry.mode;
    const observed = observe({ ...model, index: `replay-${index}`, source }, mode);
    const failure = mismatchClass(expected, observed, model);
    const positiveSteps = Number.isInteger(observed.metrics?.execution_steps) && observed.metrics.execution_steps > 0;
    requireThat(observed.outcome === 'normal' && positiveSteps && failure === witness.failure_identity,
      `retained P1 ${index === 0 ? 'original' : 'minimized'} failure did not reproduce: ${failure}`);
    return { variant: index === 0 ? 'original' : 'minimized', mode, source_sha256: entry.source_sha256,
      source_size_bytes: Buffer.byteLength(source), expected, observed: { outcome: observed.outcome,
        stack: observed.stack, execution_steps: observed.metrics.execution_steps,
        wasm_sha256: observed.wasm_sha256 }, failure_identity: failure, positive_execution_steps: positiveSteps,
      fresh_process: true };
  });
  const result = { schema: 'noble-differential-interactions-p1-replay/v1', mutation_id: witness.mutation_id,
    target_cell: witness.target_cell, failure_identity: witness.failure_identity,
    source_commit: run(git, ['rev-parse', 'HEAD'], { cwd: root }),
    working_tree: run(git, ['status', '--porcelain=v1', '--untracked-files=all'], { cwd: root }),
    candidate_cli_sha256: fileHash(cli), selected_node: node, selected_rust: selection.tool_paths.quality_rust,
    replayed: variants, corpus_regenerated: false, fresh_process_replay: true };
  writeFileSync(join(resolve(outputPath), 'replay-p1.json'), `${JSON.stringify(result, null, 2)}\n`);
  process.stdout.write(`${JSON.stringify({ ok: true, receipt: join(resolve(outputPath), 'replay-p1.json'),
    mutation_id: witness.mutation_id, target_cell: witness.target_cell,
    failure_identity: witness.failure_identity, replayed: variants.map(item => item.variant) }, null, 2)}\n`);
} catch (error) {
  console.error(`[issue125-p1-replay] FAIL ${error.message}`);
  process.exitCode = 1;
}
