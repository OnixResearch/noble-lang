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
  roots[name] = { statement: record.statement, mentions: (record.mentions ?? []).length };
}
const reviewed = {
  schema: 'noble-refinement-links-reviewed/v1',
  note: 'Bootstrap baseline measured, not a human per-link review. A record here means the triple (Rust token digest, generated declaration digest, root mention set) was reviewed at reviewedRevision; it is not a refinement proof.',
  subject: manifest.subject,
  subjectRevision,
  rootSetRevision: revision,
  links,
  roots,
};
const out = path.resolve(repo, 'verification/refinement-links/reviewed.json');
fs.writeFileSync(out, `${JSON.stringify(reviewed, null, 2)}\n`);
console.log(JSON.stringify({ out, links: Object.keys(links).length, roots: Object.keys(roots).length, revision, subjectRevision }));
