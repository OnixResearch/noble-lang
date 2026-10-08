// Only concatenated into the explicitly feature-built live test worker.
// Private FIFO handshakes delay a real V8 guest callback or checked candidate
// compilation. The exact LIVE06 control below revokes host authority, not
// guest bytes, and is absent from the default worker.
{
  const directory = process.env.NOBLE_LIVE_TEST_BARRIER_DIR;
  const point = process.env.NOBLE_LIVE_TEST_BARRIER_POINT;
  const expected = process.env.NOBLE_LIVE_TEST_BARRIER_EXPECTED;
  for (const key of ['NOBLE_LIVE_TEST_BARRIER_DIR', 'NOBLE_LIVE_TEST_BARRIER_POINT',
    'NOBLE_LIVE_TEST_BARRIER_EXPECTED']) delete process.env[key];
  if (directory || point || expected) {
    if (typeof directory !== 'string' || !path.isAbsolute(directory)
      || !['in-flight-generation', 'candidate-compiled'].includes(point)
      || typeof expected !== 'string'
      || point === 'candidate-compiled' && !/^[a-f0-9]{64}$/.test(expected)
      || point === 'in-flight-generation' && !/^[1-9][0-9]*$/.test(expected)) {
      fail('invalid private live test barrier selection');
    }
    const metadata = fs.lstatSync(directory);
    if (!metadata.isDirectory() || metadata.uid !== process.getuid()
      || (metadata.mode & 0o077) !== 0) fail('untrusted private live test barrier directory');
    const reached = path.join(directory, 'reached');
    const release = path.join(directory, 'release');
    for (const name of [reached, release]) {
      const entry = fs.lstatSync(name);
      if (!entry.isFIFO() || entry.uid !== process.getuid()
        || (entry.mode & 0o077) !== 0) fail('untrusted private live test barrier FIFO');
    }
    let used = false;
    const pause = (owner, location, value, checkedWasmModule = false, wasmSha256 = null) => {
      if (used || !owner.inProcessLive || owner.declaredLive
        || location !== point || value !== expected) return;
      used = true;
      const event = Buffer.from(JSON.stringify({ point: location, matched: value,
        source_generation: owner.liveSourceGeneration.toString(),
        guest_frame_active: owner.liveExecuting,
        checked_wasm_module: checkedWasmModule,
        checked_wasm_sha256: wasmSha256 }) + '\n');
      const notified = fs.openSync(reached, fs.constants.O_WRONLY | fs.constants.O_NOFOLLOW);
      try { if (fs.writeSync(notified, event) !== event.length) fail('short live barrier event'); }
      finally { fs.closeSync(notified); }
      const permitted = fs.openSync(release, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW);
      try {
        const token = Buffer.alloc(1);
        if (fs.readSync(permitted, token, 0, 1, null) !== 1 || token[0] !== 0x52) {
          fail('invalid live barrier release');
        }
      } finally { fs.closeSync(permitted); }
    };
    if (point === 'candidate-compiled') {
      const original = CoreEngine.prototype.liveModule;
      CoreEngine.prototype.liveModule = function (...args) {
        const result = original.apply(this, args);
        if (!(result.module instanceof WebAssembly.Module)) {
          fail('candidate barrier did not follow real Wasm compilation');
        }
        // The checked V8.Module exists, but the worker has not replied or
        // published A. A second atomic rename can now supersede it exactly.
        pause(this, point, result.record.source_sha256, true, result.record.wasm_sha256);
        return result;
      };
    } else {
      const original = CoreEngine.prototype.liveGeneration;
      CoreEngine.prototype.liveGeneration = function (...args) {
        const result = original.apply(this, args);
        // This import is executing inside the actual old Wasm frame.
        pause(this, point, result.toString());
        return result;
      };
    }
  }
}

// The test harness is the host here. This exact event is not a guest action,
// a self-edit grant, or a public CLI command. It selects the independently
// configured ledger@1 binding while its old alias is still live, then revokes
// test.emit before the next actual guest request. The immutable adapter/slot
// identity stays resident; subsequent declared snapshots cannot rearm it.
CoreEngine.prototype.revokeLive06TestEmit = function () {
  const state = this.declaredLiveState;
  let selected = null;
  for (const [slot, binding] of this.boundAdapters) {
    if (binding.module === 'ledger' && binding.version === 1
      && binding.operation === 'test.emit' && binding.adapter === 'emit-a') {
      if (selected) {
        return this.declaredStateReport('host-policy-refused', state,
          'LIVE06 test host found ambiguous ledger@1:test.emit bindings');
      }
      selected = { slot, binding };
    }
  }
  if (!this.declaredIdle() || this.stagedDeclared || !selected
    || !selected.binding.allowed || !state.activeSlots.has(selected.slot)
    || !state.aliases.some(alias => alias.slot === selected.slot)) {
    return this.declaredStateReport('host-policy-refused', state,
      'LIVE06 test host can only revoke a currently authorized ledger@1:test.emit binding');
  }
  selected.binding.allowed = false;
  state.activeSlots.delete(selected.slot);
  return { ...this.declaredStateReport('host-policy-revoked', state),
    module: 'ledger', version: 1, effect: 'test.emit',
    adapter_identity: 'emit-a', slot: selected.slot,
    previously_authorized: true, currently_authorized: false };
};
