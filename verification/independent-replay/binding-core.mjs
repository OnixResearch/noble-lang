#!/usr/bin/env node
// Shared export-side statement binding and closure audit for the issue #62 replay lanes.
//
// `binding.mjs` established these checks against the compiled foundation environment; `multi.mjs`
// applies the same binding to every checker, so the reader, the canonical statement form and the
// closure walk live here once. Nothing in this module spawns a process: it reads export records and
// returns refusals, so a lane can refuse before any checker runs.
import fs from 'node:fs';
import path from 'node:path';
import { sha256 } from './pins.mjs';

export const STATEMENT_LIMIT = 1 << 20;
export const BINDER = { default: 'd', implicit: 'i', strictImplicit: 's', instImplicit: 'c' };

/** Every record of an NDJSON export, indexed: names, levels, expressions and declarations. */
export function parseExport(file) {
  const names = new Map([[0, '']]);
  const levels = new Map([[0, { zero: true }]]);
  const exprs = new Map();
  const decls = new Map();
  const axioms = [];
  let format = null;
  let records = 0;
  const nameOf = (index) => {
    if (!names.has(index)) throw new Error(`${file}: name ${index} used before it is defined`);
    return names.get(index);
  };
  for (const line of fs.readFileSync(file, 'utf8').split('\n')) {
    if (line === '') continue;
    records++;
    const record = JSON.parse(line);
    if (record.meta) format = record.meta.format?.version ?? null;
    else if ('in' in record) {
      const part = record.str ?? record.num;
      const prefix = nameOf(part.pre);
      const leaf = record.str ? part.str : String(part.i);
      names.set(record.in, prefix ? `${prefix}.${leaf}` : leaf);
    } else if ('il' in record) levels.set(record.il, record);
    else if ('ie' in record) exprs.set(record.ie, record);
    else if (record.inductive) {
      const group = { record: record.inductive, members: [], exprs: [] };
      for (const kind of ['types', 'ctors', 'recs']) {
        for (const entry of record.inductive[kind] ?? []) {
          group.members.push(nameOf(entry.name));
          group.exprs.push(entry.type, ...(entry.rules ?? []).map((rule) => rule.rhs));
        }
      }
      for (const member of group.members) decls.set(member, { kind: 'inductive', group });
    } else {
      for (const kind of ['axiom', 'def', 'thm', 'opaque', 'quot']) {
        const decl = record[kind];
        if (!decl) continue;
        const name = nameOf(decl.name);
        decls.set(name, { kind, record: decl, exprs: 'value' in decl ? [decl.type, decl.value] : [decl.type] });
        if (kind === 'axiom') axioms.push(name);
      }
    }
  }
  return { file, names, levels, exprs, decls, axioms, format, records };
}

export function levelText(parsed, index) {
  const level = parsed.levels.get(index);
  if (!level) throw new Error(`level ${index} is not defined`);
  if (level.zero) return '0';
  if ('succ' in level) return `(s ${levelText(parsed, level.succ)})`;
  if ('max' in level) return `(max ${levelText(parsed, level.max[0])} ${levelText(parsed, level.max[1])})`;
  if ('imax' in level) return `(imax ${levelText(parsed, level.imax[0])} ${levelText(parsed, level.imax[1])})`;
  return `(u ${parsed.names.get(level.param)})`;
}

/** The canonical statement form `statements.lean` prints, rendered from export records. */
export function exportStatement(parsed, root) {
  const decl = parsed.decls.get(root);
  let length = 0;
  const text = (index) => {
    const e = parsed.exprs.get(index);
    if (!e) throw new Error(`expression ${index} is not defined`);
    const piece = (() => {
      if ('bvar' in e) return `#${e.bvar}`;
      if ('sort' in e) return `(sort ${levelText(parsed, e.sort)})`;
      if (e.const) return `(c ${parsed.names.get(e.const.name)}${e.const.us.map((u) => ` ${levelText(parsed, u)}`).join('')})`;
      if (e.app) return `(@ ${text(e.app.fn)} ${text(e.app.arg)})`;
      if (e.lam) return `(fun ${BINDER[e.lam.binderInfo]} ${text(e.lam.type)} ${text(e.lam.body)})`;
      if (e.forallE) return `(pi ${BINDER[e.forallE.binderInfo]} ${text(e.forallE.type)} ${text(e.forallE.body)})`;
      if (e.letE) return `(let ${text(e.letE.type)} ${text(e.letE.value)} ${text(e.letE.body)})`;
      if ('natVal' in e) return `(nat ${e.natVal})`;
      if ('strVal' in e) return `(str ${Buffer.from(e.strVal, 'utf8').toString('hex')})`;
      if (e.mdata) return text(e.mdata.expr);
      if (e.proj) return `(proj ${parsed.names.get(e.proj.typeName)} ${e.proj.idx} ${text(e.proj.struct)})`;
      throw new Error(`unknown expression ${JSON.stringify(e)}`);
    })();
    length += piece.length;
    if (length > STATEMENT_LIMIT) throw new Error(`${root}: statement exceeds ${STATEMENT_LIMIT} bytes`);
    return piece;
  };
  const params = decl.kind === 'inductive' ? [] : decl.record.levelParams.map((n) => parsed.names.get(n));
  return `(stmt [${params.join(' ')}] ${text(decl.kind === 'inductive' ? NaN : decl.record.type)})`;
}

/** Constants referenced by `roots`, transitively, and the referenced names the export lacks. */
export function closureOf(parsed, roots) {
  const reachable = new Set();
  const missing = new Set();
  const visitedExprs = new Set();
  const visitedGroups = new Set();
  const queue = [...roots];
  while (queue.length > 0) {
    const name = queue.pop();
    if (reachable.has(name) || missing.has(name)) continue;
    const decl = parsed.decls.get(name);
    if (!decl) {
      missing.add(name);
      continue;
    }
    let starts = decl.exprs ?? [];
    if (decl.kind === 'inductive') {
      for (const member of decl.group.members) reachable.add(member);
      starts = visitedGroups.has(decl.group) ? [] : decl.group.exprs;
      visitedGroups.add(decl.group);
    } else reachable.add(name);
    const stack = [...starts];
    while (stack.length > 0) {
      const index = stack.pop();
      if (visitedExprs.has(index)) continue;
      visitedExprs.add(index);
      const e = parsed.exprs.get(index);
      if (!e) throw new Error(`expression ${index} is not defined`);
      if (e.const) queue.push(parsed.names.get(e.const.name));
      else if (e.app) stack.push(e.app.fn, e.app.arg);
      else if (e.lam) stack.push(e.lam.type, e.lam.body);
      else if (e.forallE) stack.push(e.forallE.type, e.forallE.body);
      else if (e.letE) stack.push(e.letE.type, e.letE.value, e.letE.body);
      else if (e.mdata) stack.push(e.mdata.expr);
      else if (e.proj) {
        queue.push(parsed.names.get(e.proj.typeName));
        stack.push(e.proj.struct);
      }
    }
  }
  return { reachable, missing };
}

/** Merkle digest of every expression (binder names and mdata dropped), then of a declaration set. */
export function closureDigest(parsed, reachable) {
  const digests = new Map();
  const digest = (index) => {
    if (digests.has(index)) return digests.get(index);
    const e = parsed.exprs.get(index);
    let fields;
    if ('bvar' in e) fields = ['bvar', String(e.bvar)];
    else if ('sort' in e) fields = ['sort', levelText(parsed, e.sort)];
    else if (e.const) fields = ['const', parsed.names.get(e.const.name), ...e.const.us.map((u) => levelText(parsed, u))];
    else if (e.app) fields = ['app', digest(e.app.fn), digest(e.app.arg)];
    else if (e.lam) fields = ['lam', e.lam.binderInfo, digest(e.lam.type), digest(e.lam.body)];
    else if (e.forallE) fields = ['pi', e.forallE.binderInfo, digest(e.forallE.type), digest(e.forallE.body)];
    else if (e.letE) fields = ['let', digest(e.letE.type), digest(e.letE.value), digest(e.letE.body)];
    else if ('natVal' in e) fields = ['nat', e.natVal];
    else if ('strVal' in e) fields = ['str', e.strVal];
    else if (e.mdata) return digest(e.mdata.expr);
    else if (e.proj) fields = ['proj', parsed.names.get(e.proj.typeName), String(e.proj.idx), digest(e.proj.struct)];
    const result = sha256(fields.join('\u0000'));
    digests.set(index, result);
    return result;
  };
  // Expressions are defined before use, so digesting in index order never recurses deeply.
  for (const index of [...parsed.exprs.keys()].sort((a, b) => a - b)) digest(index);
  const lines = [];
  for (const name of reachable) {
    const decl = parsed.decls.get(name);
    if (decl.kind === 'inductive') {
      for (const kind of ['types', 'ctors', 'recs']) {
        for (const entry of decl.group.record[kind] ?? []) {
          if (parsed.names.get(entry.name) !== name) continue;
          const rules = (entry.rules ?? []).map((rule) => `${parsed.names.get(rule.ctor)}:${rule.nfields}:${digest(rule.rhs)}`);
          lines.push([kind, name, entry.levelParams.map((n) => parsed.names.get(n)).join(','), digest(entry.type),
            rules.join(',')].join('\t'));
        }
      }
    } else {
      lines.push([decl.kind, name, decl.record.levelParams.map((n) => parsed.names.get(n)).join(','),
        ...decl.exprs.map(digest)].join('\t'));
    }
  }
  lines.sort();
  return { declarations: lines.length, sha256: sha256(lines.join('\n')) };
}

/** Removes the one declaration record that declares `name`, leaving every other record. */
export function dropDeclaration(id, source, name, work) {
  const parsed = parseExport(source);
  const lines = fs.readFileSync(source, 'utf8').split('\n').filter((line) => line !== '');
  const kept = lines.filter((line) => {
    const record = JSON.parse(line);
    return !['axiom', 'def', 'thm', 'opaque'].some((kind) => record[kind] && parsed.names.get(record[kind].name) === name);
  });
  if (kept.length !== lines.length - 1) throw new Error(`${source}: ${name} is not declared exactly once`);
  const file = path.join(work, `${id}.ndjson`);
  fs.mkdirSync(work, { recursive: true });
  fs.writeFileSync(file, `${kept.join('\n')}\n`);
  return file;
}

/** Renames the theorem `from` to `to` under fresh name records, changing nothing else. */
export function renameTheorem(id, source, from, to, work) {
  const lines = fs.readFileSync(source, 'utf8').split('\n').filter((line) => line !== '');
  const records = lines.map((line) => JSON.parse(line));
  const parsed = parseExport(source);
  let fresh = Math.max(...records.filter((record) => 'in' in record).map((record) => record.in)) + 1;
  const added = [];
  let prefix = 0;
  for (const part of to.split('.')) {
    added.push(JSON.stringify({ in: fresh, str: { pre: prefix, str: part } }));
    prefix = fresh++;
  }
  const at = records.findIndex((record) => record.thm && parsed.names.get(record.thm.name) === from);
  if (at < 0) throw new Error(`${source}: no theorem ${from}`);
  const old = records[at].thm.name;
  records[at].thm.name = prefix;
  records[at].thm.all = records[at].thm.all.map((n) => (n === old ? prefix : n));
  const out = [...lines.slice(0, at), ...added, JSON.stringify(records[at]), ...lines.slice(at + 1)];
  const file = path.join(work, `${id}.ndjson`);
  fs.mkdirSync(work, { recursive: true });
  fs.writeFileSync(file, `${out.join('\n')}\n`);
  return file;
}
