// Host-owned ABI, immutable adapter dispatch and effect accounting.
function configureDeclaredAbi(engine, abi, declared_modules, declared_extension, bindings) {
    engine.profile = declared_modules ? 'Declared-Modules-v1' : 'Core-Bootstrap';
    if (declared_modules) {
      if (typeof declared_extension !== 'string') fail('missing declared module ABI extension');
      engine.declaredExtension = JSON.parse(declared_extension);
      if (engine.declaredExtension.schema !== 'noble-declared-module-abi/v1'
        || engine.declaredExtension.base_profile !== abi.profile
        || engine.declaredExtension.profile !== engine.profile
        || !Array.isArray(engine.declaredExtension.host_functions)
        || engine.declaredExtension.host_functions.length !== 2
        || engine.declaredExtension.host_functions[0].name !== 'test_emit_bound'
        || !Array.isArray(engine.declaredExtension.host_functions[0].params)
        || engine.declaredExtension.host_functions[0].params.join(',') !== 'i32,i32,i32'
        || engine.declaredExtension.host_functions[0].result !== 'i32'
        || engine.declaredExtension.host_functions[1].name !== 'test_clock_bound'
        || !Array.isArray(engine.declaredExtension.host_functions[1].params)
        || engine.declaredExtension.host_functions[1].params.join(',') !== 'i32'
        || engine.declaredExtension.host_functions[1].result !== 'i32,i64') {
        fail('invalid declared module ABI extension');
      }
    } else if (declared_extension !== null || !Array.isArray(bindings) || bindings.length !== 0) {
      fail('declared host boundary is unavailable in Core-Bootstrap');
    }
    if (engine.inProcessLive && (declared_modules
      || !Array.isArray(abi.live_host_functions)
      || abi.live_host_functions.length !== 2
      || abi.live_host_functions.some((fn, index) => fn.name !== ['live_propose', 'live_generation'][index]
        || fn.params?.join(',') !== ['i64,i64,i32', ''][index]
        || fn.result !== ['i32', 'i64'][index]))) {
      fail('invalid live host ABI');
    }
    engine.hostFunctions = declared_modules
      ? [...abi.host_functions.filter(fn => fn.name !== 'test_emit'), ...engine.declaredExtension.host_functions]
      : engine.inProcessLive ? [...abi.host_functions, ...abi.live_host_functions] : abi.host_functions;

}
function initializeHostState(engine, bindings) {
    engine.trace = [];
    engine.boundRequests = [];
    engine.protectedOperations = 0;
    engine.ambientFallbackCalls = 0;
    engine.hostRequestsTotal = 0;
    engine.boundRequestsTotal = 0;
    engine.boundTraceExhausted = 0;
    engine.boundTraceTerminal = null;
    if (!Array.isArray(bindings) || bindings.length > 64) fail('invalid module adapter bindings');
    engine.boundAdapters = new Map();
    for (const row of bindings) {
      if (!Number.isInteger(row.slot) || row.slot < 0 || row.slot >= 64
        || typeof row.module !== 'string' || row.module.length === 0 || row.module.length > 128
        || !Number.isInteger(row.version) || row.version <= 0 || row.version > 4294967295
        || typeof row.adapter !== 'string' || row.adapter.length === 0 || row.adapter.length > 64
        || !['test.emit', 'test.clock'].includes(row.operation)
        || !Array.isArray(row.script) || !Array.isArray(row.dispatch)
        || typeof row.allowed !== 'boolean' || engine.boundAdapters.has(row.slot)) {
        fail('invalid or duplicate module adapter slot');
      }
      if (row.operation === 'test.emit' && (row.script.length !== 0 || row.dispatch.length !== 0)
        || row.operation === 'test.clock' && (row.script.length < 1 || row.script.length > 16
          || row.dispatch.length !== row.script.length + 1)) {
        fail('incompatible test host adapter interface');
      }
      const scripted = [];
      for (const value of row.script) {
        if (typeof value !== 'string' || !/^(0|-[1-9][0-9]*|[1-9][0-9]*)$/.test(value)
          || BigInt.asIntN(64, BigInt(value)) !== BigInt(value)) {
          fail('invalid test.clock scripted input');
        }
        scripted.push(value);
      }
      const dispatch = [];
      for (let index = 0; index < row.dispatch.length; index++) {
        const step = row.dispatch[index];
        const expected = row.allowed
          ? index < scripted.length ? 'allow' : 'script-exhausted' : 'deny';
        if (step === null || typeof step !== 'object' || step.decision !== expected
          || step.value !== (expected === 'allow' ? scripted[index] : null)) {
          fail('incompatible test.clock dispatch plan');
        }
        dispatch.push({ decision: step.decision, value: expected === 'allow' ? BigInt(step.value) : 0n });
      }
      engine.boundAdapters.set(row.slot, { module: row.module, version: row.version,
        adapter: row.adapter, operation: row.operation, allowed: row.allowed,
        script: scripted, dispatch, scriptIndex: 0, invocations: 0 });
    }

}
function installSharedGlobals(engine, abi) {
    engine.shared[abi.memory.name] = engine.memory;
    engine.shared[abi.table.name] = engine.table;
    for (const global of abi.globals) {
      engine.shared[global.name] = new WebAssembly.Global({ value: global.type, mutable: true },
        global.type === 'i64' ? BigInt(global.value) : global.value);
    }

}
function installTestHosts(engine, declared_modules) {
    engine.shared.test_emit = (offset, count) => {
      if (declared_modules) {
        engine.hostRequestsTotal += 1;
        engine.ambientFallbackCalls += 1;
        if (engine.trace.length < 4096) engine.trace.push('ambient-test.emit:denied');
        return 1;
      }
      const text = readText(engine, offset, count);
      if (engine.trace.length >= 4096) return 1;
      engine.trace.push(`test.emit:${text}`);
      return 0;
    };
    engine.shared.test_emit_bound = (offset, count, slot) => {
      engine.hostRequestsTotal += 1;
      engine.boundRequestsTotal += 1;
      const binding = engine.boundAdapters.get(slot);
      if (binding) binding.invocations += 1;
      if (engine.trace.length >= 4096 || engine.boundRequests.length >= 4096) {
        engine.boundTraceExhausted += 1;
        engine.boundTraceTerminal = { effect: 'test.emit', module: binding?.module ?? null,
          version: binding?.version ?? null, adapter_identity: binding?.adapter ?? null,
          slot, decision: 'deny', reason: 'trace-exhausted' };
        return 1;
      }
      const text = binding?.operation === 'test.emit' ? readText(engine, offset, count) : null;
      const request = { effect: 'test.emit', module: binding?.module ?? null,
        version: binding?.version ?? null, adapter_identity: binding?.adapter ?? null,
        slot, decision: binding?.allowed && binding.operation === 'test.emit' ? 'allow' : 'deny',
        ...(!binding ? { reason: 'missing-mapping' }
          : binding.operation !== 'test.emit' ? { reason: 'unexpected-operation' }
          : !binding.allowed ? { reason: 'policy-denied' } : {}), text };
      engine.boundRequests.push(request);
      engine.trace.push(`test.emit:${binding?.adapter ?? 'unbound'}:${request.decision}:${text}`);
      if (request.decision !== 'allow') return 1;
      engine.protectedOperations += 1;
      return 0;
    };
    engine.shared.test_clock_bound = (slot) => {
      engine.hostRequestsTotal += 1;
      engine.boundRequestsTotal += 1;
      const binding = engine.boundAdapters.get(slot);
      if (binding) binding.invocations += 1;
      if (engine.trace.length >= 4096 || engine.boundRequests.length >= 4096) {
        engine.boundTraceExhausted += 1;
        engine.boundTraceTerminal = { effect: 'test.clock', module: binding?.module ?? null,
          version: binding?.version ?? null, adapter_identity: binding?.adapter ?? null,
          slot, decision: 'deny', reason: 'trace-exhausted' };
        return [1, 0n];
      }
      let step = null;
      let reason = null;
      if (!binding) reason = 'missing-mapping';
      else if (binding.operation !== 'test.clock') reason = 'unexpected-operation';
      else {
        const index = Math.min(binding.scriptIndex, binding.dispatch.length - 1);
        step = binding.dispatch[index];
        binding.scriptIndex += 1;
        if (step.decision !== 'allow') reason = step.decision === 'deny'
          ? 'policy-denied' : 'script-exhausted';
      }
      const request = { effect: 'test.clock', module: binding?.module ?? null,
        version: binding?.version ?? null, adapter_identity: binding?.adapter ?? null,
        slot, decision: step?.decision === 'allow' ? 'allow' : 'deny',
        ...(reason ? { reason } : {}),
        ...(step?.decision === 'allow' ? { value: step.value.toString() } : {}) };
      engine.boundRequests.push(request);
      engine.trace.push(`test.clock:${binding?.adapter ?? 'unbound'}:${request.decision}`);
      if (request.decision !== 'allow') return [1, 0n];
      engine.protectedOperations += 1;
      return [0, step.value];
    };
    engine.shared.test_abort = () => {
      if (declared_modules) engine.hostRequestsTotal += 1;
      if (engine.trace.length < 4096) engine.trace.push('test.abort');
      return 0;
    };

}
