#!/usr/bin/env node
// Shared NDJSON (lean4export format 3.x) inspection for the issue #62 replay lanes.
//
// One reader for the export's own records, so `refusals.mjs`, `limits.mjs` and `multi.mjs` classify
// the same artifact the same way: the declared names (so a root's presence is read from the export,
// never from a producer's exit status), the declared axioms and the format version.
import fs from 'node:fs';

export const STANDARD_AXIOMS = ['propext', 'Classical.choice', 'Quot.sound'];

/** Declared names, declared axioms and format version of an NDJSON export, from its own records. */
export function inspectExport(file) {
  const names = new Map([[0, '']]);
  const declared = new Set();
  const axioms = [];
  let format = null;
  let records = 0;
  const nameOf = (index) => {
    if (!names.has(index)) throw new Error(`${file}: name index ${index} used before it is defined`);
    return names.get(index);
  };
  for (const line of fs.readFileSync(file, 'utf8').split('\n')) {
    if (line === '') continue;
    records++;
    const record = JSON.parse(line);
    if (record.meta) format = record.meta.format?.version ?? null;
    if (record.str && 'in' in record) {
      const prefix = nameOf(record.str.pre);
      names.set(record.in, prefix ? `${prefix}.${record.str.str}` : record.str.str);
    }
    if (record.num && 'in' in record) {
      const prefix = nameOf(record.num.pre);
      names.set(record.in, prefix ? `${prefix}.${record.num.i}` : String(record.num.i));
    }
    for (const kind of ['axiom', 'def', 'thm', 'opaque', 'quot']) {
      if (record[kind]) declared.add(nameOf(record[kind].name));
    }
    if (record.axiom) axioms.push(nameOf(record.axiom.name));
    if (record.inductive) {
      for (const group of ['types', 'ctors', 'recs']) {
        for (const entry of record.inductive[group] ?? []) declared.add(nameOf(entry.name));
      }
    }
  }
  return { records, format, declared, axioms };
}

/** The names of `declared` axioms outside the three standard ones, in export order. */
export function foreignAxioms(inspection) {
  return inspection.axioms.filter((axiom) => !STANDARD_AXIOMS.includes(axiom));
}
