#!/usr/bin/env node
// Source-bound verification gate for exact-number current28 on Signed-Declared-Text-v1.
//
// Usage:
//   node verification/exact-current28/gate.mjs <binary> <binary_sha256> <output_dir>
//
// Scope:
//   Exercises declared BigInt and normalized Rat modules on the unmodified
//   28-builtin Signed-Declared-Text-v1 profile under both --opt off and --opt on,
//   verifies host-observation control frames (:release-observed-roots), checks
//   emitted WAT/Wasm digests and zero host function imports, runs a guest-mutation
//   authenticity control, and compares every result against the independent
//   JS BigInt oracle (oracle.mjs).
//   Does NOT promote CALC-01..CALC-13 or claim Worker selected-vendor acceptance.

import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import * as oracle from './oracle.mjs';

const [binary, expectedDigest, outputDir] = process.argv.slice(2);
assert.ok(binary && expectedDigest && outputDir,
  'usage: node verification/exact-current28/gate.mjs <binary> <binary_sha256> <output_dir>');
assert.match(expectedDigest, /^[0-9a-f]{64}$/, 'binary_sha256 must be 64 lowercase hex chars');
assert.ok(!fs.existsSync(outputDir), `output_dir already exists: ${outputDir}`);

const root = path.resolve(import.meta.dirname, '../..');
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const actualBinaryDigest = digest(fs.readFileSync(binary));
assert.equal(actualBinaryDigest, expectedDigest, 'binary digest mismatch');

const MODULE_ORDER = [
  ['result.noble', path.join(root, 'crates/noble-contracts/src/fixtures/result.noble')],
  ['exact-errors.noble', path.join(import.meta.dirname, 'modules/exact-errors.noble')],
  ['exact-i64.noble', path.join(import.meta.dirname, 'modules/exact-i64.noble')],
  ['exact-mag-core.noble', path.join(import.meta.dirname, 'modules/exact-mag-core.noble')],
  ['exact-mag-add.noble', path.join(import.meta.dirname, 'modules/exact-mag-add.noble')],
  ['exact-mag-sub.noble', path.join(import.meta.dirname, 'modules/exact-mag-sub.noble')],
  ['exact-mag-ml.noble', path.join(import.meta.dirname, 'modules/exact-mag-ml.noble')],
  ['exact-mag-div.noble', path.join(import.meta.dirname, 'modules/exact-mag-div.noble')],
  ['exact-knuth.noble', path.join(import.meta.dirname, 'modules/exact-knuth.noble')],
  ['exact-parse-scan.noble', path.join(import.meta.dirname, 'modules/exact-parse-scan.noble')],
  ['exact-parse-limbs.noble', path.join(import.meta.dirname, 'modules/exact-parse-limbs.noble')],
  ['exact-parse-dec.noble', path.join(import.meta.dirname, 'modules/exact-parse-dec.noble')],
  ['exact-pow10.noble', path.join(import.meta.dirname, 'modules/exact-pow10.noble')],
  ['exact-big-i64.noble', path.join(import.meta.dirname, 'modules/exact-big-i64.noble')],
  ['exact-big-cmp.noble', path.join(import.meta.dirname, 'modules/exact-big-cmp.noble')],
  ['exact-big-route.noble', path.join(import.meta.dirname, 'modules/exact-big-route.noble')],
  ['exact-num.noble', path.join(import.meta.dirname, 'modules/exact-num.noble')],
  ['exact-rat-prep.noble', path.join(import.meta.dirname, 'modules/exact-rat-prep.noble')],
];

// Certification frames executed immediately after each recursive module links.
const CERT_AFTER = new Map([
  ['exact-i64.noble', [
    '1 1 exact_i64@1.divmod_pos drop drop',
    '1 1 exact_i64@1.i64_gcd drop',
  ]],
  ['exact-mag-core.noble', [
    'nil exact_mag_core@1.mag_rev drop',
    'nil exact_mag_core@1.mag_norm drop',
    '0 nil exact_mag_core@1.mag_len_acc drop',
  ]],
  ['exact-mag-add.noble', [
    '0 nil nil exact_mag_add@1.mag_cmp_acc drop',
    '0 nil nil exact_mag_add@1.mag_add_carry drop',
  ]],
  ['exact-mag-sub.noble', [
    'nil nil exact_mag_sub@1.mag_sub drop',
  ]],
  ['exact-mag-ml.noble', [
    '1 0 nil exact_mag_ml@1.mag_mul_limb drop',
  ]],
  ['exact-mag-div.noble', [
    '1 0 nil nil exact_mag_div@1.mag_div_limb_msb drop drop',
    'nil exact_mag_div@1.mag_msb_top drop',
    '0 nil exact_mag_div@1.mag_drop_k drop',
  ]],
  ['exact-parse-scan.noble', [
    '"0" 0 exact_parse_scan@1.scan_digits drop drop drop',
    '"0" 0 1 0 exact_parse_scan@1.chunk9 drop drop',
  ]],
  ['exact-parse-limbs.noble', [
    '"0" 0 1 exact_parse_limbs@1.pack_limbs drop drop',
  ]],
  ['exact-pow10.noble', [
    '0 exact_pow10@1.pow10_mag drop',
  ]],
  ['exact-big-i64.noble', [
    '0 exact_big_i64@1.u64_to_limbs drop',
  ]],
]);

const moduleSources = new Map();
const moduleDigests = {};
for (const [name, filePath] of MODULE_ORDER) {
  const bytes = fs.readFileSync(filePath);
  moduleSources.set(name, bytes.toString('utf8'));
  moduleDigests[name] = digest(bytes);
}

fs.mkdirSync(outputDir, { recursive: true });

// Format an array of base-10^9 limbs [l0, l1, ..., lk] into a Noble List<I64> literal.
// No arithmetic: literally emits Noble's `l0 l1 ... nil cons ...`.
const limbsLiteral = limbs => {
  if (limbs.length === 0) return 'nil';
  return `${limbs.join(' ')} nil ${'cons '.repeat(limbs.length).trim()}`;
};

const boolLiteral = b => (b ? 'true' : 'false');

// Extract I64 value from a JSON report stack item.
const readI64 = item => {
  assert.equal(item.type, 'I64', `expected I64, got ${JSON.stringify(item)}`);
  return Number(item.value);
};

const readBool = item => {
  assert.equal(item.type, 'Bool', `expected Bool, got ${JSON.stringify(item)}`);
  return Boolean(item.value);
};

const readPair = item => {
  assert.equal(item.type, 'Pair', `expected Pair, got ${JSON.stringify(item)}`);
  assert.ok(Array.isArray(item.value) && item.value.length === 2, `malformed Pair: ${JSON.stringify(item)}`);
  return item.value;
};

const readLimbs = item => {
  assert.equal(item.type, 'List', `expected List, got ${JSON.stringify(item)}`);
  return item.value.map(entry => {
    assert.equal(entry.type, 'I64');
    const n = Number(entry.value);
    assert.ok(Number.isSafeInteger(n) && n >= 0 && n < 1_000_000_000, `limb out of range: ${entry.value}`);
    return n;
  });
};

class GuestSession {
  constructor(opt, logPath, emitDir = null) {
    this.opt = opt;
    this.args = [
      'session', '--framed', '--declared-modules', '--text-byte-cursor',
      '--signed-self-recursion', '--opt', opt,
    ];
    if (emitDir) this.args.push('--emit', emitDir);
    this.child = spawn(binary, this.args, {
      cwd: root,
      stdio: ['pipe', 'pipe', 'pipe'],
      env: { PATH: '/nonexistent', LANG: 'C', LC_ALL: 'C' },
    });
    this.buf = '';
    this.stderr = '';
    this.pending = [];
    this.logStream = fs.createWriteStream(logPath);
    this.moduleIds = new Map();
    this.submissions = 0;
    this.retirements = 0;
    this.maxAllocBytes = 0;
    this.maxSteps = 0;
    this.storeId = null;

    this.child.stdout.on('data', chunk => {
      const text = chunk.toString('utf8');
      this.logStream.write(text);
      this.buf += text;
      let idx;
      while ((idx = this.buf.indexOf('\n')) !== -1) {
        const line = this.buf.slice(0, idx);
        this.buf = this.buf.slice(idx + 1);
        if (line.trim()) {
          const resolve = this.pending.shift();
          if (resolve) resolve(JSON.parse(line));
        }
      }
    });
    this.child.stderr.on('data', chunk => {
      this.stderr += chunk.toString('utf8');
    });
  }

  sendRawFrame(str) {
    return new Promise((resolve, reject) => {
      const bytes = Buffer.from(str, 'utf8');
      this.pending.push(resolve);
      this.child.stdin.write(Buffer.concat([Buffer.from(`${bytes.length}\n`), bytes]), err => {
        if (err) reject(err);
      });
    });
  }

  sendControl(line) {
    return new Promise((resolve, reject) => {
      this.pending.push(resolve);
      this.child.stdin.write(Buffer.from(`${line}\n`, 'utf8'), err => {
        if (err) reject(err);
      });
    });
  }

  assertPureReport(r, label) {
    assert.equal(r.profile, 'Signed-Declared-Text-v1', `${label}: wrong profile`);
    assert.equal(r.host_requests, 0, `${label}: host_requests != 0`);
    assert.equal(r.guest_requests, 0, `${label}: guest_requests != 0`);
    assert.equal(r.ambient_fallback_calls, 0, `${label}: ambient_fallback_calls != 0`);
    assert.equal(r.acquired_authority, false, `${label}: acquired_authority != false`);
    if (r.native_trap !== undefined) {
      assert.equal(r.native_trap, null, `${label}: native_trap != null`);
    }
    if (r.request_trace !== undefined) {
      assert.deepEqual(r.request_trace, [], `${label}: non-empty request_trace`);
    }
    if (r.module) {
      assert.ok(Array.isArray(r.module.imports), `${label}: missing module.imports`);
      assert.ok(r.module.imports.every(e => e.kind !== 'function'),
        `${label}: pure module imported a host function`);
    }
    if (r.metrics) {
      if (r.metrics.allocated_since_begin_bytes > this.maxAllocBytes) {
        this.maxAllocBytes = r.metrics.allocated_since_begin_bytes;
      }
      if (r.metrics.execution_steps > this.maxSteps) {
        this.maxSteps = r.metrics.execution_steps;
      }
    }
    if (r.source_store_id) this.storeId = r.source_store_id;
  }

  async linkAll() {
    for (const [name] of MODULE_ORDER) {
      const r = await this.sendRawFrame(moduleSources.get(name));
      this.submissions += 1;
      this.assertPureReport(r, `link ${name}`);
      assert.equal(r.outcome, 'linked', `link ${name}: ${r.stage} ${r.diagnostic}`);
      this.moduleIds.set(r.resolved_module.name, r.resolved_module.identity);
      const certs = CERT_AFTER.get(name) ?? [];
      for (const expr of certs) {
        const cr = await this.sendRawFrame(expr);
        this.submissions += 1;
        this.assertPureReport(cr, `cert ${name}: ${expr}`);
        assert.equal(cr.outcome, 'normal', `cert ${name}: ${cr.stage} ${cr.diagnostic}`);
        assert.equal(cr.stack.length, 0);
        assert.equal(cr.source_epoch_retirement?.outcome, 'source-epoch-retired');
        assert.equal(cr.source_epoch_retirement?.heap_cells_after, 0);
        assert.equal(cr.source_epoch_retirement?.retained_core_entries, 4);
        this.retirements += 1;
      }
    }
  }

  // Evaluate a guest expression starting from empty stack, return stack items,
  // then drop all returned values and release observed roots so the epoch retires cleanly.
  async evalAndRetire(expr, expectedStackLen) {
    const r = await this.sendRawFrame(expr);
    this.submissions += 1;
    this.assertPureReport(r, expr);
    assert.equal(r.outcome, 'normal', `${expr} -> ${r.outcome} (${r.stage}): ${r.diagnostic}`);
    assert.equal(r.metrics.retained_before_begin_bytes, 0,
      `${expr}: expected fresh retired epoch (retained=0), got ${r.metrics.retained_before_begin_bytes}`);
    if (expectedStackLen !== undefined) {
      assert.equal(r.stack.length, expectedStackLen, `${expr}: unexpected stack length`);
    }
    const stack = r.stack;
    if (stack.length > 0) {
      const dropExpr = 'drop '.repeat(stack.length).trim();
      const dr = await this.sendRawFrame(dropExpr);
      this.submissions += 1;
      this.assertPureReport(dr, `drop after ${expr}`);
      assert.equal(dr.outcome, 'normal');
      assert.equal(dr.stack.length, 0);
      if (r.source_observation_id != null) {
        const rel = await this.sendControl(
          `:release-observed-roots ${r.source_store_id} ${r.source_observation_id}`);
        assert.equal(rel.profile, 'Signed-Declared-Text-v1', `release obs ${r.source_observation_id}: wrong profile`);
        assert.equal(rel.outcome, 'source-observation-released',
          `release failed: ${JSON.stringify(rel)}`);
        assert.equal(rel.source_epoch_retirement?.outcome, 'source-epoch-retired');
        assert.equal(rel.source_epoch_retirement?.heap_cells_after, 0);
        assert.equal(rel.source_epoch_retirement?.retained_core_entries, 4);
        this.retirements += 1;
      } else {
        assert.equal(dr.source_epoch_retirement?.outcome, 'source-epoch-retired');
        assert.equal(dr.source_epoch_retirement?.heap_cells_after, 0);
        this.retirements += 1;
      }
    } else if (r.source_epoch_retirement?.outcome === 'source-epoch-retired') {
      this.retirements += 1;
    }
    return { stack, report: r };
  }

  readBigIntNominal(item) {
    assert.equal(item.type, 'Nominal', `expected BigInt Nominal, got ${JSON.stringify(item)}`);
    assert.equal(item.module, this.moduleIds.get('exact_num'));
    assert.equal(item.ordinal, 0);
    const [signNode, limbsNode] = readPair(item.value);
    const neg = readBool(signNode);
    const limbs = readLimbs(limbsNode);
    if (limbs.length === 0) assert.equal(neg, false, 'non-canonical -0 BigInt from guest');
    if (limbs.length > 0) assert.notEqual(limbs[limbs.length - 1], 0, 'trailing zero limb from guest');
    return { neg, limbs };
  }

  readRatNominal(item) {
    assert.equal(item.type, 'Nominal', `expected Rat Nominal, got ${JSON.stringify(item)}`);
    assert.equal(item.module, this.moduleIds.get('exact_num'));
    assert.equal(item.ordinal, 1);
    const [numNode, denNode] = readPair(item.value);
    const n = this.readBigIntNominal(numNode);
    const d = this.readBigIntNominal(denNode);
    assert.equal(d.neg, false, 'Rat denominator must be positive');
    assert.ok(d.limbs.length > 0, 'Rat denominator must be non-zero');
    return { n, d };
  }

  readResult(item) {
    assert.equal(item.type, 'Nominal', `expected Result Nominal, got ${JSON.stringify(item)}`);
    assert.equal(item.module, this.moduleIds.get('result'));
    assert.equal(item.ordinal, 0);
    if (item.variant === 'left') return { ok: true, value: item.value };
    assert.equal(item.variant, 'right');
    const err = item.value;
    assert.equal(err.type, 'Nominal');
    assert.equal(err.module, this.moduleIds.get('exact_errors'));
    assert.equal(err.ordinal, 0);
    return { ok: false, error: err.variant === 'left' ? 'DivisionByZero' : 'InvalidDecimal' };
  }

  // ---- Pure-guest exact-number operations (ZERO host arithmetic) ----

  async magRev(a) {
    const { stack } = await this.evalAndRetire(`${limbsLiteral(a)} exact_mag_core@1.mag_rev`, 1);
    return readLimbs(stack[0]);
  }

  async magLen(a) {
    const { stack } = await this.evalAndRetire(`0 ${limbsLiteral(a)} exact_mag_core@1.mag_len_acc`, 1);
    return readI64(stack[0]);
  }

  async magCmp(a, b) {
    const { stack } = await this.evalAndRetire(
      `0 ${limbsLiteral(a)} ${limbsLiteral(b)} exact_mag_add@1.mag_cmp_acc`, 1);
    return readI64(stack[0]);
  }

  async magAdd(a, b) {
    const { stack } = await this.evalAndRetire(
      `0 ${limbsLiteral(a)} ${limbsLiteral(b)} exact_mag_add@1.mag_add_carry`, 1);
    return readLimbs(stack[0]);
  }

  async magSub(a, b) {
    const { stack } = await this.evalAndRetire(
      `${limbsLiteral(a)} ${limbsLiteral(b)} exact_mag_sub@1.mag_sub`, 1);
    return readLimbs(stack[0]);
  }

  async magMulLimb(m, a) {
    if (m === 0 || a.length === 0) return [];
    const { stack } = await this.evalAndRetire(
      `${m} 0 ${limbsLiteral(a)} exact_mag_ml@1.mag_mul_limb`, 1);
    return readLimbs(stack[0]);
  }

  async magMul(a, b) {
    if (a.length === 0 || b.length === 0) return [];
    let acc = [];
    for (let i = a.length - 1; i >= 0; i -= 1) {
      if (acc.length > 0) acc = [0, ...acc];
      if (a[i] !== 0) {
        const row = await this.magMulLimb(a[i], b);
        acc = acc.length === 0 ? row : await this.magAdd(acc, row);
      }
    }
    return acc;
  }

  async magDivLimbMsb(d, aMsb) {
    const { stack } = await this.evalAndRetire(
      `${d} 0 nil ${limbsLiteral(aMsb)} exact_mag_div@1.mag_div_limb_msb`, 2);
    return { q: readLimbs(stack[0]), rem: readI64(stack[1]) };
  }

  async magDivMod(a, b) {
    assert.ok(b.length > 0, 'magDivMod requires non-zero divisor');
    if ((await this.magCmp(a, b)) < 0) return { q: [], r: a };
    if (b.length === 1) {
      const aMsb = await this.magRev(a);
      const { q, rem } = await this.magDivLimbMsb(b[0], aMsb);
      return { q, r: rem === 0 ? [] : [rem] };
    }
    const k = await this.magLen(b);
    const bTop = readI64((await this.evalAndRetire(`${limbsLiteral(b)} exact_mag_div@1.mag_msb_top`, 1)).stack[0]);
    const scale = readI64((await this.evalAndRetire(`${bTop} exact_knuth@1.knuth_scale`, 1)).stack[0]);
    const aScaled = scale === 1 ? a : await this.magMulLimb(scale, a);
    const bScaled = scale === 1 ? b : await this.magMulLimb(scale, b);
    const bTopScaled = readI64((await this.evalAndRetire(`${limbsLiteral(bScaled)} exact_mag_div@1.mag_msb_top`, 1)).stack[0]);
    const aMsb = await this.magRev(aScaled);

    let r = [];
    let qLsb = [];
    for (const d of aMsb) {
      r = (r.length === 0 && d === 0) ? [] : [d, ...r];
      let qLimb = 0;
      if ((await this.magCmp(r, bScaled)) >= 0) {
        const kMinus1 = readI64((await this.evalAndRetire(`${k} 1 -`, 1)).stack[0]);
        const rTail = readLimbs((await this.evalAndRetire(
          `${kMinus1} ${limbsLiteral(r)} exact_mag_div@1.mag_drop_k`, 1)).stack[0]);
        let qEst = readI64((await this.evalAndRetire(
          `${limbsLiteral(rTail)} ${bTopScaled} exact_knuth@1.knuth_est`, 1)).stack[0]);
        let prod = await this.magMulLimb(qEst, bScaled);
        let adjustments = 0;
        while ((await this.magCmp(r, prod)) < 0) {
          adjustments += 1;
          assert.ok(adjustments <= 2, 'Knuth overestimate must be <= 2');
          qEst = readI64((await this.evalAndRetire(`${qEst} 1 -`, 1)).stack[0]);
          prod = await this.magSub(prod, bScaled);
        }
        r = await this.magSub(r, prod);
        qLimb = qEst;
      }
      if (qLsb.length > 0 || qLimb !== 0) qLsb = [qLimb, ...qLsb];
    }
    if (scale !== 1 && r.length > 0) {
      const rMsb = await this.magRev(r);
      const unscaled = await this.magDivLimbMsb(scale, rMsb);
      assert.equal(unscaled.rem, 0, 'Knuth unscaling remainder must be 0');
      r = unscaled.q;
    }
    return { q: qLsb, r };
  }

  async magGcd(a, b) {
    let x = a;
    let y = b;
    while (y.length > 0) {
      if (x.length === 0) return y;
      if (x.length === 1 && y.length === 1) {
        const g = readI64((await this.evalAndRetire(`${x[0]} ${y[0]} exact_i64@1.i64_gcd`, 1)).stack[0]);
        return g === 0 ? [] : [g];
      }
      const { r } = await this.magDivMod(x, y);
      x = y;
      y = r;
    }
    return x;
  }

  async boolXor(a, b) {
    const { stack } = await this.evalAndRetire(
      `${boolLiteral(a)} ${boolLiteral(b)} [ [ false ] [ true ] if ] [ ] if`, 1);
    return readBool(stack[0]);
  }

  async bigMake(neg, limbs) {
    const { stack } = await this.evalAndRetire(
      `${boolLiteral(neg)} ${limbsLiteral(limbs)} exact_num@1.big_make`, 1);
    return this.readBigIntNominal(stack[0]);
  }

  async bigFromI64Expr(expr) {
    const { stack } = await this.evalAndRetire(`${expr} exact_big_i64@1.i64_to_signed`, 2);
    const neg = readBool(stack[0]);
    const limbs = readLimbs(stack[1]);
    return this.bigMake(neg, limbs);
  }

  async bigParse(text) {
    const { stack } = await this.evalAndRetire(`${JSON.stringify(text)} exact_parse_dec@1.scan_int`, 5);
    const start = readI64(stack[1]);
    const end = readI64(stack[2]);
    const neg = readBool(stack[3]);
    const ok = readBool(stack[4]);
    if (!ok) {
      const errStack = (await this.evalAndRetire(
        'unit exact_errors@1.ExactError.InvalidDecimal result@1.Result.Err', 1)).stack;
      return this.readResult(errStack[0]);
    }
    const limbStack = (await this.evalAndRetire(
      `${JSON.stringify(text)} ${start} ${end} exact_parse_limbs@1.pack_limbs swap drop`, 1)).stack;
    const limbs = readLimbs(limbStack[0]);
    const big = await this.bigMake(neg, limbs);
    return { ok: true, value: big };
  }

  async bigCmp(a, b) {
    const { stack } = await this.evalAndRetire(
      `${boolLiteral(a.neg)} ${limbsLiteral(a.limbs)} ${boolLiteral(b.neg)} ${limbsLiteral(b.limbs)} exact_big_cmp@1.big_cmp_raw`,
      1);
    return readI64(stack[0]);
  }

  async bigNeg(a) {
    const { stack } = await this.evalAndRetire(
      `${boolLiteral(a.neg)} ${limbsLiteral(a.limbs)} exact_num@1.big_make exact_num@1.big_neg`, 1);
    return this.readBigIntNominal(stack[0]);
  }

  async bigAdd(a, b) {
    const c = await this.magCmp(a.limbs, b.limbs);
    const { stack } = await this.evalAndRetire(
      `${boolLiteral(a.neg)} ${limbsLiteral(a.limbs)} ${boolLiteral(b.neg)} ${limbsLiteral(b.limbs)} ${c} exact_big_route@1.big_add_route`,
      1);
    const route = stack[0];
    assert.equal(route.type, 'Nominal');
    assert.equal(route.module, this.moduleIds.get('exact_big_route'));
    const [signNode, pairNode] = readPair(route.value);
    const [leftNode, rightNode] = readPair(pairNode);
    const sign = readBool(signNode);
    const leftMag = readLimbs(leftNode);
    const rightMag = readLimbs(rightNode);
    const mag = route.variant === 'left'
      ? await this.magAdd(leftMag, rightMag)
      : await this.magSub(leftMag, rightMag);
    return this.bigMake(sign, mag);
  }

  async bigSub(a, b) {
    return this.bigAdd(a, await this.bigNeg(b));
  }

  async bigMul(a, b) {
    const mag = await this.magMul(a.limbs, b.limbs);
    const sign = await this.boolXor(a.neg, b.neg);
    return this.bigMake(sign, mag);
  }

  async bigDivMod(a, b) {
    if (b.limbs.length === 0) {
      const errStack = (await this.evalAndRetire(
        'unit exact_errors@1.ExactError.DivisionByZero result@1.Result.Err', 1)).stack;
      return this.readResult(errStack[0]);
    }
    const { q, r } = await this.magDivMod(a.limbs, b.limbs);
    const qSign = await this.boolXor(a.neg, b.neg);
    return {
      ok: true,
      value: {
        q: await this.bigMake(qSign, q),
        r: await this.bigMake(a.neg, r),
      },
    };
  }

  async bigGcd(a, b) {
    const g = await this.magGcd(a.limbs, b.limbs);
    return this.bigMake(false, g);
  }

  async ratSeal(neg, nLimbs, dLimbs) {
    const { stack } = await this.evalAndRetire(
      `${boolLiteral(neg)} ${limbsLiteral(nLimbs)} ${limbsLiteral(dLimbs)} exact_num@1.rat_seal`, 1);
    const res = this.readResult(stack[0]);
    if (!res.ok) return res;
    return { ok: true, value: this.readRatNominal(res.value) };
  }

  async ratMake(n, d) {
    const { stack } = await this.evalAndRetire(
      `${boolLiteral(n.neg)} ${limbsLiteral(n.limbs)} ${boolLiteral(d.neg)} ${limbsLiteral(d.limbs)} exact_rat_prep@1.rat_prep`,
      1);
    const prep = this.readResult(stack[0]);
    if (!prep.ok) return prep;
    const [signNode, pairNode] = readPair(prep.value);
    const [mnNode, mdNode] = readPair(pairNode);
    const sign = readBool(signNode);
    const mn = readLimbs(mnNode);
    const md = readLimbs(mdNode);
    if (mn.length === 0) return this.ratSeal(false, [], [1]);
    const g = await this.magGcd(mn, md);
    if (g.length === 1 && g[0] === 1) return this.ratSeal(sign, mn, md);
    const nDiv = await this.magDivMod(mn, g);
    const dDiv = await this.magDivMod(md, g);
    assert.deepEqual(nDiv.r, [], 'exact reduction remainder for numerator must be 0');
    assert.deepEqual(dDiv.r, [], 'exact reduction remainder for denominator must be 0');
    return this.ratSeal(sign, nDiv.q, dDiv.q);
  }

  async ratNeg(x) {
    const negN = await this.bigNeg(x.n);
    const res = await this.ratSeal(negN.neg, negN.limbs, x.d.limbs);
    assert.equal(res.ok, true);
    return res.value;
  }

  async ratAdd(x, y) {
    const n1d2 = await this.bigMul(x.n, y.d);
    const n2d1 = await this.bigMul(y.n, x.d);
    const num = await this.bigAdd(n1d2, n2d1);
    const den = await this.bigMul(x.d, y.d);
    const res = await this.ratMake(num, den);
    assert.equal(res.ok, true);
    return res.value;
  }

  async ratSub(x, y) {
    return this.ratAdd(x, await this.ratNeg(y));
  }

  async ratMul(x, y) {
    const num = await this.bigMul(x.n, y.n);
    const den = await this.bigMul(x.d, y.d);
    const res = await this.ratMake(num, den);
    assert.equal(res.ok, true);
    return res.value;
  }

  async ratDiv(x, y) {
    const num = await this.bigMul(x.n, y.d);
    const den = await this.bigMul(x.d, y.n);
    return this.ratMake(num, den);
  }

  async ratCmp(x, y) {
    const lhs = await this.bigMul(x.n, y.d);
    const rhs = await this.bigMul(y.n, x.d);
    return this.bigCmp(lhs, rhs);
  }

  async ratParseDecimal(text) {
    // Guest checks sign + integer digits; if not EOF, guest checks '.' + fractional digits.
    const lit = JSON.stringify(text);
    const firstByte = readI64((await this.evalAndRetire(`${lit} 0 text.byte swap drop`, 1)).stack[0]);
    const neg = firstByte === 45;
    const intStart = neg ? 1 : 0;
    const scan1 = (await this.evalAndRetire(
      `${lit} ${intStart} exact_parse_scan@1.scan_digits`, 3)).stack;
    const intEnd = readI64(scan1[1]);
    const isEof = readBool(scan1[2]);
    if (intEnd <= intStart) {
      const errStack = (await this.evalAndRetire(
        'unit exact_errors@1.ExactError.InvalidDecimal result@1.Result.Err', 1)).stack;
      return this.readResult(errStack[0]);
    }
    const intMag = readLimbs((await this.evalAndRetire(
      `${lit} ${intStart} ${intEnd} exact_parse_limbs@1.pack_limbs swap drop`, 1)).stack[0]);
    if (isEof) {
      const num = await this.bigMake(neg, intMag);
      const den = await this.bigMake(false, [1]);
      return this.ratMake(num, den);
    }
    const fracScan = (await this.evalAndRetire(
      `${lit} ${intEnd} exact_parse_dec@1.scan_frac`, 4)).stack;
    const fracStart = readI64(fracScan[1]);
    const fracEnd = readI64(fracScan[2]);
    const fracOk = readBool(fracScan[3]);
    if (!fracOk) {
      const errStack = (await this.evalAndRetire(
        'unit exact_errors@1.ExactError.InvalidDecimal result@1.Result.Err', 1)).stack;
      return this.readResult(errStack[0]);
    }
    const fracMag = readLimbs((await this.evalAndRetire(
      `${lit} ${fracStart} ${fracEnd} exact_parse_limbs@1.pack_limbs swap drop`, 1)).stack[0]);
    const fracLen = readI64((await this.evalAndRetire(`${fracEnd} ${fracStart} -`, 1)).stack[0]);
    const scaleMag = readLimbs((await this.evalAndRetire(
      `${fracLen} exact_pow10@1.pow10_mag`, 1)).stack[0]);
    const shifted = await this.magMul(intMag, scaleMag);
    const totalMag = await this.magAdd(shifted, fracMag);
    const num = await this.bigMake(neg, totalMag);
    const den = await this.bigMake(false, scaleMag);
    return this.ratMake(num, den);
  }

  async close() {
    this.child.stdin.end();
    const exit = await new Promise(resolve => {
      this.child.on('close', (code, signal) => resolve({ code, signal }));
    });
    await new Promise(resolve => this.logStream.end(resolve));
    return exit;
  }
}

// Helper: assert guest BigInt matches JS BigInt oracle.
const assertBigEqual = (guestBig, expectedBigInt, label) => {
  const actual = oracle.fromSignedLimbs(guestBig.neg, guestBig.limbs);
  assert.equal(actual, expectedBigInt, `${label}: guest=${actual} expected=${expectedBigInt}`);
  const expectedRepr = oracle.toSignedLimbs(expectedBigInt);
  assert.equal(guestBig.neg, expectedRepr.neg, `${label}: sign mismatch`);
  assert.deepEqual(guestBig.limbs, expectedRepr.limbs, `${label}: limb mismatch`);
};

// Helper: assert guest Rat matches JS oracle Rat { n, d }.
const assertRatEqual = (guestRat, expectedRat, label) => {
  assertBigEqual(guestRat.n, expectedRat.n, `${label}.n`);
  assertBigEqual(guestRat.d, expectedRat.d, `${label}.d`);
  assert.equal(oracle.gcd(expectedRat.n, expectedRat.d), 1n);
};

// Deterministic xorshift64* PRNG for differential fuzzing.
function makePrng(seed) {
  let s = BigInt(seed);
  const mask = (1n << 64n) - 1n;
  return () => {
    s ^= s >> 12n;
    s = (s ^ (s << 25n)) & mask;
    s ^= s >> 27n;
    return (s * 0x2545F4914F6CDD1Dn) & mask;
  };
}

const randomBigInt = (next, maxLimbs) => {
  const limbs = Number(next() % BigInt(maxLimbs + 1));
  if (limbs === 0) return 0n;
  let val = 0n;
  for (let i = 0; i < limbs; i += 1) {
    const limb = next() % oracle.BASE;
    val = val * oracle.BASE + limb;
  }
  const neg = (next() & 1n) === 1n;
  return neg ? -val : val;
};

const bigToGuest = v => {
  const r = oracle.toSignedLimbs(v);
  return { neg: r.neg, limbs: [...r.limbs] };
};

const ratToGuest = r => ({ n: bigToGuest(r.n), d: bigToGuest(r.d) });

async function runSuite(opt) {
  const logPath = path.join(outputDir, `session-${opt}.jsonl`);
  const session = new GuestSession(opt, logPath);
  await session.linkAll();

  const counts = {
    big_from_i64: 0,
    big_parse_valid: 0,
    big_parse_invalid: 0,
    big_cmp: 0,
    big_add: 0,
    big_sub: 0,
    big_mul: 0,
    big_divmod: 0,
    big_divmod_zero: 0,
    big_gcd: 0,
    rat_make: 0,
    rat_zero_refusals: 0,
    rat_add: 0,
    rat_sub: 0,
    rat_mul: 0,
    rat_div: 0,
    rat_neg: 0,
    rat_cmp: 0,
    rat_parse_decimal_valid: 0,
    rat_parse_decimal_invalid: 0,
    fuzz_big: 0,
    fuzz_rat: 0,
    control_frames: 0,
  };

  // A. BigInt from I64 (including I64_MIN and I64_MAX)
  const i64Cases = [
    ['0', 0n],
    ['1', 1n],
    ['-1', -1n],
    ['999999999', 999_999_999n],
    ['-999999999', -999_999_999n],
    ['1000000000', 1_000_000_000n],
    ['-1000000000', -1_000_000_000n],
    ['1000000000000000000', 1_000_000_000_000_000_000n],
    ['-1000000000000000000', -1_000_000_000_000_000_000n],
    ['9223372036854775807', 9_223_372_036_854_775_807n],
    ['-9223372036854775807 1 -', -9_223_372_036_854_775_808n],
  ];
  for (const [expr, expected] of i64Cases) {
    const g = await session.bigFromI64Expr(expr);
    assertBigEqual(g, expected, `bigFromI64(${expr})`);
    counts.big_from_i64 += 1;
  }
  // Canonical -0 normalization check
  assertBigEqual(await session.bigMake(true, []), 0n, 'bigMake(true, []) -> canonical 0');
  counts.big_from_i64 += 1;

  // B. BigInt decimal parsing (valid & invalid)
  const validIntStrings = [
    '0', '-0', '000000000000', '-000000000000', '42', '-42',
    '999999999', '-999999999', '1000000000', '-1000000000',
    '9223372036854775808', '-9223372036854775809',
    '123456789012345678901234567890',
    '-999999999999999999999999999999999999999999999',
    '100000000000000000000000000000000000000000000000000000000000',
  ];
  for (const text of validIntStrings) {
    const res = await session.bigParse(text);
    assert.equal(res.ok, true, `bigParse(${text}) failed`);
    assertBigEqual(res.value, oracle.parseBigInt(text), `bigParse(${text})`);
    counts.big_parse_valid += 1;
  }
  const invalidIntStrings = ['', '-', '+1', '12a3', ' 1', '1 ', '1.5', 'é', '12é', '--1'];
  for (const text of invalidIntStrings) {
    const res = await session.bigParse(text);
    assert.equal(res.ok, false, `bigParse(${text}) should fail`);
    assert.equal(res.error, 'InvalidDecimal');
    assert.throws(() => oracle.parseBigInt(text), oracle.OracleError);
    counts.big_parse_invalid += 1;
  }

  // C. BigInt compare
  const cmpPairs = [
    [0n, 0n], [0n, 1n], [0n, -1n], [1n, -1n], [-1n, 1n],
    [999_999_999n, 1_000_000_000n], [-999_999_999n, -1_000_000_000n],
    [10n ** 30n, 10n ** 30n], [10n ** 30n, 10n ** 30n + 1n],
    [-(10n ** 30n), -(10n ** 29n)],
    [123456789012345678901234567890n, 123456789012345678901234567891n],
  ];
  for (const [a, b] of cmpPairs) {
    const got = await session.bigCmp(bigToGuest(a), bigToGuest(b));
    assert.equal(got, oracle.compare(a, b), `bigCmp(${a}, ${b})`);
    counts.big_cmp += 1;
  }

  // D. BigInt add & sub (carries, borrows, zero, negatives, multi-limb)
  const addSubPairs = [
    [0n, 0n], [0n, 10n ** 25n], [10n ** 25n, 0n], [0n, -(10n ** 25n)],
    [999_999_999_999_999_999_999_999_999n, 1n],
    [999_999_999_999_999_999_999_999_999_999_999_999n, 1n],
    [9_223_372_036_854_775_807n, 1n],
    [-9_223_372_036_854_775_808n, -1n],
    [10n ** 27n, -1n],
    [10n ** 36n, -1n],
    [10n ** 36n, -999_999_999_999_999_999_999_999_999_999_999_999n],
    [-(10n ** 30n), 10n ** 30n],
    [123456789012345678901234567890123456789012345n, -987654321098765432109876543210987654321098765n],
    [-555555555555555555555555555555n, -444444444444444444444444444445n],
  ];
  for (const [a, b] of addSubPairs) {
    assertBigEqual(await session.bigAdd(bigToGuest(a), bigToGuest(b)), a + b, `bigAdd(${a}, ${b})`);
    counts.big_add += 1;
    assertBigEqual(await session.bigSub(bigToGuest(a), bigToGuest(b)), a - b, `bigSub(${a}, ${b})`);
    counts.big_sub += 1;
  }

  // E. BigInt mul (zero, signs, internal zeros, max carries, 36x36 -> 72 digits)
  const mulPairs = [
    [0n, 0n], [0n, 10n ** 30n], [-(10n ** 30n), 0n],
    [1n, -(10n ** 25n)], [-1n, -(10n ** 25n)],
    [1_000_000_000n, 1_000_000_000n],
    [10n ** 18n, 10n ** 18n],
    [1_000_000_001n, -1_000_000_001n],
    [9_223_372_036_854_775_807n, 9_223_372_036_854_775_807n],
    [-9_223_372_036_854_775_808n, -9_223_372_036_854_775_808n],
    [999_999_999_999_999_999_999_999_999n, 999_999_999_999_999_999_999_999_999n],
    [-123456789987654321555555555111111111n, 999999999888888888777777777666666666n],
  ];
  for (const [a, b] of mulPairs) {
    assertBigEqual(await session.bigMul(bigToGuest(a), bigToGuest(b)), a * b, `bigMul(${a}, ${b})`);
    counts.big_mul += 1;
  }

  // F. BigInt divmod (zero refusal, 1-limb, multi-limb Knuth scaling + overestimate adjustment, all signs)
  for (const a of [0n, 42n, -(10n ** 30n)]) {
    const res = await session.bigDivMod(bigToGuest(a), bigToGuest(0n));
    assert.equal(res.ok, false);
    assert.equal(res.error, 'DivisionByZero');
    assert.throws(() => oracle.divmod(a, 0n), oracle.OracleError);
    counts.big_divmod_zero += 1;
  }
  const divPairs = [
    [0n, 7n], [5n, 7n], [7n, 7n], [10n ** 27n, 1n],
    [10n ** 27n, 999_999_999n], [-(10n ** 27n), 999_999_999n],
    [10n ** 27n, -999_999_999n], [-(10n ** 27n), -999_999_999n],
    // Multi-limb unnormalized (b_top = 1 -> scale = 500_000_000) & Knuth overestimate cases:
    [2n * 10n ** 18n - 1n, 10n ** 9n + 999_999_999n],
    [10n ** 27n - 1n, 10n ** 18n + 999_999_999n * 10n ** 9n + 999_999_999n],
    [999_999_999_999_999_999_999_999_999n, 500_000_000_000_000_000n],
    // Exact multiple & near-multiple with 45-digit / 27-digit operands:
    [123456789012345678901234567890123456789012345n, 987654321098765432109876543n],
    [-123456789012345678901234567890123456789012345n, 987654321098765432109876543n],
    [123456789012345678901234567890123456789012345n, -987654321098765432109876543n],
    [-123456789012345678901234567890123456789012345n, -987654321098765432109876543n],
    [999_999_999_999_999_999_999_999_999n * 123_456_789_012_345_678n, 999_999_999_999_999_999_999_999_999n],
  ];
  for (const [a, b] of divPairs) {
    const res = await session.bigDivMod(bigToGuest(a), bigToGuest(b));
    assert.equal(res.ok, true, `bigDivMod(${a}, ${b}) failed`);
    const exp = oracle.divmod(a, b);
    assertBigEqual(res.value.q, exp.q, `bigDivMod.q(${a}, ${b})`);
    assertBigEqual(res.value.r, exp.r, `bigDivMod.r(${a}, ${b})`);
    counts.big_divmod += 1;
  }

  // G. BigInt GCD (zeros, signs, 1-limb primes, large common prime, multi-limb Fibonacci)
  const largePrime = 100_000_000_000_000_000_039n;
  const gcdPairs = [
    [0n, 0n], [0n, 42n], [-42n, 0n], [0n, -(10n ** 25n)],
    [1071n, 462n], [-1071n, 462n], [1071n, -462n], [-1071n, -462n],
    [1_000_000_007n, 999_999_937n],
    [largePrime * 12_345n, largePrime * 67_890n],
    // Multi-limb consecutive Fibonacci numbers F_50 and F_49 (> 10^9, coprime worst-case Euclidean)
    [12_586_269_025n, 7_778_742_049n],
  ];
  for (const [a, b] of gcdPairs) {
    const got = await session.bigGcd(bigToGuest(a), bigToGuest(b));
    assertBigEqual(got, oracle.gcd(a, b), `bigGcd(${a}, ${b})`);
    counts.big_gcd += 1;
  }

  // H. Normalized Rat arithmetic & decimal parsing
  for (const [n, d] of [[0n, 0n], [5n, 0n], [-(10n ** 25n), 0n]]) {
    const res = await session.ratMake(bigToGuest(n), bigToGuest(d));
    assert.equal(res.ok, false);
    assert.equal(res.error, 'DivisionByZero');
    assert.throws(() => oracle.ratMake(n, d), oracle.OracleError);
    counts.rat_zero_refusals += 1;
  }
  // Division by zero rational
  {
    const res = await session.ratDiv(ratToGuest(oracle.ratMake(1n, 2n)), ratToGuest(oracle.ratMake(0n, 1n)));
    assert.equal(res.ok, false);
    assert.equal(res.error, 'DivisionByZero');
    counts.rat_zero_refusals += 1;
  }
  const ratMakeCases = [
    [0n, 5n], [0n, -7n], [6n, -9n], [-6n, -9n], [-6n, 9n], [42n, 1n],
    [largePrime * 6n, largePrime * -9n],
    [-(10n ** 27n), 2n * 10n ** 27n],
  ];
  for (const [n, d] of ratMakeCases) {
    const res = await session.ratMake(bigToGuest(n), bigToGuest(d));
    assert.equal(res.ok, true);
    assertRatEqual(res.value, oracle.ratMake(n, d), `ratMake(${n}, ${d})`);
    counts.rat_make += 1;
  }

  const ratPairs = [
    [oracle.ratMake(1n, 3n), oracle.ratMake(1n, 6n)],
    [oracle.ratMake(1n, 10n), oracle.ratMake(2n, 10n)],
    [oracle.ratMake(-3n, 4n), oracle.ratMake(3n, 4n)],
    [oracle.ratMake(-5n, 6n), oracle.ratMake(-1n, 6n)],
    [oracle.ratMake(7n, 11n), oracle.ratMake(-14n, 33n)],
    [oracle.ratMake(10n ** 25n + 1n, 3n), oracle.ratMake(10n ** 25n - 1n, 6n)],
    [oracle.ratMake(9_223_372_036_854_775_807n, 10n ** 18n), oracle.ratMake(1n, 10n ** 18n)],
    [oracle.ratMake(10n ** 11n, 10n ** 11n + 1n), oracle.ratMake(10n ** 11n + 1n, 10n ** 11n + 2n)],
  ];
  for (const [x, y] of ratPairs) {
    const gx = ratToGuest(x);
    const gy = ratToGuest(y);
    assertRatEqual(await session.ratAdd(gx, gy), oracle.ratAdd(x, y), `ratAdd(${x.n}/${x.d}, ${y.n}/${y.d})`);
    counts.rat_add += 1;
    assertRatEqual(await session.ratSub(gx, gy), oracle.ratSub(x, y), `ratSub(${x.n}/${x.d}, ${y.n}/${y.d})`);
    counts.rat_sub += 1;
    assertRatEqual(await session.ratMul(gx, gy), oracle.ratMul(x, y), `ratMul(${x.n}/${x.d}, ${y.n}/${y.d})`);
    counts.rat_mul += 1;
    const divRes = await session.ratDiv(gx, gy);
    assert.equal(divRes.ok, true);
    assertRatEqual(divRes.value, oracle.ratDiv(x, y), `ratDiv(${x.n}/${x.d}, ${y.n}/${y.d})`);
    counts.rat_div += 1;
    assertRatEqual(await session.ratNeg(gx), oracle.ratNeg(x), `ratNeg(${x.n}/${x.d})`);
    counts.rat_neg += 1;
    assert.equal(await session.ratCmp(gx, gy), oracle.ratCmp(x, y), `ratCmp(${x.n}/${x.d}, ${y.n}/${y.d})`);
    counts.rat_cmp += 1;
  }

  const validDecimals = [
    '0', '-0', '0.000', '-0.000', '0.1', '0.2', '-0.125', '42', '-42',
    '12345678901234567890.0025', '-0.00000000000000000001',
    '999999999999999999.999999999999',
  ];
  for (const text of validDecimals) {
    const res = await session.ratParseDecimal(text);
    assert.equal(res.ok, true, `ratParseDecimal(${text}) failed`);
    assertRatEqual(res.value, oracle.parseDecimal(text), `ratParseDecimal(${text})`);
    counts.rat_parse_decimal_valid += 1;
  }
  const invalidDecimals = ['', '-', '.', '1.', '.5', '1.2.3', '1e5', '1.2a', 'é', '-.5', '1.'];
  for (const text of invalidDecimals) {
    const res = await session.ratParseDecimal(text);
    assert.equal(res.ok, false, `ratParseDecimal(${text}) should fail`);
    assert.equal(res.error, 'InvalidDecimal');
    assert.throws(() => oracle.parseDecimal(text), oracle.OracleError);
    counts.rat_parse_decimal_invalid += 1;
  }

  // I. Seeded deterministic differential fuzzing (BigInt & Rat)
  const next = makePrng(0x28c0ffee2026n);
  for (let i = 0; i < 10; i += 1) {
    const a = randomBigInt(next, 3);
    const b = randomBigInt(next, 2);
    const ga = bigToGuest(a);
    const gb = bigToGuest(b);
    assert.equal(await session.bigCmp(ga, gb), oracle.compare(a, b), `fuzz[${i}].cmp`);
    assertBigEqual(await session.bigAdd(ga, gb), a + b, `fuzz[${i}].add`);
    assertBigEqual(await session.bigSub(ga, gb), a - b, `fuzz[${i}].sub`);
    assertBigEqual(await session.bigMul(ga, gb), a * b, `fuzz[${i}].mul`);
    if (b !== 0n) {
      const dm = await session.bigDivMod(ga, gb);
      assert.equal(dm.ok, true);
      const exp = oracle.divmod(a, b);
      assertBigEqual(dm.value.q, exp.q, `fuzz[${i}].div`);
      assertBigEqual(dm.value.r, exp.r, `fuzz[${i}].mod`);
    }
    const g1 = randomBigInt(next, 1);
    const g2 = randomBigInt(next, 1);
    assertBigEqual(await session.bigGcd(bigToGuest(g1), bigToGuest(g2)), oracle.gcd(g1, g2), `fuzz[${i}].gcd`);
    counts.fuzz_big += 1;
  }
  for (let i = 0; i < 6; i += 1) {
    const n1 = randomBigInt(next, 1);
    const d1 = oracle.abs(randomBigInt(next, 1)) + 1n;
    const n2 = randomBigInt(next, 1);
    const d2 = oracle.abs(randomBigInt(next, 1)) + 1n;
    const rx = oracle.ratMake(n1, d1);
    const ry = oracle.ratMake(n2, d2);
    const gx = (await session.ratMake(bigToGuest(n1), bigToGuest(d1))).value;
    const gy = (await session.ratMake(bigToGuest(n2), bigToGuest(d2))).value;
    assertRatEqual(gx, rx, `fuzzRat[${i}].x`);
    assertRatEqual(gy, ry, `fuzzRat[${i}].y`);
    assertRatEqual(await session.ratAdd(gx, gy), oracle.ratAdd(rx, ry), `fuzzRat[${i}].add`);
    assertRatEqual(await session.ratSub(gx, gy), oracle.ratSub(rx, ry), `fuzzRat[${i}].sub`);
    assertRatEqual(await session.ratMul(gx, gy), oracle.ratMul(rx, ry), `fuzzRat[${i}].mul`);
    if (ry.n !== 0n) {
      const dr = await session.ratDiv(gx, gy);
      assert.equal(dr.ok, true);
      assertRatEqual(dr.value, oracle.ratDiv(rx, ry), `fuzzRat[${i}].div`);
    }
    assert.equal(await session.ratCmp(gx, gy), oracle.ratCmp(rx, ry), `fuzzRat[${i}].cmp`);
    counts.fuzz_rat += 1;
  }

  // J. Host-observation control frame negative controls (early, forged store, repeated)
  {
    const live = await session.sendRawFrame('false 42 nil cons exact_num@1.big_make');
    session.submissions += 1;
    session.assertPureReport(live, 'control live big_make');
    assert.equal(live.outcome, 'normal');
    assert.ok(live.source_observation_id != null);
    // 1) Early release while value is still on the stack -> refused
    const early = await session.sendControl(
      `:release-observed-roots ${live.source_store_id} ${live.source_observation_id}`);
    assert.equal(early.outcome, 'source-observation-refused');
    counts.control_frames += 1;
    // 2) Forged store UUID -> refused
    const forged = await session.sendControl(
      `:release-observed-roots 00000000-0000-0000-0000-000000000000 ${live.source_observation_id}`);
    assert.equal(forged.outcome, 'source-observation-refused');
    counts.control_frames += 1;
    // 3) Drop stack, then valid release -> retired with 0 heap cells and Core 0..3
    const dropped = await session.sendRawFrame('drop');
    assert.equal(dropped.outcome, 'normal');
    assert.equal(dropped.stack.length, 0);
    const okRel = await session.sendControl(
      `:release-observed-roots ${live.source_store_id} ${live.source_observation_id}`);
    assert.equal(okRel.outcome, 'source-observation-released');
    assert.equal(okRel.source_epoch_retirement?.outcome, 'source-epoch-retired');
    assert.equal(okRel.source_epoch_retirement?.heap_cells_after, 0);
    assert.equal(okRel.source_epoch_retirement?.retained_core_entries, 4);
    session.retirements += 1;
    counts.control_frames += 1;
    // 4) Repeated release of same observation -> refused
    const repeat = await session.sendControl(
      `:release-observed-roots ${live.source_store_id} ${live.source_observation_id}`);
    assert.equal(repeat.outcome, 'source-observation-refused');
    counts.control_frames += 1;
  }

  const exit = await session.close();
  assert.equal(exit.signal, null, `session-${opt} killed by signal ${exit.signal}`);
  assert.equal(exit.code, 2, `session-${opt} expected exit 2 after intentional control refusals, got ${exit.code}`);
  assert.equal(session.stderr, '', `session-${opt} unexpected stderr: ${session.stderr}`);
  const logBytes = fs.readFileSync(logPath);
  const totalChecks = Object.values(counts).reduce((a, b) => a + b, 0);
  return {
    opt,
    exit_status: exit.code,
    submissions: session.submissions,
    retirements: session.retirements,
    max_alloc_bytes: session.maxAllocBytes,
    max_steps: session.maxSteps,
    total_checks: totalChecks,
    counts,
    log_file: path.basename(logPath),
    log_sha256: digest(logBytes),
  };
}

// Guest-execution authenticity control: mutating `+` to `-` in a temporary copy of
// exact-mag-add changes 10 + 3 from 13 to 7, proving arithmetic executes in guest Wasm.
function runAuthenticityMutant() {
  const frame = bytes => Buffer.concat([Buffer.from(`${bytes.length}\n`), bytes]);
  const original = moduleSources.get('exact-mag-add.noble');
  const mutated = original.replace('+ + dup 1000000000', '- + dup 1000000000');
  assert.notEqual(mutated, original, 'mutant substitution did not apply');
  const query = Buffer.from('0 10 nil cons 3 nil cons exact_mag_add@1.mag_add_carry');
  const runOnce = modText => {
    const input = Buffer.concat([Buffer.from(modText, 'utf8'), query].map(frame));
    const res = spawnSync(binary, [
      'session', '--framed', '--declared-modules', '--text-byte-cursor',
      '--signed-self-recursion', '--opt', 'off',
    ], { input, encoding: null, timeout: 60_000, env: { PATH: '/nonexistent', LANG: 'C', LC_ALL: 'C' } });
    assert.equal(res.status, 0);
    const reports = res.stdout.toString('utf8').trim().split('\n').map(l => JSON.parse(l));
    assert.equal(reports[0].outcome, 'linked');
    assert.equal(reports[1].outcome, 'normal');
    return Number(reports[1].stack[0].value[0].value);
  };
  const cleanVal = runOnce(original);
  const mutantVal = runOnce(mutated);
  assert.equal(cleanVal, 13);
  assert.equal(mutantVal, 7);
  return { clean_10_plus_3: cleanVal, mutant_10_plus_3: mutantVal, passed: true };
}

// Emitted WAT/Wasm verification for both --opt off and --opt on.
async function runEmitCheck(opt) {
  const emitDir = path.join(outputDir, `emit-${opt}`);
  const logPath = path.join(outputDir, `emit-${opt}.jsonl`);
  const session = new GuestSession(opt, logPath, emitDir);
  await session.linkAll();
  const { report } = await session.evalAndRetire(
    '0 999999999 999999999 nil cons cons 1 nil cons exact_mag_add@1.mag_add_carry', 1);
  const exit = await session.close();
  assert.equal(exit.status ?? exit.code, 0);
  const stem = report.module.stem;
  const watPath = path.join(emitDir, 'engine', `${stem}.wat`);
  const wasmPath = path.join(emitDir, 'engine', `${stem}${report.module.optimized ? '-optimized' : ''}.wasm`);
  const srcPath = path.join(emitDir, 'engine', `${stem}.noble`);
  const wat = fs.readFileSync(watPath);
  const wasm = fs.readFileSync(wasmPath);
  const src = fs.readFileSync(srcPath);
  assert.equal(digest(wat), report.module.wat_sha256);
  assert.equal(digest(wasm), report.module.wasm_sha256);
  assert.equal(digest(src), report.module.source_sha256);
  assert.ok(report.module.imports.every(e => e.kind !== 'function'));
  return {
    opt,
    stem,
    wat_sha256: digest(wat),
    wasm_sha256: digest(wasm),
    source_sha256: digest(src),
    imports_count: report.module.imports.length,
    function_imports: 0,
  };
}

const [optOff, optOn] = await Promise.all([runSuite('off'), runSuite('on')]);
assert.deepEqual(optOff.counts, optOn.counts, 'opt-off and opt-on check counts must match');
const [emitOff, emitOn] = await Promise.all([runEmitCheck('off'), runEmitCheck('on')]);
const mutant = runAuthenticityMutant();

const receipt = {
  schema: 'noble-exact-current28-gate/v1',
  profile: 'Signed-Declared-Text-v1',
  ownership: 'ma-language-prerequisites task 4 (OWNER DECISION 2026-11-20 item 16 and 2026-11-23 item 58)',
  binary_sha256: expectedDigest,
  gate_sha256: digest(fs.readFileSync(new URL(import.meta.url))),
  oracle_sha256: digest(fs.readFileSync(new URL('./oracle.mjs', import.meta.url))),
  module_digests: moduleDigests,
  runs: { off: optOff, on: optOn },
  emit_verification: { off: emitOff, on: emitOn },
  authenticity_mutant: mutant,
  not_claimed: [
    'CALC-01 through CALC-13 conformance promotion (CALC-13 remains downstream of measured acceptance)',
    'Worker selected-vendor acceptance or combined signed-epoch/live first-guest sweep',
    'tail-position recursion reuse beyond the 512-frame continuation bound',
  ],
};

const receiptPath = path.join(outputDir, 'receipt.json');
const receiptBytes = Buffer.from(`${JSON.stringify(receipt, null, 2)}\n`, 'utf8');
fs.writeFileSync(receiptPath, receiptBytes);

console.log(JSON.stringify({
  status: 'PASS',
  receipt_path: receiptPath,
  receipt_sha256: digest(receiptBytes),
  opt_off_checks: optOff.total_checks,
  opt_on_checks: optOn.total_checks,
  opt_off_submissions: optOff.submissions,
  opt_on_submissions: optOn.submissions,
  opt_off_retirements: optOff.retirements,
  opt_on_retirements: optOn.retirements,
  authenticity_mutant: mutant,
}, null, 2));
