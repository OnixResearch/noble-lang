#!/usr/bin/env node
// Shared pin guard for the issue #62 replay lanes.
//
// Every lane refuses before it spawns any tool when a pinned artifact changed. The guard names the
// refusal class it enforces (`tool-pin-mismatch` for a checker, exporter or Lean binary;
// `export-pin-mismatch` for an export file) and exits 2 with `pin-refused class=<class>` on stderr,
// so `pin-controls.mjs` can assert the class of a refusal that happened through the real lane rather
// than through a reimplementation of the guard.
import crypto from 'node:crypto';
import fs from 'node:fs';

export const sha256 = (data) => crypto.createHash('sha256').update(data).digest('hex');
export const fileSha256 = (file) => sha256(fs.readFileSync(file));

/** Verify pinned artifacts. Throws a refusal carrying `pinClass`; spawns nothing. */
export function verifyPins(entries) {
  for (const entry of entries) {
    const actual = fileSha256(entry.file);
    if (actual !== entry.sha256) {
      const error = new Error(`${entry.role} ${entry.file} digest ${actual} differs from the pin ${entry.sha256}`);
      error.pinClass = entry.pinClass;
      throw error;
    }
  }
}

/** Run the guard for a lane: a pin mismatch exits 2 with its class, before any checker process. */
export function pinGuard(entries) {
  try {
    verifyPins(entries);
  } catch (error) {
    if (error.pinClass) {
      console.error(`pin-refused class=${error.pinClass} ${error.message}`);
      process.exit(2);
    }
    throw error;
  }
}
