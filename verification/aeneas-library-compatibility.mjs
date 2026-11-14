// Archived M5 proof lanes rebuild retained generated subjects from the older Aeneas extractor.
// Their temporary projects append this compatibility block; current extracted subjects do not use it.
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { digest } from './foundation/execution.mjs';

export const compatibilitySource = 'verification/m5-archived-library-compatibility.lean';
const marker = '/-! ## Aeneas library compatibility (#97 archived M5 input only)';

/** The archive-only block's bytes and digest; a missing or repeated block is refused. */
export function libraryCompatibility(root) {
  const text = readFileSync(join(root, compatibilitySource), 'utf8');
  const start = text.indexOf(marker);
  if (start < 0 || text.indexOf(marker, start + 1) >= 0) {
    throw new Error(`${compatibilitySource}: missing or repeated library compatibility block`);
  }
  const bytes = Buffer.from(text.slice(start));
  return { source: compatibilitySource, sha256: digest(bytes), bytes };
}

/** Append the block to a temporary archived M5 project's TypesExternal.lean. */
export function appendCompatibility(file, compatibility) {
  writeFileSync(file, Buffer.concat([readFileSync(file), Buffer.from('\n'), compatibility.bytes]));
}
