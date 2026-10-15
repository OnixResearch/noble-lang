// Margin logic of the calculator headroom check over synthetic trace records.
// No calculator execution; the full-chain run is `calculator-headroom.mjs run`.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import {
  DEFAULT_MARGINS, evaluate, environmentShare, parseMargin, parseTrace, sessions,
} from './calculator-headroom.mjs';

const caps = {
  work_issued: 2_000_000_000, type_size: 256, environment_rows: 896, named_bodies: 512,
  signature_keys: 8192, table_entries: 16_384, wat_bytes: 4_194_304,
  text_start: 524_288, text_end: 983_040,
};
function record(module, compile, extra = {}) {
  return {
    schema: 'noble-calculator-trace/1', record: 'module', session: 1, line: 1, phase: 'line',
    module, source: `stage${module}`,
    epoch: 'normal', outcome: 'normal',
    compile: {
      work_initial: 512_000_000, work_preflight_grant: 0, work_environment_grant: 0,
      work_plan_grant: 0, work_issued: 1_000_000_000, work_spent: 0,
      environment_rows: 700, environment_work: 0, named_bodies: 10, max_type_size: 100,
      signature_keys: 100, signature_key_charge: 0, identity_charge: 0,
      table_end: 100, text_end: caps.text_start, wat_bytes: 1000, caps, ...compile,
    },
    ...extra,
  };
}

test('a compile exactly at the Work margin passes; one unit under fails', () => {
  const at = evaluate([record(1, { work_issued: 1_900_000_000 })]);
  assert.equal(at.status, 'pass');
  assert.equal(at.minima.work_issued.headroom, 100_000_000);
  const under = evaluate([record(1, { work_issued: 1_900_000_001 })]);
  assert.equal(under.status, 'fail');
  assert.deepEqual(under.findings.map(f => [f.metric, f.severity, f.headroom]),
    [['work_issued', 'fail', 99_999_999]]);
});

test('the lowest headroom over all compiles is reported with its stage', () => {
  const result = evaluate([
    record(1, { max_type_size: 200 }),
    record(2, { max_type_size: 256 }),
    record(3, { max_type_size: 250 }),
  ]);
  assert.equal(result.minima.type_size.headroom, 0);
  assert.equal(result.minima.type_size.source, 'stage2');
  assert.equal(result.status, 'warn');
  assert.deepEqual(result.findings.map(f => f.metric), ['type_size']);
});

test('warn-only breaches never fail, and a margin override can promote one', () => {
  const records = [record(1, { environment_rows: 850, text_end: caps.text_end - 10 })];
  const warned = evaluate(records);
  assert.equal(warned.status, 'warn');
  assert.deepEqual(warned.findings.map(f => f.metric).sort(), ['environment_rows', 'text_bytes']);
  const strict = evaluate(records, parseMargin('environment_rows=64:fail', DEFAULT_MARGINS));
  assert.equal(strict.status, 'fail');
  const relaxed = evaluate(records, parseMargin('text_bytes=0', parseMargin('environment_rows=46', DEFAULT_MARGINS)));
  assert.equal(relaxed.status, 'pass');
});

test('margin overrides keep severity unless given and reject unknown metrics', () => {
  const margins = parseMargin('work_issued=5', DEFAULT_MARGINS);
  assert.deepEqual(margins.work_issued, { margin: 5, severity: 'fail' });
  assert.throws(() => parseMargin('bogus=1', DEFAULT_MARGINS));
  assert.throws(() => parseMargin('work_issued=-1', DEFAULT_MARGINS));
});

test('a record missing a metric is an error, not a silent pass', () => {
  const broken = record(1, {});
  delete broken.compile.max_type_size;
  assert.throws(() => evaluate([broken]));
});

test('traces keep only module records with compile metrics and reject other schemas', () => {
  const text = [
    JSON.stringify(record(1, {})),
    JSON.stringify({ schema: 'noble-calculator-trace/1', record: 'session', session: 1, line: 1 }),
    JSON.stringify({ schema: 'noble-calculator-trace/1', record: 'module', module: 2 }),
    '',
  ].join('\n');
  assert.equal(parseTrace(text).length, 1);
  assert.throws(() => parseTrace(JSON.stringify({ schema: 'other/1' })));
});

test('sessions group by candidate session and track text growth and key charge', () => {
  const grouped = sessions([
    record(1, { text_end: caps.text_start + 10, signature_key_charge: 5 }, { phase: 'replay' }),
    record(2, { text_end: caps.text_start + 30, signature_key_charge: 3 }, { epoch: 'program_free' }),
    record(3, { text_end: caps.text_start + 4 }, { session: 2, line: 2 }),
  ]);
  assert.deepEqual(grouped.map(s => [s.session, s.modules, s.replayed_modules, s.text_bytes,
    s.max_signature_key_charge, s.program_free]),
  [[1, 2, 1, 30, 5, 1], [2, 1, 0, 4, 0, 0]]);
});

test('environment share isolates the per-body allowance of the tightest compile', () => {
  const share = environmentShare([
    record(1, { work_issued: 1_000_000_000, work_environment_grant: 130, environment_work: 10, named_bodies: 10 }),
    record(2, { work_issued: 1_950_000_000, work_environment_grant: 230, environment_work: 10, named_bodies: 20 }),
  ]);
  assert.equal(share.environment, 360);
  assert.equal(share.worst.headroom, 50_000_000);
  assert.equal(share.worst.headroom_without_per_body_scans, 50_000_200);
});
