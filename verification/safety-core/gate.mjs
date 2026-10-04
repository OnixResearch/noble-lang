#!/usr/bin/env node
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const [destination] = process.argv.slice(2);
assert.ok(destination && process.argv.length === 3, 'usage: node verification/safety-core/gate.mjs NEW_EXTERNAL_DIRECTORY');
const artifacts = path.resolve(destination);
assert.ok(artifacts !== root && !artifacts.startsWith(`${root}/`), 'artifact directory must be external');
fs.mkdirSync(artifacts);
fs.mkdirSync(path.join(artifacts, 'inputs'));
const sha = value => crypto.createHash('sha256').update(value).digest('hex');
const bytes = file => fs.readFileSync(path.join(root, file));
const cases = JSON.parse(bytes('specs/conformance/safety-cases.json'));
const byId = id => cases.cases.find(row => row.id === id);
const chosen = ['S-CASE-01', 'S-CASE-05', 'S-CASE-14'].map(byId);
assert.ok(chosen.every(row => row?.kind === 'static' && row.state.execution === 'not-run' && row.state.proof === 'open'));
const [typed, effect, unsafe] = chosen;
assert.equal(typed.input.source, 'true 1 +');
assert.equal(effect.input.source, '"audit" test.emit');
assert.equal(effect.input.host_contracts['test.emit'], 'Text -- ! {test.emit}');
assert.deepEqual(effect.input.allowed_effects, []);
assert.equal(unsafe.input.namespace, 'bootstrap-only');
assert.deepEqual(unsafe.input.sources, ['0 raw-pointer.read', '1 transmute']);
fs.writeFileSync(path.join(artifacts, 'safety-cases.prepromotion.json'), bytes('specs/conformance/safety-cases.json'), {flag:'wx'});
const selected = JSON.parse(bytes('policy/tool-selection.json'));
const rust = path.join(selected.tool_paths.quality_rust.output, 'bin');
const environment = {
  ...process.env, HOME: path.join(artifacts, 'home'), CARGO_HOME: path.join(artifacts, 'cargo-home'),
  CARGO_TARGET_DIR: path.join(artifacts, 'target'), TMPDIR: path.join(artifacts, 'tmp'),
  RUSTC_WRAPPER: '', RUSTC_WORKSPACE_WRAPPER: '', RUSTFLAGS: '', CARGO_ENCODED_RUSTFLAGS: '',
  PATH: `${rust}:${selected.component_sync.linker_bin}:/run/current-system/sw/bin`,
};
for (const name of ['home','cargo-home','tmp']) fs.mkdirSync(path.join(artifacts, name));
const sources = {};
const add = file => { sources[file] = sha(bytes(file)); };
const tree = directory => {
  for (const entry of fs.readdirSync(path.join(root, directory), {withFileTypes:true}).sort((a,b) => a.name.localeCompare(b.name))) {
    const file = `${directory}/${entry.name}`;
    assert.equal(entry.isSymbolicLink(), false, `unbound source symlink ${file}`);
    if (entry.isDirectory()) tree(file);
    else if (entry.isFile()) add(file);
  }
};
for (const directory of ['crates/noble-kernel/src','crates/noble-contracts/src',
  'crates/noble-wasm/src','crates/noble-cli/src']) tree(directory);
for (const file of ['Cargo.toml','Cargo.lock','rust-toolchain.toml','policy/tool-selection.json',
  'specs/conformance/safety-cases.json','.cairn/specs/safety/spec.md',
  'verification/safety-core/check.rs','verification/safety-core/gate.mjs']) add(file);
const commands = [];
function run(label, executable, args, expected) {
  const index = commands.length;
  const result = spawnSync(executable, args, {
    cwd:root, env:environment, encoding:'utf8', timeout:900_000,
    maxBuffer:16 * 1024 * 1024,
  });
  const stem = `${String(index).padStart(2,'0')}-${label}`;
  fs.writeFileSync(path.join(artifacts, `${stem}.stdout`), result.stdout ?? '', {flag:'wx'});
  fs.writeFileSync(path.join(artifacts, `${stem}.stderr`), result.stderr ?? '', {flag:'wx'});
  const row = { label, executable, executable_sha256:sha(fs.readFileSync(executable)), args,
    status:result.status, signal:result.signal, error:result.error?.message ?? null,
    stdout:`${stem}.stdout`, stderr:`${stem}.stderr`, stdout_sha256:sha(result.stdout ?? ''),
    stderr_sha256:sha(result.stderr ?? '') };
  commands.push(row);
  assert.equal(row.error,null,`${label} failed to spawn`);
  assert.equal(row.signal,null,`${label} signaled`);
  assert.equal(row.status,expected,`${label} returned ${row.status}: ${result.stderr}`);
  return result.stdout;
}
const input = (name, source) => {
  const file = path.join(artifacts, 'inputs', `${name}.noble`);
  fs.writeFileSync(file,source,{flag:'wx'});
  return file;
};
const inputPaths = [input('S-CASE-01',typed.input.source),input('S-CASE-05',effect.input.source),
  ...unsafe.input.sources.map((source,index) => input(`S-CASE-14-${index}`,source))];
const cargoOutput = run('build',path.join(rust,'cargo'),
  ['build','-p','noble-cli','-p','noble-contracts','--all-features','--locked','--offline','--message-format=json'],0);
const artifact = (name,kind) => {
  const candidates = cargoOutput.split('\n').filter(line => line.startsWith('{'))
    .map(line => JSON.parse(line)).filter(item => item.reason === 'compiler-artifact' &&
      item.target.name === name && item.target.kind.includes(kind));
  const files = [...new Set(candidates.flatMap(item => kind === 'bin' ? [item.executable] :
    item.filenames.filter(file => file.endsWith('.rlib'))).filter(Boolean))];
  assert.equal(files.length,1,`expected unique compiled ${name}/${kind}`);
  return files[0];
};
const binary = artifact('noble','bin');
const runner = path.join(artifacts,'safety-core-check');
run('check-build',path.join(rust,'rustc'),['--edition=2021','--crate-name','safety_core_check',
  'verification/safety-core/check.rs','--extern',`noble_contracts=${artifact('noble_contracts','lib')}`,
  '--extern',`noble_kernel=${artifact('noble_kernel','lib')}`,
  '-L',`dependency=${path.join(artifacts,'target/debug/deps')}`,'-o',runner],0);
const raw = run('independent-source-and-kernel',runner,inputPaths,0);
const verified = raw.trim().split('\n').map(line => JSON.parse(line));
assert.deepEqual(verified.map(row => row.id),chosen.map(row => row.id));
for (const [row, design] of verified.map((row,index) => [row,chosen[index]])) {
  for (const [key,value] of Object.entries(design.expected)) assert.deepEqual(row[key],value,`${row.id}.${key}`);
}
assert.equal(verified[2].variants,unsafe.input.sources.length);
assert.equal(verified[1].constraint,'EffectInclusion(test.emit)');
const cli = [];
for (const [id, file, expected] of [
  ['S-CASE-01',inputPaths[0],typed.expected],
  ['S-CASE-14/pointer',inputPaths[2],unsafe.expected],
  ['S-CASE-14/transmute',inputPaths[3],unsafe.expected],
]) {
  const report = JSON.parse(run(`cli-${id.replace('/','-')}`,binary,['compile',file],2).trim());
  for (const [key,value] of Object.entries(expected)) assert.deepEqual(report[key],value,`${id}.${key}`);
  assert.equal(report.candidate_prepare_requests,0);
  cli.push({id,stage:report.stage,outcome:report.outcome,
    guest_requests:report.guest_requests,protected_operations:report.protected_operations});
}
for (const [file,digest] of Object.entries(sources)) assert.equal(sha(bytes(file)),digest,`source changed mid-flight: ${file}`);
const receipt = {schema:'noble-safety-core-receipt/v1',result:'passed',kind:'test',
  cases:chosen.map(({id,input,expected}) => ({id,input,expected})),
  claim:'Exact Core-Bootstrap type/resolve refusals and independently bounded test.emit effect refusal execute with zero guest requests or protected operations; no runtime or universal proof is claimed.',
  source_revision:`sha256:${sha(JSON.stringify(Object.entries(sources).sort()))}`,
  source_sha256:sources,prepromotion_case_sha256:sources['specs/conformance/safety-cases.json'],
  binary:{path:binary,sha256:sha(fs.readFileSync(binary))},
  check_binary:{path:runner,sha256:sha(fs.readFileSync(runner))},
  selected_rust:selected.tool_paths.quality_rust,
  commands,observed:{independent:verified,cli},
  assumptions:['The Rust compiler and source/kernel checker are trusted tooling; tests do not prove universal type/effect soundness.',
    'The selected bootstrap resource-free test.emit contract is an explicit test host, not general authority or a protected operation.',
    'No guest code executes in these static/independent-admission checks; execution and proof remain separate.']};
fs.writeFileSync(path.join(artifacts,'acceptance.json'),JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({result:receipt.result,receipt:path.join(artifacts,'acceptance.json'),
  binary:receipt.binary,check_binary:receipt.check_binary,source_revision:receipt.source_revision,
  prepromotion_case_sha256:receipt.prepromotion_case_sha256,observed:receipt.observed}));
