// Measures the authoring forms of the issue #79 bounded examples against the existing checked
// stack operations. It never edits a form between runs and never claims one form's numbers for
// another form.
//
// For every `.noble` form it records:
//   * authored source size: bytes, whitespace tokens, how many tokens are stack-manipulation
//     words (`dup`, `drop`, `swap`, `dip`, `compose`, `run`, `quote`, `pair`, `unpair`, ...),
//     which is the authoring complexity this issue is about;
//   * checking cost: wall time of `noble compile --opt off` over `--runs` runs after a warm-up,
//     the exit status and the generated module `compile` printed (bytes, lines, sha256), which is
//     the generated-code size that this profile has: `--emit` belongs to `editor admit`, so the
//     text Wasm module on stdout is the artifact a reader can compare;
//   * executed shape: `noble live repl` metrics, the ordered typed stack the CLI reports, the
//     stack height the CLI reports (`metrics.stack_length`, cross-checked against the number of
//     reported cells) and the top-of-stack value, so two forms can be compared on the *same*
//     value rather than on plausibility;
//   * what the profile does *not* expose, recorded as such instead of guessed: resource
//     obligations have no must-use word in this source profile, so there is nothing to observe;
//     effect traces are reported by the CLI as `guest_requests`/`request_trace`, which is what
//     the receipt records (empty for every pure form here).
//
// It also costs the proposed focus sugar by the same token rule, reading the snippets out of
// `forms/proposed-sugar.md` (fenced `noble` blocks carrying `role=sugar`/`role=expansion`
// markers). Sugar rows are authored sizes only and are labelled `"compilable": false`: no parser,
// elaborator or checker admits them, so timing one would be inventing data. Every `role=expansion`
// block is checked against the compiled form it restates (`matchesForm`), so the document cannot
// drift from the form files.
//
// Finally it binds the receipt to the source revision it was produced from: the commit, and the
// working-tree state. A dirty tree is refused unless `--allow-dirty` is passed, in which case the
// dirty paths are recorded and the receipt says it is not clean-tree evidence.
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
const proposal = resolve(option('--proposal', join(here, 'forms', 'proposed-sugar.md')));
const output = resolve(option('--output', join(here, 'receipts', 'measure.json')));
const runs = Number(option('--runs', '7'));
const optimize = option('--opt', 'off');
const git = option('--git', 'git');
const allowDirty = process.argv.includes('--allow-dirty');
// Two cells pushed in order. The report must list the implied cell first: that is the only
// mechanical way to state which end of the reported array is the top of the stack.
const orderProbe = option('--stack-order-probe', '1 5');

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

const authored = source => {
  const tokens = words(source);
  return {
    bytes: Buffer.byteLength(source),
    tokens: tokens.length,
    // The sugar's grouping parentheses are authored syntax, but in these snippets they are glued
    // to the adjacent word (`(+`, `1)`), so the headline whitespace count already ignores them as
    // words. This second column strips the characters before tokenizing, so a reader can see that
    // grouping costs no extra tokens at these sizes instead of having to trust the snippet layout.
    tokensExcludingParentheses: words(source.replace(/[()]/g, ' ')).length,
    stackManipulationTokens: tokens.filter(token => stackWords.has(token)).length,
    quotations: (source.match(/\[/g) ?? []).length,
  };
};

const gitOutput = args => {
  const result = spawnSync(git, ['-C', root, ...args], { encoding: 'utf8' });
  if (result.status !== 0) return null;
  return result.stdout.replace(/\n+$/, '');
};

const commit = gitOutput(['rev-parse', 'HEAD']);
const status = gitOutput(['status', '--porcelain', '--untracked-files=normal']);
if (commit === null || status === null) {
  console.error(`cannot bind the receipt to a source revision: ${git} failed in ${root}`);
  process.exit(2);
}
const dirtyPaths = status.split('\n').filter(path => path.length > 0);
if (dirtyPaths.length > 0 && !allowDirty) {
  console.error(
    `refusing to produce a receipt from a dirty tree (${dirtyPaths.length} path(s)); ` +
    'commit first, or pass --allow-dirty and label the receipt as non-clean-tree evidence',
  );
  for (const path of dirtyPaths.slice(0, 20)) console.error(`  ${path}`);
  process.exit(3);
}
const source = {
  commit,
  shortCommit: commit.slice(0, 12),
  tree: dirtyPaths.length === 0 ? 'clean' : 'dirty',
  dirtyPaths,
  cleanTreeEvidence: dirtyPaths.length === 0,
};

const compileOnce = file => {
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

const repl = sourceText => {
  const live = spawnSync(noble, ['live', 'repl'], {
    input: sourceText,
    encoding: 'utf8',
    timeout: 300_000,
    maxBuffer: 64 * 1024 * 1024,
  });
  const lines = (live.stdout ?? '').split('\n').filter(line => line.length > 0);
  const last = lines.at(-1);
  if (!last) {
    return { report: null, diagnostic: (live.stderr ?? '').trim() || 'no report on stdout' };
  }
  try {
    return { report: JSON.parse(last), diagnostic: null };
  } catch (error) {
    return { report: null, diagnostic: `unparsable report: ${error.message}` };
  }
};

const bounded = value => (Number.isInteger(value) ? value : null);

const observe = report => {
  if (report === null) return null;
  const metrics = report.metrics ?? {};
  const stack = Array.isArray(report.stack) ? report.stack : [];
  const heightMetric = bounded(metrics.stack_length);
  const top = stack.at(-1) ?? null;
  return {
    status: report.status ?? metrics.status ?? null,
    outcome: report.outcome ?? null,
    // What the CLI exposes about the executed shape, recorded whole and unmodified.
    stack,
    stackOrder: stack.map(cell => cell.type),
    stackCells: stack.length,
    stackLengthMetric: heightMetric,
    stackHeightConsistent: heightMetric === null ? null : heightMetric === stack.length,
    // `readStack` enumerates slot 0 first, so the last reported cell is the top of the stack;
    // `stackOrderProbe` below is the mechanical check of that direction on this build.
    topOfStack: top,
    value: top,
    guestRequests: report.guest_requests ?? null,
    requestTrace: Array.isArray(report.request_trace) ? report.request_trace : null,
    effectRequests: Array.isArray(report.effect_requests) ? report.effect_requests : null,
    quotaReason: report.quota_reason ?? null,
    executionSteps: metrics.execution_steps ?? null,
    quoteInvocations: metrics.quote_invocations ?? null,
    reflectionSteps: metrics.reflection_steps ?? null,
    allocatedTotalBytes: metrics.allocated_total_bytes ?? null,
    operandPeakBytes: metrics.operand_peak_bytes ?? null,
    continuationPeakBytes: metrics.continuation_peak_bytes ?? null,
  };
};

// The CLI's report has no field for resource obligations: this profile has no must-use resource
// word, so a form here cannot own, transition or close one. Recorded once per form so no reader
// mistakes silence for a pass.
const resourceObligations = {
  observable: false,
  reason:
    'the selected source profile has no must-use resource word, so this program owns no ' +
    'obligation to observe; the live report exposes values, stack cells and metrics only. ' +
    'The ownership axis is the #59 fragment (proofs/foundation/NobleFoundation/Ownership.lean)',
};

const measure = file => {
  const sourceText = readFileSync(file, 'utf8');
  const warm = compileOnce(file);
  const times = [];
  let last = warm;
  for (let run = 0; run < runs; run += 1) {
    last = compileOnce(file);
    times.push(last.milliseconds);
  }
  const module = last.stdout;
  const live = repl(sourceText);
  const observed = observe(live.report);
  const effects = {
    observable: observed !== null && observed.guestRequests !== null,
    reason: observed !== null && observed.requestTrace !== null
      ? 'reported by noble live repl as guest_requests/request_trace'
      : 'the live report did not carry guest_requests/request_trace on this run',
    guestRequests: observed?.guestRequests ?? null,
    requestTrace: observed?.requestTrace ?? null,
  };
  return {
    form: file.split('/').pop().replace(/\.noble$/, ''),
    path: file.slice(root.length + 1),
    authored: authored(sourceText),
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
    executed: observed,
    liveDiagnostic: live.diagnostic,
    effectTrace: effects,
    resourceObligations,
  };
};

const normalize = text => text.trim().split(/\s+/).join(' ');

// Proposed sugar and the expansion snippets it is compared against. The markers are ordinary
// fenced-code info strings so the document still renders as Markdown:
//   ```noble role=sugar name=nested-product expands-to=nested-product.dip \
//     compare=nested-product.explicit,nested-product.dip,nested-product.library
//   ```noble role=expansion name=nested-product.dip form=nested-product.dip
const proposalEntries = () => {
  const lines = readFileSync(proposal, 'utf8').split('\n');
  const entries = [];
  for (let index = 0; index < lines.length; index += 1) {
    const fence = /^```(.*)$/.exec(lines[index]);
    if (!fence) continue;
    const info = fence[1].trim();
    const body = [];
    let cursor = index + 1;
    while (cursor < lines.length && !/^```\s*$/.test(lines[cursor])) {
      body.push(lines[cursor]);
      cursor += 1;
    }
    index = cursor;
    const fields = words(info);
    if (fields.shift() !== 'noble') continue;
    const markers = Object.fromEntries(
      fields.map(field => {
        const at = field.indexOf('=');
        return at < 0 ? [field, ''] : [field.slice(0, at), field.slice(at + 1)];
      }),
    );
    const snippet = body.join('\n');
    const entry = {
      role: markers.role ?? 'unknown',
      name: markers.name ?? null,
      snippet,
      authored: authored(snippet),
      compilable: markers.role === 'expansion',
    };
    if (markers.role === 'sugar') {
      entry.expandsTo = markers['expands-to'] ?? null;
      entry.comparedForms = (markers.compare ?? '').split(',').filter(name => name.length > 0);
    }
    if (markers.role === 'expansion') {
      entry.form = markers.form ?? null;
      const formPath = entry.form === null ? null : join(forms, `${entry.form}.noble`);
      entry.path = formPath === null ? null : formPath.slice(root.length + 1);
      entry.matchesForm = formPath !== null && existsSync(formPath) &&
        normalize(readFileSync(formPath, 'utf8')) === normalize(snippet);
    }
    entries.push(entry);
  }
  return entries;
};

const cases = readdirSync(forms).filter(name => name.endsWith('.noble')).sort();
const results = cases.map(name => measure(join(forms, name)));

// Stack order direction, measured on the reporting surface itself: push `1`, push `5`.
const probe = repl(orderProbe);
const probeObserved = observe(probe.report);
const stackOrderProbe = {
  source: orderProbe,
  stack: probeObserved?.stack ?? null,
  cells: probeObserved?.stackCells ?? null,
  // The probe's first authored word must be the first reported cell, i.e. the reported array runs
  // from the bottom of the stack to the top, so `topOfStack` above is the last element.
  bottomToTop: probeObserved !== null && probeObserved.stackCells === 2 &&
    probeObserved.stack?.[0]?.value === '1' && probeObserved.stack?.[1]?.value === '5',
  note: 'each authored word is pushed in order, so ascending reported index is ascending stack position',
  diagnostic: probe.diagnostic,
};

const sugar = proposalEntries();
const comparisons = sugar.filter(entry => entry.role === 'sugar').map(entry => ({
  sugar: entry.name,
  sugarTokens: entry.authored.tokens,
  sugarTokensExcludingParentheses: entry.authored.tokensExcludingParentheses,
  expandsTo: entry.expandsTo,
  forms: entry.comparedForms.map(name => {
    const measured = results.find(result => result.form === name);
    return {
      form: name,
      tokens: measured?.authored.tokens ?? null,
      tokensExcludingParentheses: measured?.authored.tokensExcludingParentheses ?? null,
      compilable: measured !== null,
    };
  }),
}));

mkdirSync(dirname(output), { recursive: true });
const receipt = {
  schema: 'noble-focus-elaboration-measure/v2',
  source,
  noble,
  nobleSha256: createHash('sha256').update(readFileSync(noble)).digest('hex'),
  // Cargo's profile is visible in the artifact path; wall times from a debug build are not
  // comparable with one from a release build, so the receipt states which one it used.
  nobleProfile: noble.includes('/release/') ? 'release' : noble.includes('/debug/') ? 'debug' : 'unknown',
  nobleVersionString: null,
  nobleVersionNote: 'the CLI exposes no --version string; identity is the binary sha256',
  host: `${process.platform} ${process.arch}`,
  opt: optimize,
  runs,
  formsDirectory: forms.slice(root.length + 1),
  stackOrderProbe,
  results,
  proposal: proposal.slice(root.length + 1),
  sugar,
  comparisons,
};
const failures = results.filter(result => result.compile.status !== 0 ||
  result.executed === null || result.executed.status !== 0);
const mismatchedExpansions = sugar.filter(entry => entry.role === 'expansion' && !entry.matchesForm);
const mismatchedComparisons = comparisons.filter(comparison =>
  comparison.forms.some(form => form.tokens === null));
const values = new Map();
for (const result of results) {
  const key = JSON.stringify(result.executed?.stack ?? null);
  values.set(key, [...(values.get(key) ?? []), result.form]);
}
receipt.summary = {
  forms: results.length,
  compiled: results.filter(result => result.compile.status === 0).length,
  executed: results.filter(result => result.executed?.status === 0).length,
  failures: failures.map(result => result.form),
  stackOrderEstablished: stackOrderProbe.bottomToTop,
  heightConsistent: results.every(result => result.executed?.stackHeightConsistent !== false),
  expansionBlocksMatchForms: mismatchedExpansions.length === 0,
  comparisonsResolve: mismatchedComparisons.length === 0,
};
writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);

for (const result of results) {
  const stack = JSON.stringify(result.executed?.stack ?? null);
  console.log(
    `${result.form}\ttokens=${result.authored.tokens}\tstackTokens=${result.authored.stackManipulationTokens}` +
    `\tmedian=${result.compile.medianMilliseconds}ms\tmodule=${result.compile.moduleBytes}B` +
    `\theight=${result.executed?.stackCells}/${result.executed?.stackLengthMetric}` +
    `\tsteps=${result.executed?.executionSteps}\tstatus=${result.executed?.status}\tstack=${stack}`,
  );
}
for (const entry of sugar) {
  console.log(
    `${entry.role}:${entry.name}\ttokens=${entry.authored.tokens}` +
    `\ttokensWithoutParentheses=${entry.authored.tokensExcludingParentheses}` +
    (entry.role === 'expansion' ? `\tform=${entry.form}\tmatchesForm=${entry.matchesForm}` : '') +
    (entry.role === 'sugar' ? `\texpandsTo=${entry.expandsTo}` : ''),
  );
}
console.log(`stack-order probe ${orderProbe} -> bottomToTop=${stackOrderProbe.bottomToTop}`);
console.log(`clean tree: ${source.tree} at ${source.shortCommit}`);

if (!stackOrderProbe.bottomToTop) {
  console.error('the stack-order probe did not establish the reported array direction');
  process.exit(1);
}
if (mismatchedExpansions.length > 0) {
  console.error(`expansion block(s) do not restate their form: ${mismatchedExpansions.map(e => e.name)}`);
  process.exit(1);
}
if (failures.length > 0) {
  console.error(`form(s) failed to compile or execute: ${failures.map(result => result.form)}`);
  process.exit(1);
}
