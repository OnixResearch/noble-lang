// Only concatenated into the explicitly feature-built live test worker.
// Private FIFO handshakes delay a real V8 guest callback or checked candidate
// compilation; host policy changes use the ordinary live control protocol.
{
  const directory = process.env.NOBLE_LIVE_TEST_BARRIER_DIR;
  const point = process.env.NOBLE_LIVE_TEST_BARRIER_POINT;
  const expected = process.env.NOBLE_LIVE_TEST_BARRIER_EXPECTED;
  for (const key of ['NOBLE_LIVE_TEST_BARRIER_DIR', 'NOBLE_LIVE_TEST_BARRIER_POINT',
    'NOBLE_LIVE_TEST_BARRIER_EXPECTED']) delete process.env[key];
  if (directory || point || expected) {
    if (typeof directory !== 'string' || !path.isAbsolute(directory)
      || !['in-flight-generation', 'candidate-compiled', 'in-flight-declared-emit'].includes(point)
      || typeof expected !== 'string'
      || point === 'candidate-compiled' && !/^[a-f0-9]{64}$/.test(expected)
      || point === 'in-flight-generation' && !/^[1-9][0-9]*$/.test(expected)
      || point === 'in-flight-declared-emit' && expected !== 'emit-a') {
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
    const pause = (owner, location, value, checkedWasmModule = false, wasmSha256 = null,
      activeCodeQuery = null) => {
      if (used || !owner.inProcessLive
        || owner.declaredLive !== (location === 'in-flight-declared-emit')
        || location !== point || value !== expected) return;
      used = true;
      const event = Buffer.from(JSON.stringify({ point: location, matched: value,
        source_generation: (owner.declaredLive
          ? owner.declaredLiveState.generation : owner.liveSourceGeneration).toString(),
        guest_frame_active: owner.liveExecuting,
        checked_wasm_module: checkedWasmModule,
        checked_wasm_sha256: wasmSha256,
        ...(activeCodeQuery === null ? {} : { active_code_query: activeCodeQuery }) }) + '\n');
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
    if (point === 'in-flight-declared-emit') {
      const original = installTestHosts;
      installTestHosts = function (owner, declaredModules) {
        original(owner, declaredModules);
        if (!owner.declaredLive) return;
        const emit = owner.shared.test_emit_bound;
        // Wrap the actual imported callback before any Wasm instance is
        // created. The host remains serial while this old frame is active.
        owner.shared.test_emit_bound = (offset, count, slot) => {
          const binding = owner.boundAdapters.get(slot);
          if (binding?.module === 'ledger' && binding?.version === 1
            && binding?.operation === 'test.emit' && binding?.adapter === expected) {
            const spans = Array.from(owner.liveCodeSpans.values(), record => record.span);
            if (spans.length === 0) fail('declared callback has no installed old code');
            const observations = spans.map(span =>
              owner.runtime().code_owners(span.start, span.length));
            if (observations.some(owners => owners !== -1)) {
              fail('active declared frame exposed an idle code-owner count');
            }
            pause(owner, point, binding.adapter, false, null, -1);
          }
          return emit(offset, count, slot);
        };
      };
    } else if (point === 'candidate-compiled') {
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
