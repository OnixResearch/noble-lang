// Measures the authoring forms of the issue #79 bounded examples against the existing checked
// stack operations. It never edits a form between runs and never claims one form's numbers for
// another form.
//
// For every `.noble` form it records:
//   * authored source size: bytes, whitespace tokens, and how many tokens are stack-manipulation
//     words (`dup`, `drop`, `swap`, `dip`, `compose`, `run`, `quote`, `pair`, `unpair`, ...),
//     which is the authoring complexity this issue is about;
//   * checking cost: wall time of `noble compile --opt off` over `--runs` runs after a warm-up,
//     the exit status and the generated module `compile` printed (bytes, lines, sha256), which is
//     the generated-code size that this profile has: `--emit` belongs to `editor admit`, so the
//     text Wasm module on stdout is the artifact a reader can compare;
//   * executed shape: `noble live repl` metrics and the resulting stack value, so two forms can
//     be compared on the *same* value rather than on plausibility.
//
// What it does NOT measure: the proposed focus sugar. No sugar exists in the selected source
// profile, so sugar rows are authored-source sizes of the expansion and are labelled model-only
// in the lane README. Timing a form that cannot be compiled would be inventing data.
import { spawnSync } from 'node:child_process';
import { mkdirSync, readdirSync, readFileSync, writeFileSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
const option = (name, fallback) => {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : fallback;
};

const noble = resolve(option('--noble', join(root, 'result/bin/noble')));
const forms = resolve(option('--forms', join(here, 'forms')));
const output = resolve(option('--output', join(here, 'receipts', 'measure.json')));
const runs = Number(option('--runs', '7'));
const optimize = option('--opt', 'off');

if (!existsSync(noble)) {
  console.error(`noble binary not found: ${noble}`);
  process.exit(2);
}

const stackWords = new Set([
  'dup', 'drop', 'swap', 'dip', 'compose', 'run', 'quote',
  'pair', 'unpair', 'inl', 'inr', 'case', 'if', '+', '=', 'let', 'nil', 'cons',
]);

const words = source => source.split(/\s+/).filter(token => token.length > 0);

const median = values => {
  const sorted = [...values].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 1 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
};

const round = value => Math.round(value * 1000) / 1000;

const measure = file => {
  const source = readFileSync(file, 'utf8');
  const tokens = words(source);
  const runOnce = () => {
    const started = process.hrtime.bigint();
    const result = spawnSync(noble, ['compile', file, '--opt', optimize], {
      encoding: 'utf8',
      maxBuffer: 64 * 1024 * 1024,
    });
    const ended = process.hrtime.bigint();
    return {
      milliseconds: Number(ended - started) / 1e6,
      status: result.status,
      stdout: result.stdout ?? '',
      stderr: (result.stderr ?? '').trim(),
    };
  };
  const warm = runOnce();
  const times = [];
  let last = warm;
  for (let run = 0; run < runs; run += 1) {
    last = runOnce();
    times.push(last.milliseconds);
  }
  const module = last.stdout;
  const live = spawnSync(noble, ['live', 'repl'], {
    input: source,
    encoding: 'utf8',
    timeout: 300_000,
    maxBuffer: 64 * 1024 * 1024,
  });
  let executed = null;
  const lines = (live.stdout ?? '').split('\n').filter(line => line.length > 0);
  const lastReport = lines.at(-1);
  if (lastReport) {
    const report = JSON.parse(lastReport);
    executed = {
      status: report.metrics.status,
      stack: report.stack,
      executionSteps: report.metrics.execution_steps,
      quoteInvocations: report.metrics.quote_invocations,
      allocatedTotalBytes: report.metrics.allocated_total_bytes,
      operandPeakBytes: report.metrics.operand_peak_bytes,
      continuationPeakBytes: report.metrics.continuation_peak_bytes,
      reflectionSteps: report.metrics.reflection_steps,
    };
  }
  return {
    form: file.split('/').pop().replace(/\.noble$/, ''),
    path: file.slice(root.length + 1),
    authored: {
      bytes: Buffer.byteLength(source),
      tokens: tokens.length,
      stackManipulationTokens: tokens.filter(token => stackWords.has(token)).length,
      quotations: (source.match(/\[/g) ?? []).length,
    },
    compile: {
      opt: optimize,
      runs,
      status: last.status,
      stderr: last.stderr,
      medianMilliseconds: round(median(times)),
      minMilliseconds: round(Math.min(...times)),
      maxMilliseconds: round(Math.max(...times)),
      warmupMilliseconds: round(warm.milliseconds),
      moduleBytes: Buffer.byteLength(module),
      moduleLines: module.split('\n').length - 1,
      moduleSha256: createHash('sha256').update(module).digest('hex'),
    },
    executed,
  };
};

const cases = readdirSync(forms).filter(name => name.endsWith('.noble')).sort();
const results = cases.map(name => measure(join(forms, name)));
mkdirSync(dirname(output), { recursive: true });
const receipt = {
  schema: 'noble-focus-elaboration-measure/v1',
  noble,
  nobleSha256: createHash('sha256').update(readFileSync(noble)).digest('hex'),
  host: `${process.platform} ${process.arch}`,
  opt: optimize,
  runs,
  formsDirectory: forms.slice(root.length + 1),
  results,
};
writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
for (const result of results) {
  const value = JSON.stringify(result.executed?.stack ?? null);
  console.log(
    `${result.form}\ttokens=${result.authored.tokens}\tstackTokens=${result.authored.stackManipulationTokens}` +
    `\tmedian=${result.compile.medianMilliseconds}ms\tmodule=${result.compile.moduleBytes}B` +
    `\tsteps=${result.executed?.executionSteps}\tstatus=${result.executed?.status}\tstack=${value}`,
  );
}
