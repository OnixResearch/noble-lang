// B-LSLOT-01: the LIVE-01..10 contract fields must not change from the
// change's base commit. State/evidence transitions made by another change are
// recorded, not treated as design changes. Exactly two owner-approved LIVE
// contract changes are allowed: owner decision 21a replaces LIVE-02's
// `input.after_reload_ack`, and owner decision 30 drops LIVE-05's
// `missing-import` variant (and nothing else) from `input.variants`. The
// comparison is a pure function so the gate and its regression can exercise
// the original, amended and rejected states.

// The change's base commit (full hash; the abbreviated `8075d3c0` used before
// was ambiguous as a baseline pin).
export const LIVE_BASELINE_COMMIT = '8075d3c083474884f9a7c72b5b1a483f73eec314';

export const APPROVED_LIVE_CHANGES = {
  'LIVE-02': {
    path: ['input', 'after_reload_ack'],
    original: ['20 twice swap 20 swap run'],
    amended: ['20 swap run 20 twice swap'],
    decision: 'OWNER DECISION 2026-11-20 (decision 21a, verbatim): "Decision 21\'s script'
      + ' [\'20 twice swap run\'] was based on a wrong derivation: from the retained old'
      + ' Program P it yields [25] (`run` consumes 24 and the old capture adds 1), not the'
      + ' case\'s unchanged expected stack [24, 21] (the integrator\'s question also misquoted'
      + ' that stack as [21, 22], LIVE-01\'s). The integrator, under the owner\'s delegation,'
      + ' replaces decision 21\'s script with ["20 swap run 20 twice swap"]: [P] -> 20 ->'
      + ' [P,20] -> swap -> [20,P] -> run (old capture, +1) -> [21] -> 20 -> [21,20] -> twice'
      + ' (rebuilt dependent, +4) -> [21,24] -> swap -> [24,21]. Nothing else in the case'
      + ' changes; LIVE-02 is promoted only if a fresh receipt observes exactly [24, 21] and'
      + ' every other expected field."',
  },
  'LIVE-05': {
    path: ['input', 'variants'],
    // The amended value is the baseline array with exactly this variant removed.
    dropped: 'missing-import',
    decision: 'OWNER DECISION 2026-11-21 (decision 30, verbatim): "Drop the variant: record it'
      + ' as inapplicable (live profiles have no import grammar). LIVE-05 can then promote'
      + ' from the merged run."',
  },
};

export const liveDesign = bytes => JSON.parse(bytes).cases
  .filter(row => row.id.startsWith('LIVE-'))
  .map(row => ({ id: row.id, profile: row.profile, kind: row.kind,
    requirements: row.requirements, input: row.input, expected: row.expected }));

const copy = record => JSON.parse(JSON.stringify(record));
const fieldOf = (record, path) => path.reduce((node, key) => node?.[key], record);
const strip = (record, path) => {
  const out = copy(record);
  let node = out;
  for (const key of path.slice(0, -1)) node = node[key];
  delete node[path.at(-1)];
  return out;
};

// Case ids whose contract differs from the pinned baseline; empty when the
// only difference is the owner-approved amendment (or no difference at all).
export function liveContractDiffs({ baselineBytes, currentBytes,
  approved = APPROVED_LIVE_CHANGES }) {
  const baselineByCase = Object.fromEntries(liveDesign(baselineBytes)
    .map(row => [row.id, row]));
  const currentByCase = Object.fromEntries(liveDesign(currentBytes)
    .map(row => [row.id, row]));
  return Object.keys(baselineByCase).filter(id => {
    const before = baselineByCase[id], after = currentByCase[id];
    if (!after) return true;
    const approval = approved[id];
    if (!approval) return JSON.stringify(before) !== JSON.stringify(after);
    if (JSON.stringify(strip(before, approval.path))
      !== JSON.stringify(strip(after, approval.path))) return true;
    const beforeField = fieldOf(before, approval.path);
    const afterField = fieldOf(after, approval.path);
    if (JSON.stringify(beforeField) === JSON.stringify(afterField)) return false;
    if (approval.dropped !== undefined) {
      if (!Array.isArray(beforeField)
        || beforeField.filter(variant => variant?.name === approval.dropped).length !== 1) return true;
      return JSON.stringify(afterField)
        !== JSON.stringify(beforeField.filter(variant => variant?.name !== approval.dropped));
    }
    return JSON.stringify(beforeField) !== JSON.stringify(approval.original)
      || JSON.stringify(afterField) !== JSON.stringify(approval.amended);
  });
}
