// Workload definitions for the allocation-strategy baselines. Every generator is a
// pure function of the bounded sizes below, so a receipt can be replayed from the
// harness source without an external corpus. The W1 tree workloads are the
// BE-LIMIT-01 shapes: alternating increment/double leaves composed at runtime.
export const TREE_INITIAL = 20n;

export const leaf = index => (index % 2 === 0 ? '[ 1 + ]' : '[ 2 * ]');

// Independent wrapping oracle. The recipe alternates builtin i64.add (4) and
// i64.mul (6) exactly as the checked leaves lower.
export function treeOracle(leaves, input = TREE_INITIAL) {
  let value = BigInt(input);
  const recipe = [];
  for (let index = 0; index < leaves; index += 1) {
    const add = index % 2 === 0;
    value = BigInt.asIntN(64, add ? value + 1n : value * 2n);
    recipe.push({ literal: add ? 1 : 2, invoke: add ? 4 : 6 });
  }
  return { value: String(value), recipe };
}

export const leafCounts = [8, 32, 255, 256, 257];

// One source expression that builds `count` leaves into one Program and runs it on
// the initial value. Only used while the expression stays inside the source work
// and operand budgets: a 32-leaf chunk compiles (16-leaf chunks at 256 leaves).
export function treeExpression(shape, count, chunk = 16) {
  if (shape === 'left') {
    const parts = [String(TREE_INITIAL)];
    for (let start = 0; start < count; start += chunk) {
      const size = Math.min(chunk, count - start);
      for (let index = 0; index < size; index += 1) parts.push(leaf(start + index));
      for (let index = 0; index + 1 < size; index += 1) parts.push('compose');
      if (start > 0) parts.push('compose');
    }
    parts.push('run');
    return parts.join(' ');
  }
  const parts = [String(TREE_INITIAL)];
  for (let index = 0; index < count; index += 1) parts.push(leaf(index));
  for (let index = 1; index < count; index += 1) parts.push('compose');
  parts.push('run');
  return parts.join(' ');
}

// Multi-submission builders for counts that exceed one expression's compile budget.
// Every line is one checked submission; the session stack carries the partial
// composition Programs between submissions.
export function treeBuildLines(shape, count, chunk = 16) {
  const lines = [`${TREE_INITIAL}`];
  const chunks = [];
  for (let start = 0; start < count; start += chunk) {
    const size = Math.min(chunk, count - start);
    chunks.push(Array.from({ length: size }, (_, index) => leaf(start + index)).join(' '));
  }
  if (shape === 'left') {
    // Each chunk composes into the accumulator immediately: one accumulator slot.
    for (const [index, source] of chunks.entries()) {
      lines.push(index === 0
        ? `${source} ${Array.from({ length: chunk - 1 }, () => 'compose').join(' ')}`
        : `${source} ${Array.from({ length: chunk - 1 }, () => 'compose').join(' ')} compose`);
    }
    lines.push('run');
    return lines;
  }
  if (shape === 'right') {
    // All chunks stay on the stack, then fold from the top.
    for (const source of chunks) lines.push(`${source} ${Array.from({ length: chunk - 1 }, () => 'compose').join(' ')}`);
    lines.push(Array.from({ length: chunks.length - 1 }, () => 'compose').join(' '));
    lines.push('run');
    return lines;
  }
  // Balanced at chunk granularity: pair chunk Programs after every two pushes.
  for (const source of chunks) {
    lines.push(`${source} ${Array.from({ length: chunk - 1 }, () => 'compose').join(' ')}`);
    if (lines.filter(line => line.includes('[ ')).length % 2 === 0) lines.push('compose');
  }
  lines.push('run');
  return lines;
}

// The shape the balanced builder actually realizes, recorded rather than implied.
export const treeBuildShape = (shape, count, chunk = 16) => {
  if (shape === 'left') return 'left-deep-at-chunk-boundaries';
  if (shape === 'right') return 'right-deep-at-chunk-boundaries';
  return count > chunk ? 'paired-at-chunk-boundaries' : 'single-chunk';
};

export const shortRoot = '41 1 +';

export function repeatedRootLines(count) {
  return Array.from({ length: count }, () => shortRoot);
}

// W4: definitions and replacements. Fifty names are redefined every fifty lines,
// so a long session keeps replacing checked definitions.
export const definitionNames = 50;
export function definitionSessionLines(count) {
  const lines = [];
  let at = 0;
  while (lines.length < count) {
    const name = `n${at % definitionNames}`;
    lines.push(`def ${name} [ ${at + 1} + ]`);
    if (lines.length < count) lines.push(`40 ${name}`);
    at += 1;
  }
  return lines.slice(0, count);
}

// W5: interleaved retained Program values and scratch work. Two retained Programs
// are merged every third line, so the session stack stays inside the operand budget.
export function mixedLifetimeLines(count) {
  const lines = [];
  for (let index = 0; lines.length < count; index += 1) {
    if (index % 3 === 0) lines.push(`40 ${leaf(index)}`, '41 1 +');
    else if (index % 3 === 1) lines.push(`40 ${leaf(index)}`);
    else lines.push('compose');
  }
  return lines.slice(0, count);
}

// W7: a trap after an ordered effect prefix, and a step-exhausted root.
export const trapSource = '[ "first" test.emit test.abort ] [ "second" test.emit ] compose run';
export const stepExhaustionSource = '40000 1 + drop 40000 1 + drop 40000 1 + drop';

// A4 live-slot operations. The authority must name exactly the installed sources.
export const slotSources = {
  tree32: treeExpression('left', 32),
  short: shortRoot,
  quoted: '40 [ 1 + ]',
  stepped: '30000 1 + drop 30000 1 + drop 30000 1 + drop 30000 1 + drop',
};

// W8 Calculator-v1: definitions and expressions in one terminal session. The
// calculator replays retained definitions at every submission boundary, so the
// bound is kept small enough to observe the session, not a compile stress test.
export const calculatorSessionLines = 40;
export function calculatorLines() {
  const lines = [];
  for (let index = 0; index < 10; index += 1) lines.push(`let a${index}=${index + 1}`, `fn f${index}(x)=x*a${index}+${index}`);
  let value = 0;
  while (lines.length < calculatorSessionLines) {
    lines.push(`${value + 1}+${value + 2}`);
    lines.push(`f${value % 10}(${value + 1})`);
    value += 1;
  }
  return lines.slice(0, calculatorSessionLines);
}

// W1 experiment inputs: the M3 build_tree selector (0 left, 1 right, 2 balanced).
export const treeShapes = { left: 0, right: 1, balanced: 2 };
