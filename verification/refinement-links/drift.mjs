#!/usr/bin/env node
// Issue #81 drift classification for the Charon->Aeneas refinement links.
//
// Compares the current manifest (`links.mjs`) against the reviewed record and classifies every link.
// Every class except `unchanged` fails the gate until the link's reviewed revision is bumped, which
// is what makes the bump a review record rather than a rubber stamp.
//
// The classes, and why each exists:
// - unchanged          current digests equal the reviewed ones.
// - rust-changed       the Rust body's token digest differs.
// - generated-changed  same Rust tokens, different generated declaration text: tool drift.
// - proof-changed      the root set or a root's statement/closure differs.
// - unlinked           an extracted in-tree function that no declared root mentions.
// - orphaned           a root mentions a generated constant that no longer exists.
// - closure-unavailable  the environment that would answer "which theorem mentions this" does not
//                        compile. This is not unlinked: nobody could look. It fails the gate too,
//                        because a green lane would be a lie.
// - external-source    the translator recorded a source outside this repository; there is no body
//                        here to digest, so the link is recorded without a Rust digest and never
//                        counted as covered.
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(process.env.NOBLE_REFINEMENT_REPO ?? path.resolve(here, '../..'));
const option = (name) => {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
};

const manifestFile = path.resolve(repoRoot, option('--manifest') ?? 'verification/refinement-links/links.json');
const reviewedFile = path.resolve(repoRoot, option('--reviewed') ?? 'verification/refinement-links/reviewed.json');
const reportFile = option('--report');

const manifest = JSON.parse(fs.readFileSync(manifestFile, 'utf8'));
const reviewed = fs.existsSync(reviewedFile)
  ? JSON.parse(fs.readFileSync(reviewedFile, 'utf8'))
  : { schema: 'noble-refinement-links-reviewed/v1', links: {}, roots: {} };

const reviewedLinks = reviewed.links ?? {};
const reviewedRoots = reviewed.roots ?? {};
const currentLinks = new Map(manifest.links.map((link) => [link.key, link]));
const currentRoots = manifest.closures ?? {};

const findings = [];
const add = (link, classification, detail) => findings.push({ link, classification, ...detail });

for (const link of manifest.links) {
  const reviewedLink = reviewedLinks[link.key];
  if (link.rustSource === 'external') {
    add(link.key, 'external-source', { rust: link.rust, reason: 'recorded source is outside this repository' });
    continue;
  }
  if (!reviewedLink) {
    add(link.key, 'unreviewed', { rust: link.rust, reason: 'no reviewed record for this link' });
    continue;
  }
  const classes = [];
  if (link.rustDigest !== null && link.rustDigest !== reviewedLink.rustDigest) classes.push('rust-changed');
  if (link.generatedDigest !== reviewedLink.generatedDigest) classes.push('generated-changed');
  const reviewedTheorems = (reviewedLink.theorems ?? []).join(' ');
  if (reviewedTheorems !== link.theorems.join(' ')) classes.push('proof-changed');
  if (classes.length === 0 && link.coverage !== 'closure-unavailable' && link.coverage !== reviewedLink.coverage) {
    classes.push(link.coverage === 'linked' ? 'newly-linked' : 'unlinked');
  }
  if (classes.length === 0 && link.coverage === 'unlinked' && (reviewedLink.coverage ?? '') !== 'unlinked') {
    classes.push('unlinked');
  }
  if (classes.length === 0 && link.coverage === 'closure-unavailable') {
    classes.push('closure-unavailable');
  }
  for (const classification of classes.length === 0 ? ['unchanged'] : classes) {
    add(link.key, classification, {
      rust: link.rust,
      rustDigest: link.rustDigest,
      reviewedRustDigest: reviewedLink.rustDigest ?? null,
      generatedDigest: link.generatedDigest,
      reviewedGeneratedDigest: reviewedLink.generatedDigest ?? null,
      theorems: link.theorems,
      reviewedTheorems: reviewedLink.theorems ?? [],
    });
  }
}

for (const pair of manifest.orphaned) add(pair.missing, 'orphaned', { root: pair.root });

// A reviewed link that the manifest no longer contains has vanished, not passed: the generated
// declaration may have been renamed, dropped by the translator, or lost by an unparsable edit. The
// gate must fail rather than count the shrunken manifest as clean.
for (const key of Object.keys(reviewedLinks)) {
  if (!currentLinks.has(key) && !findings.some((finding) => finding.link === key && finding.classification === 'removed')) {
    add(key, 'removed', { reason: 'reviewed link absent from the current manifest' });
  }
}

for (const [name, record] of Object.entries(currentRoots)) {
  const reviewedRoot = reviewedRoots[name];
  if (!reviewedRoot) {
    add(name, 'unreviewed-root', { reason: 'root is declared but has no reviewed statement digest' });
    continue;
  }
  if (reviewedRoot.statement !== record.statement) {
    add(name, 'proof-changed', { root: name, reason: 'root statement differs from the reviewed statement' });
  }
}

const failing = findings.filter((finding) => finding.classification !== 'unchanged');
const counts = findings.reduce((accumulator, finding) => {
  accumulator[finding.classification] = (accumulator[finding.classification] ?? 0) + 1;
  return accumulator;
}, {});
// Coverage is measured and reported, never a pass/fail class on its own: a 1023-function extraction
// does not become a lie because a function has no root yet, and a gate that can only go green once
// every function is linked would be either never green or bumped without reading. What fails is
// drift *away* from the reviewed record (in either direction), plus an unavailable closure.
const coverage = manifest.links.reduce((accumulator, link) => {
  const bucket = link.rustSource === 'external' ? 'external'
    : link.rustSource === 'missing-in-tree' ? 'missing-in-tree'
    : link.coverage;
  accumulator[bucket] = (accumulator[bucket] ?? 0) + 1;
  return accumulator;
}, {});
const report = {
  schema: 'noble-refinement-links-drift/v1',
  subject: manifest.subject,
  rootsDeclared: manifest.rootSet.declared.length,
  links: manifest.links.length,
  coverage,
  auxiliaryMentions: manifest.auxiliaryMentions ?? 0,
  typeMentions: manifest.typeMentions ?? 0,
  unsourcedDefinitions: (manifest.unsourcedDefinitions ?? []).length,
  counts,
  failing: failing.length,
  findings: failing,
};

if (reportFile) fs.writeFileSync(path.resolve(repoRoot, reportFile), `${JSON.stringify(report, null, 2)}\n`);
console.log(JSON.stringify({ schema: report.schema, subject: report.subject, coverage, counts, failing: report.failing }));
process.exitCode = failing.length === 0 ? 0 : 1;
