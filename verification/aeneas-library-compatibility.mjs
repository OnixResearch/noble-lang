// The Aeneas Lean library compatibility block (the VT-PIN-02 exception of issue #86).
//
// A lane that compiles an archived generated subject cannot change its bytes. The selected Aeneas Lean
// library (`sources.aeneas_lean`) no longer provides some names such a subject uses; the current kernel
// restates them in one reviewed block at the end of proofs/m3/NobleKernel/TypesExternal.lean. The lane
// appends exactly that block to its copy of the archived TypesExternal.lean and records the block's digest.
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { digest } from './foundation/execution.mjs';

export const compatibilitySource = 'proofs/m3/NobleKernel/TypesExternal.lean';
const marker = '/-! ## Aeneas library compatibility (#86)';

/** The block's bytes and digest, read from the current kernel; a missing or repeated block is refused. */
export function libraryCompatibility(root) {
  const text = readFileSync(join(root, compatibilitySource), 'utf8');
  const start = text.indexOf(marker);
  if (start < 0 || text.indexOf(marker, start + 1) >= 0) {
    throw new Error(`${compatibilitySource}: missing or repeated library compatibility block`);
  }
  const bytes = Buffer.from(text.slice(start));
  return { source: compatibilitySource, sha256: digest(bytes), bytes };
}

/** Append the block to a project's copy of an archived TypesExternal.lean. */
export function appendCompatibility(file, compatibility) {
  writeFileSync(file, Buffer.concat([readFileSync(file), Buffer.from('\n'), compatibility.bytes]));
}

/** The recorded bytes of a kernel file that ends with exactly the appended block, or null otherwise. */
export function withoutCompatibility(bytes, compatibility) {
  const suffix = Buffer.concat([Buffer.from('\n'), compatibility.bytes]);
  if (bytes.length < suffix.length || !bytes.subarray(bytes.length - suffix.length).equals(suffix)) return null;
  return bytes.subarray(0, bytes.length - suffix.length);
}
