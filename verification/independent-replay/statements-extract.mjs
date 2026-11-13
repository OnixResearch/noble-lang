#!/usr/bin/env node
// Shared statement extraction for the issue #62 replay lanes.
//
// `statements.lean` prints each root's canonical statement from Noble's compiled environment through
// Lean's own `Expr`, never from a lean4export file. `binding.mjs` compares the committed manifest to
// this extraction; `multi.mjs` does the same before binding any checker verdict to a statement.
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const extractorPath = path.join(path.dirname(fileURLToPath(import.meta.url)), 'statements.lean');

/** Canonical statements of `roots`, keyed by root, plus how long the extraction took. */
export function leanStatements({ leanBin, project, roots }) {
  const childEnv = {
    HOME: process.env.HOME ?? '/', LANG: 'C.UTF-8',
    PATH: [leanBin, '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
    TMPDIR: process.env.TMPDIR,
  };
  const started = process.hrtime.bigint();
  const result = spawnSync(path.join(leanBin, 'lake'), ['env', 'lean', extractorPath], {
    cwd: project, env: { ...childEnv, NOBLE_REPLAY_ROOTS: roots.join(' ') }, encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024, timeout: 1_800_000, killSignal: 'SIGKILL',
  });
  if (result.status !== 0) {
    throw new Error(`statement extraction exited ${result.status}: ${`${result.stdout}${result.stderr}`.slice(-2000)}`);
  }
  const statements = new Map();
  for (const line of result.stdout.split('\n')) {
    const [tag, root, text] = line.split('\t');
    if (tag === 'STATEMENT') statements.set(root, text);
  }
  const absent = roots.filter((root) => !statements.has(root));
  if (absent.length > 0) throw new Error(`no statement printed for ${absent.join(', ')}`);
  return { statements, elapsed_ms: Number((process.hrtime.bigint() - started) / 1_000_000n) };
}
