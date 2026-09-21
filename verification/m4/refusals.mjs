// Controls mutate fresh extraction data or fresh Lean input modules. They never
// alter repository sources, reviewed expectations, or the positive audit input.
import fs from 'node:fs';
import path from 'node:path';
import { account, auditPacket, auditPackets, rootsFor, lanes, equal, exactBytes, fail } from './accounting.mjs';

export function refusals({ raw, accounts, sources, tools, packet, packets, audit, run, proofRoot, lake, artifacts, save }) {
  const results = [];
  function reject(label, code, action) {
    try { action(); } catch (error) {
      if (!String(error).includes(`M4-${code}:`)) fail('REFUSAL', `${label}: wrong failure: ${error}`);
      results.push({ label, expected: `M4-${code}`, result: 'refused', diagnostic: String(error),
        kind: 'mutated-extraction-accounting-control; not additional Rust extraction' });
      return;
    }
    fail('REFUSAL', `${label}: mutation was accepted`);
  }
  const wasm = lanes.find(lane => lane.id === 'wasm');
  const required = rootsFor(accounts).find(root => root.lane === 'wasm' && root.method === 'prepare');
  const subject = raw.wasm.translation.functions.find(item => item.lean_name === required.declaration);
  const copied = () => ({ llbc: structuredClone(raw.wasm.llbc), translation: structuredClone(raw.wasm.translation) });
  reject('omitted-local-body', 'OMITTED-BODY', () => {
    const changed = copied();
    changed.translation.functions = changed.translation.functions.filter(item => item.def_id !== subject.def_id);
    account(wasm, changed.llbc, changed.translation, sources);
  });
  reject('omitted-function-with-retained-loop-helpers', 'OMITTED-BODY', () => {
    const changed = copied();
    const loop = changed.translation.functions.find(item => item.is_local && item.lean_name.endsWith('_loop.body'));
    if (!loop) fail('REFUSAL', 'actual Wasm translation unexpectedly contains no loop helper');
    changed.translation.functions = changed.translation.functions.filter(item => item.def_id !== loop.def_id ||
      item.lean_name.endsWith('_loop.body') || item.lean_name.endsWith('_loop'));
    account(wasm, changed.llbc, changed.translation, sources);
  });
  reject('new-local-opaque-body', 'LOCAL-OPAQUE', () => {
    const changed = copied();
    changed.llbc.translated.fun_decls[subject.def_id].body = 'Opaque';
    // An opaque parent has no translated loop helpers. Keep this a coherent
    // opaque-body mutation, not an earlier invalid loop-origin rejection.
    changed.translation.functions = changed.translation.functions.filter(item =>
      item.def_id !== subject.def_id || !item.loop);
    for (const item of changed.translation.functions.filter(item => item.def_id === subject.def_id)) {
      item.is_opaque = true;
      item.lean_file = 'FunsExternal_Template.lean';
    }
    account(wasm, changed.llbc, changed.translation, sources);
  });
  reject('selective-extraction', 'EXTRACTION-SCOPE', () => {
    const changed = copied();
    changed.llbc.translated.options.start_from = ['noble_wasm::compile'];
    account(wasm, changed.llbc, changed.translation, sources);
  });
  reject('collector-source-drift', 'LLBC-SOURCE', () => {
    const changed = copied();
    changed.llbc.translated.files.find(file => file?.crate_name === 'noble_wasm').contents += '\n';
    account(wasm, changed.llbc, changed.translation, sources);
  });
  reject('contradictory-normalized-source-alias', 'LLBC-SOURCE', () => {
    const changed = copied();
    const file = changed.llbc.translated.files.find(item => item?.crate_name === 'noble_wasm');
    const components = file.name.Local.split('/');
    components.splice(-1, 0, '.');
    changed.llbc.translated.files.push({ ...file, name: { Local: components.join('/') }, contents: file.contents + '\n' });
    account(wasm, changed.llbc, changed.translation, sources);
  });
  reject('forged-local-origin', 'INVENTORY', () => {
    const changed = copied();
    for (const item of changed.translation.functions.filter(item => item.def_id === subject.def_id)) {
      item.is_local = false;
    }
    account(wasm, changed.llbc, changed.translation, sources);
  });
  reject('forged-loop-macro-origin', 'LOCAL-OWNER', () => {
    const kernel = lanes.find(lane => lane.id === 'kernel');
    const translation = structuredClone(raw.kernel.translation);
    const loop = translation.functions.find(item => item.is_local && item.loop &&
      item.source.file.startsWith('/rustc/'));
    if (!loop) fail('REFUSAL', 'actual kernel translation has no standard-macro loop span');
    loop.source.file = '/not-a-compiler-source';
    account(kernel, raw.kernel.llbc, translation, sources);
  });
  reject('omitted-required-frontend-root', 'REQUIRED-ROOT', () => {
    const changed = structuredClone(accounts);
    changed.contracts.translation.functions = changed.contracts.translation.functions.filter(item =>
      !(item.source.file.startsWith('crates/noble-contracts/src/source') && item.rust_name.endsWith('::Session}::prepare')));
    rootsFor(changed);
  });
  reject('source-receipt-drift', 'SOURCE', () => equal({ ...sources, 'forged.rs': 'changed' }, sources, 'SOURCE', 'control'));
  reject('tool-receipt-drift', 'TOOL', () => equal({ ...tools, lean: { ...tools.lean, sha256: 'changed' } }, tools, 'TOOL', 'control'));
  reject('stale-generated-file', 'GENERATED', () => {
    const generated = fs.readFileSync(path.join(artifacts, 'wasm/generated/Funs.lean'));
    exactBytes(Buffer.concat([generated, Buffer.from('\n-- stale generated bytes\n')]), generated, 'control');
  });
  reject('unexpected-logical-dependency', 'DEPENDENCIES', () => {
    const changed = structuredClone(audit);
    changed.wasm.root_audits[0].reachable_project_declarations.push('noble_wasm.unreviewed_dependency');
    equal(changed, audit, 'DEPENDENCIES', 'control');
  });
  reject('unassigned-audit-owner', 'PACKET', () => {
    const changed = structuredClone(packet);
    changed.declarations[0].owner = 'Unreviewed.Funs';
    auditPackets(changed);
  });
  reject('unassigned-audit-lane', 'PACKET', () => {
    const changed = structuredClone(packet);
    changed.boundaries[0].lane = 'unreviewed';
    auditPackets(changed);
  });
  reject('missing-new-kernel-metadata', 'REQUIRED-ROOT', () => {
    const changed = structuredClone(accounts);
    changed.kernel.translation.types = changed.kernel.translation.types.filter(item => item.rust_name !== 'noble_kernel::execution::Submission');
    auditPacket(changed, packet.retained_models);
  });

  function lean(label, code, prelude, mutate = () => {}, lane = 'wasm') {
    const input = structuredClone(packets[lane]);
    mutate(input);
    const inputFile = path.join(artifacts, `refusal-${label}.json`);
    fs.writeFileSync(inputFile, JSON.stringify(input) + '\n', { flag: 'wx' });
    const source = `M4Refusal_${label.replaceAll('-', '_')}.lean`;
    const subject = lane === 'contracts' ? 'NobleContractImpl.Projection' : 'NobleWasmImpl';
    fs.writeFileSync(path.join(proofRoot, source),
      `import ${subject}\nimport M4Audit\n\n${prelude}\n\ncheck_m4_extraction "${lane}"\n`, { flag: 'wx' });
    const observation = run(`refusal-${label}`, lake, ['env', 'lean', source], proofRoot,
      { NOBLE_M4_EXTRACTION_SUBJECTS: inputFile }, true);
    if (observation.exit === 0 || observation.signal || observation.error ||
        !observation.output.includes(`M4-${code}:`)) fail('REFUSAL', `${label}: failure was not the required compiled audit refusal`);
    results.push({ label, expected: `M4-${code}`, result: 'refused', kind: 'executed-Lean-audit-control',
      log: observation.log, output_sha256: observation.output_sha256 });
    save('refusals.json', results);
  }
  lean('unused-model-axiom', 'AXIOM', 'axiom noble_wasm.m4_refusal_axiom : Bool');
  lean('unused-proof-hole', 'AXIOM', 'theorem noble_wasm.m4_refusal_hole : True := by sorry');
  lean('forged-native-permission', 'AXIOM', 'axiom noble_wasm.m4_refusal._native.decide.ax_1 : True');
  lean('native-strict-theorem', 'AXIOM', 'theorem noble_wasm.KernelBridge.m4_refusal_native : (17 : Nat) + 1 = 18 := by native_decide');
  lean('handwritten-local-substitute', 'OWNER', 'def noble_wasm.m4_refusal_local : Bool := true', input => {
    input.declarations.push({ name: 'noble_wasm.m4_refusal_local', owner: 'NobleWasmImpl.Funs',
      transparent: true, kind: 'functions', local: true });
  });
  lean('opaque-boundary-body', 'TRANSPARENT', 'opaque noble_wasm.m4_refusal_opaque : Bool := true', input => {
    input.boundaries.push({ name: 'noble_wasm.m4_refusal_opaque', lane: 'wasm', kind: 'functions', rust: 'control', inherited_opaque: false });
  });
  lean('unreviewed-boundary-owner', 'BOUNDARY-OWNER', 'def noble_wasm.m4_refusal_boundary : Bool := true', input => {
    input.boundaries.push({ name: 'noble_wasm.m4_refusal_boundary', lane: 'wasm', kind: 'functions', rust: 'control', inherited_opaque: false });
  });
  lean('substituted-extracted-dependency', 'BRIDGE', 'def noble_wasm.m4_refusal_actual : Bool := true', input => {
    input.bridges[0].actual = 'noble_wasm.m4_refusal_actual';
  });
  lean('mismatched-audit-lane', 'PACKET', '', input => { input.lane = 'contracts'; });
  lean('postulated-external-type', 'TRANSPARENT', 'axiom noble_wasm.m4_refusal_type : Type', input => {
    input.boundaries[0] = { ...input.boundaries[0], name: 'noble_wasm.m4_refusal_type', kind: 'types' };
  });
  lean('frontend-unused-model-axiom', 'AXIOM', 'axiom noble_contracts.m4_refusal_axiom : Bool',
    () => {}, 'contracts');
  save('refusals.json', results);
  return results;
}
