#!/usr/bin/env node
// Causal mutation checks for the native-async-interfaces gate. Run from a
// throwaway worktree at the tested commit; each mutation is semantic (one
// shell obligation removed), committed, and must make the gate fail.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const root=fs.realpathSync(process.cwd());
const hostPath=path.join(root,'verification/worker-send/peer/src/host.rs');
const MUTATIONS={
  'suspension-not-observed':{
    find:`        self.suspension(callback)?;\n        self.retire(callback,Retire::Suspension)`,
    replace:`        self.retire(callback,Retire::Suspension)`,
    claim:'a suspended guest must observe its still-pending owner before retirement',
  },
  'delivery-does-not-return-caller-owner':{
    find:`        if let Some(owner)=returned {\n            self.owner_returns+=1;\n            self.owner=Some(owner);\n        }`,
    replace:`        if let Some(owner)=returned {\n            self.owner_returns+=1;\n            drop(owner);\n        }`,
    claim:'one-shot delivery must return the caller-owned resource',
  },
  'invocation-receipt-not-issued':{
    find:`            let receipt=checked(self.authority.deliver_success(attempt,&observation))?;`,
    replace:`            let receipt=checked(self.authority.receipt(attempt,
                auth::ReceiptClaim::OperationSuccess,Some(&observation)))?;`,
    claim:'the guest explicit finish must approve the invocation delivery receipt',
  },
};
function main(){
  const name=process.argv[2];
  const mutation=MUTATIONS[name];
  assert.ok(mutation,`usage: mutations.mjs ${Object.keys(MUTATIONS).join('|')}`);
  const original=fs.readFileSync(hostPath,'utf8');
  assert.ok(original.includes(mutation.find),`mutation ${name} anchor missing`);
  fs.writeFileSync(hostPath,original.replace(mutation.find,mutation.replace));
  const commit=spawnSync('git',['-c','user.email=mut@local','-c','user.name=mut',
    'commit','-am',`mutation: ${name}`],{cwd:root,encoding:'utf8'});
  assert.equal(commit.status,0,commit.stderr);
  const out=path.join('/var/tmp',`worker-send-mutation-${name}-${process.pid}`);
  const gate=spawnSync(process.execPath,[path.join(root,'verification/worker-send/gate.mjs'),
    '--out',out],{cwd:root,encoding:'utf8',maxBuffer:256*1024*1024,timeout:1_800_000});
  assert.notEqual(gate.status,0,`mutation ${name} did not fail the gate:\n${gate.stdout}`);
  const tried=path.join(out,'receipt.json');
  const reasoned=failures=>failures.length
    ? failures
    : [/AssertionError(?: \[[^\]]*\])?: ([^\n]*)/.exec(gate.stderr)?.at(1)
      || (gate.stdout.split('\n').filter(Boolean).at(-1)) || 'gate failed'];
  const evidence=fs.existsSync(tried)
    ? JSON.parse(fs.readFileSync(tried,'utf8'))
    : {result:'failed',failures:reasoned([])};
  assert.equal(evidence.result,'failed',`mutation ${name} produced no failure receipt`);
  assert.ok(evidence.failures.length,`mutation ${name} failed without a reason`);
  console.log(JSON.stringify({mutation:name,claim:mutation.claim,
    gate_exit:gate.status,failures:evidence.failures,
    source_revision:evidence.source_revision||null,
    failing_output_sha256:crypto.createHash('sha256')
      .update(gate.stdout+gate.stderr).digest('hex'),commit:commit.stdout}));
  process.exitCode=0;
}
main();
