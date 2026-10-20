import { join } from 'node:path';
import { writeFileSync } from 'node:fs';
import { refRun } from './arithmetic.mjs';
import { assess, selectedMutation } from './emi-model.mjs';
import { show, requireThat, evidenceMatches, discrepancy, shrinkTriple } from './emi-evidence.mjs';

export function wrongSelectedLowering(wat) {
  const original = '(if (local.get $condition)\n  (then (call $enqueue_program (local.get $then)))';
  requireThat(wat.split(original).length === 2, 'if lowering mutation site is absent or ambiguous');
  return wat.replace(original, '(if (i32.eqz (local.get $condition))\n  (then (call $enqueue_program (local.get $then)))');
}

export function controls({ eligible, executor, evaluate, verifyFoundation, foundation, initial, out }) {
  const results = [];
  const record = (id, passed, details) => {
    results.push({ id, detected: passed, ...details });
    requireThat(passed, `EMI control was not detected: ${id}`);
  };
  const witness = eligible.find(entry => entry.pair.kind === 'if' &&
    show(refRun(entry.pair.variant[1 - entry.pair.selected], entry.pair.input)) !== show(entry.reference.reference));
  requireThat(witness, 'generated corpus lacks a wrong-branch output witness');
  const { pair, evidence, observations } = witness;
  const expected = witness.reference.expected, observed = observations.off.variant;

  const changed = selectedMutation(pair);
  // The shared evaluator admits the whole source even when the model gate
  // refuses a selected change; static validity does not establish nonexecution.
  const selected = evaluate(changed, 'selected-control', 'off', false);
  record('selected-branch-mutation', !assess(changed).eligible && selected.admissions.variant.admitted,
    { eligibility: assess(changed), frontend_admitted: selected.admissions.variant.admitted });

  const reordered = { ...observed, stack: [...observed.stack.slice(0, -2),
    observed.stack.at(-1), observed.stack.at(-2)] };
  record('ordered-output', discrepancy(expected, reordered, pair.prefix.length) === 'ordered-output',
    { expected: expected.stack, changed: reordered.stack });
  requireThat(pair.prefix.length > 0, 'generated control witness has no preserved prefix');
  const prefix = observed.stack.map(item => [...item]);
  prefix[0] = prefix[0][0] === 'Bool' ? ['Bool', prefix[0][1] === 'true' ? 'false' : 'true']
    : ['I64', String(BigInt(prefix[0][1]) + 1n)];
  record('prefix-corruption', discrepancy(expected, { ...observed, stack: prefix }, pair.prefix.length) === 'prefix',
    { prefix_length: pair.prefix.length, expected: expected.stack, changed: prefix });

  const host = executor.admit('"EMI trace control" test.emit\n', 'host-trace-control');
  requireThat(host.admitted, 'trace-control source was not admitted');
  const actualTrace = executor.execute(host, 'off');
  const forged = { ...actualTrace, trace: [] };
  record('forged-empty-trace', actualTrace.guest_requests > 0 && actualTrace.trace.length > 0 &&
    discrepancy({ outcome: 'normal', stack: [], trace: [] }, forged, 0) === 'trace',
    { actual: actualTrace.report, forged_request_trace: [], retained_guest_requests: forged.guest_requests });

  record('stale-subject', !evidenceMatches(changed, evidence),
    { original_subject: evidence.subject, changed_subject_rejected: true });
  let staleFoundation = false;
  try { verifyFoundation({ ...foundation, source_tree: '0'.repeat(40) }, initial); }
  catch { staleFoundation = true; }
  record('stale-foundation', staleFoundation, { changed_tree_rejected: staleFoundation });

  const lowering = [];
  for (const mode of ['off', 'on']) {
    const result = evaluate(pair, `wrong-lowering-${mode}`, mode, true);
    lowering.push({ mode, observed: result.observations.variant.report, discrepancy: result.discrepancy,
      evidence: result.evidence, fault_injected: true });
    requireThat(result.discrepancy === 'ordered-output', `wrong lowering was not an ordered-output discrepancy (${mode})`);
  }
  const shrunk = shrinkTriple(pair, {
    mode: 'off', wanted: 'ordered-output',
    evaluate: (candidate, label, mode) => evaluate(candidate, `control-${label}`, mode, true),
  });
  const replay = { schema: 'noble-foundation-emi-replay/v1', pair: shrunk.pair,
    mode: 'off', fault: 'wrong-selected-branch-lowering', discrepancy: shrunk.discrepancy };
  writeFileSync(join(out, 'minimal-control.json'), show(replay) + '\n');
  const minimized = assess(shrunk.pair);
  const interfacePreserved = show([minimized.input_types, minimized.output_types]) ===
    show([witness.reference.input_types, witness.reference.output_types]);
  record('wrong-selected-branch-lowering', interfacePreserved && lowering.every(row => row.discrepancy === 'ordered-output'),
    { modes: lowering, shrink: shrunk, interface_preserved: interfacePreserved, replay: 'minimal-control.json' });
  return results;
}
