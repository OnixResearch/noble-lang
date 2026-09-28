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
        || engine.declaredExtension.host_functions.length !== 1
        || engine.declaredExtension.host_functions[0].name !== 'test_emit_bound'
        || !Array.isArray(engine.declaredExtension.host_functions[0].params)
        || engine.declaredExtension.host_functions[0].params.join(',') !== 'i32,i32,i32'
        || engine.declaredExtension.host_functions[0].result !== 'i32') {
        fail('invalid declared module ABI extension');
      }
    } else if (declared_extension !== null || !Array.isArray(bindings) || bindings.length !== 0) {
      fail('declared host boundary is unavailable in Core-Bootstrap');
    }
    engine.hostFunctions = declared_modules
      ? [...abi.host_functions.filter(fn => fn.name !== 'test_emit'), ...engine.declaredExtension.host_functions]
      : abi.host_functions;

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
        || typeof row.allowed !== 'boolean' || engine.boundAdapters.has(row.slot)) {
        fail('invalid or duplicate module adapter slot');
      }
      engine.boundAdapters.set(row.slot, { module: row.module, version: row.version,
        adapter: row.adapter, allowed: row.allowed, invocations: 0 });
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
      const text = readText(engine, offset, count);
      const request = { effect: 'test.emit', module: binding?.module ?? null,
        version: binding?.version ?? null, adapter_identity: binding?.adapter ?? null,
        slot, decision: binding?.allowed ? 'allow' : 'deny', text };
      engine.boundRequests.push(request);
      engine.trace.push(`test.emit:${binding?.adapter ?? 'unbound'}:${request.decision}:${text}`);
      if (!binding?.allowed) return 1;
      engine.protectedOperations += 1;
      return 0;
    };
    engine.shared.test_abort = () => {
      if (declared_modules) engine.hostRequestsTotal += 1;
      if (engine.trace.length < 4096) engine.trace.push('test.abort');
      return 0;
    };

}
