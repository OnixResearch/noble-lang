import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { root } from './record.mjs';

function once(source, before, after) {
  assert.ok(source.includes(before), `fixture replacement not found: ${before}`);
  assert.equal(source.indexOf(before), source.lastIndexOf(before), `ambiguous fixture replacement: ${before}`);
  return source.replace(before, after);
}
const bytes = source => Buffer.from(`${source}\n`, 'utf8');
const caseRows = JSON.parse(fs.readFileSync(path.join(root,
  'specs/conformance/contract-cases.json'))).cases;
const source = id => {
  const rows = caseRows.filter(row => row.id === id);
  assert.equal(rows.length, 1, `not a unique canonical case: ${id}`);
  assert.ok(typeof rows[0].input.source === 'string', `missing canonical source: ${id}`);
  return rows[0].input.source;
};

const poly = source('CONTRACT-17');
const increment = source('CONTRACT-18');
const falseEq = source('CONTRACT-19');
assert.equal(source('CONTRACT-25'), poly, 'dependent equality baseline diverged');
const proofBody = '(intro (A Type0) (intro (x A) (refl x)))';
const moduleWithProof = (name, proposition, proof) =>
  `module logic@1 [ proof 1 ${name} : [ ${proposition} ] [ ${proof} ] ]`;
const appendToPoly = (name, proposition, proof) => {
  const extended = poly.replace(/ \]$/u,
    ` proof 1 ${name} : [ ${proposition} ] [ ${proof} ] ]`);
  assert.notEqual(extended, poly, 'named proof was not added to polymorphic module');
  return extended;
};
const instantiate = (name, code) => appendToPoly(name,
  `(Pi (x ${code}) (Eq ${code} x x))`,
  `(intro (x ${code}) (apply (apply (use poly-refl) ${code}) x))`);
const eqI64 = '(Pi (x I64) (Eq I64 x x))';
const substProof = '(intro (x I64) (subst (motive (v I64) (Eq I64 v v)) (refl x) (refl x)))';
const hostileI64 = term => moduleWithProof('bogus', eqI64, `(intro (x I64) ${term})`);
const exportedPoly = poly.replace(/ \]$/u, ' export poly-refl ]');
assert.notEqual(exportedPoly, poly, 'proof export fixture unchanged');
const importedUse = `module imported@1 [ proof 1 imported-list :
  [ (Pi (x (List I64)) (Eq (List I64) x x)) ]
  [ (intro (x (List I64)) (apply (apply (use alias.poly-refl) (List I64)) x)) ] ]`;
const rebindPoly = once(exportedPoly, 'module logic@1', 'module logic@2');
const deepType = `${'(List '.repeat(200)}I64${')'.repeat(200)}`;
const deepBinders = Array.from({ length: 200 }, (_, index) => `x${index}`);
const deepClaim = deepBinders.reduceRight((goal, binder) =>
  `(Pi (${binder} I64) ${goal})`, '(Eq I64 0 0)');
const deepProof = deepBinders.reduceRight((term, binder) =>
  `(intro (${binder} I64) ${term})`, '(refl 0)');
const arithmetic = once(increment, 'def increment [ 1 + ]',
  'export increment def increment [ 1 + ]');
const proofFree = arithmetic.replace(/ proof 1 increment-correct for increment-law \[ .* \] \]$/u, ' ]');
assert.notEqual(proofFree, arithmetic, 'proof-free source fixture was not derived');
const proofChanged = arithmetic.replace(/ \]$/u,
  ' proof 1 extra-refl : [ (Pi (z I64) (Eq I64 z z)) ] [ (intro (z I64) (refl z)) ] ]');
assert.notEqual(proofChanged, arithmetic, 'proof-only change did not change source');

// Every submission is a real UTF-8 Noble source frame, not generated Lean or a
// simulated checker result. Variant names and expected outcomes come from the
// separately frozen canonical input/expected record.
export function scenarios() {
  return [
    { id: 'CONTRACT-17', baseline: bytes(poly), variants: {
      'wrong-type-witness': bytes(once(poly, '(Eq A x x)', '(Eq I64 x x)')),
      'Type0-as-Type0': bytes(once(poly, '(Pi (x A) (Eq A x x))', '(Eq Type0 A A)')),
      'unrestricted-Lean-Type-binder': bytes(once(poly, '(Pi (A Type0)', '(Pi (A Type)')),
      'wrong-dependent-motive': bytes(once(poly, proofBody,
        '(intro (A Type0) (intro (x A) (subst (Eq A x x) (refl x) (refl x))))')),
      'partial-value-as-Eq-witness': bytes(once(poly, '(refl x)', '(refl (head x))')),
      'proof-lexer-escape-into-def': bytes(once(poly, '[ (intro (A Type0)',
        '[ ) def injected [ 1 ] (intro (A Type0)')),
    }, extras: {
      'type-I64': bytes(instantiate('i64-refl', 'I64')),
      'type-Bool': bytes(instantiate('bool-refl', 'Bool')),
      'type-Pair-I64-List-Bool': bytes(instantiate('pair-refl', '(Pair I64 (List Bool))')),
      ...Object.fromEntries([
        ['Resource-test-counter', '(Resource test.counter)'],
        ['Program-I64-I64-pure', '(Program I64 I64 pure)'], ['Syntax', 'Syntax'],
        ['Pair-I64-Resource', '(Pair I64 (Resource test.counter))'],
        ['Type1', 'Type1'], ['forged-PureTyCode', 'forged-PureTyCode'],
      ].map(([name, ty]) => [name, bytes(instantiate('forbidden', ty))])),
    } },
    { id: 'CONTRACT-18', baseline: bytes(increment), variants: {
      'missing-bridge': bytes(once(increment,
        ' (bridge (by-exact-append-assoc))', '')),
      'missing-join': bytes(once(increment, ' (join (by-exact-result))', '')),
      'foreign-append-rewrite': bytes(once(increment,
        '(by-exact-append-assoc)', '(by-foreign-append-assoc)')),
      'swapped-add-operands': bytes(once(increment,
        '(pc-exact (exec-literal 1)) (pc-exact (exec-add))',
        '(pc-exact (exec-add)) (pc-exact (exec-literal 1))')),
      'forged-intermediate-equality': bytes(once(increment,
        '(bridge (by-exact-append-assoc))', '(bridge (refl 0))')),
      'unrelated-final-equality': bytes(once(increment,
        '(join (by-exact-result))', '(join (refl 0))')),
      'PC-without-exportedClaim-bridge': bytes(once(increment,
        '(export-unary-I64 ', '(pc-sequence ')),
      'changed-ensures-expression': bytes(once(increment,
        '(add (in x) 1)', '(add (in x) 2)')),
    } },
    { id: 'CONTRACT-19', baseline: bytes(poly), variants: {
      'false-Eq': bytes(falseEq),
      'axiom': bytes(hostileI64('(axiom bogus)')),
      'admit': bytes(hostileI64('(admit)')),
      'sorry': bytes(hostileI64('(sorry)')),
      'raw-Lean-declaration': bytes(hostileI64('theorem forged : False := by trivial')),
      'Lean-by-tactic': bytes(hostileI64('(by trivial)')),
      'unchecked-native-decision': bytes(hostileI64('(native_decide)')),
      'partial-list-head': bytes(hostileI64('(refl (head []))')),
      'unknown-rule': bytes(hostileI64('(unknown-rule x)')),
      'proof-parenthesis-escape-into-def': bytes(once(hostileI64('(refl x)'),
        '[ (intro (x I64) (refl x)) ]', '[ (intro (x I64) (refl x))) ] def injected [ 1 ]')),
      'editor-hole': bytes(hostileI64('?hole')),
    } },
    { id: 'CONTRACT-21', baseline: bytes(poly), variants: {
      'cyclic-use-dependencies': bytes(`module cycles@1 [ proof 1 a : [ ${eqI64} ] [ (use b) ] proof 1 b : [ ${eqI64} ] [ (use a) ] ]`),
      'oversized-type-code': bytes(moduleWithProof('oversized',
        `(Pi (x ${deepType}) (Eq ${deepType} x x))`,
        `(intro (x ${deepType}) (refl x))`)),
      'over-nesting': bytes(moduleWithProof('nested', deepClaim, deepProof)),
      'normalization-work-exhaustion': bytes(instantiate('normalize', '(List I64)')),
      'substitution-work-exhaustion': bytes(moduleWithProof('substitute', eqI64, substProof)),
      'proof-timeout': bytes(poly),
      'internal-checker-error': bytes(poly),
      'partial-selector-used-in-Eq': bytes(moduleWithProof('head-eq',
        '(Eq I64 (head []) (head []))', '(refl (head []))')),
      'invalid-import-transitive-axiom': bytes(
        'module imported@1 [ proof 1 foreign : [ (Pi (x I64) (Eq I64 x x)) ] [ (use foreign.axiom) ] ]'),
    }, extras: {
      'accepted-exported-proof': bytes(exportedPoly),
      'import-exported-proof': bytes('import logic@1 as alias'),
      'accepted-imported-use': bytes(importedUse),
      'untrusted-axiom-module': bytes('module foreign@1 [ proof 1 axiom : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (axiom foreign)) ] export axiom ]'),
      'import-untrusted-axiom': bytes('import foreign@1 as foreign'),
    } },
    { id: 'CONTRACT-22', baseline: bytes(arithmetic), variants: {
      'proof-only-change': bytes(proofChanged),
      'explicit-MC2-evidence-inspection': bytes(arithmetic),
      'ordinary-quote-compose-run-reflect': bytes(arithmetic),
      'proof-rejection-while-prior-module-snapshot-remains': bytes(once(
        once(arithmetic, 'module arithmetic@1', 'module arithmetic@2'),
        '(mc1-true-eq-wrap)', '(refl 0)')),
    }, extras: {
      'without-proof': bytes(proofFree),
      'import': bytes('import arithmetic@1 as arithmetic'),
      'import-rejected-v2': bytes('import arithmetic@2 as rejected'),
      'invoke-41': bytes('41 arithmetic.increment'),
      'invoke-wrap': bytes('9223372036854775807 arithmetic.increment'),
      'reflect': bytes('[ 1 + ] reflect'),
      'named-reflect': bytes('[ arithmetic.increment ] reflect'),
      'compose-run': bytes('41 [ 1 ] [ + ] compose run'),
    } },
    { id: 'CONTRACT-25', baseline: bytes(poly), variants: {
      'apply-proof-to-valid-List-I64-code': bytes(instantiate('list-refl', '(List I64)')),
      'subst-with-typed-motive-and-checked-premise': bytes(moduleWithProof('subst-ok',
        eqI64, substProof)),
      'subst-with-wrong-motive-type': bytes(moduleWithProof('subst-bad',
        eqI64, '(intro (x I64) (subst (motive (v Bool) (Eq Bool v v)) (refl x) (refl x)))')),
      'subst-with-forged-equality': bytes(moduleWithProof('subst-forged',
        eqI64, '(intro (x I64) (subst (motive (v I64) (Eq I64 v v)) (axiom equality) (refl x)))')),
      'binder-capture': bytes(moduleWithProof('shadow',
        '(Pi (x I64) (Pi (y I64) (Eq I64 x x)))',
        '(intro (x I64) (intro (y I64) (refl y)))')),
      'foreign-private-proof-import': bytes(importedUse),
      'reuse-after-module-rebind': bytes(importedUse),
      'Eq-transferred-to-undefined-MC1-eq-without-Holds': bytes(moduleWithProof('undefined',
        '(Eq I64 (div 1 0) (div 1 0))', '(refl (div 1 0))')),
    }, extras: {
      'accepted-exported-proof': bytes(exportedPoly),
      'private-proof': bytes(poly),
      'rebound-exported-proof': bytes(rebindPoly),
      'import-version-1': bytes('import logic@1 as alias'),
      'import-version-2': bytes('import logic@2 as alias'),
      'accepted-imported-use': bytes(importedUse),
    } },
  ];
}
