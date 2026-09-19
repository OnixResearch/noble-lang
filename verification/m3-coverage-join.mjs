// M3 per-function coverage join (task 6.1; succeeded from the M2 pair).
//
// Parses the Aeneas-generated function surface of the M3 extraction subject
// (`NobleKernel/Funs.lean` + `NobleKernel/FunsExternal.lean`) into a derived
// function list, and joins it with the reviewed classification record
// (`verification/m3-coverage.json`). Used by verification/m3-coverage-gate.sh.
//
// Modes:
//   bun m3-coverage-join.mjs derive PROOF_ROOT        -> derived list (stdout)
//   bun m3-coverage-join.mjs emit   PROOF_ROOT        -> record skeleton (stdout)
//   bun m3-coverage-join.mjs check  PROOF_ROOT RECORD -> join report (stdout);
//     exit 0 = consistent, exit 1 = named violations
//   bun m3-coverage-join.mjs leanpayload PROOF_ROOT RECORD OUT-A OUT-B
//     -> writes the constant/citation payloads consumed by the gate's two
//        Lean environment checkers (the root module and the Refinement
//        module cannot be imported together, so citations are split by
//        prefix: NobleM2.Refinement.* and NobleM3.Refine.* -> OUT-B,
//        everything else -> OUT-A)
//
// The derived list is the binding inventory: one entry per generated
// function declaration (def / impl_def) with its Rust-side name (doc header
// or rust_fun attribute) and source span. A keyword-parity check fails
// loudly if the parser misses any top-level declaration.

const NAME_RE = /^(?:noncomputable\s+)?(def|impl_def)\s+([^\s(:{][^\s(:{]*)/;
const KEYWORD_ONLY_RE = /^(?:noncomputable\s+)?(def|impl_def)\s*$/;
const STATUSES = ['extracted', 'proved', 'modeled', 'excepted', 'open'];

// Strip leading `@[ ... ]` attribute groups (possibly multi-line) and
// `set_option ... in` modifiers starting at line `k`; return the first
// content line with attributes removed.
function stripAttrs(lines, k) {
  while (k < lines.length) {
    let t = lines[k].trim();
    if (/^set_option\b.*\bin$/.test(t)) { k++; continue; }
    while (t.startsWith('@[')) {
      let depth = 0, end = -1;
      for (let p = 2; p < t.length; p++) {
        if (t[p] === '[') depth++;
        else if (t[p] === ']') {
          if (depth === 0) { end = p; break; }
          depth--;
        }
      }
      if (end === -1) {
        // the attribute continues on following lines
        let d = 0;
        for (let p = 1; p < t.length; p++) {
          if (t[p] === '[') d++;
          else if (t[p] === ']') d--;
        }
        k++;
        while (k < lines.length) {
          for (const ch of lines[k]) {
            if (ch === '[') d++;
            else if (ch === ']') d--;
          }
          if (d <= 0) break;
          k++;
        }
        t = '';
        break;
      }
      t = t.slice(end + 1).trim();
    }
    if (t !== '') return { k, rest: t };
    k++;
  }
  return null;
}

async function parseGeneratedFuns(path, nsPrefix) {
  const lines = (await Bun.file(path).text()).split('\n');
  const out = [];
  let i = 0;
  while (i < lines.length) {
    if (!lines[i].startsWith('/--') && !lines[i].startsWith('/- [')) { i++; continue; }
    const doc = [lines[i]];
    let j = i;
    while (!lines[j].trimEnd().endsWith('-/')) {
      j++;
      if (j >= lines.length) throw new Error(`${path}: unterminated doc at line ${i + 1}`);
      doc.push(lines[j]);
    }
    const joined = doc.join(' ').replace(/\s+/g, ' ');
    // Rust-side name: `/-- [rust::path]: ... -/`, `/-- [rust::path]: loop
    // body N: ... -/`, or `/-- Trait implementation: [pattern] ... -/`.
    const m = joined.match(/\/-- Trait implementation: \[([^\]]+)\]/)
           ?? joined.match(/\/-+ \[([^\]]+)\]/);
    const rustDoc = m ? m[1].replace(/: loop body \d+$/, '') : null;
    const loopBody = /: loop body \d+:/.test(joined);
    const sm = joined.match(/Source: '([^']+)', lines (\d+:\d+)-(\d+:\d+)/);
    const source = sm ? { path: sm[1], span: `${sm[2]}-${sm[3]}` } : null;
    const traitImplDoc = joined.includes('Trait implementation:');

    const hit = stripAttrs(lines, j + 1);
    if (!hit) throw new Error(`${path}: doc at line ${i + 1} attaches to no declaration`);
    let kind = null, name = null, declLine = hit.k;
    const dm = hit.rest.match(NAME_RE);
    if (dm) { kind = dm[1]; name = dm[2]; }
    else if (KEYWORD_ONLY_RE.test(hit.rest) && hit.k + 1 < lines.length) {
      // `def` keyword alone on its line; the name follows on the next line
      const kw = hit.rest.trim().match(/^(?:noncomputable\s+)?(def|impl_def)/);
      const nm = lines[hit.k + 1].trim().match(/^([^\s(:{][^\s(:{]*)/);
      if (kw && nm) { kind = kw[1]; name = nm[1]; declLine = hit.k + 1; }
    }
    if (!kind) throw new Error(
      `${path}: doc at line ${i + 1} attaches to a non-function declaration: ${hit.rest.slice(0, 60)}`);
    // rust_fun attribute (FunsExternal): between the doc and the decl.
    const attrText = lines.slice(j + 1, hit.k + 2).join(' ');
    const rf = attrText.match(/rust_fun\s+"([^"]+)"/);
    const rustAttr = rf ? rf[1] : null;
    out.push({
      lean: nsPrefix + name,
      kind,
      rust: rustAttr ?? rustDoc,
      rustDoc,
      rustAttr,
      loopBody,
      traitImpl: traitImplDoc || /rust_trait_impl/.test(joined),
      source,
      file: path.slice(path.lastIndexOf('/') + 1),
      line: declLine + 1,
    });
    i = declLine + 1;
  }
  return out;
}

// Parity: count top-level declaration keyword lines (column-0 `def` /
// `impl_def` / `noncomputable def`, including the `@[attrs] def` same-line
// form). Must equal the parsed entry count, else the parser missed something.
async function countDeclKeywords(path) {
  const lines = (await Bun.file(path).text()).split('\n');
  let n = 0;
  for (const raw of lines) {
    if (/^(noncomputable\s+)?(def|impl_def)(\s|$)/.test(raw)) n++;
    else if (/^@\[[^\]]*\]\s*(noncomputable\s+)?(def|impl_def)\s/.test(raw)) n++;
  }
  return n;
}

export async function derive(root) {
  const funs = await parseGeneratedFuns(`${root}/NobleKernel/Funs.lean`, 'noble_kernel.');
  const ext = await parseGeneratedFuns(`${root}/NobleKernel/FunsExternal.lean`, 'noble_kernel.');
  const parity = [
    { file: 'Funs.lean', parsed: funs.length, keywords: await countDeclKeywords(`${root}/NobleKernel/Funs.lean`) },
    { file: 'FunsExternal.lean', parsed: ext.length, keywords: await countDeclKeywords(`${root}/NobleKernel/FunsExternal.lean`) },
  ];
  return { funs, ext, parity };
}

function joinCheck(derived, parity, record) {
  const problems = [];
  const fail = (code, msg) => problems.push({ code, msg });

  for (const p of parity) {
    if (p.parsed !== p.keywords) {
      fail('PARSE-PARITY', `${p.file}: parsed ${p.parsed} declarations but counted ${p.keywords} top-level def/impl_def keyword lines — parser gap`);
    }
  }

  const entries = record.functions;
  if (!Array.isArray(entries)) {
    fail('RECORD-FORM', 'record has no functions array');
    return { ok: false, total: derived.length, classified: 0,
             counts: Object.fromEntries(STATUSES.map(s => [s, 0])), problems };
  }
  const seen = new Map();
  for (const e of entries) {
    if (!e.lean || typeof e.lean !== 'string') {
      fail('RECORD-FORM', `entry without a lean name: ${JSON.stringify(e).slice(0, 80)}`);
      continue;
    }
    if (seen.has(e.lean)) fail('RECORD-FORM', `duplicate entry for ${e.lean}`);
    seen.set(e.lean, e);
    if (!STATUSES.includes(e.status)) {
      fail('STATUS', `${e.lean}: invalid status ${JSON.stringify(e.status)} (must be one of ${STATUSES.join('|')})`);
    }
    if (e.status === 'proved' && (!Array.isArray(e.citations) || e.citations.length === 0)) {
      fail('STATUS', `${e.lean}: status proved requires a non-empty citations array`);
    }
    if (e.status !== 'proved' && e.citations) {
      fail('STATUS', `${e.lean}: citations present but status is ${e.status}`);
    }
    if ((e.status === 'modeled' || e.status === 'excepted') && !e.rust) {
      fail('STATUS', `${e.lean}: status ${e.status} requires the Rust-side name`);
    }
  }

  const byLean = new Map(derived.map(d => [d.lean, d]));
  for (const d of derived) {
    if (!seen.has(d.lean)) {
      fail('UNCLASSIFIED', `${d.lean} (${d.rust ?? 'no rust name'}, ${d.file}:${d.line}) is generated but has no record entry`);
    }
  }
  for (const [lean, e] of seen) {
    const d = byLean.get(lean);
    // `open` is the only status allowed to name a subject function that is
    // NOT generated (an extraction gap); everything else must be generated.
    if (!d && e.status !== 'open') {
      fail('STALE-ENTRY', `${lean} is classified in the record but not generated (stale entry)`);
    }
  }

  for (const d of derived) {
    const e = seen.get(d.lean);
    if (!e) continue;
    const inFuns = d.file === 'Funs.lean';
    if ((e.status === 'extracted' || e.status === 'proved') && !inFuns) {
      fail('STATUS-PLACEMENT', `${d.lean}: status ${e.status} requires a def in Funs.lean, found in ${d.file}`);
    }
    if ((e.status === 'modeled' || e.status === 'excepted') && inFuns) {
      fail('STATUS-PLACEMENT', `${d.lean}: status ${e.status} requires a def in FunsExternal.lean, found in ${d.file}`);
    }
    if (e.status === 'open') {
      fail('OPEN-ENTRY', `${d.lean}: marked open but the function IS generated — open is reserved for subject functions missing from the extraction output`);
    }
    // Field drift: the record must restate the current generated facts.
    if (e.kind !== d.kind) fail('DRIFT', `${d.lean}: record kind ${JSON.stringify(e.kind)} != generated ${JSON.stringify(d.kind)}`);
    if (e.rust !== d.rust) fail('DRIFT', `${d.lean}: record rust ${JSON.stringify(e.rust)} != generated ${JSON.stringify(d.rust)}`);
    if (e.file !== d.file) fail('DRIFT', `${d.lean}: record file ${JSON.stringify(e.file)} != generated ${JSON.stringify(d.file)}`);
    if (e.line !== undefined && e.line !== d.line) fail('DRIFT', `${d.lean}: record line ${e.line} != generated ${d.line}`);
    const src = d.source ? `${d.source.path} ${d.source.span}` : null;
    if (e.source !== undefined && e.source !== src) fail('DRIFT', `${d.lean}: record source ${JSON.stringify(e.source)} != generated ${JSON.stringify(src)}`);
    if (e.source === undefined && src !== null && e.source_optional !== true) {
      fail('DRIFT', `${d.lean}: generated source span ${JSON.stringify(src)} missing from the record`);
    }
    if (!!e.loop_body !== d.loopBody) fail('DRIFT', `${d.lean}: record loop_body != generated ${d.loopBody}`);
    if (!!e.trait_impl !== d.traitImpl) fail('DRIFT', `${d.lean}: record trait_impl != generated ${d.traitImpl}`);
  }

  const counts = {};
  for (const s of STATUSES) counts[s] = 0;
  for (const e of entries) if (STATUSES.includes(e.status)) counts[e.status]++;
  return {
    ok: problems.length === 0,
    total: derived.length,
    classified: entries.filter(e => byLean.has(e.lean)).length,
    counts,
    problems,
  };
}

// --- CLI ---
if (import.meta.main) {
  const [mode, a, b, c, d] = Bun.argv.slice(2);
  if (mode === 'derive') {
    const { funs, ext, parity } = await derive(a);
    process.stderr.write(JSON.stringify({ parity }) + '\n');
    await Bun.write(Bun.stdout, JSON.stringify([...funs, ...ext], null, 1) + '\n');
  } else if (mode === 'emit') {
    const { funs, ext } = await derive(a);
    const record = {
      subject: 'M3 kernel extraction output (proofs/m3/NobleKernel)',
      record_version: 1,
      functions: [...funs, ...ext].map(d => ({
        lean: d.lean,
        kind: d.kind,
        rust: d.rust,
        loop_body: d.loopBody || undefined,
        trait_impl: d.traitImpl || undefined,
        source: d.source ? `${d.source.path} ${d.source.span}` : undefined,
        file: d.file,
        line: d.line,
        status: 'REVIEW',
      })),
    };
    await Bun.write(Bun.stdout, JSON.stringify(record, null, 1) + '\n');
  } else if (mode === 'check') {
    const { funs, ext, parity } = await derive(a);
    const record = JSON.parse(await Bun.file(b).text());
    const report = joinCheck([...funs, ...ext], parity, record);
    report.parity = parity;
    await Bun.write(Bun.stdout, JSON.stringify(report, null, 1) + '\n');
    process.exit(report.ok ? 0 : 1);
  } else if (mode === 'leanpayload') {
    const { funs, ext } = await derive(a);
    const record = JSON.parse(await Bun.file(b).text());
    const byLean = new Map([...funs, ...ext].map(d => [d.lean, d]));
    const payloadAll = record.functions
      .filter(e => byLean.has(e.lean))
      .map(e => ({
        lean: e.lean,
        module: byLean.get(e.lean).file === 'Funs.lean' ? 'NobleKernel.Funs' : 'NobleKernel.FunsExternal',
        status: e.status,
        citations: e.citations ?? [],
      }));
    // Each payload is imported into a checker whose import surface can see
    // exactly one group of citation namespaces (checker A: NobleKernel +
    // NobleM2 root; checker B: NobleM2.Refinement; checker C: the NobleM3
    // root, which reaches NobleM3.Refine and NobleM2.WordRefinement). The
    // citation sets must partition, not the entries: an entry that cites
    // both an M2-floor theorem and an M3 theorem is routed into A, B and C
    // with only the citations each checker can see, so every citation is
    // verified exactly once and none is silently dropped.
    const isB = (c) => c.startsWith('NobleM2.Refinement.');
    const isC = (c) => c.startsWith('NobleM3.Refine.') || c.startsWith('NobleM2.WordRefinement.');
    const isRefinement = (c) => isB(c) || isC(c);
    const withCits = (e, keep) => ({ ...e, citations: e.citations.filter(keep) });
    const payloadA = payloadAll.map(e => withCits(e, c => !isRefinement(c)));
    const payloadB = payloadAll
      .filter(e => e.citations.some(isB))
      .map(e => withCits(e, isB));
    const payloadC = payloadAll
      .filter(e => e.citations.some(isC))
      .map(e => withCits(e, isC));
    await Bun.write(c, JSON.stringify(payloadA, null, 1) + "\n");
    await Bun.write(d ?? "/dev/null", JSON.stringify(payloadB, null, 1) + "\n");
    if (process.argv[7]) {
      await Bun.write(process.argv[7], JSON.stringify(payloadC, null, 1) + "\n");
    }
  } else {
    console.error('usage: m3-coverage-join.mjs derive|emit|check|leanpayload PROOF_ROOT [RECORD] [OUT-A] [OUT-B] [OUT-C]');
    process.exit(2);
  }
}
