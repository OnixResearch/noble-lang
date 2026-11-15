// Source-order controls for the real synchronous wrapper; these are syntactic
// mutation discriminators, not a proof of the emitted ABI or of reachability.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const source = readFileSync(`${root}/crates/noble-cli/src/component/bounded_region.rs`, 'utf8');
const start = source.indexOf('    fn read_region(');
const end = source.indexOf('    fn release(', start);
if (start < 0 || end < start) throw Error('missing real read_region/release wrapper boundary');
const wrapper = source.slice(start, end);
const requireThat = (condition, description) => { if (!condition) throw Error(description); };
const steps = [
  'let range = region_read::preflight(',
  'retained: self.owner.as_ref().map(ownership::Owner::handle)',
  'let owner = self\n            .owner\n            .take()',
  'self.table.begin(owner, required)',
  'let authorized = region_read::authorize_copy(',
  'self.table.native_access(&admitted.borrow)',
  'let output = if authorized {',
  'crate::safety_observe::event("protected-operation")',
  'self.bytes[range.start..range.finish].to_vec()',
  'self.table.complete(',
  'self.owner = completed.owner;',
  'self.record(HostEvent::Commit {',
];
function check(candidate) {
  let last = -1;
  for (const step of steps) {
    const next = candidate.indexOf(step);
    requireThat(next > last && candidate.indexOf(step, next + 1) < 0,
      `missing, duplicated or moved wrapper step: ${step}`);
    last = next;
  }
  const failure = candidate.slice(candidate.indexOf('let Ok(completed) = self.table.complete('),
    candidate.indexOf('if completed.owner.is_none()'));
  requireThat(failure.includes('return Err(Refusal::invalid(RefusalClass::Completion));') &&
    !failure.includes('self.owner =') && !failure.includes('HostEvent::Commit'),
  'failed completion cannot restore owner or report a successful commit');
  requireThat(candidate.includes('range.start..range.finish') &&
    candidate.includes('claim,\n            scope,\n            range,'),
  'copy range and exact admitted claim must enter the native authorization');
}
check(wrapper);
const mutants = [
  ['authorization-removed', wrapper.replace('let authorized = region_read::authorize_copy(',
    'let authorized = false /* removed */;\n        let ignored = (' )],
  ['authorization-after-copy', wrapper.replace('let authorized = region_read::authorize_copy(',
    'let authorized = self.bytes[range.start..range.finish].to_vec();\n        let ignored = region_read::authorize_copy(')],
  ['owner-substituted', wrapper.replace('claim,\n            scope,\n            range,',
    'admitted.borrow.scope().handle,\n            scope,\n            range,')],
  ['protected-commit-omitted', wrapper.replace('self.record(HostEvent::Commit {',
    '/* protected report missing */\n        self.record(HostEvent::Response {')],
  ['failure-pretends-owner-restored', wrapper.replace(
    'return Err(Refusal::invalid(RefusalClass::Completion));',
    'self.owner = Some(ownership::Owner::from(claim));\n            return Err(Refusal::invalid(RefusalClass::Completion));')],
];
for (const [name, mutated] of mutants) {
  requireThat(mutated !== wrapper, `mutant missed its subject: ${name}`);
  let refused = false;
  try { check(mutated); } catch { refused = true; }
  requireThat(refused, `wrapper invariant accepted mutant: ${name}`);
}
process.stdout.write(JSON.stringify({ result: 'passed', positive: 'real-wrapper',
  controls: mutants.map(([name]) => ({ name, observed: 'refused' })) }) + '\n');
