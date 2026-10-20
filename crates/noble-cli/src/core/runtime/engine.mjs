// A raw WebAssembly.Module is not an admission capability. Only the selected
// compiler path and the byte-exact artifact path may cross into install().
const installationCapability = Symbol('compiler-validated module');

const artifactEffects = new Map([
  ['test_emit', 'test.emit'],
  ['test_abort', 'test.abort'],
  ['test_emit_bound', 'test.emit'],
  ['test_clock_bound', 'test.clock'],
]);

export class CoreEngine {
  constructor(selection, abi, { optimized = false, artifacts = null, bindings = [], declared_modules = false, text_byte_cursor = false, calculator = false, calculator_shell_state = false, calculator_assembler = null, declared_extension = null } = {}) {
    this.selection = selection;
    // This larger bounded cell arena belongs only to Calculator-v1. Do not
    // mutate the parsed ABI shared with the ordinary and cursor profiles.
    this.abi = calculator
      ? { ...abi, memory: { ...abi.memory, maximum: 64 },
        limits_maximum: { ...abi.limits_maximum,
          allocation_bytes: 1376208, session_cells: 72362, session_literal_bytes: 589824 },
        cells: { ...abi.cells,
          layout: '48 immutable bytes: kind:i32,id:i32,payload:i64,a:i32,b:i32,c:i32,x:i32,y:i32,z:i32,w:i32,n:i32. Calculator-v1 handles 1..4096 address 65536+(handle-1)*48, handles 4097..6826 address 393216+(handle-4097)*48, and handles 6827..72362 address 1048576+(handle-6827)*48. Dynamic Text [262144,393216), static Text [524288,983040), raw ingress [983040,1048576); extra cells [1048576,4194304) grow selected imported memory at most 64 pages. a/b/c are nullable backward cell edges; inspect only live handles obtained from checked output. Before each Calculator-v1 begin, the runtime collect export compacts the cells reachable from the operand stack and their dynamic Text in handle order and zeroes the vacated tail, so a handle is valid only until the next execute.' } }
      : abi;
    configureDeclaredAbi(this, this.abi, declared_modules, text_byte_cursor, calculator, declared_extension, bindings);
    if (!calculator && calculator_assembler !== null) fail('calculator assembler cannot enter another profile');
    if (calculator_shell_state && !calculator) fail('opaque calculator state requires the calculator profile');
    this.calculatorShellState = calculator_shell_state;
    this.calculatorAssembler = calculator_assembler;
    this.optimized = optimized;
    initializeHostState(this, bindings);
    this.pending = null;
    this.parked = null;
    this.poisoned = false;
    // Calculator-v1 signature ID -> canonical key, as published by the
    // latest executed module and its predecessors (see recordSignatures).
    this.signatures = [];
    this.compilations = 0;
    this.lastTraceStart = 0;
    this.tools = this.checkTools();
    this.owned = artifacts === null;
    this.directory = this.owned ? fs.mkdtempSync(path.join(os.tmpdir(), 'noble-core-')) : path.join(artifacts, 'engine');
    if (!this.owned) fs.mkdirSync(this.directory);
    this.memory = new WebAssembly.Memory(this.abi.memory);
    this.table = new WebAssembly.Table(this.abi.table);
    this.shared = Object.create(null);
    installSharedGlobals(this, this.abi);
    installTestHosts(this, declared_modules || calculator);
    this.save('tools.json', this.tools);
    if (this.declaredExtension) this.save('declared-abi.json', this.declaredExtension);
  }

  checkTools() {
    const selected = this.selection.tools;
    if (process.versions.node !== selected.node.version || process.versions.v8 !== selected.node.v8
      || fs.realpathSync(process.execPath) !== fs.realpathSync(selected.node.path)) {
      fail('Node/V8 does not match the selected immutable engine');
    }
    const tools = { node: { ...selected.node, sha256: sha256(fs.readFileSync(process.execPath)) } };
    if (this.profile === 'Calculator-v1') {
      if (this.optimized || typeof this.calculatorAssembler !== 'string'
        || !path.isAbsolute(this.calculatorAssembler)
        || fs.realpathSync(this.calculatorAssembler) !== this.calculatorAssembler)
        fail('invalid calculator-only assembler selection');
      const version = spawnSync(this.calculatorAssembler, ['--internal-calculator-wat-version'],
        { encoding: 'utf8', env: {}, timeout: 10000, maxBuffer: 65536 });
      if (version.error || version.status !== 0
        || version.stdout !== 'noble-calculator-wat 1.261.0\n')
        fail('incompatible calculator-only WAT assembler');
      tools.calculator_wat = { path: this.calculatorAssembler,
        version: version.stdout.trim(), sha256: sha256(fs.readFileSync(this.calculatorAssembler)) };
      return tools;
    }
    for (const [name, executable, expected] of [
      ['wasm_tools', selected.wasm_tools.path, `wasm-tools ${selected.wasm_tools.version}`],
      ['wasm_opt', selected.wasm_opt.path, `wasm-opt version ${selected.wasm_opt.version}`],
    ]) {
      if (!path.isAbsolute(executable)) fail('tool path must be absolute');
      const result = spawnSync(executable, ['--version'], { encoding: 'utf8', env: {}, timeout: 10000, maxBuffer: 65536 });
      if (result.error || result.status !== 0 || !result.stdout.trim().startsWith(expected)) fail(`incompatible ${name}`);
      tools[name] = { path: executable, version: result.stdout.trim(), sha256: sha256(fs.readFileSync(executable)) };
    }
    return tools;
  }

  save(file, value) {
    const content = Buffer.isBuffer(value) || typeof value === 'string' ? value : JSON.stringify(value, null, 2) + '\n';
    fs.writeFileSync(path.join(this.directory, file), content, { flag: 'wx' });
  }

  tool(name, args, label) {
    const result = spawnSync(this.selection.tools[name].path, args, {
      env: {}, encoding: 'utf8', timeout: 60000, killSignal: 'SIGKILL', maxBuffer: MAX_FRAME,
    });
    this.save(`${label}.json`, { binary: this.selection.tools[name].path, args,
      status: result.status, signal: result.signal, stdout: result.stdout, stderr: result.stderr,
      error: result.error ? String(result.error) : null });
    if (result.error || result.status !== 0) fail(`${label} failed: ${result.error ?? result.stderr}`);
  }

  prepare(wat, source = Buffer.alloc(0), submission = this.compilations + 1) {
    if (this.pending || this.poisoned) fail('session is pending or poisoned');
    if (wat.length > MAX_FRAME || source.length > 65536) fail('preparation byte limit exceeded');
    const stem = `module-${submission}`;
    const persistCompiledAssets = this.profile !== 'Calculator-v1' || !this.owned;
    if (persistCompiledAssets) this.save(`${stem}.wat`, wat);
    this.save(`${stem}.noble`, source);
    let wasmPath = null;
    let bytes;
    if (this.profile === 'Calculator-v1') {
      if (sha256(fs.readFileSync(this.calculatorAssembler)) !== this.tools.calculator_wat.sha256)
        fail('calculator assembler changed after selection');
      const result = spawnSync(this.calculatorAssembler, ['--internal-calculator-wat-assemble'],
        { input: wat, env: {}, timeout: 60000, maxBuffer: MAX_FRAME });
      this.save(`${stem}-calculator-assemble.json`, { toolchain: 'calculator-experimental-wat-v1',
        assembler_sha256: this.tools.calculator_wat.sha256, wat_sha256: sha256(wat),
        status: result.status, signal: result.signal,
        stderr: result.stderr.toString(), error: result.error ? String(result.error) : null });
      if (result.error || result.status !== 0 || !result.stdout.length || result.stdout.length > MAX_FRAME)
        fail(`calculator-only WAT assembly refused: ${result.error ?? result.stderr.toString()}`);
      if (persistCompiledAssets) {
        wasmPath = path.join(this.directory, `${stem}.wasm`);
        fs.writeFileSync(wasmPath, result.stdout, { flag: 'wx' });
      } else {
        bytes = result.stdout;
      }
    } else {
      const watPath = path.join(this.directory, `${stem}.wat`);
      wasmPath = path.join(this.directory, `${stem}.wasm`);
      this.tool('wasm_tools', ['parse', watPath, '-o', wasmPath], `${stem}-assemble`);
      this.tool('wasm_tools', ['validate', wasmPath], `${stem}-validate`);
    }
    if (this.optimized) {
      const optimized = path.join(this.directory, `${stem}-optimized.wasm`);
      this.tool('wasm_opt', [wasmPath, ...this.selection.optimizer_flags, '-o', optimized], `${stem}-optimize`);
      wasmPath = optimized;
      this.tool('wasm_tools', ['validate', wasmPath], `${stem}-validate-optimized`);
    }
    if (wasmPath !== null) {
      if (fs.statSync(wasmPath).size > MAX_FRAME) fail('module byte limit exceeded');
      bytes = fs.readFileSync(wasmPath);
    }
    rejectStartSection(bytes);
    const module = new WebAssembly.Module(bytes);
    this.compilations += 1;
    return this.install(module, { submission, wasm_sha256: sha256(bytes), source_sha256: sha256(source),
      wat_sha256: sha256(wat), optimized: this.optimized, compilations: this.compilations, stem,
      ...(this.profile === 'Calculator-v1' ? { assembly_toolchain: 'calculator-experimental-wat-v1',
        assembler_sha256: this.tools.calculator_wat.sha256 } : {}) },
    installationCapability);
  }

  // Candidate claims and source paths are data. Only the CLI invoker selects
  // source and allowed effects; neither can be inherited from the manifest.
  admit(bytes, wat, source, claimedEffects, allowedEffects, sourceSelected) {
    const refusal = (outcome, diagnostic, actualImports = []) => ({
      schema: 'noble-artifact-admission/v1',
      profile: this.profile === 'Calculator-v1' ? 'Calculator-v1' : 'Wasm-Draft',
      stage: 'admission', outcome, diagnostic, actual_imports: actualImports,
      claimed_effects: claimedEffects, allowed_effects: allowedEffects,
      guest_requests: 0, protected_operations: 0,
    });
    // Calculator-v1 selects only its own checked WAT assembler. The generic
    // artifact path requires wasm-tools, which this worker never validates.
    if (this.profile === 'Calculator-v1') {
      return refusal('unsupported', 'Calculator-v1 does not admit generic artifacts');
    }
    if (this.pending || this.compilations || this.poisoned) {
      return refusal('stale-session-reject', 'artifact admission requires a fresh engine');
    }
    if (!Buffer.isBuffer(bytes) || bytes.length > MAX_FRAME || !Array.isArray(claimedEffects)
      || !Array.isArray(allowedEffects) || !Buffer.isBuffer(wat) || !Buffer.isBuffer(source)) {
      return refusal('invalid-artifact', 'invalid bounded admission request');
    }
    let module;
    try {
      rejectStartSection(bytes);
      module = new WebAssembly.Module(bytes);
    } catch (error) {
      return refusal('invalid-artifact', String(error));
    }
    const imports = WebAssembly.Module.imports(module);
    const actual = new Set();
    const names = new Set();
    const globals = new Set(this.abi.globals.map(global => global.name));
    for (const entry of imports) {
      const name = `${entry.module}.${entry.name}:${entry.kind}`;
      if (names.has(name)) return refusal('invalid-artifact', 'duplicate module import');
      names.add(name);
      if (entry.module !== this.abi.module) {
        return refusal('invalid-artifact', 'unknown module import');
      }
      if (entry.kind === 'function') {
        const effect = artifactEffects.get(entry.name);
        if (!effect || !this.hostFunctions.some(host => host.name === entry.name)
          || actual.has(effect)) {
          return refusal('invalid-artifact', 'unknown or ambiguous function import');
        }
        actual.add(effect);
      } else if (!(entry.kind === 'memory' && entry.name === this.abi.memory.name
        || entry.kind === 'table' && entry.name === this.abi.table.name
        || entry.kind === 'global' && globals.has(entry.name))) {
        return refusal('invalid-artifact', 'unknown non-function import');
      }
    }
    const actualImports = [...actual].sort();
    const declared = new Set(claimedEffects);
    if (declared.size !== claimedEffects.length
      || claimedEffects.some(effect => !['test.emit', 'test.abort', 'test.clock'].includes(effect))
      || actualImports.length !== declared.size
      || actualImports.some(effect => !declared.has(effect))) {
      return refusal('effect-manifest-reject', 'candidate effects do not match validated binary imports', actualImports);
    }
    const allowed = new Set(allowedEffects);
    if (allowed.size !== allowedEffects.length
      || allowedEffects.some(effect => !['test.emit', 'test.abort', 'test.clock'].includes(effect))
      || actualImports.some(effect => !allowed.has(effect))) {
      return refusal('effect-policy-reject', 'host did not allow the required effects', actualImports);
    }
    if (!sourceSelected || !wat.length || !source.length || wat.length > MAX_FRAME || source.length > 65536) {
      return refusal('correspondence-reject', 'host-selected source is required', actualImports);
    }
    const stem = 'admitted-module';
    this.save(`${stem}.wat`, wat);
    this.save(`${stem}.noble`, source);
    const watPath = path.join(this.directory, `${stem}.wat`);
    let wasmPath = path.join(this.directory, `${stem}.wasm`);
    this.tool('wasm_tools', ['parse', watPath, '-o', wasmPath], `${stem}-assemble`);
    this.tool('wasm_tools', ['validate', wasmPath], `${stem}-validate`);
    if (this.optimized) {
      const optimized = path.join(this.directory, `${stem}-optimized.wasm`);
      this.tool('wasm_opt', [wasmPath, ...this.selection.optimizer_flags, '-o', optimized], `${stem}-optimize`);
      wasmPath = optimized;
      this.tool('wasm_tools', ['validate', wasmPath], `${stem}-validate-optimized`);
    }
    const rebuilt = fs.readFileSync(wasmPath);
    if (rebuilt.length > MAX_FRAME || !bytes.equals(rebuilt)) {
      return refusal('correspondence-reject', 'final artifact bytes differ from independent host-selected compilation', actualImports);
    }
    // All candidate checks precede instantiation. Even a module with no start
    // function may mutate imported memory/table through active segments.
    // The admitted bytes are independently rebuilt and this engine is fresh.
    rejectStartSection(bytes);
    this.compilations += 1;
    const ready = this.install(module, {
      submission: 1, wasm_sha256: sha256(bytes), source_sha256: sha256(source),
      wat_sha256: sha256(wat), optimized: this.optimized, compilations: this.compilations,
      actual_imports: actualImports, claimed_effects: claimedEffects,
      allowed_effects: allowedEffects, correspondence: 'byte-exact',
    }, installationCapability);
    if (ready.outcome !== 'ready') return ready;
    return { ...this.execute(), admission: {
      actual_imports: actualImports, claimed_effects: claimedEffects,
      allowed_effects: allowedEffects, correspondence: 'byte-exact',
      optimized: this.optimized,
    } };
  }

  install(module, record = {}, capability) {
    if (capability !== installationCapability) fail('raw module installation is not artifact admission');
    if (this.pending || this.poisoned) fail('session is pending or poisoned');
    const imports = WebAssembly.Module.imports(module);
    const globals = new Set(this.abi.globals.map(global => global.name));
    const functions = new Set(this.hostFunctions.map(fn => fn.name));
    for (const entry of imports) {
      const allowed = entry.module === this.abi.module && (
        entry.kind === 'memory' && entry.name === this.abi.memory.name
        || entry.kind === 'table' && entry.name === this.abi.table.name
        || entry.kind === 'global' && globals.has(entry.name)
        || entry.kind === 'function' && functions.has(entry.name));
      if (!allowed) fail(`undeclared module import: ${JSON.stringify(entry)}`);
    }
    const requests = this.trace.length;
    const boundRequests = this.boundRequests.length;
    const hostRequestsTotal = this.hostRequestsTotal;
    const boundRequestsTotal = this.boundRequestsTotal;
    const ambientFallbackCalls = this.ambientFallbackCalls;
    const protectedOperations = this.protectedOperations;
    const sp = this.shared.sp.value;
    const instance = new WebAssembly.Instance(module, { [this.abi.module]: this.shared });
    if (requests !== this.trace.length || boundRequests !== this.boundRequests.length
      || hostRequestsTotal !== this.hostRequestsTotal || boundRequestsTotal !== this.boundRequestsTotal
      || ambientFallbackCalls !== this.ambientFallbackCalls
      || protectedOperations !== this.protectedOperations || sp !== this.shared.sp.value) {
      fail('instantiation executed candidate body');
    }
    // Shared table entries keep callable instances alive; do not retain inert
    // modules in an unbounded side list across otherwise empty submissions.
    this.pending = { instance, module, record: { ...record, imports }, traceStart: requests };
    return { schema: 'noble-core-report/v1', profile: this.profile, backend: 'managed-linear-memory',
      stage: 'wasm', outcome: 'ready', module: this.pending.record, guest_requests: 0, protected_operations: 0,
      ...(this.profile === 'Declared-Modules-v1' ? { host_requests: 0, acquired_authority: false,
        ambient_fallback_calls: 0, real_host_evidence: false, real_host_fallback_calls: 0 } : {}),
      candidate_prepare_requests: 0 };
  }

  execute({ inputs = [], limits = {} } = {}) {
    if (!this.pending || this.poisoned) fail('no prepared module or poisoned session');
    const { instance, record, traceStart } = this.pending;
    const boundStart = this.boundRequests.length;
    const boundTotalStart = this.boundRequestsTotal;
    const hostStart = this.hostRequestsTotal;
    const traceExhaustedStart = this.boundTraceExhausted;
    const adapterStarts = this.profile === 'Declared-Modules-v1'
      ? new Map(Array.from(this.boundAdapters, ([slot, binding]) => [slot, binding.invocations]))
      : null;
    const protectedStart = this.protectedOperations;
    const fallbackStart = this.ambientFallbackCalls;
    this.lastTraceStart = traceStart;
    this.pending = null;
    const runtime = instance.exports;
    // Adopt this module's published signatures before it becomes the latest
    // inspected instance. A refused range poisons the session fail-closed.
    if (this.profile === 'Calculator-v1') {
      this.poisoned = true;
      this.signatures = recordSignatures(this, runtime);
      this.poisoned = false;
    }
    // Retain exactly one executed instance so post-execution inspection reads
    // the same session state. The shared table already owns its callables.
    this.last = instance;
    const defaults = this.abi.limits_maximum;
    const keys = ['allocation_bytes', 'recipe_leaves', 'program_depth', 'operand_bytes', 'continuation_bytes', 'steps'];
    if (Object.keys(limits).some(key => !keys.includes(key))) fail('unknown runtime limit');
    const selectedLimits = keys.map(key => integer(limits[key] ?? defaults[key], defaults[key], key));
    let nativeTrap = null;
    let status = 0;
    // Calculator-v1 compacts its arena at the submission boundary, before
    // begin fixes the per-submission baseline. A refusal poisons the session.
    let collection = null;
    if (this.profile === 'Calculator-v1') {
      const before = this.shared.heap_cursor.value;
      try {
        status = runtime.collect();
      } catch (error) {
        nativeTrap = `${error.name}: ${error.message}`;
        status = 6;
      }
      collection = { cells_before: before, cells_after: this.shared.heap_cursor.value };
    }
    // After a normal Calculator-v1 submission, the runtime reports whether a
    // Program reachable from the operand stack runs compiled code. Only when
    // none does may the next module reuse the committed compiled table slots.
    let compiledProgramsReachable = null;
    // A stricter boundary answer: no Program, Syntax or recipe cell (the only
    // holders of signature IDs) is reachable, so signature IDs may restart.
    let programsReachable = null;
    if (status === 0) status = runtime.begin(...selectedLimits);
    try {
      for (const input of inputs) {
        if (status) break;
        status = this.inject(runtime, input);
      }
      if (status === 0) status = runtime.submit();
      if (status === 0 && this.profile === 'Calculator-v1') {
        compiledProgramsReachable = runtime.compiled_programs_reachable() !== 0;
        programsReachable = compiledProgramsReachable || runtime.programs_reachable() !== 0;
      }
    } catch (error) {
      nativeTrap = `${error.name}: ${error.message}`;
      status = 6;
    }
    this.poisoned = status !== 0;
    const metrics = Object.fromEntries(Object.entries(this.abi.metrics).map(([id, name]) => [name, Number(runtime.metric(Number(id)))]));
    const outcome = status === 0 ? 'normal' : status === 1 || status === 2 ? 'runtime-exhausted' : 'trap';
    let stack = [];
    if (status === 0) stack = readStack(this, runtime);
    const isDeclared = this.profile === 'Declared-Modules-v1';
    const boundRequests = isDeclared ? this.boundRequests.slice(boundStart) : null;
    const adapterInvocations = isDeclared
      ? Array.from(this.boundAdapters.entries(), ([slot, binding]) => ({
        slot, module: binding.module, version: binding.version,
        adapter_identity: binding.adapter,
        count: binding.invocations - adapterStarts.get(slot),
      })) : null;
    const unrecordedBound = this.boundRequestsTotal - boundTotalStart - (boundRequests?.length ?? 0);
    const report = { schema: 'noble-core-report/v1', profile: this.profile, backend: 'managed-linear-memory',
      submission: record.submission ?? null, stage: 'wasm', outcome, status,
      quota_reason: this.abi.quota_reason[String(metrics.quota_reason)] ?? null, stack,
      request_trace: isDeclared
        ? boundRequests : this.trace.slice(traceStart),
      guest_requests: isDeclared ? this.hostRequestsTotal - hostStart : this.trace.length - traceStart,
      ...(isDeclared ? {
        host_requests: this.hostRequestsTotal - hostStart,
        effect_requests: boundRequests.map(request => request.effect),
        effect_requests_unrecorded: unrecordedBound,
        request_trace_complete: unrecordedBound === 0,
        ...(this.boundTraceExhausted !== traceExhaustedStart
          ? { request_trace_terminal: { ...this.boundTraceTerminal,
            denied_requests: this.boundTraceExhausted - traceExhaustedStart } } : {}),
        adapter_invocations: adapterInvocations,
        substitutions: adapterInvocations.map(binding => ({
          module: binding.module, version: binding.version,
          operation: this.boundAdapters.get(binding.slot).operation,
          adapter_identity: binding.adapter_identity, slot: binding.slot,
        })),
        adapter_versions: adapterInvocations.map(binding => ({
          module: binding.module, version: binding.version,
          adapter_identity: binding.adapter_identity,
        })),
        scripted_inputs: Array.from(this.boundAdapters.values()).filter(binding =>
          binding.operation === 'test.clock').map(binding => ({
            module: binding.module, version: binding.version,
            adapter_identity: binding.adapter, operation: binding.operation,
            values: binding.script,
          })),
        declared_effect_preserved: boundRequests.every(request => {
          const binding = this.boundAdapters.get(request.slot);
          return binding && request.effect === binding.operation;
        }),
        ambient_fallback_calls: this.ambientFallbackCalls - fallbackStart,
        real_host_fallback_calls: 0,
        real_host_evidence: false,
        acquired_authority: false,
      } : {}),
      protected_operations: this.protectedOperations - protectedStart,
      candidate_prepare_requests: 0, runtime_prover_calls: 0, native_trap: nativeTrap,
      session_state: this.poisoned ? 'terminated-after-runtime-failure' : 'retained',
      metrics, ...(collection ? { collection } : {}),
      ...(compiledProgramsReachable === null ? {}
        : { compiled_programs_reachable: compiledProgramsReachable,
            programs_reachable: programsReachable }),
      module: record };
    if (record.stem) this.save(`${record.stem}-execution.json`, report);
    return report;
  }

  // Standalone typed injection outside one submission. Values stay first-class
  // session state; each push fails closed on a missing or mistyped cell.
  push(inputs) {
    if (!this.last && (inputs.length !== 0 || this.shared.sp.value !== 0)) fail('no initialized runtime for injection');
    const runtime = this.last ? this.runtime() : null;
    let status = 0;
    for (const input of inputs) {
      if (status) break;
      status = this.inject(runtime, input);
    }
    let stack = [];
    if (!status && runtime) stack = readStack(this, runtime);
    return { schema: 'noble-core-report/v1', profile: this.profile, backend: 'managed-linear-memory',
      stage: 'wasm', outcome: status ? 'injection-refused' : 'pushed', status, stack,
      guest_requests: 0, protected_operations: 0, candidate_prepare_requests: 0,
      session_state: 'retained' };
  }

  // This engine owns the snapshot; no caller-supplied bytes become values.
  // Cells are immutable and session handles stay live while the frame is parked.
  park() {
    // Compaction renames handles, so Calculator-v1 never keeps them outside Wasm.
    if (this.profile === 'Calculator-v1') fail('calculator does not park operand frames');
    if (this.poisoned || this.parked !== null || this.shared.cp.value !== 0) fail('cannot park this session');
    const layout = this.abi.operand_slots;
    const count = integer(this.shared.sp.value, layout.maximum, 'operand count');
    const bytes = new Uint8Array(this.memory.buffer, layout.offset, count * layout.stride);
    this.parked = { count, bytes: bytes.slice() };
    bytes.fill(0);
    this.shared.sp.value = 0;
    return { outcome: 'parked', stack_length: count };
  }

  restore() {
    if (this.poisoned || this.parked === null || this.shared.cp.value !== 0) fail('cannot restore this session');
    const layout = this.abi.operand_slots;
    const bytes = new Uint8Array(this.memory.buffer, layout.offset, layout.maximum * layout.stride);
    bytes.fill(0);
    bytes.set(this.parked.bytes);
    this.shared.sp.value = this.parked.count;
    this.parked = null;
    return { outcome: 'restored', stack: readStack(this, this.runtime()) };
  }

  inject(runtime, input) {
    if (input.type === 'Text') {
      if (this.profile !== 'Calculator-v1' || typeof runtime.push_text !== 'function'
        || !Array.isArray(input.bytes) || input.bytes.length > 65536
        || input.bytes.some(byte => !Number.isInteger(byte) || byte < 0 || byte > 255))
        fail('invalid calculator Text input');
      // Keep host ingress away from the calculator's extra private cells.
      const offset = 983040;
      if (this.memory.buffer.byteLength < offset + input.bytes.length)
        fail('calculator Text scratch exceeds runtime memory');
      const memory = new Uint8Array(this.memory.buffer, offset, input.bytes.length);
      for (let index = 0; index < input.bytes.length; index++) memory[index] = input.bytes[index];
      return runtime.push_text(offset, input.bytes.length);
    }
    if (input.type === 'I64') {
      const value = BigInt(input.value);
      if (BigInt.asIntN(64, value) !== value) fail('input is outside signed I64');
      return runtime.push_i64(value);
    }
    if (input.type === 'Bool') {
      if (typeof input.value !== 'boolean') fail('invalid Bool input');
      return runtime.push_bool(input.value ? 1 : 0);
    }
    // Calculator phase cells (Plan, EvalFrame, RawRat and successors) move
    // only through the retained checked guest stack. No host-supplied handle
    // or capability can stand in for a nominal produced by a prior phase.
    if (this.profile === 'Calculator-v1') fail('calculator only accepts Text, I64 and Bool input');
    if (input.type === 'Unit') return runtime.push_unit();
    if (input.type === 'Program') return runtime.push_program(integer(input.handle, 4096, 'program handle'));
    if (input.type === 'Contract') return runtime.push_contract(
      integer(input.index, 4294967295, 'contract index'), BigInt(input.statement),
      integer(input.claim_kind ?? 1, 1, 'claim kind'), integer(input.revision ?? 0, 4294967295, 'claim revision'));
    if (input.type === 'Evidence') return runtime.push_evidence(
      integer(input.index, 4294967295, 'evidence index'), integer(input.class ?? 1, 255, 'evidence class'),
      integer(input.ruleset ?? 1, 4294967295, 'ruleset version'),
      integer(input.contract, 4096, 'contract handle'));
    if (input.type === 'Certified') return runtime.push_certified(
      integer(input.subject, 4096, 'subject handle'), integer(input.contract, 4096, 'contract handle'),
      integer(input.evidence, 4096, 'evidence handle'), BigInt(input.identity));
    fail('unsupported runtime input type');
  }

  // Inspect one stack slot without executing candidate code. Only the bounded
  // observation walk runs, and it issues no guest host requests.
  observe(index) {
    const runtime = this.runtime();
    const status = runtime.observe(integer(index, 128, 'stack index'));
    return { schema: 'noble-core-report/v1', profile: this.profile, backend: 'managed-linear-memory',
      stage: 'wasm', outcome: status ? 'observation-failed' : 'observed', status, index,
      events: status ? [] : readReflection(this, runtime),
      guest_requests: 0, protected_operations: 0, candidate_prepare_requests: 0 };
  }

  // Explicit projection of a Certified cell to its live subject Program handle.
  project(handle) {
    if (this.profile === 'Calculator-v1') fail('calculator does not project host-supplied handles');
    const runtime = this.runtime();
    const subject = runtime.certified_subject(integer(handle, this.abi.limits_maximum.session_cells, 'cell handle'));
    return { schema: 'noble-core-report/v1', profile: this.profile, backend: 'managed-linear-memory',
      stage: 'wasm', outcome: subject ? 'projected' : 'projection-failed', handle, subject,
      identity_unchanged: subject !== 0,
      guest_requests: 0, protected_operations: 0, candidate_prepare_requests: 0 };
  }

  runtime() {
    if (!this.last) fail('no executed instance to inspect');
    return this.last.exports;
  }

  close() {
    this.pending = null;
    if (this.owned) fs.rmSync(this.directory, { recursive: true });
  }
}

