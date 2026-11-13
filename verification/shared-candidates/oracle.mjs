// Independent I64 oracle for issue #117's shared-candidate experiment.
//
// The candidate families are written as Core stack programs (the same text the
// CLI's extractor consumes). This module implements the *fragment* of Core that
// those programs use — integer literals, `dup`, `swap`, `drop`, `+`, `-`, `*`
// and named `def` bodies — as a JS stack machine over wrapping `i64`. It shares
// no code with `noble-ngraph`, the CLI or the core interpreter, so agreement
// between this machine and a CLI report is finite differential evidence, not a
// proof of correspondence.
//
// Anything outside the implemented fragment throws: a fragment this oracle
// cannot model must fail the lane, never be silently approximated.

/** The wrapping `i64` value of `v` (`2^64` two's-complement). */
export function wrap(v) {
  return BigInt.asIntN(64, v);
}

/** The frozen `i64` limits, as literals the vector file also accepts. */
export const I64_MIN = -(2n ** 63n);
export const I64_MAX = 2n ** 63n - 1n;

const INTEGER = /^-?[0-9]+$/;

/** Splits one submission into tokens, keeping `[` and `]` separate. */
export function tokens(text) {
  const spaced = text.replaceAll('[', ' [ ').replaceAll(']', ' ] ');
  return spaced.split(/\s+/).filter((t) => t.length > 0);
}

/**
 * Parses a source file into its `def` bodies and its root submission.
 * The last submission is the root; every earlier one must be a `def`.
 */
export function parseSource(text) {
  const lines = text
    .split('\n')
    .map((l) => l.trim())
    .filter((l) => l.length > 0);
  const definitions = new Map();
  if (lines.length === 0) throw new Error('oracle: empty source');
  const root = tokens(lines[lines.length - 1]);
  for (const line of lines.slice(0, -1)) {
    const t = tokens(line);
    if (t[0] !== 'def' || t.length < 4 || t[2] !== '[' || t[t.length - 1] !== ']') {
      throw new Error(`oracle: submission ${JSON.stringify(line)} is not a def`);
    }
    const body = t.slice(3, -1);
    if (body.includes('[') || body.includes(']')) {
      throw new Error(`oracle: nested brackets in def ${t[1]}`);
    }
    if (definitions.has(t[1])) throw new Error(`oracle: duplicate def ${t[1]}`);
    definitions.set(t[1], body);
  }
  return { definitions, root };
}

/**
 * Runs one program with `inputs` on the stack and returns the final stack.
 * `definitions` are the named bodies of the same source.
 */
export function run(program, definitions, inputs) {
  const stack = inputs.map((v) => wrap(v));
  walk(program, definitions, stack);
  return stack;
}

function walk(program, definitions, stack) {
  for (const token of program) {
    if (INTEGER.test(token)) {
      stack.push(wrap(BigInt(token)));
      continue;
    }
    if (definitions.has(token)) {
      walk(definitions.get(token), definitions, stack);
      continue;
    }
    switch (token) {
      case 'dup':
        need(stack, 1, token);
        stack.push(stack[stack.length - 1]);
        break;
      case 'swap': {
        need(stack, 2, token);
        const b = stack.pop();
        const a = stack.pop();
        stack.push(b, a);
        break;
      }
      case 'drop':
        need(stack, 1, token);
        stack.pop();
        break;
      case '+':
      case '-':
      case '*': {
        need(stack, 2, token);
        const b = stack.pop();
        const a = stack.pop();
        stack.push(wrap(token === '+' ? a + b : token === '-' ? a - b : a * b));
        break;
      }
      default:
        throw new Error(`oracle: token ${JSON.stringify(token)} is outside the modelled fragment`);
    }
  }
}

function need(stack, n, token) {
  if (stack.length < n) {
    throw new Error(`oracle: ${token} needs ${n} stack value(s), the stack holds ${stack.length}`);
  }
}

/** The frozen row set: hand-fixed boundary rows, then `splitmix64` values. */
export function rows(spec) {
  const boundary = spec.boundary.map((v) => wrap(BigInt(v)));
  const generated = [];
  let state = BigInt(spec.seed) & ((1n << 64n) - 1n);
  while (boundary.length + generated.length < spec.rows) {
    state = wrap(state + 0x9e3779b97f4a7c15n);
    let z = state;
    z = wrap((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n);
    z = wrap((z ^ (z >> 27n)) * 0x94d049bb133111ebn);
    z = z ^ (z >> 31n);
    const magnitude = BigInt(spec.range) + 1n;
    const value = z < 0n ? -z : z;
    generated.push(wrap(value % magnitude));
  }
  return boundary.concat(generated);
}

/** Parses a `noble-ngraph-report/v1` value string (`i64:N`, `bool:true`). */
export function parseValue(text) {
  const [kind, rest] = splitOnce(text, ':');
  if (kind === 'i64') return wrap(BigInt(rest));
  throw new Error(`oracle: report value ${JSON.stringify(text)} is not an i64`);
}

function splitOnce(text, separator) {
  const at = text.indexOf(separator);
  if (at < 0) throw new Error(`oracle: malformed report value ${JSON.stringify(text)}`);
  return [text.slice(0, at), text.slice(at + 1)];
}

/**
 * Parses the report lines of one `eval` run into one output vector per row,
 * keyed by the `vector` field. A line that is not a normal evaluation with
 * `outputs` becomes an error: every row of every arm must evaluate normally.
 */
export function evalOutputs(stdout, label) {
  const result = new Map();
  for (const line of stdout.split('\n')) {
    if (line.trim().length === 0) continue;
    let report;
    try {
      report = JSON.parse(line);
    } catch {
      throw new Error(`${label}: report line is not JSON: ${line.slice(0, 120)}`);
    }
    if (report.vector === undefined) continue;
    if (report.outcome !== 'normal' || !Array.isArray(report.outputs)) {
      throw new Error(
        `${label}: row ${report.vector} did not evaluate normally (${report.outcome}` +
          `${report.code ? ` ${report.code}` : ''}${report.detail ? `: ${report.detail}` : ''})`,
      );
    }
    result.set(report.vector, report.outputs.map(parseValue));
  }
  const ordered = [];
  for (let i = 0; i < result.size; i += 1) {
    if (!result.has(i)) throw new Error(`${label}: report has no row ${i}`);
    ordered.push(result.get(i));
  }
  return ordered;
}

/**
 * Compares one arm's per-row outputs with the oracle's, and reports the first
 * row and index that disagree. `expected` is one candidate-major or
 * row-major structure per call; callers decide the orientation.
 */
export function firstDisagreement(armRows, oracleRows, at) {
  if (armRows.length !== oracleRows.length) {
    return { row: null, index: null, detail: `arm has ${armRows.length} rows, expected ${oracleRows.length}` };
  }
  for (let row = 0; row < oracleRows.length; row += 1) {
    const got = armRows[row];
    const want = oracleRows[row];
    if (got === undefined || got.length !== want.length) {
      return {
        row,
        index: null,
        detail: `row ${row} has ${got === undefined ? 'no' : got.length} output(s), expected ${want.length}`,
      };
    }
    for (let index = 0; index < want.length; index += 1) {
      if (got[index] !== want[index]) {
        return {
          row,
          index,
          detail: `${at} row ${row} candidate ${index}: ${got[index]} != ${want[index]}`,
        };
      }
    }
  }
  return null;
}

// ---------------------------------------------------------------------------
// The frozen programs
//
// These are the only functions that assemble candidate text. `run.mjs` and the
// unit tests both call them, so the measured programs and the checked programs
// cannot drift apart.
// ---------------------------------------------------------------------------

/** How many candidates a family declares. */
export function candidateCount(family) {
  return (family.sharing === 'shared' ? family.tails : family.fragments).length;
}

/** One program per candidate, in candidate order. */
export function candidatePrograms(family) {
  const programs = family.sharing === 'shared' ? family.tails : family.fragments;
  return programs.map((text, index) => ({
    name: `candidate-${String(index + 1).padStart(2, '0')}`,
    source: family.sharing === 'shared' ? `${family.prefix} ${text}\n` : `${text}\n`,
  }));
}

/** The shared value alone: one output, reused by every candidate of the family. */
export function prefixProgram(family) {
  return { name: 'prefix-only', source: `${family.prefix}\n` };
}

/**
 * The merged program: every candidate in one root, in candidate order.
 *
 * `duplicated` recomputes the shared value before every candidate instead of
 * reusing one computed value. For a family whose candidates share nothing the
 * two forms are the same program, and `identical` says so.
 */
export function mergedProgram(family, duplicated) {
  const body = [];
  if (family.sharing === 'none') {
    family.fragments.forEach((fragment, index) => {
      const last = index === family.fragments.length - 1;
      if (!last) body.push('dup');
      body.push(fragment);
      if (!last) body.push('swap');
    });
    return { name: 'merged', source: `${body.join(' ')}\n`, identical: true };
  }
  family.tails.forEach((tail, index) => {
    // Every candidate but the last duplicates the reused value, applies its
    // tail and moves the result beneath the reused value.
    const last = index === family.tails.length - 1;
    if (!last) body.push('dup');
    if (duplicated) body.push('prefix');
    body.push(tail);
    if (!last) body.push('swap');
  });
  const root = duplicated ? body : ['prefix', ...body];
  return {
    name: duplicated ? 'merged-duplicated' : 'merged',
    source: `def prefix [ ${family.prefix} ]\n${root.join(' ')}\n`,
    identical: false,
  };
}
