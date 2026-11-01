#!/usr/bin/env node
// One-shot bootstrap of `reviewed.json` from a freshly measured manifest. Run once (the reviewed
// baseline), never as part of the gate: after this, every change to a link is a class that needs a
// deliberate bump of that link's reviewed revision.
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const repo = process.env.NOBLE_REFINEMENT_REPO ?? path.resolve('.');
const manifest = JSON.parse(fs.readFileSync(path.resolve(repo, 'verification/refinement-links/links.json'), 'utf8'));
const revision = execFileSync('git', ['-C', repo, 'rev-parse', '--short', 'HEAD'], { encoding: 'utf8' }).trim();
const subjectRevision = execFileSync('git', ['-C', repo, 'log', '-1', '--format=%h', '--', manifest.subject], { encoding: 'utf8' }).trim();

const links = {};
for (const link of manifest.links) {
  links[link.key] = {
    subject: manifest.subject,
    rust: link.rust,
    rustSource: link.rustSource,
    rustDigest: link.rustDigest,
    generatedDigest: link.generatedDigest,
    theorems: link.theorems,
    coverage: link.coverage,
    reviewedRevision: revision,
  };
}
const roots = {};
for (const [name, record] of Object.entries(manifest.closures ?? {})) {
  roots[name] = { subject: manifest.subject, statement: record.statement, mentions: (record.mentions ?? []).length };
}
const out = path.resolve(repo, 'verification/refinement-links/reviewed.json');
// One baseline covers every subject: link keys and root names are fully qualified, so merging is
// additive. A re-measure of one subject must not silently drop the other subject's reviewed records.
const existing = fs.existsSync(out) ? JSON.parse(fs.readFileSync(out, 'utf8')) : null;
// Records of the subject being re-measured are replaced, not merged: a stale root or link of the same
// subject would otherwise read as `removed`/`removed-root` forever. Legacy records without a subject
// are treated as the re-measured subject's, because every earlier baseline covered exactly one.
const keep = (record) => (record?.subject ?? manifest.subject) !== manifest.subject;
const retainedLinks = Object.fromEntries(Object.entries(existing?.links ?? {}).filter(([, r]) => keep(r)));
const retainedRoots = Object.fromEntries(Object.entries(existing?.roots ?? {}).filter(([, r]) => keep(r)));
const subjects = { ...(existing?.subjects ?? {}) };
subjects[manifest.subject] = { subjectRevision, rootSetRevision: revision, links: Object.keys(links).length, roots: Object.keys(roots).length };
const reviewed = {
  schema: 'noble-refinement-links-reviewed/v1',
  note: 'Measured baseline, not a human per-link review. A record means the triple (Rust token digest, generated declaration digest, root mention set) matched at reviewedRevision; it is not a refinement proof.',
  subjects,
  links: { ...retainedLinks, ...links },
  roots: { ...retainedRoots, ...roots },
};
fs.writeFileSync(out, `${JSON.stringify(reviewed, null, 2)}\n`);
console.log(JSON.stringify({ out, subject: manifest.subject, links: Object.keys(links).length, roots: Object.keys(roots).length, revision, subjectRevision, mergedLinks: Object.keys(reviewed.links).length }));
