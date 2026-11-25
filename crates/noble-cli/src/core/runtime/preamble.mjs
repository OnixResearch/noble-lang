#!/usr/bin/env node
// Resource-free execution shell. Recipes are observed, never used to select code.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';

const MAX_FRAME = 4 * 1024 * 1024;
const MAX_OBSERVATION_WORK = 100000;
const utf8 = new TextDecoder('utf-8', { fatal: true });
const sha256 = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const names = ['dup', 'drop', 'swap', 'dip', 'i64.add', 'i64.sub', 'i64.mul', 'i64.eq',
  'quote', 'compose', 'run', 'reflect', 'unit', 'pair', 'unpair', 'inl', 'inr', 'case',
  'if', 'nil', 'cons', 'list.case', 'test.emit', 'test.abort', 'self.propose', 'self.generation',
  'text.byte', 'iterate', 'i64.cmp', 'i64.div-rem', 'text.from-ascii', 'text.length'];
const checkedI64Names = ['i64.checked-add', 'i64.checked-sub', 'i64.checked-mul',
  'i64.checked-neg', 'i64.checked-abs', 'i64.lt', 'i64.checked-div-rem'];
const fail = message => { throw Error(message); };
const integer = (value, maximum, label) => {
  if (!Number.isSafeInteger(value) || value < 0 || value > maximum) fail(`invalid ${label}`);
  return value;
};
const isEngineOomError = error => {
  if (!error || typeof error !== 'object') return false;
  if (error.code === 'ERR_MEMORY_ALLOCATION_FAILED') return true;
  const message = String(error.message ?? '');
  if (error instanceof RangeError) {
    return /out of memory|allocation failed|could not allocate|webassembly\.memory/i.test(message);
  }
  return /out of memory|allocation failed|could not allocate/i.test(message);
};
const classifyFailure = (abi, { status = 0, quotaCode = 0, extraExhausted = null, oom = false } = {}) => {
  if (oom) return 'F4-engine-oom';
  if (status !== 1 && status !== 2) return null;
  if (extraExhausted !== null) return 'F1-logical-quota';
  return abi?.failure_class?.[String(quotaCode)] ?? null;
};

