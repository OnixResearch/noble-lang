#!/usr/bin/env node
// Issue #81 refinement-link manifest for the Charon->Aeneas route.
//
// Generated, never hand-maintained, per VT-CI-05: every extracted function in one generated module
// records its Rust identity, its generated Lean identity and the handwritten theorems whose
// transitive closure mentions the generated constant. `drift.mjs` compares this manifest against
// the reviewed record and classifies each link.
//
// Decisions this file makes, and why:
// - The Rust body digest is TOKEN-based: comments are dropped and every whitespace run collapses
//   to one space before hashing. A formatting-only or comment-only edit therefore does not change
//   it, while any token change (an operator, literal, name, lifetime) does. A MIR/LLBC digest would
//   bind formatting to the pinned Charon and hide source edits behind it; this lane wants source
//   edits visible and formatting invisible.
// - The generated digest is the text of the generated declaration from its `[rust-path]:` header
//   to the next header, so a translator change with identical Rust is detected.
// - The theorem set comes from the compiled Lean environment (`closure.lean`), never from grep:
//   each root's own type and value nest is walked transitively and only `NobleKernel.*` constants
//   are kept. The root set itself is declared in `roots.json`, because which theorems count as
//   "the" refinement roots is an owner decision this lane must not make silently.
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(process.env.NOBLE_REFINEMENT_REPO ?? path.resolve(here, '../..'));

const sha256 = (data) => crypto.createHash('sha256').update(data).digest('hex');

const option = (name) => {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
};

const generatedFile = path.resolve(
  repoRoot,
  option('--generated') ?? 'proofs/m3/NobleKernel/Funs.lean',
);
const rootsFile = path.resolve(repoRoot, option('--roots') ?? 'verification/refinement-links/roots.json');
const closureFile = option('--closure');
const output = path.resolve(repoRoot, option('--out') ?? 'verification/refinement-links/links.json');

/** Drop Rust comments, keeping string and char literals verbatim, then collapse whitespace. */
export function tokenText(source) {
  const kept = [];
  let index = 0;
  while (index < source.length) {
    const current = source[index];
    const next = source[index + 1];
    if (current === '/' && next === '/') {
      index += 2;
      while (index < source.length && source[index] !== '\n') index += 1;
      kept.push(' ');
      continue;
    }
    if (current === '/' && next === '*') {
      let depth = 1;
      index += 2;
      while (index < source.length && depth > 0) {
        if (source[index] === '/' && source[index + 1] === '*') {
          depth += 1;
          index += 2;
          continue;
        }
        if (source[index] === '*' && source[index + 1] === '/') {
          depth -= 1;
          index += 2;
          continue;
        }
        index += 1;
      }
      kept.push(' ');
      continue;
    }
    if (current === '"' || current === "'") {
      const quote = current;
      let literal = quote;
      index += 1;
      while (index < source.length) {
        literal += source[index];
        if (source[index] === '\\') {
          index += 1;
          if (index < source.length) literal += source[index];
          index += 1;
          continue;
        }
        const closing = source[index] === quote;
        index += 1;
        if (closing) break;
      }
      kept.push(literal);
      continue;
    }
    kept.push(current);
    index += 1;
  }
  return kept.join('').replace(/\s+/gu, ' ').trim();
}

export const tokenDigest = (source) => sha256(tokenText(source));

/** Rust body digest for one generated `Source:` span, inclusive of its first line, exclusive end. */
// Charon records absolute paths from the checkout that produced the extraction. Mapping that
// checkout prefix into `repoRoot` lets the drift controls run on a scratch copy of the tree with
// `NOBLE_REFINEMENT_REPO`, instead of editing the real source.
const rustRoot = option('--rust-root') ?? process.env.NOBLE_REFINEMENT_RUST_ROOT ?? '';
const rebased = (file) =>
  rustRoot !== '' && file.startsWith(rustRoot)
    ? path.join(repoRoot, path.relative(rustRoot, file))
    : path.resolve(repoRoot, file);

export function rustDigest(file, span) {
  const absolute = rebased(file);
  if (!fs.existsSync(absolute)) return { file, digest: null, missing: true };
  const lines = fs.readFileSync(absolute, 'utf8').split('\n');
  const body = lines.slice(span.startLine - 1, span.endLine).join('\n');
  return { file, digest: tokenDigest(body), missing: false };
}

const headerPattern = /^\/-- \[(?<rust>[^\]]+)\]:$/u;
const sourcePattern = /^\s+Source: '(?<file>[^']+)', lines (?<startLine>\d+):(?<startColumn>\d+)-(?<endLine>\d+):(?<endColumn>\d+) -\/$/u;
const declarationPattern = /^(?:@\[[^\]]*\]\s*)?(?:def|abbrev|opaque|theorem|inductive)\s+(?<name>[^\s:(]+)/u;

/** Every generated declaration that carries an Aeneas source header, in file order. */
export function parseGenerated(text) {
  const lines = text.split('\n');
  const entries = [];
  let pending = null;
  // Full constant names carry every enclosing namespace, so the declaration's own text (which is
  // relative to that namespace) is not a key that a closure mention can match.
  const namespaces = [];
  const fullName = (declaration) =>
    namespaces.length === 0 ? declaration : `${namespaces.join('.')}.${declaration}`;
  for (const [index, line] of lines.entries()) {
    const namespace = /^namespace\s+(?<name>\S+)\s*$/u.exec(line);
    if (namespace) {
      namespaces.push(namespace.groups.name);
      continue;
    }
    const end = /^end(?:\s+(?<name>\S+))?\s*$/u.exec(line);
    if (end && namespaces.length > 0) {
      namespaces.pop();
      continue;
    }
    const header = headerPattern.exec(line);
    if (header) {
      pending = { rust: header.groups.rust, headerLine: index + 1 };
      continue;
    }
    if (!pending) continue;
    const source = sourcePattern.exec(line);
    if (source) {
      pending.source = {
        file: source.groups.file,
        startLine: Number(source.groups.startLine),
        startColumn: Number(source.groups.startColumn),
        endLine: Number(source.groups.endLine),
        endColumn: Number(source.groups.endColumn),
      };
      continue;
    }
    if (line.startsWith('/--')) continue;
    const declaration = declarationPattern.exec(line);
    if (!declaration) continue;
    let endIndex = index;
    while (endIndex + 1 < lines.length) {
      const next = lines[endIndex + 1];
      if (headerPattern.test(next) || next.startsWith('namespace ') || next.startsWith('section ')) break;
      endIndex += 1;
    }
    entries.push({
      rust: pending.rust,
      lean: fullName(declaration.groups.name),
      headerLine: pending.headerLine,
      generatedSpan: `${pending.headerLine}-${endIndex + 1}`,
      source: pending.source ?? null,
      generatedDigest: sha256(lines.slice(pending.headerLine - 1, endIndex + 1).join('\n')),
    });
    pending = null;
  }
  return entries;
}

const generatedText = fs.readFileSync(generatedFile, 'utf8');
const functions = parseGenerated(generatedText).map((entry) => {
  const rust = entry.source ? rustDigest(entry.source.file, entry.source) : { digest: null, missing: true };
  // A source path outside this repository (the Rust standard library, or an external crate the
  // translator recorded) has no body here to digest. That is a property of the subject, not an
  // unlinked function, and it must never be folded into `missing-source`.
  const external = rust.missing && entry.source !== null && !fs.existsSync(rebased(entry.source.file)) &&
    (entry.source.file.startsWith('/rustc/') || entry.source.file.startsWith('/nix/store/'));
  return {
    key: `${entry.lean}`,
    lean: entry.lean,
    rust: entry.rust,
    rustFile: entry.source?.file ?? null,
    rustSpan: entry.source
      ? `${entry.source.startLine}:${entry.source.startColumn}-${entry.source.endLine}:${entry.source.endColumn}`
      : null,
    rustDigest: rust.digest,
    rustDigestMissing: rust.missing,
    rustSource: external ? 'external' : (rust.missing ? 'missing-in-tree' : 'in-tree'),
    generatedFile: path.relative(repoRoot, generatedFile),
    generatedSpan: entry.generatedSpan,
    generatedDigest: entry.generatedDigest,
  };
});

const roots = fs.existsSync(rootsFile) ? JSON.parse(fs.readFileSync(rootsFile, 'utf8')) : { subjects: {} };
const closureAvailable = Boolean(closureFile && fs.existsSync(closureFile));
const closure = closureAvailable ? JSON.parse(fs.readFileSync(closureFile, 'utf8')) : { roots: { records: [], declared: [] } };
const records = closure.roots?.records ?? [];
const byRoot = new Map(records.map((record) => [record.name, record]));
// The theorem names themselves exist only in a compiled environment; `roots.json` declares module
// scope, never names. Without a closure there is no root set to claim, and that absence is exactly
// what the `closure-unavailable` coverage class records.
const declaredRoots = closureAvailable ? (closure.roots?.declared ?? []) : [];
// A closure can declare any modules it likes, so the declared scope must equal the reviewed scope in
// `roots.json` for this subject. Otherwise a run could widen its own root set (and its coverage
// numbers) without any reviewed edit, which is the one thing a root-set declaration must prevent.
const subjectKey = path.relative(repoRoot, generatedFile);
const declaredScope = closure.roots?.modules ?? [];
if (closureAvailable) {
  if (roots.schema !== 'noble-refinement-links-roots/v2' || !roots.subjects) {
    console.error(`roots declaration must be schema noble-refinement-links-roots/v2 with a subjects map: ${path.relative(repoRoot, rootsFile)}`);
    process.exit(2);
  }
  const scope = roots.subjects[subjectKey];
  if (!scope) {
    console.error(`no reviewed root scope for ${subjectKey} in ${path.relative(repoRoot, rootsFile)}`);
    process.exit(2);
  }
  const declared = [...declaredScope].sort().join(',');
  const reviewed = [...scope.modules].sort().join(',');
  if (declared !== reviewed) {
    console.error(`root scope mismatch for ${subjectKey}: closure declares [${declared}], roots.json reviews [${reviewed}]`);
    process.exit(2);
  }
}

const byLean = new Map(functions.map((entry) => [entry.lean, entry]));
const linked = new Map(functions.map((entry) => [entry.lean, new Set()]));
// Lean derives constants from a declaration (`X.match_1`, `X.eq_2`, `X._proof_1_17`). A mention of
// such a derived name is not an orphan: it belongs to a link that exists. Only a mention whose
// longest declaration prefix is absent is a real orphan, e.g. a root referencing a function the
// extraction no longer generates.
const derivedFrom = (mention) => {
  const parts = mention.split('.');
  for (let take = parts.length - 1; take >= 1; take -= 1) {
    const key = parts.slice(0, take).join('.');
    if (byLean.has(key)) return key;
  }
  return null;
};
// A root can mention a generated definition the translator recorded no source span for (structure
// wrappers, field accessors, loop helpers). Those cannot be links at all, because a link needs a Rust
// identity. They are a coverage gap of the extraction inventory, not drift, and they never fail the
// gate; `drift.mjs` reports them so the gap is visible instead of silently counted as covered.
const unsourcedDefinitions = [];
let orphaned = [];
let auxiliaryMentions = 0;
let typeMentions = 0;
for (const rootName of declaredRoots) {
  const record = byRoot.get(rootName);
  if (!record) continue;
  for (const entry of record.mentions ?? []) {
    const mention = typeof entry === 'string' ? entry : entry.name;
    const kind = typeof entry === 'string' ? 'defn' : entry.kind;
    // A root mentioning an inductive type, constructor or recursor is not evidence that a lost
    // function link disappeared; those constants were never function links.
    if (kind !== 'defn' && kind !== 'opaque') {
      typeMentions += 1;
      continue;
    }
    const owner = byLean.has(mention) ? mention : derivedFrom(mention);
    if (owner === null) {
      unsourcedDefinitions.push({ root: rootName, missing: mention, kind });
      continue;
    }
    if (owner !== mention) auxiliaryMentions += 1;
    linked.get(owner).add(rootName);
  }
}

const manifest = {
  schema: 'noble-refinement-links/v1',
  subject: path.relative(repoRoot, generatedFile),
  subjectDigest: sha256(generatedText),
  rootSet: {
    file: path.relative(repoRoot, rootsFile),
    modules: closure.roots?.modules ?? [],
    declared: declaredRoots,
    rationale: roots.rationale ?? 'theorem set read from the compiled environment',
  },
  decisions: {
    rustDigest: 'source-token: comments dropped, whitespace collapsed, literals verbatim, then sha256',
    generatedDigest: 'declaration text from its rust-path header to the next header, then sha256',
    linkage: 'transitive constant closure of each declared root, read from the compiled Lean environment',
  },
  closures: Object.fromEntries(records.map((record) => [record.name, {
    statement: record.statement,
    mentions: record.mentions ?? [],
  }])),
  links: functions.map((entry) => ({
    ...entry,
    theorems: [...(linked.get(entry.lean) ?? [])].sort(),
    // Linkage is only claimed from a compiled environment. A subject whose lane does not compile
    // is `closure-unavailable`, never `unlinked`: the difference between "no theorem mentions it"
    // and "nobody could look" must stay visible.
    coverage: !closureAvailable
      ? 'closure-unavailable'
      : ((linked.get(entry.lean)?.size ?? 0) > 0 ? 'linked' : 'unlinked'),
  })),
  orphaned,
  auxiliaryMentions,
  typeMentions,
  unsourcedDefinitions,
};

fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify(manifest, null, 2)}\n`);

const counts = manifest.links.reduce((accumulator, link) => {
  accumulator[link.coverage] = (accumulator[link.coverage] ?? 0) + 1;
  return accumulator;
}, {});
console.log(JSON.stringify({
  schema: manifest.schema,
  subject: manifest.subject,
  functions: manifest.links.length,
  counts,
  declaredRoots: manifest.rootSet.declared.length,
  orphaned: orphaned.length,
  missingRustBodies: manifest.links.filter((link) => link.rustDigestMissing).length,
  output: path.relative(repoRoot, output),
}));
