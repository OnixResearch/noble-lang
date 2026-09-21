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
  'if', 'nil', 'cons', 'list.case', 'test.emit', 'test.abort'];
const fail = message => { throw Error(message); };
const integer = (value, maximum, label) => {
  if (!Number.isSafeInteger(value) || value < 0 || value > maximum) fail(`invalid ${label}`);
  return value;
};

