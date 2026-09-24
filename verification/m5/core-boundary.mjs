#!/usr/bin/env node
// Independent JS Canonical ABI host probes for an unchanged production core.
// These probes do not claim to be a second Component Model implementation.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import crypto from 'node:crypto';

const [corePath, exportName, fault, point = 'none'] = process.argv.slice(2);
if (!corePath || !['echo-text', 'echo-bytes'].includes(exportName)
    || !['none', 'allocation-quota', 'malformed-peer-bytes', 'malformed-peer-range'].includes(fault)
    || process.argv.length < 5 || process.argv.length > 6) {
  throw Error('usage: NODE core-boundary.mjs CORE_WASM echo-text|echo-bytes FAULT [QUOTA_POINT]');
}
const inputBytes = exportName === 'echo-text' ? Buffer.from('héllo', 'utf8') : Buffer.from([0, 127, 255]);
const heapStart = 65_536;
const memoryBytes = 1_048_576;
const align = value => Math.ceil(value / 4) * 4;
const n = inputBytes.length;
const inputEnd = n;
const importAreaEnd = align(inputEnd) + 8;
const peerResultEnd = importAreaEnd + n;
const copiedResultEnd = peerResultEnd + n;
const exportedAreaEnd = align(copiedResultEnd) + 8;
const quotas = {
  'argument-lowering': n - 1,
  'import-result-area': inputEnd,
  'peer-result-lowering': importAreaEnd,
  'export-result-copy': peerResultEnd,
  'export-result-area': copiedResultEnd,
};
const expectedQuota = {
  'argument-lowering': { allocations: 0, allocated_bytes: 0, copied_bytes: 0, live_bytes: 0, requests: 0 },
  'import-result-area': { allocations: 1, allocated_bytes: n, copied_bytes: 0, live_bytes: inputEnd, requests: 0 },
  'peer-result-lowering': { allocations: 2, allocated_bytes: n + 8, copied_bytes: 0, live_bytes: importAreaEnd, requests: 1 },
  'export-result-copy': { allocations: 3, allocated_bytes: 2 * n + 8, copied_bytes: 0, live_bytes: peerResultEnd, requests: 1 },
  'export-result-area': { allocations: 4, allocated_bytes: 3 * n + 8, copied_bytes: n, live_bytes: copiedResultEnd, requests: 1 },
};
const bytes = fs.readFileSync(corePath);
const report = {
  schema: 'noble-m5-core-boundary/v1', result: 'running',
  core: { path: corePath, sha256: crypto.createHash('sha256').update(bytes).digest('hex') },
  engine: { executable: process.execPath, versions: process.versions },
  export: exportName, fault, quota_point: point,
  ABI_revision: 'wasm-tools-1.245.1-sync-memory32-utf8',
  trust_scope: 'actual production core Wasm with an independent hostile JS import provider; not independent canonical component linking',
  buffer_owners: [
    'JS caller owns typed input bytes; cabi_realloc creates the lowered copy in the guest arena',
    'JS import provider validates its input and writes its return range into the guest arena',
    'guest validates returned ranges before copying and exposing its own result record',
    'generated post-return reclaims the arena on success; explicit host trap cleanup reclaims it on failure',
  ],
  non_claims: ['physical engine allocation/copy counts', 'zero-copy conversion', 'resource host correctness',
    'malformed Component Model values accepted by Wasmtime', 'core diagnostic exports are WIT exports'],
  input: { type: exportName === 'echo-text' ? 'string' : 'list<u8>', value: exportName === 'echo-text' ? 'héllo' : [...inputBytes] },
  events: [], host: { requests: [], unexpected_requests: [], input_written_bytes: 0,
    input_read_bytes: 0, result_written_bytes: 0, result_read_bytes: 0, response_written: false },
  results: [], partial_trusted_values_published: 0,
};
let instance;
let returned;
let callError;
function snapshot() {
  const e = instance.exports;
  return {
    allocations: Number(e['noble$allocation-count']()),
    allocated_bytes: Number(e['noble$allocated-bytes']()),
    copied_bytes: Number(e['noble$copied-bytes']()),
    live_bytes: e['noble$live-bytes'](),
    cleanup_count: Number(e['noble$cleanup-count']()),
  };
}
function range(ptr, len) {
  assert.ok(Number.isInteger(ptr) && ptr >= 0 && Number.isInteger(len) && len >= 0);
  assert.ok(ptr <= instance.exports.memory.buffer.byteLength && len <= instance.exports.memory.buffer.byteLength - ptr,
    'provider attempted an out-of-range memory access');
}
function respond(module, name, args) {
  const identity = `${module}#${name}`;
  report.host.requests.push(identity);
  const selected = exportName === 'echo-text' ? 'text' : 'bytes';
  if (module !== 'noble-test:sync/echo@1.0.0' || name !== selected) {
    report.host.unexpected_requests.push({ identity, arguments: args.map(String) });
    throw Error(`unapproved host request: ${identity}; no dummy resource or authority handle is returned`);
  }
  assert.equal(args.length, 3, 'string/list import uses a flattened pointer, length and return area');
  const [ptr, len, retptr] = args.map(value => value >>> 0);
  range(ptr, len); range(retptr, 8);
  const received = Buffer.from(new Uint8Array(instance.exports.memory.buffer, ptr, len));
  report.host.input_read_bytes += len;
  assert.deepEqual(received, inputBytes, 'real compiled call passes the declared bytes');
  const view = new DataView(instance.exports.memory.buffer);
  let outptr;
  let outlen;
  if (fault === 'malformed-peer-range') {
    assert.equal(exportName, 'echo-bytes');
    outptr = memoryBytes - 1;
    outlen = 3;
  } else {
    const output = fault === 'malformed-peer-bytes' ? Buffer.from([0xc3, 0x28]) : received;
    if (fault === 'malformed-peer-bytes') assert.equal(exportName, 'echo-text');
    report.events.push({ event: 'peer-result-allocation-attempt', bytes: output.length, accounting: snapshot() });
    outptr = instance.exports.cabi_realloc(0, 0, 1, output.length) >>> 0;
    outlen = output.length;
    range(outptr, outlen);
    new Uint8Array(instance.exports.memory.buffer, outptr, outlen).set(output);
    report.host.result_written_bytes += outlen;
  }
  view.setUint32(retptr, outptr, true);
  view.setUint32(retptr + 4, outlen, true);
  report.host.response_written = true;
  report.events.push({ event: 'peer-result-returned', pointer: outptr, length: outlen, accounting: snapshot() });
}
try {
  assert.equal(fault === 'allocation-quota', Object.hasOwn(quotas, point), 'quota point must name an actual conversion allocation');
  if (fault !== 'allocation-quota') assert.equal(point, 'none');
  const module = new WebAssembly.Module(bytes);
  report.module_imports = WebAssembly.Module.imports(module);
  report.module_exports = WebAssembly.Module.exports(module);
  const imports = Object.create(null);
  for (const entry of report.module_imports) {
    assert.equal(entry.kind, 'function', 'only explicit function imports are supported by this hostile host');
    imports[entry.module] ??= Object.create(null);
    assert.equal(Object.hasOwn(imports[entry.module], entry.name), false, 'duplicate import');
    imports[entry.module][entry.name] = (...args) => respond(entry.module, entry.name, args);
  }
  instance = new WebAssembly.Instance(module, imports);
  const e = instance.exports;
  for (const name of [`cm32p2||${exportName}`, `cm32p2||${exportName}_post`, 'cabi_realloc', 'noble$set-allocation-limit',
    'noble$allocation-count', 'noble$allocated-bytes', 'noble$copied-bytes', 'noble$live-bytes',
    'noble$cleanup-count', 'noble$cleanup']) assert.equal(typeof e[name], 'function', `missing core diagnostic or ABI function: ${name}`);
  assert.ok(e.memory instanceof WebAssembly.Memory);
  assert.equal(e.memory.buffer.byteLength, memoryBytes);
  report.initial = snapshot();
  assert.deepEqual(report.initial, { allocations: 0, allocated_bytes: 0, copied_bytes: 0, live_bytes: 0, cleanup_count: 0 });
  report.allocation_limit = fault === 'allocation-quota' ? quotas[point] : memoryBytes - heapStart;
  e['noble$set-allocation-limit'](report.allocation_limit);
  try {
    report.events.push({ event: 'argument-lowering-allocation-attempt', bytes: n });
    const input = e.cabi_realloc(0, 0, 1, n) >>> 0;
    range(input, n);
    new Uint8Array(e.memory.buffer, input, n).set(inputBytes);
    report.host.input_written_bytes = n;
    report.events.push({ event: 'compiled-export-entered', accounting: snapshot() });
    returned = e[`cm32p2||${exportName}`](input, n);
    report.events.push({ event: 'compiled-export-returned', pointer: returned, accounting: snapshot() });
  } catch (error) {
    callError = error;
    report.trap = { name: error.name, message: error.message, wasm_runtime_error: error instanceof WebAssembly.RuntimeError };
  }
  report.before_cleanup = snapshot();
  assert.deepEqual(report.host.unexpected_requests, [], 'resource and authority imports must never be mocked into success');
  if (fault === 'none') {
    if (callError) throw callError;
    range(returned, 8);
    const view = new DataView(e.memory.buffer);
    const ptr = view.getUint32(returned, true);
    const len = view.getUint32(returned + 4, true);
    range(ptr, len);
    const output = Buffer.from(new Uint8Array(e.memory.buffer, ptr, len));
    report.host.result_read_bytes = len;
    assert.deepEqual(output, inputBytes);
    const value = exportName === 'echo-text' ? new TextDecoder('utf-8', { fatal: true }).decode(output) : [...output];
    assert.deepEqual(report.before_cleanup, { allocations: 5, allocated_bytes: 3 * n + 16,
      copied_bytes: n, live_bytes: exportedAreaEnd, cleanup_count: 0 });
    assert.equal(report.host.requests.length, 1);
    e[`cm32p2||${exportName}_post`](returned);
    report.cleanup = { path: 'generated-canonical-post-return', completed: true };
    report.results = [{ type: report.input.type, value }];
  } else {
    assert.ok(callError instanceof WebAssembly.RuntimeError, 'failure must occur in real Wasm conversion, not in the JS oracle');
    assert.equal(returned, undefined, 'no result record is published on conversion failure');
    if (fault === 'allocation-quota') {
      const expected = expectedQuota[point];
      const { requests, ...accounting } = expected;
      assert.deepEqual(report.before_cleanup, { ...accounting, cleanup_count: 0 });
      assert.equal(report.host.requests.length, requests);
      if (['export-result-copy', 'export-result-area'].includes(point)) assert.equal(report.host.response_written, true);
    } else {
      assert.equal(report.host.requests.length, 1);
      assert.equal(report.host.response_written, true, 'malformed bytes must actually return from the hostile import');
      assert.deepEqual(report.before_cleanup, {
        allocations: fault === 'malformed-peer-bytes' ? 3 : 2,
        allocated_bytes: n + 8 + (fault === 'malformed-peer-bytes' ? 2 : 0),
        copied_bytes: 0,
        live_bytes: importAreaEnd + (fault === 'malformed-peer-bytes' ? 2 : 0),
        cleanup_count: 0,
      });
    }
    e['noble$cleanup']();
    report.cleanup = { path: 'explicit-host-cleanup-after-wasm-trap', completed: true };
    assert.deepEqual(report.results, []);
  }
  report.after_cleanup = snapshot();
  assert.equal(report.after_cleanup.live_bytes, 0);
  assert.equal(report.after_cleanup.cleanup_count, 1);
  for (const key of ['allocations', 'allocated_bytes', 'copied_bytes']) {
    assert.equal(report.after_cleanup[key], report.before_cleanup[key], 'cleanup must not erase cumulative accounting');
  }
  assert.equal(crypto.createHash('sha256').update(fs.readFileSync(corePath)).digest('hex'), report.core.sha256);
  report.outcome = fault === 'none' ? 'lossless-round-trip'
    : fault === 'allocation-quota' ? 'defined-failure-with-cleanup' : 'reject-with-cleanup';
  report.result = 'passed';
} catch (error) {
  report.result = 'failed';
  report.failure = String(error.stack ?? error);
  report.results = [];
  if (instance && !report.after_cleanup) {
    try {
      report.failed_before_cleanup = snapshot();
      instance.exports['noble$cleanup']();
      report.failed_after_cleanup = snapshot();
      report.cleanup = { path: 'best-effort-cleanup-after-harness-failure', completed: true };
    } catch (cleanupError) { report.cleanup_error = String(cleanupError); }
  }
  process.exitCode = 1;
}
console.log(JSON.stringify(report));
