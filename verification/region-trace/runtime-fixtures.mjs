// Generate `proofs/m5/RegionTraceRuntime.lean` and its theorem inventory from the host traces the
// effect-protocol lane recorded. Each fixture is one executed case's `noble-bounded-region/v2`
// trace written as `RegionTrace.ObservedEvent`s, exactly in the host's order. The source trace in
// each statement is this script's own projection of the observation. A JavaScript mirror of the
// relation decides every verdict, and the Lean kernel re-checks it by `decide`, so the two must
// agree. These are finite tests of recorded traces, not a correspondence theorem.
//
// `node verification/region-trace/runtime-fixtures.mjs` rewrites both files;
// `--check` only compares them with what the receipt generates.
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
export const receiptFile = 'verification/effect-protocol/execution-9b1fe313.json';
const receiptSha256 = 'b38f9bcc3013137fa528578d503089c8b9c012583df9cbbbb1d62cdcd065e5d8';
export const leanFile = 'proofs/m5/RegionTraceRuntime.lean';
export const inventoryFile = 'proofs/m5/region-trace-runtime-inventory.json';
const axioms = ['propext', 'Classical.choice', 'Quot.sound'];
// A region no executed case requests: the control's read commit can explain itself by no crossing.
const forged = { start: 7, length: 1 };
const requireThat = (condition, message) => { if (!condition) throw Error(message); };

// A JSON integer keeps its exact decimal text, so i64::MAX is not rounded.
function parseExact(text) {
  return JSON.parse(text, (key, value, context) =>
    (typeof value === 'number' ? JSON.rawJSON(context.source) : value));
}
const decimal = value => (JSON.isRawJSON(value) ? value.rawJSON : String(value));
const natural = value => {
  const text = decimal(value);
  requireThat(/^(?:0|[1-9][0-9]*)$/.test(text), `not a natural number: ${text}`);
  return text;
};
const integer = value => {
  const text = decimal(value);
  requireThat(/^-?(?:0|[1-9][0-9]*)$/.test(text), `not an integer: ${text}`);
  return text;
};

// The observed events of one v2 trace. A host retirement is its request and its answer together; a
// guest release is its request and its response. A trace may end mid-request only if truncated.
function observe(row) {
  const { events, events_truncated: truncated } = row.observed;
  const observed = [];
  let pending = null;
  for (const event of events) {
    if (event.kind === 'request') {
      requireThat(pending === null, `${row.id}/${row.variant}: request while one is unanswered`);
      pending = event;
      if (event.operation === 'region.read') {
        observed.push({ type: 'request', op: 'regionRead', identity: natural(event.identity),
          offset: integer(event.offset), length: integer(event.length) });
      } else if (event.operation === 'region.release') {
        observed.push({ type: 'request', op: 'guestRelease', identity: natural(event.identity),
          offset: '0', length: '0' });
      } else {
        requireThat(event.operation === 'host.retire',
          `${row.id}/${row.variant}: no observed event for ${event.operation}`);
      }
    } else if (event.kind === 'commit') {
      requireThat(pending?.operation === 'region.read', `${row.id}/${row.variant}: commit outside a read`);
      observed.push({ type: 'commit', identity: natural(event.identity),
        start: natural(pending.offset), length: natural(pending.length) });
    } else if (event.kind === 'response') {
      requireThat(pending !== null, `${row.id}/${row.variant}: unsolicited response`);
      if (pending.operation === 'host.retire') {
        requireThat(event.result === 'value', `${row.id}/${row.variant}: a refused retirement`);
        observed.push({ type: 'retire', identity: natural(pending.identity) });
      } else {
        observed.push({ type: 'response',
          op: pending.operation === 'region.read' ? 'regionRead' : 'guestRelease',
          value: event.result === 'value' && pending.operation === 'region.read' ? natural(event.bytes) : null,
          reason: event.result === 'value' ? null : event.reason });
      }
      pending = null;
    } else {
      throw Error(`${row.id}/${row.variant}: unknown event kind ${event.kind}`);
    }
  }
  requireThat(pending === null || (truncated && pending.operation !== 'host.retire'),
    `${row.id}/${row.variant}: the trace ends mid-request`);
  return observed;
}

// The source events `lower` then `filterMap projectStep` yields for an observation.
function project(observed) {
  return observed.map(event => {
    if (event.type === 'request') return { ...event, type: 'req' };
    if (event.type === 'commit') return { ...event, type: 'readCommit' };
    if (event.type === 'response') return { ...event, type: 'resp' };
    return { type: 'retireCommit', identity: event.identity };
  });
}

// RegionTrace.wellFormed of the lowered observation: every commit identity decided by a request,
// every read commit explained by a crossing of exactly its region, at most one retirement.
function wellFormed(observed) {
  const decided = observed.filter(event => event.type === 'request').map(event => event.identity);
  const crossings = observed.filter(event => event.type === 'request' && event.op === 'regionRead')
    .map(event => `${event.offset},${event.length}`);
  const commits = observed.filter(event => event.type === 'commit' || event.type === 'retire');
  return commits.every(event => decided.includes(event.identity))
    && observed.filter(event => event.type === 'commit')
      .every(event => crossings.includes(`${event.start},${event.length}`))
    && observed.filter(event => event.type === 'retire').length <= 1;
}

// RegionTrace.wellFormedFrom [registered]: as `wellFormed`, except that the owner the host
// registered before guest start also authorises a host retirement, and nothing else.
function wellFormedFrom(observed, registered) {
  const decided = observed.filter(event => event.type === 'request').map(event => event.identity);
  const crossings = observed.filter(event => event.type === 'request' && event.op === 'regionRead')
    .map(event => `${event.offset},${event.length}`);
  return observed.filter(event => event.type === 'commit').every(event => decided.includes(event.identity))
    && observed.filter(event => event.type === 'retire')
      .every(event => decided.includes(event.identity) || event.identity === registered)
    && observed.filter(event => event.type === 'commit')
      .every(event => crossings.includes(`${event.start},${event.length}`))
    && observed.filter(event => event.type === 'retire').length <= 1;
}

const int = text => (text.startsWith('-') ? `(${text})` : text);
const option = (value, render) => (value === null ? 'none' : `(some ${render(value)})`);
const string = text => {
  requireThat(/^[ -~]*$/.test(text) && !/["\\]/.test(text), `unexpected reason text ${text}`);
  return `"${text}"`;
};

function leanObserved(event) {
  if (event.type === 'request') return `.request .${event.op} ${event.identity} ${int(event.offset)} ${int(event.length)}`;
  if (event.type === 'commit') return `.protectedReadCommit ${event.identity} ${event.start} ${event.length}`;
  if (event.type === 'response') return `.response .${event.op} ${option(event.value, String)} ${option(event.reason, string)}`;
  return `.hostRetirement ${event.identity}`;
}

function leanSource(event) {
  if (event.type === 'req') return `.req .${event.op} ${int(event.offset)} ${int(event.length)}`;
  if (event.type === 'readCommit') return `.commit (.regionRead ${event.start} ${event.length}) ${event.identity}`;
  if (event.type === 'resp') return `.resp .${event.op} ${option(event.value, String)} ${option(event.reason, string)}`;
  return `.commit .hostRetire ${event.identity}`;
}

const list = (items, indent) => (items.length === 0 ? '[]'
  : `[${items.join(`,\n${indent} `)}]`);

function fixtureName(row) {
  const number = row.id.match(/^EPROTO-CASE-(\d\d)$/)?.[1];
  requireThat(number, `unexpected case id ${row.id}`);
  const variant = row.variant.replace(/--/g, '-neg').replace(/[^a-z0-9]+/g, '_');
  return `runtime_case${number}_${variant}`;
}

export function generate() {
  const bytes = readFileSync(join(root, receiptFile));
  requireThat(createHash('sha256').update(bytes).digest('hex') === receiptSha256,
    `${receiptFile} is not the receipt these fixtures were generated from`);
  const receipt = parseExact(bytes.toString('utf8'));
  requireThat(receipt.schema === 'noble-effect-protocol-execution/v1' && receipt.clean_source === true,
    'not a clean effect-protocol receipt');
  const rows = receipt.cases.filter(row => row.status === 'passed' && Array.isArray(row.observed?.events));
  const sections = [];
  const theorems = [];
  let related = 0;
  let registered = 0;
  for (const row of rows) {
    const name = fixtureName(row);
    const observed = observe(row);
    const source = project(observed);
    const label = `${row.id} ${row.variant}`;
    const owner = natural(row.observed.registration?.identity);
    requireThat(observed.every(event => event.type === 'response' || event.identity === owner),
      `${label}: an event names an identity other than the registered owner ${owner}`);
    const exit = row.observed.exit_class;
    const truncated = row.observed.events_truncated ? ', its trace truncated at the host\'s 64-event bound' : '';
    sections.push(`/-- ${label}: exit \`${exit}\`${truncated}. -/\ndef ${name} : List ObservedEvent :=\n  ${list(observed.map(leanObserved), '  ')}`);
    if (wellFormed(observed)) {
      const identity = observed.find(event => event.type === 'request').identity;
      requireThat(!observed.some(event => event.type === 'request' && event.op === 'regionRead'
        && event.offset === String(forged.start) && event.length === String(forged.length)),
      `${label} requests the control region`);
      const control = { type: 'commit', identity, start: String(forged.start), length: String(forged.length) };
      sections.push(`theorem ${name}_relates :\n    related\n      ${list(source.map(leanSource), '      ')}\n      (lower ${name}) = true := by\n  decide`);
      sections.push(`theorem ${name}_refuses_an_unexplained_commit :\n    related\n      ${list([...source, ...project([control])].map(leanSource), '      ')}\n      (lower (${name} ++ [${leanObserved(control)}])) = false := by\n  decide`);
      theorems.push(
        { name: `RegionTrace.${name}_relates`,
          role: `WIT-TRACE-05 runtime fixture: the recorded ${label} trace (exit ${exit}) relates to its projection`,
          allowed_standard_axioms: axioms },
        { name: `RegionTrace.${name}_refuses_an_unexplained_commit`,
          role: `WIT-TRACE-02 control: the ${label} trace with a read commit no crossing explains is refused`,
          allowed_standard_axioms: axioms });
      related += 1;
    } else {
      // A host retirement before any guest request: no decision step authorises it, so `related`
      // refuses it, and the registration the host reports does.
      requireThat(observed.length === 1 && observed[0].type === 'retire',
        `${label}: refused by the relation for an unexpected reason`);
      requireThat(wellFormedFrom(observed, owner), `${label}: refused even under its registration`);
      const forgedIdentity = String(BigInt(owner) + 1n);
      const forgedRetirement = [{ type: 'retire', identity: forgedIdentity }];
      requireThat(!wellFormedFrom(forgedRetirement, owner), `${label}: a forged retirement is not refused`);
      sections.push(`/-- Without a registration premise, no decision authorises the retirement and
\`identitiesDecided\` refuses it. -/\ntheorem ${name}_is_refused_without_a_registration_decision :\n    related\n      ${list(source.map(leanSource), '      ')}\n      (lower ${name}) = false := by\n  decide`);
      sections.push(`/-- Under the registration the host reports (\`registration.identity\` ${owner}), the retirement relates. -/\ntheorem ${name}_relates_under_its_registration :\n    relatedFrom [${owner}]\n      ${list(source.map(leanSource), '      ')}\n      (lower ${name}) = true := by\n  decide`);
      sections.push(`/-- Control: a retirement of an identity the host did not register is refused under the same premise. -/\ntheorem ${name}_refuses_a_forged_retirement :\n    relatedFrom [${owner}]\n      ${list(project(forgedRetirement).map(leanSource), '      ')}\n      (lower ${list(forgedRetirement.map(leanObserved), '      ')}) = false := by\n  decide`);
      theorems.push(
        { name: `RegionTrace.${name}_is_refused_without_a_registration_decision`,
          role: `WIT-TRACE-05: without a registration premise, the recorded ${label} trace, a host retirement before any guest request, is refused`,
          allowed_standard_axioms: axioms },
        { name: `RegionTrace.${name}_relates_under_its_registration`,
          role: `WIT-TRACE-05 runtime fixture (#50): the recorded ${label} trace relates under the registration its report names`,
          allowed_standard_axioms: axioms },
        { name: `RegionTrace.${name}_refuses_a_forged_retirement`,
          role: `WIT-TRACE-02 control (#50): a retirement of an identity the ${label} host did not register is refused under its registration`,
          allowed_standard_axioms: axioms });
      registered += 1;
    }
  }
  const lean = `import RegionTrace

/-!
# Runtime fixtures (generated)

Generated by \`verification/region-trace/runtime-fixtures.mjs\` from
\`${receiptFile}\`, the effect-protocol lane's receipt from the clean commit
\`${receipt.source_commit.slice(0, 8)}\` (SHA-256 \`${receiptSha256}\`).
Do not edit by hand: the region-trace check regenerates this file and requires it byte for byte.

Each \`def\` is one executed case's recorded \`noble-bounded-region/v2\` trace, written as
\`ObservedEvent\`s in the host's order. A fixture that relates is paired with a control that appends a
read commit no crossing explains, which the relation must refuse. ${registered} traces, a host retirement
before any guest request, have no decision for the retired identity, so \`related\` refuses them. They
relate under the registration premise (\`relatedFrom\`) with the identity the host's report names as
registered, never one taken from the retirement, and a forged retirement identity stays refused.

These are finite tests of recorded traces. The exit class is outside \`ObservedEvent\`, so a trap, a
quota exhaustion and a cancellation that leave the same events lower to the same trace. No
correspondence between emitted-component steps and the model is claimed.
-/

namespace RegionTrace

${sections.join('\n\n')}

end RegionTrace
`;
  const inventory = {
    schema: 'noble-region-trace-inventory/v1',
    scope: `Generated runtime fixtures: ${related} recorded traces relate, each with a refused control, and ${registered} host retirements before any request relate under the registration their reports name, each with a refused forged retirement. Source: ${receiptFile}.`,
    theorems,
  };
  return { lean, inventory: `${JSON.stringify(inventory, null, 2)}\n`, related, registered };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const generated = generate();
  if (process.argv.includes('--check')) {
    requireThat(readFileSync(join(root, leanFile), 'utf8') === generated.lean, `${leanFile} differs from its generator`);
    requireThat(readFileSync(join(root, inventoryFile), 'utf8') === generated.inventory,
      `${inventoryFile} differs from its generator`);
  } else {
    writeFileSync(join(root, leanFile), generated.lean);
    writeFileSync(join(root, inventoryFile), generated.inventory);
  }
  process.stdout.write(`runtime fixtures: ${generated.related} related, ${generated.registered} related under registration\n`);
}
