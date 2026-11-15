import { createHash } from 'node:crypto';
import { MAX, opsSource, refRun } from '../foundation/arithmetic.mjs';

const hash = text => createHash('sha256').update(text).digest('hex');
const push = value => ({ push: BigInt(value) });
const one = value => [push(value)];
const programSource = program => program.arith
  ? `[ ${opsSource(program.arith)} ]`
  : `${programSource(program.compose[0])} ${programSource(program.compose[1])} compose`;

export function renderP1Case(model) {
  const parts = model.prefix.map(([, value]) => value);
  if (model.kind === 'straight') parts.push(opsSource(model.ops));
  else parts.push(opsSource(model.ins), programSource(model.program), 'run');
  return `${parts.filter(Boolean).join(' ')}\n`;
}

function arithmetic(shape, boundary) {
  if (shape === 'plain') {
    if (boundary === 'min_max_wrap') return { kind: 'straight', ops: [push(MAX), push(1n), 'add'] };
    if (boundary === 'zero_negative_one') return { kind: 'straight', ops: [push(0n), push(-1n), 'sub'] };
    return { kind: 'straight', ops: [push(2n), push(3n), 'mul'] };
  }
  if (shape === 'quoted') {
    const [ins, op] = boundary === 'min_max_wrap' ? [[push(MAX), push(1n)], 'add']
      : boundary === 'zero_negative_one' ? [[push(0n), push(-1n)], 'sub']
        : [[push(2n), push(3n)], 'mul'];
    return { kind: 'program', ins, program: { arith: [op] } };
  }
  if (boundary === 'min_max_wrap') {
    return { kind: 'program', ins: one(MAX), program: { compose: [
      { arith: [push(1n), 'add'] }, { arith: [push(2n), 'mul'] },
    ] } };
  }
  if (boundary === 'zero_negative_one') {
    return { kind: 'program', ins: one(0n), program: { compose: [
      { arith: [push(-1n), 'add'] }, { arith: [push(-1n), 'mul'] },
    ] } };
  }
  return { kind: 'program', ins: one(2n), program: { compose: [
    { arith: [push(3n), 'add'] }, { arith: [push(4n), 'mul'] },
  ] } };
}

/** The P1 cases frozen by matrix-v1; output bytes are retained with the run. */
export function structuredP1Cases() {
  const cases = [];
  for (const shape of ['plain', 'quoted', 'composed']) {
    for (const boundary of ['min_max_wrap', 'zero_negative_one', 'small']) {
      for (const output of ['ordered_single', 'preserved_prefix_multiple']) {
        const model = {
          ...arithmetic(shape, boundary),
          prefix: output === 'ordered_single' ? [] : [['I64', '23'], ['Bool', 'true']],
          interaction: {
            profile: 'P1-core-source-wasm',
            cell: `P1/${shape}/${boundary}/${output}`,
            quotation_composition: shape,
            boundary_arithmetic: boundary,
            output_shape: output,
            source_operator_witness: shape === 'plain' ? ['arithmetic']
              : shape === 'quoted' ? ['quote', 'run', 'arithmetic']
                : ['quote', 'compose', 'run', 'arithmetic'],
            witness_kind: 'successful-output-and-positive-execution-steps; no per-operator runtime trace',
          },
        };
        const text = renderP1Case(model);
        cases.push({ ...model, origin: { unit: 'issue125-p1-matrix-v1', text, sha256: hash(text) } });
      }
    }
  }
  return cases;
}

export function invalidP1Case() {
  const invalid = {
    kind: 'straight', prefix: [], ops: [push(MAX + 1n)],
    interaction: {
      profile: 'P1-core-source-wasm', cell: null, variant: 'one-premise-invalid-literal',
      outcome: 'refused', changed_premise: 'I64 literal remains in range',
      witness_kind: 'frontend parse refusal; no execution claimed',
    },
  };
  const text = renderP1Case(invalid);
  return { ...invalid, origin: { unit: 'issue125-p1-invalid-control', text, sha256: hash(text) } };
}

export function differentialImport() {
  return { schema: 'noble-fuzz-differential-cases/v1', corpus: 'issue125-matrix-v1',
    counts: { structured_p1_cells: 18, one_premise_invalid_variants: 1,
      compute_budget_cases_per_mode: 18, campaign_seed: 125 },
    cases: structuredP1Cases(), invalid_cases: [invalidP1Case()] };
}

function flat(program) {
  if (program.arith) return program.arith;
  if (program.quoted !== undefined) return [{ push: program.quoted }];
  return [...flat(program.compose[0]), ...flat(program.compose[1])];
}

export function p1CaseOps(model) {
  if (model.kind === 'straight') return model.ops;
  if (model.kind === 'nested') return [...model.ins, ...model.ops];
  return [...model.ins, ...flat(model.program)];
}

export function p1Expected(model) {
  const result = refRun(p1CaseOps(model), []);
  if (result === null) return { outcome: 'reject' };
  return { outcome: 'normal', stack: [
    ...model.prefix,
    ...[...result].reverse().map(value => ['I64', String(value)]),
  ] };
}

export function p1Eligible(model) {
  const interaction = model.interaction;
  if (interaction?.profile !== 'P1-core-source-wasm') return false;
  const shape = interaction.quotation_composition;
  const shapeMatches = shape === 'plain'
    ? model.kind === 'straight'
    : shape === 'quoted'
      ? model.kind === 'program' && Boolean(model.program?.arith)
      : shape === 'composed'
        ? model.kind === 'program' && Boolean(model.program?.compose)
        : false;
  if (!shapeMatches) return false;
  const operations = p1CaseOps(model);
  const values = operations.filter(operation => typeof operation === 'object').map(operation => operation.push);
  const has = value => values.some(candidate => candidate === value);
  const arithmeticMatches = interaction.boundary_arithmetic === 'min_max_wrap'
    ? has(MAX) && has(1n) && operations.includes('add')
    : interaction.boundary_arithmetic === 'zero_negative_one'
      ? has(0n) && has(-1n) && (operations.includes('sub') || shape === 'composed' && operations.includes('add'))
      : interaction.boundary_arithmetic === 'small'
        ? has(2n) && has(3n) && operations.includes('mul')
        : false;
  const output = p1Expected(model);
  const outputMatches = interaction.output_shape === 'ordered_single'
    ? model.prefix.length === 0 && output.outcome === 'normal' && output.stack.length === 1
    : interaction.output_shape === 'preserved_prefix_multiple'
      ? model.prefix.length >= 2 && output.outcome === 'normal' && output.stack.length >= 3 &&
        JSON.stringify(output.stack.slice(0, model.prefix.length)) === JSON.stringify(model.prefix)
      : false;
  return arithmeticMatches && outputMatches;
}

export function p1RandomCell(model) {
  if (model.kind === 'nested') return null;
  const shape = model.kind === 'straight' ? 'plain'
    : model.program?.compose ? 'composed' : 'quoted';
  const operations = p1CaseOps(model);
  const literals = operations.filter(operation => typeof operation === 'object').map(operation => operation.push);
  const has = value => literals.some(candidate => candidate === value);
  const wrapEdge = operations.some((operation, index) => {
    const left = operations[index]?.push;
    const right = operations[index + 1]?.push;
    const opcode = operations[index + 2];
    return left === MAX && right === 1n && opcode === 'add' ||
      left === -MAX - 1n && right === -1n && opcode === 'sub';
  });
  const boundary = wrapEdge ? 'min_max_wrap'
    : has(0n) && has(-1n) && operations.some(operation => ['add', 'sub', 'mul'].includes(operation))
      ? 'zero_negative_one'
      : has(2n) && has(3n) && operations.includes('mul') ? 'small' : null;
  const output = model.prefix.length === 0 ? 'ordered_single'
    : model.prefix.length >= 2 ? 'preserved_prefix_multiple' : null;
  if (!boundary || !output) return null;
  return `P1/${shape}/${boundary}/${output}`;
}
