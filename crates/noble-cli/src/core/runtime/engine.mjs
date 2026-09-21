export class CoreEngine {
  constructor(selection, abi, { optimized = false, artifacts = null } = {}) {
    this.selection = selection;
    this.abi = abi;
    this.optimized = optimized;
    this.trace = [];
    this.pending = null;
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
      const text = this.text(offset, count);
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
    const defaults = this.abi.limits_maximum;
    const keys = ['allocation_bytes', 'recipe_leaves', 'program_depth', 'operand_bytes', 'continuation_bytes', 'steps'];
    if (Object.keys(limits).some(key => !keys.includes(key))) fail('unknown runtime limit');
    let status = runtime.begin(...keys.map(key => integer(limits[key] ?? defaults[key], defaults[key], key)));
    let nativeTrap = null;
    try {
      for (const input of inputs) {
        if (status) break;
        if (input.type === 'I64') {
          const value = BigInt(input.value);
          if (BigInt.asIntN(64, value) !== value) fail('input is outside signed I64');
          status = runtime.push_i64(value);
        } else if (input.type === 'Bool') {
          if (typeof input.value !== 'boolean') fail('invalid Bool input');
          status = runtime.push_bool(input.value ? 1 : 0);
        } else if (input.type === 'Unit') status = runtime.push_unit();
        else if (input.type === 'Program') status = runtime.push_program(integer(input.handle, 4096, 'program handle'));
        else fail('unsupported runtime input type');
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
    if (status === 0) stack = this.stack(runtime);
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

  text(offset, count) {
    integer(offset, this.memory.buffer.byteLength, 'text offset');
    integer(count, this.memory.buffer.byteLength - offset, 'text length');
    return utf8.decode(new Uint8Array(this.memory.buffer, offset, count));
  }

  signature(runtime, identity) {
    integer(identity, 16384, 'signature identity');
    return this.text(runtime.signature_address(identity), runtime.signature_length(identity));
  }

  stack(runtime) {
    const count = integer(runtime.stack_length(), 128, 'stack height');
    this.observationWork = 0;
    return Array.from({ length: count }, (_, at) => this.value(runtime, runtime.stack_kind(at), runtime.stack_value(at), 0));
  }

  charge(depth) {
    if (depth > 512 || ++this.observationWork > MAX_OBSERVATION_WORK) fail('observation limit exhausted');
  }

  boxed(runtime, handle, depth) {
    integer(handle, this.abi.limits_maximum.session_cells, 'cell handle');
    if (handle === 0 || handle > this.shared.heap_cursor.value) fail('invalid live cell handle');
    const kind = runtime.cell_kind(handle);
    return this.value(runtime, kind, kind < 4 ? runtime.cell_payload(handle) : BigInt(handle), depth);
  }

  value(runtime, kind, value, depth) {
    this.charge(depth);
    if (kind === 1) return { type: 'I64', value: String(value) };
    if (kind === 2) {
      if (value !== 0n && value !== 1n) fail('invalid Bool value');
      return { type: 'Bool', value: value === 1n };
    }
    if (kind === 3) return { type: 'Unit', value: null };
    const handle = integer(Number(value), this.shared.heap_cursor.value, 'value handle');
    if (!handle || runtime.cell_kind(handle) !== kind) fail('value tag disagrees with cell');
    if (kind === 4) return { type: 'Program', interface: {
      stack_in: this.signature(runtime, runtime.cell_y(handle)),
      stack_out: this.signature(runtime, runtime.cell_z(handle)),
      effects: [0, 1].filter(bit => Number(runtime.cell_payload(handle)) & (1 << bit))
        .map(bit => bit === 0 ? 'test.emit' : 'test.abort'),
    }, ...this.recipe(runtime, runtime.cell_c(handle), depth + 1) };
    if (kind === 5) return { type: 'Pair', value: [this.boxed(runtime, runtime.cell_a(handle), depth + 1), this.boxed(runtime, runtime.cell_b(handle), depth + 1)] };
    if (kind === 6 || kind === 7) {
      const items = [];
      let tail = handle;
      while (runtime.cell_kind(tail) === 6) {
        this.charge(depth);
        items.push(this.boxed(runtime, runtime.cell_a(tail), depth + 1));
        const next = runtime.cell_b(tail);
        if (next <= 0 || next >= tail) fail('cyclic/non-backward list');
        tail = next;
      }
      if (runtime.cell_kind(tail) !== 7) fail('invalid list tail');
      return { type: 'List', value: items };
    }
    if (kind === 10) return { type: 'Syntax', ...this.recipe(runtime, runtime.cell_a(handle), depth + 1) };
    if (kind === 11) return { type: 'Text', value: this.text(runtime.cell_x(handle), runtime.cell_y(handle)) };
    if (kind === 12 || kind === 13) return { type: 'Sum', variant: kind === 12 ? 'left' : 'right', value: this.boxed(runtime, runtime.cell_a(handle), depth + 1) };
    fail(`unsupported observable value kind: ${kind}`);
  }

  recipe(runtime, root, depth) {
    this.charge(depth);
    const result = [];
    const witnesses = [];
    const pending = root ? [root] : [];
    while (pending.length) {
      this.charge(depth);
      const handle = pending.pop();
      integer(handle, this.shared.heap_cursor.value, 'recipe handle');
      const kind = runtime.cell_kind(handle);
      if (kind === 9) {
        const left = runtime.cell_a(handle), right = runtime.cell_b(handle);
        if (left <= 0 || right <= 0 || left >= handle || right >= handle) fail('non-backward recipe graph');
        pending.push(right, left);
        continue;
      }
      if (kind !== 8) fail('invalid recipe node');
      const atom = runtime.cell_x(handle), value = runtime.cell_payload(handle), child = runtime.cell_a(handle);
      if (atom === 1) result.push({ literal: { type: 'I64', value: String(value) } });
      else if (atom === 4) result.push({ literal: { type: 'Bool', value: value !== 0n } });
      else if (atom === 5) result.push({ literal: { type: 'Unit', value: null } });
      else if (atom === 2) {
        const id = integer(Number(value), 23, 'builtin identity');
        result.push({ invoke: `${id < 22 ? 'builtin' : 'host'}:${names[id]}` });
      } else if (atom === 15) result.push({ invoke: `definition:${BigInt.asUintN(64, value)}` });
      else if (atom === 3) {
        const program = this.boxed(runtime, child, depth + 1);
        if (program.type !== 'Program') fail('quotation recipe has no program');
        result.push({ quotation: program.recipe, interface: program.interface, witnesses: program.witnesses });
      } else if (atom === 8) result.push({ literal: {
        ...this.boxed(runtime, child, depth + 1),
        schema: this.signature(runtime, runtime.cell_y(handle)),
      } });
      else fail(`unsupported recipe atom: ${atom}`);
      if (atom === 2 || atom === 15) {
        witnesses.push({
          node: result.length - 1,
          stack_in: this.signature(runtime, runtime.cell_y(handle)),
          stack_out: this.signature(runtime, runtime.cell_z(handle)),
          effects: [0, 1].filter(bit => runtime.cell_w(handle) & (1 << bit))
            .map(bit => bit === 0 ? 'test.emit' : 'test.abort'),
        });
      }
    }
    return { recipe: result, witnesses };
  }

  close() {
    this.pending = null;
    if (this.owned) fs.rmSync(this.directory, { recursive: true });
  }
}

