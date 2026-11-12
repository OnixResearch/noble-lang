#!/usr/bin/env node
// Forbidden-construct scan for Lean proof sources (the m3 gates' NO-PROOF-HOLE
// check). A plain `grep -w sorry` misses `admit`, local `axiom` declarations,
// `debug.skipKernelTC` and `addDecl`, and it matches inside comments and string
// literals. This scanner masks comments and literal text while preserving line
// and column positions, then looks for whole constructs in the remaining code.
//
// Usage (from the repository root):
//   node verification/lean-source-scan.mjs [--only CONSTRUCT]... DIRECTORY_OR_FILE [...]
//
// `--only` restricts the scan to the named construct (repeatable), so a caller
// can keep a whole-tree check for `sorry` while scanning human proof sources
// for every construct (the generated kernel has data-model functions named
// `admit`, which are not proof holes).
//
// Exit 0: no forbidden construct. Exit 1: at least one hit, printed
// `path:line: construct` on stderr. Exit 2: usage error.
//
// `native_decide` is deliberately not scanned here: the gates allow it per
// declaration through the axiom audit, which names the theorem that used it.

import fs from 'node:fs';
import path from 'node:path';

// Constructs that are a whole Lean token. `admit` is a `sorry` alias, so its
// presence is a proof hole even though the kernel reports `sorryAx`.
const WORD_CONSTRUCTS = [
  ['sorry', /\bsorry\b/],
  ['admit', /\badmit\b/],
  ['axiom declaration', /\baxiom\b(?=\s)/],
  ['debug.skipKernelTC', /\bdebug\s*\.\s*skipKernelTC\b/],
  ['addDecl', /\baddDecl\b/],
];

// Replace comments and literal text with spaces, keeping every newline (and so
// every line number). String literals are masked whole, interpolation
// expressions included; an escape hatch hidden there is still caught for a
// theorem by the gates' axiom audit (`sorry`/`admit` leave `sorryAx` in the
// theorem's transitive axioms).
export function maskSource(text) {
  const out = text.split('');
  let index = 0;
  const blank = (from, to) => {
    for (let at = from; at < to && at < out.length; at += 1) if (out[at] !== '\n') out[at] = ' ';
  };
  while (index < text.length) {
    const rest = text.slice(index);
    if (rest.startsWith('/-')) {
      // Nested block comments: `/- … /- … -/ … -/`.
      let depth = 0;
      let at = index;
      while (at < text.length) {
        if (text.startsWith('/-', at)) { depth += 1; at += 2; continue; }
        if (text.startsWith('-/', at)) { depth -= 1; at += 2; if (depth === 0) break; continue; }
        at += 1;
      }
      blank(index, at);
      index = at;
      continue;
    }
    if (rest.startsWith('--')) {
      const end = text.indexOf('\n', index);
      const stop = end === -1 ? text.length : end;
      blank(index, stop);
      index = stop;
      continue;
    }
    // Raw strings: r"…", r#"…"#, r##"…"##.
    const raw = /^r(#*)"/.exec(rest);
    if (raw) {
      const open = index + 1 + raw[1].length;
      const close = text.indexOf(`"${raw[1]}`, open + 1);
      const stop = close === -1 ? text.length : close + 1 + raw[1].length;
      blank(index, stop);
      index = stop;
      continue;
    }
    if (rest[0] === '"') {
      let at = index + 1;
      while (at < text.length) {
        if (text[at] === '\\') { at += 2; continue; }
        if (text[at] === '"') { at += 1; break; }
        at += 1;
      }
      blank(index, at);
      index = at;
      continue;
    }
    // Character literals only: a prime in an identifier (`h'`) must not open one.
    if (rest[0] === "'" && /^'(?:\\.|[^'\\\n])'/.test(rest)) {
      blank(index, index + /^'(?:\\.|[^'\\\n])'/.exec(rest)[0].length);
      index += /^'(?:\\.|[^'\\\n])'/.exec(rest)[0].length;
      continue;
    }
    index += 1;
  }
  return out.join('');
}

export function scanText(text, only = []) {
  const constructs = only.length === 0 ? WORD_CONSTRUCTS
    : WORD_CONSTRUCTS.filter(([name]) => only.includes(name));
  const masked = maskSource(text);
  const source = text.split('\n');
  const hits = [];
  masked.split('\n').forEach((line, at) => {
    for (const [name, pattern] of constructs)
      if (pattern.test(line)) hits.push({ line: at + 1, construct: name, text: source[at].trim() });
  });
  return hits;
}

// The .lean sources of a directory tree, without following symlinks and
// without descending into hidden tool directories (.lake and friends). A
// broken symlink is not a source, not a crash.
function leanFiles(target) {
  const stat = fs.lstatSync(target);
  if (stat.isSymbolicLink()) return [];
  if (stat.isFile()) return target.endsWith('.lean') ? [target] : [];
  if (!stat.isDirectory()) return [];
  return fs.readdirSync(target, { withFileTypes: true })
    .filter(entry => !(entry.isDirectory() && entry.name.startsWith('.')))
    .flatMap(entry => leanFiles(path.join(target, entry.name)))
    .sort();
}

function main(argv) {
  const only = [];
  const targets = [];
  for (let at = 0; at < argv.length; at += 1) {
    if (argv[at] === '--only') {
      at += 1;
      if (at >= argv.length) { console.error('lean-source-scan: --only needs a construct name'); return 2; }
      only.push(argv[at]);
    } else if (argv[at].startsWith('--')) {
      console.error(`lean-source-scan: unknown option ${argv[at]}`);
      return 2;
    } else {
      targets.push(argv[at]);
    }
  }
  const unknown = only.filter(name => !WORD_CONSTRUCTS.some(([known]) => known === name));
  if (targets.length === 0 || unknown.length > 0) {
    console.error('usage: node verification/lean-source-scan.mjs [--only CONSTRUCT]... DIRECTORY_OR_FILE [...]');
    return 2;
  }
  const files = targets.flatMap(leanFiles);
  let failed = false;
  for (const file of files)
    for (const hit of scanText(fs.readFileSync(file, 'utf8'), only)) {
      console.error(`${file}:${hit.line}: forbidden construct '${hit.construct}': ${hit.text}`);
      failed = true;
    }
  if (failed) return 1;
  console.log(`lean-source-scan: ${files.length} sources, no forbidden construct`);
  return 0;
}

if (import.meta.main) process.exit(main(process.argv.slice(2)));
