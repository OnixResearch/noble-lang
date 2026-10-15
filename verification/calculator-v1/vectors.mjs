// Derives runnable `noble calculator` vectors from the canonical cases file
// through the single reviewed CLI mapping table. No expected value, category
// or span is written here; they are read from the cases file.
import assert from 'node:assert/strict';

export const VECTOR_SCHEMA = 'noble-calculator-vectors/v1';
// A display line that is some exact value (`n`, `n/d` or a finite decimal).
export const EXACT_DISPLAY = /^-?\d+(?:\/\d+|\.\d+)?$/;

const pad = index => String(index + 1).padStart(2, '0');

export const display = (numerator, denominator) => {
  assert.match(numerator, /^-?\d+$/);
  assert.match(denominator, /^\d+$/);
  return denominator === '1' ? numerator : `${numerator}/${denominator}`;
};

const fraction = text => {
  const match = /^(-?\d+)\/(\d+)$/.exec(text);
  assert.ok(match, `unsupported expected fraction ${text}`);
  return display(match[1], match[2]);
};

// A successful `let` prints the bound value (calculator.rs formats it before
// committing State). The cases file states no value for a definition, so the
// echo is judged only when the expression is a canonical integer literal,
// whose display is the literal itself; otherwise any exact display matches.
const echo = (mapping, operation) => {
  assert.equal(mapping['define-variable-success'], 'echo');
  return { echo: /^(?:0|-?[1-9]\d*)$/.test(operation.expression) ? operation.expression : null };
};

export const render = (mapping, operation) => {
  const spelling = mapping.spelling;
  switch (operation.operation) {
  case 'define-variable':
    return spelling['define-variable'].replace('{name}', operation.name)
      .replace('{expression}', operation.expression);
  case 'define-function':
    return spelling['define-function'].replace('{name}', operation.name)
      .replace('{parameters}', operation.parameters.join(spelling['parameter-separator']))
      .replace('{body}', operation.body);
  case 'evaluate':
    return spelling.evaluate.replace('{expression}', operation.expression);
  default:
    throw new Error(`unmapped calculator operation ${operation.operation}`);
  }
};

const diagnostic = (category, span = null) => {
  assert.match(category, /^[A-Za-z]+$/, `invalid diagnostic category ${category}`);
  if (span !== null) {
    assert.ok(Array.isArray(span) && span.length === 2 && span.every(Number.isInteger));
  }
  return { diagnostic: { category, span } };
};

const single = (id, caseId, line, expect, flags = []) => ({
  id, case: caseId, kind: 'line', support: 'cli', flags,
  operations: [{ line, expect }],
});

const session = (id, caseId, kind, operations, flags = []) => ({
  id, case: caseId, kind, support: 'cli', flags, operations,
});

export const generate = (cases, mapping) => {
  const byId = new Map(cases.cases.map(item => [item.id, item]));
  const named = id => {
    const found = byId.get(id);
    assert.ok(found, `cases file lacks ${id}`);
    return found;
  };
  const vectors = [];
  const unsupported = [];

  named('CALC-01').input.vectors.forEach((vector, index) => vectors.push(single(
    `CALC-01/${pad(index)}`, 'CALC-01', vector.expression,
    { value: display(vector.numerator, vector.denominator) })));
  named('CALC-02').input.vectors.forEach((vector, index) => vectors.push(single(
    `CALC-02/${pad(index)}`, 'CALC-02', vector.expression,
    diagnostic(vector.category, vector.span))));
  named('CALC-03').input.vectors.forEach((vector, index) => vectors.push(single(
    `CALC-03/${pad(index)}`, 'CALC-03', vector.expression, diagnostic(vector.category))));

  const lexical = named('CALC-06').input.operations.map(operation => {
    let expect;
    if (operation.expected_error !== undefined) {
      expect = diagnostic(operation.expected_error);
    } else if (operation.expected_fraction !== undefined) {
      expect = { value: fraction(operation.expected_fraction) };
    } else {
      assert.ok(operation.operation.startsWith('define-'), 'CALC-06 evaluation lacks an expectation');
      expect = operation.operation === 'define-variable' ? echo(mapping, operation) : { silent: true };
    }
    return { line: render(mapping, operation), expect, source: operation };
  });
  vectors.push(session('CALC-06/session', 'CALC-06', 'session', lexical));

  const admission = mapping['CALC-07'];
  const caseVariants = named('CALC-07').input.variants;
  assert.deepEqual(Object.keys(admission).sort(), caseVariants.map(item => item.case).sort(),
    'CALC-07 mapping must list every case variant, supported or not');
  for (const variant of caseVariants) {
    const entry = admission[variant.case];
    const id = `CALC-07/${variant.case}`;
    if (entry.support !== 'cli') {
      assert.equal(entry.support, 'unsupported-on-cli');
      assert.ok(entry.reason, `${id}: unsupported variant needs a reason`);
      unsupported.push({ id, case: 'CALC-07', variant: variant.case, outcome: variant.outcome,
        support: entry.support, reason: entry.reason });
      continue;
    }
    const outcome = variant.outcome === 'definition-accepted-body-not-evaluated'
      ? { silent: true } : diagnostic(variant.outcome);
    const operations = entry.operations.map(operation => {
      let expect;
      if (operation.expect === 'case-outcome') {
        expect = outcome;
      } else if (operation.expect === 'silent') {
        expect = { silent: true };
      } else {
        expect = diagnostic(operation.expect.diagnostic);
      }
      return { line: render(mapping, operation), expect, source: operation };
    });
    vectors.push(session(id, 'CALC-07', 'line', operations));
  }

  named('CALC-09').input.vectors.forEach((vector, index) => {
    const flags = mapping.modes[vector.mode];
    assert.ok(Array.isArray(flags), `CALC-09 mode ${vector.mode} is unmapped`);
    vectors.push(single(`CALC-09/${pad(index)}`, 'CALC-09', vector.fraction,
      { value: vector.display }, flags));
  });
  return { schema: VECTOR_SCHEMA, vectors, unsupported };
};

export const validate = file => {
  assert.equal(file.schema, VECTOR_SCHEMA, 'custom vector file schema');
  assert.ok(Array.isArray(file.vectors) && file.vectors.length > 0, 'custom vector file has no vectors');
  const seen = new Set();
  for (const vector of file.vectors) {
    assert.match(vector.id, /^[A-Za-z0-9._/#-]{1,128}$/, `invalid vector id ${vector.id}`);
    assert.ok(!seen.has(vector.id), `duplicate vector id ${vector.id}`);
    seen.add(vector.id);
    assert.ok(['line', 'session'].includes(vector.kind), `${vector.id}: kind`);
    assert.equal(vector.support, 'cli', `${vector.id}: only CLI-supported vectors run`);
    assert.ok(Array.isArray(vector.flags) && vector.flags.every(flag => typeof flag === 'string'));
    assert.ok(Array.isArray(vector.operations) && vector.operations.length > 0);
    for (const operation of vector.operations) {
      assert.equal(typeof operation.line, 'string');
      assert.ok(!operation.line.includes('\n'), `${vector.id}: one line per operation`);
      const kinds = ['value', 'echo', 'diagnostic', 'silent'].filter(key => operation.expect?.[key] !== undefined);
      assert.equal(kinds.length, 1, `${vector.id}: each operation expects exactly one outcome`);
    }
  }
  return file;
};

// Expected streams for the whole process. `fn` prints nothing on success,
// values and `let` echoes go to stdout (null: any exact display), guest
// diagnostics to stderr; exit 1 iff any line failed.
export const transcript = vector => ({
  stdout: vector.operations.filter(op => op.expect.value !== undefined || op.expect.echo !== undefined)
    .map(op => op.expect.value ?? op.expect.echo),
  stderr: vector.operations.filter(op => op.expect.diagnostic !== undefined)
    .map(op => op.expect.diagnostic),
  status: vector.operations.some(op => op.expect.diagnostic !== undefined) ? 1 : 0,
});
