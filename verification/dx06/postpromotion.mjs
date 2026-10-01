import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { restoreReplayEvidence } from '../current-source-replay/projection.mjs';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const casePath = 'specs/conformance/developer-experience-cases.json';
const receiptPath = 'verification/dx06/acceptance.json';
const receiptSha256 = '650e38c8e38f1f3a753a4e7e8b92f5e15175d13eef35bccb5cff65f8d78fe7b5';
// This is the complete postpromotion DX-06 state/evidence byte span, not a
// reserialization. The earlier full-file hash separately binds everything
// outside this span after current-source DX-01 evidence is removed.
const promotedBlockSha256 = 'e5659d5b44c7c012966d813c0979aa58e8b63fa978483d5185a743ede5a06213';
const prior = '      "state": {"implementation": "absent", "execution": "not-run", "proof": "open", "trust": "unassessed"}, "evidence": []';
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const read = relative => fs.readFileSync(path.join(root, relative));

function inventory(folder, base) {
  const files = {};
  function visit(directory) {
    for (const item of fs.readdirSync(directory, { withFileTypes: true })
      .sort((left, right) => left.name.localeCompare(right.name))) {
      const file = path.join(directory, item.name);
      if (item.isDirectory()) visit(file);
      else {
        assert.ok(item.isFile() && !item.isSymbolicLink(), `unfrozen file type: ${file}`);
        const relative = path.relative(base, file).split(path.sep).join('/');
        const bytes = fs.readFileSync(file);
        files[relative] = { sha256: sha(bytes), bytes: bytes.length };
      }
    }
  }
  visit(folder);
  return files;
}

function sourceInventory(scope) {
  const files = {};
  for (const selected of scope) {
    assert.ok(typeof selected === 'string' && !path.isAbsolute(selected) &&
      !selected.split('/').includes('..'), 'untrusted source scope');
    const selectedPath = path.join(root, selected);
    const stat = fs.lstatSync(selectedPath);
    assert.ok(!stat.isSymbolicLink(), `source scope symlink: ${selected}`);
    Object.assign(files, stat.isDirectory() ? inventory(selectedPath, root) :
      { [selected]: { sha256: sha(fs.readFileSync(selectedPath)), bytes: stat.size } });
  }
  return files;
}

const expectedScope = [
  'crates/noble-kernel/src', 'crates/noble-contracts/src',
  'crates/noble-wasm/src', 'crates/noble-cli/src',
  'verification/dx06/gate.mjs', 'README.md',
];
const expectedVariants = [
  'authorized-matching-interface', 'denied', 'missing-mapping',
  'incompatible-interface', 'second-request-script-exhausted', 'unexpected-operation',
];
const expectedCommands = [
  ['matching-interface', 0], ['latent-effect', 0], ['denied', 1],
  ['missing-mapping', 2], ['incompatible-interface', 2], ['script-exhausted', 1],
];
const expectedAssumptions = [
  'The receipt-bound frozen CLI, source compiler, checked Rust kernel, selected Node/V8, wasm-tools and independently supplied finite test-host policy are trusted mechanisms, not proved refinements.',
  'The scoped Charon kernel extraction succeeded, but pinned Aeneas failed before Lean emission; nominal M3 proof remains open and no real host-clock evidence is claimed.',
];

// Optional in-memory copies support hostile tests; direct CLI execution always
// checks the canonical case and the receipt-selected frozen smoke bytes.
export function verifyDx06Postpromotion({ caseBytes, smokeBytes } = {}) {
  assert.equal(fs.realpathSync(process.cwd()), root, 'run from source root for replay projection');
  const receiptBytes = read(receiptPath);
  assert.equal(sha(receiptBytes), receiptSha256, 'immutable DX-06 receipt changed');
  const receipt = JSON.parse(receiptBytes);
  assert.equal(receipt.schema, 'noble-scripted-test-host-acceptance/v1');
  assert.deepEqual([receipt.case_id, receipt.profile, receipt.result, receipt.proof],
    ['DX-06', 'Test-Host-Draft', 'passed', 'open']);
  assert.deepEqual(receipt.source_scope, expectedScope);
  assert.equal(receipt.gate_sha256, sha(read('verification/dx06/gate.mjs')));
  const sourceFiles = sourceInventory(expectedScope);
  assert.equal(Object.keys(sourceFiles).length, receipt.source_files);
  assert.equal(`sha256:${sha(JSON.stringify(sourceFiles))}`, receipt.source_revision,
    'source scope differs from the frozen source revision');

  const directory = receipt.retained.directory;
  assert.equal(fs.realpathSync(directory), directory, 'frozen output directory changed');
  assert.equal(receipt.retained.terminal_smoke, 'smoke.json');
  const frozenSmoke = smokeBytes ?? fs.readFileSync(path.join(directory, 'smoke.json'));
  assert.equal(sha(frozenSmoke), receipt.retained.terminal_smoke_sha256,
    'terminal smoke bytes differ from immutable receipt');
  const smoke = JSON.parse(frozenSmoke);
  assert.deepEqual([smoke.case_id, smoke.profile, smoke.result, smoke.proof],
    ['DX-06', 'Test-Host-Draft', 'six-controls-pass', 'open']);
  assert.equal(smoke.source_root, root);
  assert.equal(smoke.source_revision, receipt.source_revision);
  assert.deepEqual(smoke.source_files, sourceFiles);
  assert.equal(smoke.pre_promotion_case_sha256, receipt.pre_promotion_case_sha256);
  assert.equal(smoke.production_cli_sha256, receipt.retained.frozen_cli_sha256);
  assert.equal(smoke.selected_node_sha256, receipt.retained.selected_node_sha256);
  assert.equal(smoke.compiled_wasm_sha256, receipt.retained.compiled_wasm_sha256);
  assert.deepEqual(smoke.latent_effect_witness, ['test.clock']);
  assert.equal(smoke.real_host_evidence, false);
  assert.equal(smoke.real_host_fallback_calls, 0);
  assert.deepEqual(Object.keys(smoke.controls), expectedVariants);
  assert.equal(Object.keys(smoke.retained).length, receipt.retained.files_before_terminal_smoke);
  const actual = inventory(directory, directory);
  assert.deepEqual(Object.keys(actual).sort(),
    [...Object.keys(smoke.retained), 'smoke.json'].sort(),
    'frozen artifacts added, removed or renamed');
  for (const [relative, record] of Object.entries(smoke.retained)) {
    assert.deepEqual(actual[relative], record, `frozen artifact changed: ${relative}`);
  }
  assert.equal(actual['smoke.json'].sha256, receipt.retained.terminal_smoke_sha256);
  assert.equal(smoke.retained[receipt.retained.frozen_cli]?.sha256,
    receipt.retained.frozen_cli_sha256);
  assert.equal(smoke.retained[receipt.retained.compiled_wasm]?.sha256,
    receipt.retained.compiled_wasm_sha256);
  const selection = JSON.parse(read('crates/noble-cli/src/core/runtime/config.json'));
  assert.equal(selection.tools.wasm_tools.path, receipt.retained.selected_wasm_tools);
  assert.equal(sha(fs.readFileSync(selection.tools.wasm_tools.path)),
    receipt.retained.selected_wasm_tools_sha256);
  assert.equal(sha(fs.readFileSync(selection.tools.node.path)),
    receipt.retained.selected_node_sha256);
  assert.equal(smoke.commands.length, receipt.retained.production_cli_commands);
  assert.deepEqual(smoke.commands.map(command => [command.label, command.status]), expectedCommands);
  for (const command of smoke.commands) {
    const stem = command.label === 'incompatible-interface' ? 'incompatible' : command.label;
    for (const kind of ['stdout', 'stderr']) {
      assert.equal(smoke.retained[`${stem}.${kind}`]?.sha256,
        command[`${kind}_sha256`], `${stem} ${kind} changed`);
    }
  }

  const promoted = (caseBytes ?? read(casePath)).toString('utf8');
  const promotedPacket = JSON.parse(promoted);
  const dx01 = promotedPacket.cases.filter(row => row.id === 'DX-01');
  assert.equal(dx01.length, 1, 'DX-01 unique current-source row');
  assert.equal(dx01[0].evidence.length, 2, 'current-source DX-01 replay promotion missing');
  // This authenticates and strips only the later DX-01 replay evidence, then
  // proves that reverse DX-06 projection reconstructs the older full file.
  const restored = restoreReplayEvidence(promoted, casePath);
  const packet = JSON.parse(restored);
  assert.equal(packet.cases.filter(row => row.id === 'DX-06').length, 1);
  const row = packet.cases.find(item => item.id === 'DX-06');
  assert.equal(row.profile, 'Test-Host-Draft');
  assert.equal(row.kind, 'adapter');
  assert.deepEqual(row.requirements, ['DX-HOST-01', 'DX-HOST-02']);
  assert.deepEqual(row.input, { harness: 'explicit-scripted-test-host', operation: 'test.clock',
    script: [42], variants: expectedVariants });
  assert.deepEqual(row.expected, { stage: 'adapter',
    outcome: 'authorized-returns-42-all-other-variants-fail-explicitly',
    real_host_fallback_calls: 0, declared_effect_preserved: true,
    denied_request_recorded: true,
    report_fields: ['substitutions', 'adapter_versions', 'scripted_inputs', 'host_requests'],
    real_host_evidence: false });
  assert.deepEqual(row.state, { implementation: 'implemented', execution: 'passed',
    proof: 'open', trust: 'explicit' });
  assert.equal(row.evidence.length, 1);
  const evidence = row.evidence[0];
  assert.deepEqual([evidence.kind, evidence.subject, evidence.revision, evidence.result],
    ['test', 'DX-06', packet.revision, 'passed']);
  assert.equal(evidence.source_revision, receipt.source_revision);
  assert.deepEqual(evidence.assumptions, expectedAssumptions);
  assert.deepEqual(evidence.configuration, {
    profile: receipt.profile, case: receipt.case_id,
    receipt: path.posix.relative(path.posix.dirname(casePath), receiptPath),
    receipt_sha256: receiptSha256, raw_artifacts: directory, canonical_variants: 6,
    additional_latent_effect_witness: true, production_cli_commands: 6,
    frozen_cli_sha256: receipt.retained.frozen_cli_sha256,
  });
  assert.ok(typeof evidence.claim === 'string' && evidence.claim.length > 0);
  const anchor = '"id": "DX-06"';
  assert.equal(restored.split(anchor).length - 1, 1, 'DX-06 exact textual identifier');
  const id = restored.indexOf(anchor);
  const start = restored.indexOf('      "state": ', id);
  const end = restored.indexOf('\n    }\n  ]', start);
  assert.ok(start > id && end > start && restored.indexOf('"id": "', start) < 0,
    'DX-06 is the final canonical row and only its state/evidence may be projected');
  assert.equal(sha(restored.slice(start, end)), promotedBlockSha256,
    'DX-06 promoted state/evidence bytes changed');
  const prepromotion = restored.slice(0, start) + prior + restored.slice(end);
  assert.equal(sha(prepromotion), receipt.pre_promotion_case_sha256,
    'reverse DX-06 projection did not recover the prepromotion full-file bytes');
  return { result: 'passed', case_id: 'DX-06', receipt_sha256: receiptSha256,
    source_revision: receipt.source_revision, terminal_smoke_sha256: sha(frozenSmoke),
    pre_promotion_case_sha256: sha(prepromotion),
    current_source_replay_stripped_first: true, real_host_evidence: false, proof: 'open' };
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  console.log(JSON.stringify(verifyDx06Postpromotion()));
}
