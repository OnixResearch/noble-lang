// Only concatenated into the explicitly feature-built live test worker.
// Private FIFO handshakes delay a real V8 guest callback or checked candidate
// compilation; host policy changes use the ordinary live control protocol.
{
  const directory = process.env.NOBLE_LIVE_TEST_BARRIER_DIR;
  const point = process.env.NOBLE_LIVE_TEST_BARRIER_POINT;
  const expected = process.env.NOBLE_LIVE_TEST_BARRIER_EXPECTED;
  const tamper = process.env.NOBLE_LIVE_TEST_PROPOSAL_TAMPER;
  const guardFault = process.env.NOBLE_LIVE_TEST_GUARD_FAULT;
  const shadowFault = process.env.NOBLE_LIVE_TEST_SHADOW_FAULT;
  const generationShift = process.env.NOBLE_LIVE_TEST_GENERATION_SHIFT;
  for (const key of ['NOBLE_LIVE_TEST_BARRIER_DIR', 'NOBLE_LIVE_TEST_BARRIER_POINT',
    'NOBLE_LIVE_TEST_BARRIER_EXPECTED', 'NOBLE_LIVE_TEST_PROPOSAL_TAMPER',
    'NOBLE_LIVE_TEST_GUARD_FAULT', 'NOBLE_LIVE_TEST_SHADOW_FAULT',
    'NOBLE_LIVE_TEST_GENERATION_SHIFT']) delete process.env[key];
  if (shadowFault !== undefined) {
    // Harness fault model for LIVE-10's shadow-stage-failure variant (integrator decision 41):
    // the one staged candidate whose source has the selected SHA-256 fails its isolated shadow
    // instantiation, as a candidate that cannot link would. One-shot; the engine's own staging
    // refusal path is unchanged and nothing is published.
    if (!/^[a-f0-9]{64}$/.test(shadowFault)) fail('invalid private shadow fault selection');
    let armed = true;
    const originalStage = CoreEngine.prototype.stageBinary;
    const originalArena = CoreEngine.prototype.stageArena;
    CoreEngine.prototype.stageBinary = function (bytes, source, ...rest) {
      this.liveTestShadowFault = armed && sha256(source) === shadowFault;
      if (this.liveTestShadowFault) armed = false;
      try { return originalStage.call(this, bytes, source, ...rest); }
      finally { this.liveTestShadowFault = false; }
    };
    CoreEngine.prototype.stageArena = function (...args) {
      if (this.liveTestShadowFault) {
        throw new WebAssembly.LinkError('test-only shadow-stage failure of the selected candidate');
      }
      return originalArena.apply(this, args);
    };
  }
  if (generationShift !== undefined) {
    // Harness fault model for LIVE-10's generation-changed-between-enqueue-and-publication-CAS
    // variant (integrator decision 41). The serial live host admits a guest proposal right after
    // the invocation returns, so no real publication can come between; the first queued proposal
    // for the selected name is inspected as if the selected source had advanced one generation
    // after the enqueue, and the true generation is restored afterwards. One-shot.
    if (!/^[a-z][a-z0-9_.-]{0,63}$/.test(generationShift)) fail('invalid private generation shift selection');
    let armed = true;
    const original = CoreEngine.prototype.takeLiveProposal;
    CoreEngine.prototype.takeLiveProposal = function (...args) {
      if (!armed || this.liveProposal?.grant?.name !== generationShift) {
        return original.apply(this, args);
      }
      armed = false;
      const current = this.liveSourceGeneration;
      this.liveSourceGeneration = current + 1n;
      try { return original.apply(this, args); }
      finally { this.liveSourceGeneration = current; }
    };
  }
  if (guardFault !== undefined) {
    // Harness fault model for GUARD-CASE-11 (owner decision 33): the one real
    // execution whose checked source has the selected SHA-256 either runs
    // under a host-selected step cap, so the guest's own meter exhausts, or
    // raises a Wasm trap at the top of its real guest frame. It is one-shot,
    // selects no other execution and grants no authority; the engine's own
    // classification, poisoning and report paths are unchanged.
    const selected = /^exhaust:([1-9][0-9]{0,5}):([a-f0-9]{64})$/.exec(guardFault)
      ?? /^trap:()([a-f0-9]{64})$/.exec(guardFault);
    if (!selected) fail('invalid private guard fault selection');
    const [, cap, source] = selected;
    const steps = cap === '' ? null : Number(cap);
    let armed = true;
    const original = CoreEngine.prototype.execute;
    CoreEngine.prototype.execute = function (options = {}) {
      if (!armed || this.pending?.record?.source_sha256 !== source) {
        return original.call(this, options);
      }
      armed = false;
      if (steps !== null) {
        return original.call(this, { ...options, limits: { ...(options.limits ?? {}), steps } });
      }
      this.liveTestPause = () => {
        throw new WebAssembly.RuntimeError('test-only guard fault at the top of the guest frame');
      };
      return original.call(this, options);
    };
  }
  if (tamper !== undefined) {
    if (tamper !== 'snapshot-cell') fail('invalid private live proposal tamper selection');
    // Hostile-harness control: change one byte of a reachable capture cell
    // after the guest callback accepted its snapshot and before the host
    // rechecks those exact callback-time bytes. It is not a guest write and
    // grants no source-level mutation route.
    const original = CoreEngine.prototype.takeLiveProposal;
    CoreEngine.prototype.takeLiveProposal = function (...args) {
      const queued = this.liveProposal;
      const entry = queued?.snapshot?.cells?.[0];
      if (entry !== undefined) {
        const memory = new Uint8Array(this.memory.buffer);
        if (!Number.isInteger(entry.address) || entry.address < 0
          || entry.address >= memory.length) fail('invalid private proposal tamper address');
        memory[entry.address] ^= 0x01;
      }
      return original.apply(this, args);
    };
  }
  if (directory || point || expected) {
    if (typeof directory !== 'string' || !path.isAbsolute(directory)
      || !['in-flight-generation', 'in-flight-execution', 'candidate-compiled',
        'in-flight-declared-emit'].includes(point)
      || typeof expected !== 'string'
      || point === 'candidate-compiled' && !/^[a-f0-9]{64}$/.test(expected)
      || point === 'in-flight-generation' && !/^[1-9][0-9]*$/.test(expected)
      || point === 'in-flight-execution' && !/^[a-f0-9]{64}$/.test(expected)
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
    } else if (point === 'in-flight-execution') {
      // Hold the worker at the top of one real guest frame whose checked
      // source matches. The host already accepted the submission and is
      // blocked on this reply, so a reload line written now is queued behind
      // the active frame, exactly the case's in-flight condition.
      const original = CoreEngine.prototype.execute;
      CoreEngine.prototype.execute = function (...args) {
        const record = this.pending?.record;
        if (this.inProcessLive && !this.declaredLive && record
          && record.source_sha256 === expected) {
          this.liveTestPause = (owner, current) => {
            if (current.source_sha256 === expected) {
              pause(owner, point, current.source_sha256);
            }
          };
        }
        return original.apply(this, args);
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
