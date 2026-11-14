import { createHash } from 'node:crypto';

export const digest = bytes => createHash('sha256').update(bytes).digest('hex');
export const canonical = value => JSON.stringify(sort(value));

function sort(value) {
  if (Array.isArray(value)) return value.map(sort);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.keys(value).sort().map(key => [key, sort(value[key])]));
  }
  return value;
}

function quoted(text, key) {
  return text.match(new RegExp(`(?:^|\\n)\\s*${key}\\s*=\\s*"([^"]+)"`))?.[1] ?? null;
}

/** Read Lake dependency declarations as inert text. This deliberately does not load Lakefile code. */
export function directDependencies(lakefile, format) {
  if (format === 'toml') {
    const entries = [];
    const blocks = lakefile.split(/^\[\[require\]\]\s*$/m).slice(1);
    let complete = !/^\[\[require\]\]\s*$/m.test(lakefile) || blocks.length > 0;
    for (const block of blocks) {
      const name = quoted(block, 'name');
      if (!name) { complete = false; continue; }
      const git = block.match(/(?:^|\n)\s*git\s*=\s*\{([^}]*)\}/s)?.[1];
      const url = git?.match(/\burl\s*=\s*"([^"]+)"/)?.[1] ?? null;
      const rev = git?.match(/\brev\s*=\s*"([^"]+)"/)?.[1] ?? quoted(block, 'rev');
      const entryComplete = Boolean(git && url && rev);
      entries.push({ name, url: git ? url : null, rev: git ? rev : null,
        kind: git ? 'git' : 'path', complete: entryComplete });
      complete &&= entryComplete;
    }
    return { entries, complete, evaluator: 'toml-data-parser' };
  }
  if (format === 'lean') {
    const entries = [];
    let complete = true;
    for (const line of lakefile.split('\n')) {
      if (!/^\s*require\b/.test(line) || /^\s*--/.test(line)) continue;
      const git = line.match(/^\s*require\s+(\S+)\s+from\s+git\s+"([^"]+)"\s+@\s+"([^"]+)"/);
      const path = line.match(/^\s*require\s+(\S+)\s+from\s+"([^"]+)"/);
      if (git) entries.push({ name: git[1], url: git[2], rev: git[3], kind: 'git', complete: true });
      else if (path) { entries.push({ name: path[1], url: path[2], rev: null, kind: 'path', complete: false }); complete = false; }
      else complete = false;
    }
    return { entries, complete: complete && entries.every(item => item.complete),
      evaluator: 'lean-lakefile-static-require-scan' };
  }
  return { entries: [], complete: false, evaluator: 'unsupported-lakefile-format' };
}

export function manifestPackages(manifest) {
  if (!Array.isArray(manifest?.packages)) return null;
  return manifest.packages.map(pkg => ({ ...pkg })).sort((a, b) =>
    String(a.name).localeCompare(String(b.name)) || String(a.url).localeCompare(String(b.url)));
}

/** Exact package-entry equality: no rev, URL, source, or resolver-field discrepancy is ignored. */
export function compareManifests(expectedManifest, actualManifest) {
  const expected = manifestPackages(expectedManifest);
  const actual = manifestPackages(actualManifest);
  if (!expected || !actual) return { equal: false, discrepancies: [{ kind: 'invalid-manifest' }] };
  const duplicates = items => items.map(item => item.name)
    .filter((name, index, names) => names.indexOf(name) !== index);
  const duplicateExpected = [...new Set(duplicates(expected))];
  const duplicateActual = [...new Set(duplicates(actual))];
  if (duplicateExpected.length || duplicateActual.length) return { equal: false,
    discrepancies: [{ kind: 'invalid-manifest', duplicate_expected: duplicateExpected,
      duplicate_actual: duplicateActual }] };
  const byName = items => new Map(items.map(item => [item.name, item]));
  const left = byName(expected), right = byName(actual);
  const discrepancies = [];
  const metadata = manifest => Object.fromEntries(Object.entries(manifest)
    .filter(([key]) => key !== 'packages'));
  if (canonical(metadata(expectedManifest)) !== canonical(metadata(actualManifest))) {
    discrepancies.push({ kind: 'manifest-metadata-mismatch',
      expected: metadata(expectedManifest), actual: metadata(actualManifest) });
  }
  for (const name of [...new Set([...left.keys(), ...right.keys()])].sort()) {
    if (!left.has(name)) discrepancies.push({ kind: 'unpredicted-dependency', name, actual: right.get(name) });
    else if (!right.has(name)) discrepancies.push({ kind: 'missing-dependency', name, expected: left.get(name) });
    else if (canonical(left.get(name)) !== canonical(right.get(name))) {
      discrepancies.push({ kind: 'dependency-identity-mismatch', name,
        expected: left.get(name), actual: right.get(name) });
    }
  }
  return { equal: discrepancies.length === 0, discrepancies, expected, actual };
}

export function compareDirectToManifest(direct, manifest) {
  const packages = manifestPackages(manifest);
  const discrepancies = [];
  if (!packages) return { equal: false, discrepancies: [{ kind: 'invalid-manifest' }] };
  const byName = new Map(packages.map(item => [item.name, item]));
  for (const item of direct.entries) {
    const actual = byName.get(item.name);
    if (!actual) discrepancies.push({ kind: 'lakefile-dependency-not-in-manifest', name: item.name,
      url: item.url, rev: item.rev, source: 'pre-update-plan' });
    else if (item.kind === 'git' && (item.url !== actual.url || item.rev !== actual.rev)) {
      discrepancies.push({ kind: 'direct-pin-disagrees-with-manifest', name: item.name,
        expected: { url: item.url, rev: item.rev }, actual: { url: actual.url, rev: actual.rev } });
    }
  }
  return { equal: discrepancies.length === 0, discrepancies,
    visibility: direct.complete ? 'static-declarations-complete' : 'lakefile-has-unparsed-or-dynamic-requires' };
}

export function dependencyToolchainMismatches(snapshot, side) {
  return snapshot.package_sources.flatMap(row => {
    const consumer = snapshot.projects[row.project]?.toolchain;
    if (!row.toolchain || !consumer || row.toolchain === consumer) return [];
    return [{ side, project: row.project, package: row.package, consumer_toolchain: consumer,
      dependency_toolchain: row.toolchain, kind: 'lean-library-toolchain-mismatch' }];
  });
}

export function classifyChanges(paths) {
  const unique = [...new Set(paths)].sort();
  const pin = [], repair = [], semantic = [], verifier = [];
  for (const path of unique) {
    if (/^policy\//.test(path) || /(?:inventory|proof-evidence|checker|gate)(?:\.|\/|$)/i.test(path)
        || /^verification\/lean-qualify\//.test(path)) {
      verifier.push(path);
    } else if (/^proofs\/.*(?:lake-manifest\.json|lakefile\.(?:toml|lean)|lean-toolchain|UPSTREAM\.json)$/.test(path)) {
      pin.push(path);
    } else if (/^specs\//.test(path) || /^proofs\/.*\.lean$/.test(path)) {
      // A source diff is semantic until an independent reviewer establishes that only proof terms changed.
      semantic.push(path);
    } else if (/^proofs\//.test(path)) {
      repair.push(path);
    }
  }
  return { proposed_pin_changes: pin, proof_only_repairs: repair,
    semantic_changes: semantic, verifier_policy_changes: verifier };
}

/**
 * Native monotone bisection. The caller must supply an ordered immutable revision list;
 * the first revision is verified good here, before any midpoint is tested.
 */
export async function bisectRegression(revisions, probe) {
  if (revisions.length < 2) throw new Error('bisect requires a baseline and at least one proposal revision');
  const probes = [];
  const record = async revision => {
    const result = await probe(revision);
    if (!['pass', 'fail'].includes(result.outcome)) throw new Error(`invalid outcome for ${revision}: ${result.outcome}`);
    const row = { revision, toolchain: result.toolchain ?? null, outcome: result.outcome,
      details: result.details ?? null };
    probes.push(row);
    return row;
  };
  const first = await record(revisions[0]);
  if (first.outcome !== 'pass') return { qualified: false, reason: 'baseline-not-good', probes };
  const last = await record(revisions.at(-1));
  if (last.outcome !== 'fail') return { qualified: false, reason: 'proposal-does-not-fail', probes };
  let good = 0, bad = revisions.length - 1;
  while (bad - good > 1) {
    const middle = Math.floor((good + bad) / 2);
    const result = await record(revisions[middle]);
    if (result.outcome === 'pass') good = middle;
    else bad = middle;
  }
  return { qualified: true, algorithm: 'native-binary-search-monotonic-assumption',
    boundary: { last_good: revisions[good], first_bad: revisions[bad] }, probes };
}

/** Certify every revision and expose non-monotone fail-then-pass behavior. */
export async function linearScan(revisions, probe) {
  if (revisions.length < 2) throw new Error('linear scan requires a baseline and at least one proposal revision');
  const probes = [];
  for (const revision of revisions) {
    const result = await probe(revision);
    if (!['pass', 'fail'].includes(result.outcome)) throw new Error(`invalid outcome for ${revision}: ${result.outcome}`);
    probes.push({ revision, toolchain: result.toolchain ?? null, outcome: result.outcome,
      details: result.details ?? null });
  }
  const baselineGood = probes[0].outcome === 'pass';
  return { qualified: baselineGood && probes.at(-1).outcome === 'fail',
    algorithm: 'linear-scan-every-revision', baseline_good: baselineGood,
    transitions: probes.slice(1).map((row, index) => ({ from: probes[index].outcome, to: row.outcome,
      from_revision: probes[index].revision, to_revision: row.revision }))
      .filter(row => row.from !== row.to), probes };
}

export function sourceRevision(value) {
  if (typeof value !== 'string' || !/^[0-9a-f]{40,64}$/.test(value)) {
    throw new Error('revision set requires a full immutable 40- or 64-digit commit SHA');
  }
  return value;
}
