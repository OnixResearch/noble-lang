export class CoreEngine {
  constructor(selection, abi, { optimized = false, artifacts = null } = {}) {
    this.selection = selection;
    this.abi = abi;
    this.optimized = optimized;
    this.trace = [];
    this.pending = null;
    this.parked = null;
    this.poisoned = false;
    this.compilations = 0;
    this.lastTraceStart = 0;
    this.tools = this.checkTools();
    this.owned = artifacts === null;
    this.directory = this.owned ? fs.mkdtempSync(path.join(os.tmpdir(), 'noble-core-')) : path.join(artifacts, 'engine');
    if (!this.owned) fs.mkdirSync(this.directory);
    this.memory = new WebAssembly.Memory(abi.memory);
    this.table = new WebAssembly.Table(abi.table);
    this.shared = Object.create(null);
    this.shared[abi.memory.name] = this.memory;
    this.shared[abi.table.name] = this.table;
    for (const global of abi.globals) {
      this.shared[global.name] = new WebAssembly.Global({ value: global.type, mutable: true },
        global.type === 'i64' ? BigInt(global.value) : global.value);
    }
    this.shared.test_emit = (offset, count) => {
      const text = readText(this, offset, count);
      if (this.trace.length >= 4096) return 1;
      this.trace.push(`test.emit:${text}`);
      return 0;
    };
    this.shared.test_abort = () => {
      if (this.trace.length < 4096) this.trace.push('test.abort');
      return 0;
    };
    this.save('tools.json', this.tools);
  }

  checkTools() {
    const selected = this.selection.tools;
    if (process.versions.node !== selected.node.version || process.versions.v8 !== selected.node.v8
      || fs.realpathSync(process.execPath) !== fs.realpathSync(selected.node.path)) {
      fail('Node/V8 does not match the selected immutable engine');
    }
    const tools = { node: { ...selected.node, sha256: sha256(fs.readFileSync(process.execPath)) } };
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
    if (fs.statSync(wasmPath).size > MAX_FRAME) fail('module byte limit exceeded');
    const bytes = fs.readFileSync(wasmPath);
    rejectStartSection(bytes);
    const module = new WebAssembly.Module(bytes);
    this.compilations += 1;
    return this.install(module, { submission, wasm_sha256: sha256(bytes), source_sha256: sha256(source),
      wat_sha256: sha256(wat), optimized: this.optimized, compilations: this.compilations, stem });
  }

  install(module, record = {}) {
    if (this.pending || this.poisoned) fail('session is pending or poisoned');
    const imports = WebAssembly.Module.imports(module);
    const globals = new Set(this.abi.globals.map(global => global.name));
    const functions = new Set(this.abi.host_functions.map(fn => fn.name));
    for (const entry of imports) {
      const allowed = entry.module === this.abi.module && (
        entry.kind === 'memory' && entry.name === this.abi.memory.name
        || entry.kind === 'table' && entry.name === this.abi.table.name
        || entry.kind === 'global' && globals.has(entry.name)
        || entry.kind === 'function' && functions.has(entry.name));
      if (!allowed) fail(`undeclared module import: ${JSON.stringify(entry)}`);
    }
    const requests = this.trace.length;
    const sp = this.shared.sp.value;
    const instance = new WebAssembly.Instance(module, { [this.abi.module]: this.shared });
    if (requests !== this.trace.length || sp !== this.shared.sp.value) fail('instantiation executed candidate body');
    // Shared table entries keep callable instances alive; do not retain inert
    // modules in an unbounded side list across otherwise empty submissions.
    this.pending = { instance, module, record: { ...record, imports }, traceStart: requests };
    return { schema: 'noble-core-report/v1', profile: 'Core-Bootstrap', backend: 'managed-linear-memory',
      stage: 'wasm', outcome: 'ready', module: this.pending.record, guest_requests: 0, protected_operations: 0,
      candidate_prepare_requests: 0 };
  }

  execute({ inputs = [], limits = {} } = {}) {
    if (!this.pending || this.poisoned) fail('no prepared module or poisoned session');
    const { instance, record, traceStart } = this.pending;
    this.lastTraceStart = traceStart;
    this.pending = null;
    const runtime = instance.exports;
    // Retain exactly one executed instance so post-execution inspection reads
    // the same session state. The shared table already owns its callables.
    this.last = instance;
    const defaults = this.abi.limits_maximum;
    const keys = ['allocation_bytes', 'recipe_leaves', 'program_depth', 'operand_bytes', 'continuation_bytes', 'steps'];
    if (Object.keys(limits).some(key => !keys.includes(key))) fail('unknown runtime limit');
    let status = runtime.begin(...keys.map(key => integer(limits[key] ?? defaults[key], defaults[key], key)));
    let nativeTrap = null;
    try {
      for (const input of inputs) {
        if (status) break;
        status = this.inject(runtime, input);
      }
      if (status === 0) status = runtime.submit();
    } catch (error) {
      nativeTrap = `${error.name}: ${error.message}`;
      status = 6;
    }
    this.poisoned = status !== 0;
    const metrics = Object.fromEntries(Object.entries(this.abi.metrics).map(([id, name]) => [name, Number(runtime.metric(Number(id)))]));
    const outcome = status === 0 ? 'normal' : status === 1 || status === 2 ? 'runtime-exhausted' : 'trap';
    let stack = [];
    if (status === 0) stack = readStack(this, runtime);
    const report = { schema: 'noble-core-report/v1', profile: 'Core-Bootstrap', backend: 'managed-linear-memory',
      submission: record.submission ?? null, stage: 'wasm', outcome, status,
      quota_reason: this.abi.quota_reason[String(metrics.quota_reason)] ?? null, stack,
      request_trace: this.trace.slice(traceStart), guest_requests: this.trace.length - traceStart,
      protected_operations: 0, candidate_prepare_requests: 0, native_trap: nativeTrap,
      session_state: this.poisoned ? 'terminated-after-runtime-failure' : 'retained',
      metrics, module: record };
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
    return { schema: 'noble-core-report/v1', profile: 'Core-Bootstrap', backend: 'managed-linear-memory',
      stage: 'wasm', outcome: status ? 'injection-refused' : 'pushed', status, stack,
      guest_requests: 0, protected_operations: 0, candidate_prepare_requests: 0,
      session_state: 'retained' };
  }

  // This engine owns the snapshot; no caller-supplied bytes become values.
  // Cells are immutable and session handles stay live while the frame is parked.
  park() {
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
    if (input.type === 'I64') {
      const value = BigInt(input.value);
      if (BigInt.asIntN(64, value) !== value) fail('input is outside signed I64');
      return runtime.push_i64(value);
    }
    if (input.type === 'Bool') {
      if (typeof input.value !== 'boolean') fail('invalid Bool input');
      return runtime.push_bool(input.value ? 1 : 0);
    }
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
    return { schema: 'noble-core-report/v1', profile: 'Core-Bootstrap', backend: 'managed-linear-memory',
      stage: 'wasm', outcome: status ? 'observation-failed' : 'observed', status, index,
      events: status ? [] : readReflection(this, runtime),
      guest_requests: 0, protected_operations: 0, candidate_prepare_requests: 0 };
  }

  // Explicit projection of a Certified cell to its live subject Program handle.
  project(handle) {
    const runtime = this.runtime();
    const subject = runtime.certified_subject(integer(handle, this.abi.limits_maximum.session_cells, 'cell handle'));
    return { schema: 'noble-core-report/v1', profile: 'Core-Bootstrap', backend: 'managed-linear-memory',
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

