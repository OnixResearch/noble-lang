export const requiredDifferentialControlIds = Object.freeze([
  'reversed-subtraction',
  'unsigned-decoding',
  'prefix-corruption',
  'wrong-composition-order',
  'trace-mismatch',
  'literal-wrapping-instead-of-refusal',
  'stale-proof-subject',
]);

export function verifyRequiredDifferentialControls(controls) {
  const ids = controls.map(control => control.id);
  const missing = requiredDifferentialControlIds.filter(id => !ids.includes(id));
  const duplicates = ids.filter((id, index) => ids.indexOf(id) !== index);
  const failed = controls.filter(control => control.demonstrably_fails !== true).map(control => control.id);
  if (missing.length || duplicates.length || failed.length) {
    throw new Error(`missing or invalid required negative controls: ${JSON.stringify({ missing, duplicates, failed })}`);
  }
  return { required_control_ids: [...requiredDifferentialControlIds], observed_control_ids: ids };
}
