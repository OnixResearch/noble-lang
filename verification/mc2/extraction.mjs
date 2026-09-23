// MC2 adds required roots to the existing whole-crate accounting route.
// It does not replace the compiler inventory with handwritten function counts.
import { equal, fail } from '../m4/accounting.mjs';

const directory = 'crates/noble-contracts/src/companion/';
const methods = ['admit', 'begin_admission', 'complete_admission', 'replay',
  'derive_compose', 'instantiate', 'bind', 'bind_artifact', 'release', 'correspond',
  'guard_templates', 'wrapper_source', 'invocation_wrapper', 'observe_and_register',
  'set_policy', 'set_semantic_revision', 'set_host_contract', 'set_environment_fact'];

export function companionPacket(accounts, sources, llbc) {
  const inventory = accounts.contracts;
  const sourceFiles = Object.fromEntries(Object.entries(sources).filter(([file]) =>
    file.startsWith(directory) && file.endsWith('.rs')));
  if (!Object.keys(sourceFiles).length) fail('MC2-SOURCE', 'missing production companion source');
  for (const [file, digest] of Object.entries(sourceFiles)) {
    equal(inventory.collected_files[file], digest, 'MC2-SOURCE', `uncollected companion source: ${file}`);
  }
  // Generated loop bodies may carry a standard macro's span. Compiler-owned
  // Rust names, not the macro's file, bind those bodies to this production module.
  const bodies = inventory.translation.functions.filter(row =>
    row.is_local && row.rust_name.startsWith('noble_contracts::companion::'));
  if (!bodies.length || bodies.some(row => row.is_opaque || row.lean_file !== 'Funs.lean')) {
    fail('MC2-BODY', 'companion authored bodies must be actual transparent translations');
  }
  if (bodies.some(row => row.rust_name.endsWith('::Core}::admit_checked')) ||
      inventory.translation.trait_decls.some(row => row.is_local &&
        row.rust_name.startsWith('noble_contracts::companion::') &&
        row.rust_name.endsWith('::IndependentVerifier'))) {
    fail('MC2-OBSERVATION', 'obsolete callback-based admission boundary remains');
  }
  const rootFor = (type, method) => {
    const receiver = `noble_contracts::companion::${type === 'Core' ? '' : 'admit::'}${type}`;
    const regions = type === 'AdmissionRequest' ? "(?:<'[A-Za-z_][A-Za-z_0-9]*>)?" : '';
    const identity = new RegExp(`\\{${receiver}${regions}\\}::${method}$`);
    const matches = bodies.filter(row => !row.loop &&
      identity.test(row.rust_name));
    if (matches.length !== 1) fail('MC2-ROOT', `${type}::${method}: found ${matches.length}`);
    return { lane: 'contracts', kind: 'functions', rust: matches[0].rust_name,
      declaration: matches[0].lean_name, owner: 'NobleContractImpl.Funs',
      requires_acceptance: false };
  };
  const roots = methods.map(method => rootFor('Core', method));
  const constructor = rootFor('CheckObservation', 'new');
  const expected = rootFor('AdmissionRequest', 'expected');
  const offer = rootFor('AdmissionRequest', 'offer');
  roots.push(constructor, expected, offer);
  const concreteType = type => {
    const matches = inventory.translation.types.filter(row => row.is_local &&
      row.rust_name.startsWith('noble_contracts::companion::') &&
      row.rust_name.endsWith(`::${type}`));
    if (matches.length !== 1 || matches[0].lean_file !== 'Types.lean') {
      fail('MC2-OBSERVATION', `missing generated concrete ${type} type`);
    }
    const compiler = llbc.translated.type_decls[matches[0].def_id];
    if (!Array.isArray(compiler?.kind?.Struct) || !compiler.kind.Struct.length) {
      fail('MC2-OBSERVATION', `${type} is not a compiler-confirmed concrete record`);
    }
    const declaration = matches[0].lean_name.startsWith('noble_contracts.')
      ? matches[0].lean_name : `noble_contracts.${matches[0].lean_name}`;
    return { declaration, compiler_type_id: compiler.def_id, source: matches[0].source };
  };
  const boundary = {
    classification: 'authentic-sound-host-check-observation',
    request: concreteType('AdmissionRequest'), observation: concreteType('CheckObservation'),
    observation_constructor: constructor.declaration,
    expected_prepared_accessor: expected.declaration, submitted_offer_accessor: offer.declaration,
    begin: roots.find(row => row.rust.endsWith('::Core}::begin_admission')).declaration,
    complete: roots.find(row => row.rust.endsWith('::Core}::complete_admission')).declaration,
    assumption: 'The source-bound CLI constructs CheckObservation only from the actual isolated independent Lean check of the exact statement and submitted source bytes. Arbitrary host Rust can construct observations; authenticity and host-checker soundness are explicit boundary assumptions, not proved by a data constructor.'
  };
  return { schema: 'mc2-extraction-subjects/v2', source_files: sourceFiles,
    bodies: bodies.map(row => ({ declaration: row.lean_name, source: row.source,
      def_id: row.def_id, rust: row.rust_name, loop: row.loop ?? null })), roots,
    host_observation: boundary };
}

export function companionCoverage(packet, audit) {
  const declarations = new Map(audit.project_declarations.map(row => [row.declaration, row]));
  const bodies = packet.bodies.map(row => {
    const compiled = declarations.get(row.declaration);
    if (!compiled || compiled.owner !== 'NobleContractImpl.Funs' || compiled.kind !== 'definition') {
      fail('MC2-COMPILED', `missing/substituted compiled body: ${row.declaration}`);
    }
    return { ...row, owner: compiled.owner, axioms: compiled.axioms };
  });
  const roots = packet.roots.map(row => {
    const compiled = audit.root_audits.find(item => item.declaration === row.declaration);
    if (!compiled) fail('MC2-COMPILED', `missing root audit: ${row.declaration}`);
    return compiled;
  });
  const boundary = packet.host_observation;
  for (const type of [boundary.request, boundary.observation]) {
    const compiled = declarations.get(type.declaration);
    if (compiled?.owner !== 'NobleContractImpl.Types' || compiled.kind !== 'inductive') {
      fail('MC2-OBSERVATION', `missing/substituted compiled observation record: ${type.declaration}`);
    }
  }
  for (const [root, requiredTypes] of [
    [boundary.begin, [boundary.request.declaration]],
    [boundary.complete, [boundary.request.declaration, boundary.observation.declaration]],
    [boundary.observation_constructor, [boundary.observation.declaration]]
  ]) {
    const compiled = roots.find(row => row.declaration === root);
    for (const type of requiredTypes) {
      if (!compiled?.reachable_project_declarations.includes(type)) {
        fail('MC2-OBSERVATION', `${root} is detached from concrete ${type}`);
      }
    }
  }
  return { schema: 'mc2-compiled-coverage/v2', source_files: packet.source_files, bodies, roots,
    host_observation: boundary };
}
