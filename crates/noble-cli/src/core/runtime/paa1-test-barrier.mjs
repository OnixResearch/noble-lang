// Test-only barrier for the PAA-1 post-check replacement control
// (ACORR-CASE-09). It is concatenated into the embedded worker host only when
// the `paa1-test-barrier` feature is enabled, and it is never part of the
// shipped binary. The barrier runs inside the admission worker exactly between
// the final-byte comparison and instantiation, overwrites the named on-disk
// files with the replacement bytes, and returns what it replaced so the
// admission record retains it.
//
// The environment names one JSON plan file:
//   { "replacement": PATH, "paths": [PATH, ...] }
// A missing variable is a no-op. The barrier cannot alter the bytes the worker
// instantiated: those are the in-memory buffers the comparison accepted.
function paa1CompareBarrier(compared) {
  const planPath = process.env.NOBLE_PAA1_TEST_REPLACE_AFTER_COMPARE;
  if (!planPath) return null;
  const parsed = JSON.parse(fs.readFileSync(planPath, 'utf8'));
  if (typeof parsed.replacement !== 'string' || !Array.isArray(parsed.paths)
    || parsed.paths.length === 0 || parsed.paths.some(target => typeof target !== 'string')) {
    fail('invalid paa1 test barrier plan');
  }
  const replacement = fs.readFileSync(parsed.replacement);
  const replaced = [];
  for (const target of parsed.paths) {
    fs.writeFileSync(target, replacement);
    replaced.push({ path: target, sha256: sha256(replacement) });
  }
  return {
    replacement: parsed.replacement,
    replacement_sha256: sha256(replacement),
    compared_candidate_sha256: sha256(compared.candidate),
    compared_module_sha256: sha256(compared.module),
    replaced,
    test_only: true,
  };
}
