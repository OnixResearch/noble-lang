// A raw WebAssembly.Module is not an admission capability. Only the selected
// compiler path and the byte-exact artifact path may cross into install().
const installationCapability = Symbol('compiler-validated module');

// Upper bound on the WAT plus Wasm bytes a Calculator-v1 worker retains for
// exact module reuse (compiled V8 code for those modules is owned by V8).
const CALCULATOR_MODULE_CACHE_BYTES = 64 * 1024 * 1024;

const artifactEffects = new Map([
  ['test_emit', 'test.emit'],
  ['test_abort', 'test.abort'],
  ['test_emit_bound', 'test.emit'],
  ['test_clock_bound', 'test.clock'],
]);

// The independent Rust source/byte validator is the admission boundary. This
// second, deliberately narrow binary check protects the resident imported
// arena even if a malformed frame reaches the worker directly. V8 validates
// instructions; Rust validates the allowed instruction/feature subset.
const liveExportTypes = {
  begin: 'i32,i32,i32,i32,i32,i32->i32', submit: '->i32',
  push_i64: 'i64->i32', push_bool: 'i32->i32', push_unit: '->i32',
  push_program: 'i32->i32', push_contract: 'i32,i64,i32,i32->i32',
  push_evidence: 'i32,i32,i32,i32->i32',
  push_certified: 'i32,i32,i32,i64->i32', certified_subject: 'i32->i32',
  stack_length: '->i32', stack_kind: 'i32->i32', stack_value: 'i32->i64',
  cell_kind: 'i32->i32', cell_payload: 'i32->i64',
  cell_a: 'i32->i32', cell_b: 'i32->i32', cell_c: 'i32->i32',
  cell_x: 'i32->i32', cell_y: 'i32->i32', cell_z: 'i32->i32',
  cell_w: 'i32->i32', cell_n: 'i32->i32',
  observe: 'i32->i32', reflect: '->i32', recipe_length: '->i32',
  recipe_kind: 'i32->i32', recipe_value: 'i32->i64',
  metric: 'i32->i64', signature_address: 'i32->i32',
  signature_length: 'i32->i32',
};
const liveImportTypes = {
  live_propose: 'i64,i64,i32->i32',
  live_generation: '->i64',
};

function checkLiveBytes(bytes, abi) {
  if (!Buffer.isBuffer(bytes) || bytes.length < 8 || bytes.length > MAX_FRAME
    || bytes.readUInt32LE(0) !== 0x6d736100 || bytes.readUInt32LE(4) !== 1) {
    fail('invalid bounded Wasm module');
  }
  let at = 8, end = bytes.length, lastSection = 0;
  const byte = () => {
    if (at >= end) fail('truncated Wasm section');
    return bytes[at++];
  };
  const uint = () => {
    let value = 0, shift = 0;
    for (let n = 0; n < 5; n += 1, shift += 7) {
      const digit = byte();
      if (n === 4 && digit > 15) fail('invalid Wasm integer');
      value += (digit & 127) * 2 ** shift;
      if (!(digit & 128)) {
        if (n && digit === 0) fail('noncanonical Wasm integer');
        return value;
      }
    }
    fail('invalid Wasm integer');
  };
  const text = () => {
    const length = uint();
    if (length > end - at) fail('truncated Wasm name');
    const value = utf8.decode(bytes.subarray(at, at + length));
    at += length;
    return value;
  };
  const count = () => integer(uint(), MAX_FRAME, 'Wasm vector');
  const signature = () => {
    if (byte() !== 0x60) fail('unsupported Wasm function type');
    const params = [], results = [];
    const types = ['i32', 'i64'];
    const valueType = () => {
      const type = bytes[at - 1];
      if (type === 0x7f) return types[0];
      if (type === 0x7e) return types[1];
      fail('unsupported Wasm value type');
    };
    const p = count();
    if (p > 16) fail('oversized Wasm function signature');
    for (let i = 0; i < p; i += 1) { byte(); params.push(valueType()); }
    const r = count();
    if (r > 2) fail('oversized Wasm function result');
    for (let i = 0; i < r; i += 1) { byte(); results.push(valueType()); }
    return `${params.join(',')}->${results.join(',')}`;
  };
  const limits = (initial, maximum) => {
    if (byte() !== 1 || uint() !== initial || uint() !== maximum) fail('incompatible Wasm arena limits');
  };
  const expectedGlobals = new Map(abi.globals.map(global => [global.name, global.type]));
  const types = [], functionTypes = [], imports = [], exports = [];
  const seenImports = new Set(), seenExports = new Set();
  let memoryImports = 0, tableImports = 0, globalImports = 0, definedFunctions = 0;
  while (at < bytes.length) {
    end = bytes.length;
    const id = byte(), size = uint();
    if (size > bytes.length - at) fail('truncated Wasm section');
    end = at + size;
    if (id !== 0) {
      // Data count is ordered before code by the bulk-memory proposal.
      const order = id === 12 ? 9.5 : id;
      if (id > 12 || order <= lastSection || id === 8) fail('invalid or start Wasm section');
      lastSection = order;
    }
    if (id === 1) {
      const n = count();
      if (n > 16384) fail('too many Wasm types');
      for (let i = 0; i < n; i += 1) types.push(signature());
    } else if (id === 2) {
      const n = count();
      if (n > 256) fail('too many Wasm imports');
      for (let i = 0; i < n; i += 1) {
        const module = text(), name = text(), kind = byte();
        if (module !== abi.module || seenImports.has(name)) fail('unexpected or duplicate Wasm import');
        seenImports.add(name);
        if (kind === 0) {
          const type = types[uint()];
          if (!Object.hasOwn(liveImportTypes, name) || type !== liveImportTypes[name]) {
            fail('undeclared or mistyped live host effect');
          }
          functionTypes.push(type);
          imports.push({ module, name, kind: 'function' });
        } else if (kind === 1) {
          if (name !== abi.table.name || byte() !== 0x70) fail('incompatible Wasm table import');
          limits(abi.table.initial, abi.table.maximum);
          tableImports += 1;
          imports.push({ module, name, kind: 'table' });
        } else if (kind === 2) {
          if (name !== abi.memory.name) fail('incompatible Wasm memory import');
          limits(abi.memory.initial, abi.memory.maximum);
          memoryImports += 1;
          imports.push({ module, name, kind: 'memory' });
        } else if (kind === 3) {
          const type = byte(), mutable = byte();
          if (mutable !== 1 || expectedGlobals.get(name) !==
            (type === 0x7f ? 'i32' : type === 0x7e ? 'i64' : null)) {
            fail('incompatible Wasm global import');
          }
          globalImports += 1;
          imports.push({ module, name, kind: 'global' });
        } else fail('unsupported Wasm import kind');
      }
    } else if (id === 3) {
      definedFunctions = count();
      if (definedFunctions > 16384) fail('too many Wasm functions');
      for (let i = 0; i < definedFunctions; i += 1) {
        const type = types[uint()];
        if (type === undefined) fail('invalid Wasm function type');
        functionTypes.push(type);
      }
    } else if (id === 7) {
      const n = count();
      if (n !== Object.keys(liveExportTypes).length + 1
        || Object.keys(abi.exports).length !== n - 1) fail('incompatible Wasm export count');
      for (let i = 0; i < n; i += 1) {
        const name = text(), kind = byte(), index = uint();
        if (seenExports.has(name)) fail('duplicate Wasm export');
        seenExports.add(name);
        if (name === 'memory') {
          if (kind !== 2 || index !== 0) fail('incompatible Wasm memory export');
        } else if (!Object.hasOwn(abi.exports, name) || kind !== 0
          || functionTypes[index] !== liveExportTypes[name]) fail('incompatible Wasm function export');
        exports.push({ name, kind: kind === 0 ? 'function' : 'memory' });
      }
    } else if (id === 4 || id === 5) {
      fail('module must import its only table and memory');
    } else if (id === 9) {
      const n = count();
      if (n > 16384) fail('too many Wasm element segments');
      for (let i = 0; i < n; i += 1) {
        const flags = uint();
        if (flags !== 1 && flags !== 3 || byte() !== 0) fail('active or unsupported Wasm element segment');
        const length = count();
        if (length > 16384) fail('oversized Wasm element segment');
        for (let j = 0; j < length; j += 1) uint();
      }
    } else if (id === 11) {
      const n = count();
      for (let i = 0; i < n; i += 1) {
        if (uint() !== 1) fail('active or unsupported Wasm data segment');
        const length = uint();
        if (length > end - at) fail('truncated Wasm data segment');
        at += length;
      }
    } else {
      // V8 validates code, locals, data count, and every skipped section.
      at = end;
    }
    if (at !== end) fail('invalid Wasm section length');
  }
  if (memoryImports !== 1 || tableImports !== 1 || globalImports !== expectedGlobals.size
    || seenExports.size !== Object.keys(liveExportTypes).length + 1) fail('incomplete Wasm arena ABI');
  return { imports, exports };
}

export class CoreEngine {
  constructor(selection, abi, { optimized = false, artifacts = null, bindings = [],
    declared_modules = false, declared_extension = null, in_process_live = false,
    text_byte_cursor = false, calculator = false, calculator_shell_state = false,
    calculator_assembler = null, calculator_trace = false, calculator_module_cache = false } = {}) {
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
    this.inProcessLive = in_process_live === true;
    configureDeclaredAbi(this, this.abi, declared_modules, declared_extension, bindings,
      text_byte_cursor, calculator);
    if (!calculator && calculator_assembler !== null) fail('calculator assembler cannot enter another profile');
    if (calculator_shell_state && !calculator) fail('opaque calculator state requires the calculator profile');
    if (calculator_trace !== false && (calculator_trace !== true || !calculator))
      fail('calculator timings require the calculator profile');
    if (calculator_module_cache !== false && (calculator_module_cache !== true || !calculator))
      fail('module reuse requires the calculator profile');
    this.calculatorShellState = calculator_shell_state;
    this.calculatorAssembler = calculator_assembler;
    // Observation only: ready reports gain a `timing` object, nothing else.
    this.calculatorTrace = calculator_trace;
    this.optimized = optimized;
    initializeHostState(this, bindings);
    this.pending = null;
    this.parked = null;
    this.poisoned = false;
    // Calculator-v1 signature ID -> canonical key, as published by the
    // latest executed module and its predecessors (see recordSignatures).
    this.signatures = [];
    // Exact-WAT module reuse within this worker (Calculator-v1 only).
    this.calculatorModuleCache = calculator_module_cache;
    this.calculatorModules = new Map();
    this.calculatorModuleBytes = 0;
    this.compilations = 0;
    this.lastTraceStart = 0;
    this.staged = null;
    this.liveInstalling = false;
    this.liveSourceGeneration = 0n;
    this.liveGrant = null;
    this.liveProposal = null;
    this.liveProposalDenied = false;
    this.liveExecuting = false;
    this.proposalInspectionFailed = false;
    // Artifact files in creation order, so an arena rollback can also remove
    // the scratch files of the submissions it undoes.
    this.savedFiles = [];
    // The last committed Calculator-v1 arena (see calculatorCommit).
    this.calculatorCommitted = null;
    this.tools = this.checkTools();
    this.owned = artifacts === null;
    this.directory = this.owned ? fs.mkdtempSync(path.join(os.tmpdir(), 'noble-core-')) : path.join(artifacts, 'engine');
    if (!this.owned) fs.mkdirSync(this.directory);
    // Calculator-v1 modules import a 16..64-page memory. The engine supplies
    // all 64 pages up front: Wasm memory cannot shrink, so a restore after a
    // failed submission grew the arena could never match the committed size.
    // At the maximum, runtime growth is a no-op and growth beyond 64 pages
    // still fails exactly as before.
    this.memory = new WebAssembly.Memory(this.profile === 'Calculator-v1'
      ? { ...this.abi.memory, initial: this.abi.memory.maximum } : this.abi.memory);
    this.table = new WebAssembly.Table(this.abi.table);
    this.shared = Object.create(null);
    installSharedGlobals(this, this.abi);
    installTestHosts(this, declared_modules || calculator);
    if (this.inProcessLive) {
      this.shared.live_propose = (owner, expected, program) => this.livePropose(owner, expected, program);
      this.shared.live_generation = () => this.liveGeneration();
      // Includes callbacks retained by *older* instances in the live table.
      for (const host of this.hostFunctions) {
        const callback = this.shared[host.name];
        this.shared[host.name] = (...args) => {
          if (this.liveInstalling) fail('host callback during live installation');
          return callback(...args);
        };
      }
    }
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
    if (this.inProcessLive) return tools;
    if (this.profile === 'Calculator-v1') {
      if (this.optimized || typeof this.calculatorAssembler !== 'string'
        || !path.isAbsolute(this.calculatorAssembler)
        || fs.realpathSync(this.calculatorAssembler) !== this.calculatorAssembler)
        fail('invalid calculator-only assembler selection');
      const version = spawnSync(this.calculatorAssembler, ['--internal-calculator-wat-version'],
        { encoding: 'utf8', env: {}, timeout: 10000, maxBuffer: 65536 });
      if (version.error || version.status !== 0
        || version.stdout !== 'noble-calculator-wat 1.243.0\n')
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
    this.savedFiles.push(file);
  }

  tool(name, args, label) {
    if (this.inProcessLive) fail('external Wasm tools are unavailable in live mode');
    const result = spawnSync(this.selection.tools[name].path, args, {
      env: {}, encoding: 'utf8', timeout: 60000, killSignal: 'SIGKILL', maxBuffer: MAX_FRAME,
    });
    this.save(`${label}.json`, { binary: this.selection.tools[name].path, args,
      status: result.status, signal: result.signal, stdout: result.stdout, stderr: result.stderr,
      error: result.error ? String(result.error) : null });
    if (result.error || result.status !== 0) fail(`${label} failed: ${result.error ?? result.stderr}`);
  }

  prepare(wat, source = Buffer.alloc(0), submission = this.compilations + 1, generation = null) {
    if (this.inProcessLive) fail('live mode requires binary preparation');
    if (this.pending || this.poisoned) fail('session is pending or poisoned');
    if (wat.length > MAX_FRAME || source.length > 65536) fail('preparation byte limit exceeded');
    // Only a calculator module imports the compiler generation its host
    // validated it against; every other profile bakes its own guard.
    if ((this.profile === 'Calculator-v1') !== (generation !== null)) fail('invalid submit generation');
    if (generation !== null) integer(generation, 0x7fffffff, 'submit generation');
    const stem = `module-${submission}`;
    const persistCompiledAssets = this.profile !== 'Calculator-v1' || !this.owned;
    if (persistCompiledAssets) this.save(`${stem}.wat`, wat);
    this.save(`${stem}.noble`, source);
    let wasmPath = null;
    let bytes;
    let reused = null;
    const clock = () => process.hrtime.bigint();
    const micros = (start, end) => Number((end - start) / 1000n);
    const timing = this.calculatorTrace ? { worker_cache: 'miss' } : null;
    const watDigest = sha256(wat);
    if (this.calculatorModuleCache) {
      const cached = this.calculatorModules.get(watDigest);
      if (cached && cached.wat.equals(wat)) reused = cached;
    }
    if (reused) {
      // The exact WAT bytes were already assembled by the selected assembler
      // in this worker and compiled by this V8; reuse that Wasm and Module.
      this.calculatorModules.delete(watDigest);
      this.calculatorModules.set(watDigest, reused);
      if (timing) timing.worker_cache = 'hit';
      if (timing) Object.assign(timing, { rehash_us: 0, assemble_us: 0 });
      // Content-addressed provenance, not the earlier stem: a calculator
      // restore removes the files of the submissions it undoes and later
      // reissues their stems to different modules.
      this.save(`${stem}-calculator-assemble.json`, { toolchain: 'calculator-experimental-wat-v1',
        assembler_sha256: this.tools.calculator_wat.sha256, wat_sha256: watDigest,
        reused_assembly: true });
      if (persistCompiledAssets) this.save(`${stem}.wasm`, reused.bytes);
      bytes = reused.bytes;
    } else if (this.profile === 'Calculator-v1') {
      const rehashing = clock();
      if (sha256(fs.readFileSync(this.calculatorAssembler)) !== this.tools.calculator_wat.sha256)
        fail('calculator assembler changed after selection');
      const assembling = clock();
      const result = spawnSync(this.calculatorAssembler, ['--internal-calculator-wat-assemble'],
        { input: wat, env: {}, timeout: 60000, maxBuffer: MAX_FRAME });
      if (timing) {
        const assembled = clock();
        timing.rehash_us = micros(rehashing, assembling);
        timing.assemble_us = micros(assembling, assembled);
      }
      this.save(`${stem}-calculator-assemble.json`, { toolchain: 'calculator-experimental-wat-v1',
        assembler_sha256: this.tools.calculator_wat.sha256, wat_sha256: watDigest,
        status: result.status, signal: result.signal,
        stderr: result.stderr.toString(), error: result.error ? String(result.error) : null });
      if (result.error || result.status !== 0 || !result.stdout.length || result.stdout.length > MAX_FRAME)
        fail(`calculator-only WAT assembly refused: ${result.error ?? result.stderr.toString()}`);
      if (persistCompiledAssets) {
        wasmPath = path.join(this.directory, `${stem}.wasm`);
        // Tracked like every saved file, so an arena rollback removes it.
        this.save(`${stem}.wasm`, result.stdout);
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
    const compiling = clock();
    const module = reused ? reused.module : new WebAssembly.Module(bytes);
    const compiled = clock();
    if (this.calculatorModuleCache && !reused) this.retainCalculatorModule(watDigest, wat, bytes, module);
    this.compilations += 1;
    const record = { submission, wasm_sha256: sha256(bytes), source_sha256: sha256(source),
      wat_sha256: watDigest, optimized: this.optimized, compilations: this.compilations, stem,
      ...(this.profile === 'Calculator-v1' ? { assembly_toolchain: 'calculator-experimental-wat-v1',
        assembler_sha256: this.tools.calculator_wat.sha256 } : {}) };
    const instantiating = clock();
    const ready = this.install(module, record, installationCapability, generation);
    if (timing) {
      timing.compile_us = micros(compiling, compiled);
      timing.instantiate_us = micros(instantiating, clock());
      timing.wasm_bytes = bytes.length;
      ready.timing = timing;
    }
    return ready;
  }

  // Calculator-v1 module reuse: exact WAT bytes -> the Wasm the selected
  // assembler produced for them and its compiled V8 Module, never keyed or
  // matched by digest alone. Instances are not retained. Retained WAT and
  // Wasm bytes are bounded; the least recently used entries are evicted.
  // The cache is not part of a calculator commit and a restore never reads
  // it: a compiled Module is immutable code without instance state, so one
  // compiled during a failed, restored submission stays exact.
  retainCalculatorModule(digest, wat, bytes, module) {
    const size = wat.length + bytes.length;
    if (size > CALCULATOR_MODULE_CACHE_BYTES) return;
    // A digest collision with different bytes replaces the older entry.
    const previous = this.calculatorModules.get(digest);
    if (previous) {
      this.calculatorModules.delete(digest);
      this.calculatorModuleBytes -= previous.size;
    }
    this.calculatorModules.set(digest, { wat: Buffer.from(wat), bytes: Buffer.from(bytes), module, size });
    this.calculatorModuleBytes += size;
    for (const [key, entry] of this.calculatorModules) {
      if (this.calculatorModuleBytes <= CALCULATOR_MODULE_CACHE_BYTES) break;
      this.calculatorModules.delete(key);
      this.calculatorModuleBytes -= entry.size;
    }
  }

  liveModule(bytes, source, submission) {
    if (!this.inProcessLive || this.pending || this.parked || this.staged || this.poisoned
      || this.liveProposal) {
      fail('live session is pending, parked, staged, or poisoned');
    }
    if (!Buffer.isBuffer(source) || source.length > 65536) fail('invalid live source frame');
    integer(submission, Number.MAX_SAFE_INTEGER, 'submission');
    const policy = checkLiveBytes(bytes, this.abi);
    const module = new WebAssembly.Module(bytes);
    for (const [actual, expected] of [
      [WebAssembly.Module.imports(module), policy.imports],
      [WebAssembly.Module.exports(module), policy.exports],
    ]) {
      if (actual.length !== expected.length || actual.some((entry, index) =>
        Object.keys(expected[index]).some(key => entry[key] !== expected[index][key]))) {
        fail('V8 disagrees with bounded Wasm ABI');
      }
    }
    return { module, record: { submission, wasm_sha256: sha256(bytes),
      source_sha256: sha256(source), optimized: false,
      compilations: this.compilations + 1, imports: policy.imports } };
  }

  compileBinary(bytes, source, submission) {
    // Interactive expressions remain ordinary pending preparations against
    // the same arena, including after a staged source-file publication.
    if (!this.inProcessLive || this.pending || this.staged || this.parked || this.poisoned) {
      fail('live compilation requires an idle arena');
    }
    let candidate;
    try { candidate = this.liveModule(bytes, source, submission); }
    catch (error) { return this.liveRefusal(`${error.name}: ${error.message}`, 'compile-refused'); }
    const { module, record } = candidate;
    this.liveInstalling = true;
    try {
      const ready = this.install(module, record, installationCapability);
      this.compilations += 1;
      return ready;
    } finally {
      this.liveInstalling = false;
    }
  }

  liveRefusal(diagnostic, outcome = 'reload-refused') {
    const previousWork = this.observationWork;
    const previousGuard = this.liveInstalling;
    this.liveInstalling = true;
    try {
      return { schema: 'noble-core-report/v1', profile: this.profile,
        backend: 'managed-linear-memory', stage: 'wasm', outcome,
        diagnostic: String(diagnostic), generation: this.shared.generation.value,
        stack: this.last ? readStack(this, this.last.exports) : [],
        guest_requests: 0, protected_operations: 0, candidate_prepare_requests: 0,
        session_state: 'retained' };
    } finally {
      this.observationWork = previousWork;
      this.liveInstalling = previousGuard;
    }
  }

  // A staged old table entry must NOT be copied: it would retain an old
  // instance bound to the live memory/globals, permitting indirect calls in a
  // supposedly isolated preflight. Empty-root checking needs no old code.
  stageArena() {
    if (this.memory.buffer.byteLength !== this.abi.memory.maximum * 65536
      || this.table.length !== this.abi.table.maximum) fail('live arena exceeds fixed limits');
    const memory = new WebAssembly.Memory(this.abi.memory);
    new Uint8Array(memory.buffer).set(new Uint8Array(this.memory.buffer));
    const table = new WebAssembly.Table(this.abi.table);
    const shared = Object.create(null);
    shared[this.abi.memory.name] = memory;
    shared[this.abi.table.name] = table;
    for (const global of this.abi.globals) {
      shared[global.name] = new WebAssembly.Global({ value: global.type, mutable: true },
        this.shared[global.name].value);
    }
    for (const host of this.hostFunctions) {
      shared[host.name] = () => fail('host callback during isolated live preflight');
    }
    return { memory, table, shared };
  }

  liveStackSnapshot(memory, globals) {
    const slots = this.abi.operand_slots;
    const sp = integer(globals.sp.value, slots.maximum, 'live stack height');
    const cells = integer(globals.heap_cursor.value,
      this.abi.limits_maximum.session_cells, 'live cell count');
    const end = 65536 + cells * 48;
    if (slots.offset + sp * slots.stride > memory.buffer.byteLength || end > memory.buffer.byteLength) {
      fail('live snapshot exceeds memory');
    }
    return { sp, heapCursor: cells, operand: Uint8Array.from(new Uint8Array(memory.buffer,
      slots.offset, sp * slots.stride)), cells: Uint8Array.from(new Uint8Array(memory.buffer, 65536, cells * 48)) };
  }

  checkLivePreserved(snapshot, memory, shared) {
    const slots = this.abi.operand_slots;
    if (shared.sp.value !== snapshot.sp || shared.cp.value !== 0 || shared.failure.value !== 0
      || shared.heap_cursor.value < snapshot.heapCursor
      || !Buffer.from(memory.buffer, slots.offset, snapshot.operand.length)
        .equals(Buffer.from(snapshot.operand))
      || !Buffer.from(memory.buffer, 65536, snapshot.cells.length)
        .equals(Buffer.from(snapshot.cells))) {
      fail('empty live root altered existing stack or cells');
    }
  }

  liveRoot(instance, memory, shared, original) {
    const runtime = instance.exports, maxima = this.abi.limits_maximum;
    const before = shared.generation.value;
    if (runtime.memory !== memory) fail('live module exported a different memory');
    const begun = runtime.begin(maxima.allocation_bytes, maxima.recipe_leaves,
      maxima.program_depth, maxima.operand_bytes, maxima.continuation_bytes, maxima.steps);
    if (begun !== 0) fail(`live begin failed: ${begun}`);
    const status = runtime.submit();
    if (status !== 0) fail(`empty live root failed: ${status}`);
    if (shared.generation.value !== (before + 1 | 0)) fail('empty live root did not advance generation');
    this.checkLivePreserved(original, memory, shared);
  }

  stageBinary(bytes, source, submission) {
    try {
      const { module, record } = this.liveModule(bytes, source, submission);
      if (this.pending || this.shared.cp.value !== 0) {
        fail('live staging requires an idle arena');
      }
      const original = this.liveStackSnapshot(this.memory, this.shared);
      const arena = this.stageArena();
      this.liveInstalling = true;
      try {
        const instance = new WebAssembly.Instance(module, { [this.abi.module]: arena.shared });
        // On the first generation the empty root can execute in a completely
        // independent arena. Later modules refer to the already-installed
        // core dispatch slots (0..3). Copying those slots would bind this
        // shadow to live memory and violate isolation, so later shadow stages
        // validate and instantiate only. The no-effect empty root executes
        // after publication starts, with full live-state rollback on failure.
        if (!this.last) this.liveRoot(instance, arena.memory, arena.shared, original);
      } finally {
        this.liveInstalling = false;
      }
      // No Wasm bytes or staging arena need survive the preflight.
      this.staged = { module, record,
        generation: this.shared.generation.value };
      return { schema: 'noble-core-report/v1', profile: this.profile,
        backend: 'managed-linear-memory', stage: 'wasm', outcome: 'ready',
        staged: true, module: record, generation: this.shared.generation.value,
        guest_requests: 0, protected_operations: 0, candidate_prepare_requests: 0 };
    } catch (error) {
      return this.liveRefusal(`${error.name}: ${error.message}`);
    }
  }

  discardBinary() {
    if (!this.inProcessLive || !this.staged) fail('no staged live module to discard');
    this.staged = null;
    return { outcome: 'discarded', generation: this.shared.generation.value };
  }

  setLiveGrant(name, owner, sourceGeneration) {
    const selected = this.inProcessLive && !this.poisoned && !this.pending && !this.parked
      && !this.staged && !this.liveProposal && !this.liveExecuting
      && typeof name === 'string' && Buffer.byteLength(name) > 0
      && Buffer.byteLength(name) <= 256 && typeof owner === 'bigint'
      && owner >= 0n && owner <= 0xffffffffffffffffn
      && sourceGeneration === this.liveSourceGeneration
      && sourceGeneration <= 0x7fffffffffffffffn;
    if (selected) this.liveGrant = Object.freeze({ name, owner, sourceGeneration });
    return { schema: 'noble-core-report/v1', profile: this.profile,
      stage: 'wasm', outcome: selected ? 'grant-selected' : 'grant-refused',
      source_generation: this.liveSourceGeneration.toString(),
      ...(selected ? { name, owner: owner.toString() } : {}),
      guest_requests: 0, protected_operations: 0, candidate_prepare_requests: 0 };
  }

  liveGeneration() {
    globalThis.nobleSafetyObserve?.('guest-request');
    if (!this.inProcessLive || !this.liveExecuting || this.liveInstalling) {
      fail('live generation observation outside execution');
    }
    if (this.trace.length >= 4096) fail('live observation trace exhausted');
    this.trace.push(`live.observe-generation:${this.liveSourceGeneration}`);
    return this.liveSourceGeneration;
  }

  livePropose(owner, expected, program) {
    globalThis.nobleSafetyObserve?.('guest-request');
    if (!this.inProcessLive || !this.liveExecuting || this.liveInstalling) {
      fail('live proposal outside execution');
    }
    const deny = reason => {
      this.liveProposalDenied = true;
      if (this.trace.length < 4096) this.trace.push(`live.propose:denied:${reason}`);
      return 1;
    };
    if (this.trace.length >= 4096) return deny('trace-exhausted');
    const grant = this.liveGrant;
    if (!grant || grant.sourceGeneration !== this.liveSourceGeneration) return deny('no-current-grant');
    if (BigInt.asUintN(64, owner) !== grant.owner) return deny('owner-mismatch');
    if (expected !== grant.sourceGeneration) return deny('generation-mismatch');
    if (this.liveProposal) return deny('already-queued');
    let snapshot;
    try { snapshot = captureLiveProgram(this, program); }
    catch (error) { return deny(`invalid-program:${error.message}`); }
    this.liveProposal = { grant, expected, program, snapshot };
    this.trace.push(`live.propose:${grant.name}:queued`);
    return 0;
  }

  takeLiveProposal() {
    if (!this.inProcessLive || this.poisoned || this.pending || this.staged || this.liveExecuting) {
      fail('live proposal is unavailable');
    }
    const queued = this.liveProposal;
    this.liveProposal = null;
    if (!queued) return { schema: 'noble-core-report/v1', profile: this.profile,
      stage: 'wasm', outcome: 'proposal-none',
      guest_requests: 0, protected_operations: 0, candidate_prepare_requests: 0 };
    let snapshotVerified = false;
    const refused = diagnostic => ({ schema: 'noble-core-report/v1', profile: this.profile,
      stage: 'wasm', outcome: 'proposal-refused', diagnostic,
      name: queued.grant.name, owner: queued.grant.owner.toString(),
      expected_generation: queued.expected.toString(),
      source_generation: queued.grant.sourceGeneration.toString(),
      inspected_source_generation: this.liveSourceGeneration.toString(),
      candidate_snapshot_sha256: queued.snapshot.sha256,
      postreturn_snapshot_verified: snapshotVerified,
      guest_requests: 0, protected_operations: 0, candidate_prepare_requests: 0 });
    if (queued.grant !== this.liveGrant
      || queued.grant.sourceGeneration !== this.liveSourceGeneration) {
      return refused('proposal grant or selected-source generation changed');
    }
    try { checkLiveProgramSnapshot(this, queued.snapshot); }
    catch (error) {
      this.poisoned = true;
      this.proposalInspectionFailed = true;
      fail(`immutable live proposal changed: ${error.message}`);
    }
    snapshotVerified = true;
    let runtime, saved;
    try {
      runtime = this.runtime();
      saved = this.snapshotArena();
    } catch (error) {
      this.poisoned = true;
      this.proposalInspectionFailed = true;
      fail(`cannot snapshot live proposal inspection: ${error.message}`);
    }
    const restore = () => {
      try { this.rollbackArena(saved); }
      catch (error) {
        this.poisoned = true;
        this.proposalInspectionFailed = true;
        fail(`live proposal rollback failed: ${error.message}`);
      }
    };
    try {
      if (runtime.cell_kind(queued.program) !== 4
        || runtime.cell_payload(queued.program) !== 0n
        || runtime.cell_y(queued.program) !== queued.snapshot.inputId
        || runtime.cell_z(queued.program) !== queued.snapshot.outputId
        || readSignature(this, runtime, queued.snapshot.inputId) !== '[I64]'
        || readSignature(this, runtime, queued.snapshot.outputId) !== '[I64]') {
        restore();
        return refused('proposal Program is not pure [I64] -> [I64]');
      }
      const before = this.shared.observed_program.value;
      let events;
      try {
        this.shared.observed_program.value = queued.program;
        const status = runtime.reflect();
        if (status !== 0) {
          restore();
          return refused(`bounded proposal reflection failed: ${status}`);
        }
        events = readReflection(this, runtime);
      } finally {
        this.shared.observed_program.value = before;
      }
      if (events.length > 2045) {
        restore();
        return refused('proposal reflection exceeds bounded event envelope');
      }
      // reflect() walks the root Program's recipe, not its own interface.
      // These three headers are taken from the callback-time immutable cell,
      // already compared with the live cell and checked signature descriptors.
      events.unshift(
        { kind: 17, value: String(queued.snapshot.inputId) },
        { kind: 18, value: String(queued.snapshot.outputId) },
        { kind: 19, value: '0' },
      );
      const signatures = new Map();
      let signatureBytes = 0;
      for (const event of events) {
        if (event.kind !== 16 && event.kind !== 17 && event.kind !== 18
          && event.kind !== 20 && event.kind !== 21) continue;
        const id = integer(Number(event.value), 4096, 'proposal signature identity');
        if (signatures.has(id)) continue;
        if (signatures.size >= 64) {
          restore();
          return refused('proposal has too many signature identities');
        }
        const length = runtime.signature_length(id);
        if (!Number.isInteger(length) || length < 0 || length > 16384 - signatureBytes) {
          restore();
          return refused('proposal signatures exceed bounded receipt');
        }
        const descriptor = readText(this, runtime.signature_address(id), length);
        signatureBytes += length;
        signatures.set(id, descriptor);
      }
      const report = { schema: 'noble-core-report/v1', profile: this.profile,
        stage: 'wasm', outcome: 'proposal-pending',
        name: queued.grant.name, owner: queued.grant.owner.toString(),
        expected_generation: queued.expected.toString(),
        source_generation: this.liveSourceGeneration.toString(),
        input_signature: '[I64]', output_signature: '[I64]',
        input_signature_id: queued.snapshot.inputId,
        output_signature_id: queued.snapshot.outputId, effects: 0,
        candidate_snapshot_sha256: queued.snapshot.sha256,
        postreturn_snapshot_verified: true,
        signatures: Array.from(signatures, ([id, descriptor]) => ({ id, descriptor })),
        events, guest_requests: 0, protected_operations: 0, candidate_prepare_requests: 0 };
      restore();
      return report;
    } catch (error) {
      if (this.proposalInspectionFailed) throw error;
      restore();
      return refused(`proposal inspection failed: ${error.message}`);
    }
  }

  // Byte-exact engine arena snapshot: all of memory, every table entry, every
  // ABI global, the Calculator-v1 signature table and the engine counters.
  // It requires the arena at its fixed maximum, so no restore can meet a
  // resized memory or table.
  snapshotArena() {
    if (this.memory.buffer.byteLength !== this.abi.memory.maximum * 65536
      || this.table.length !== this.abi.table.maximum) fail('arena is not at its fixed maximum');
    return { memory: new Uint8Array(this.memory.buffer).slice(),
      table: Array.from({ length: this.table.length }, (_, index) => this.table.get(index)),
      globals: this.abi.globals.map(global => this.shared[global.name].value),
      signatures: this.signatures.slice(), files: this.savedFiles.length,
      trace: this.trace.slice(), boundRequests: this.boundRequests.slice(),
      hostRequestsTotal: this.hostRequestsTotal, boundRequestsTotal: this.boundRequestsTotal,
      protectedOperations: this.protectedOperations, ambientFallbackCalls: this.ambientFallbackCalls,
      boundTraceExhausted: this.boundTraceExhausted, boundTraceTerminal: this.boundTraceTerminal,
      adapters: Array.from(this.boundAdapters.values(), adapter => [adapter.scriptIndex, adapter.invocations]),
      pending: this.pending, last: this.last, lastTraceStart: this.lastTraceStart,
      liveSourceGeneration: this.liveSourceGeneration, liveGrant: this.liveGrant,
      compilations: this.compilations, parked: this.parked, poisoned: this.poisoned,
      observationWork: this.observationWork };
  }

  rollbackArena(saved) {
    if (this.memory.buffer.byteLength !== saved.memory.length || this.table.length !== saved.table.length) {
      fail('cannot restore a resized arena');
    }
    new Uint8Array(this.memory.buffer).set(saved.memory);
    for (let index = 0; index < saved.table.length; index += 1) this.table.set(index, saved.table[index]);
    this.abi.globals.forEach((global, index) => { this.shared[global.name].value = saved.globals[index]; });
    this.signatures = saved.signatures.slice();
    for (const file of this.savedFiles.splice(saved.files)) {
      fs.rmSync(path.join(this.directory, file), { force: true });
    }
    this.trace = saved.trace.slice();
    this.boundRequests = saved.boundRequests.slice();
    this.hostRequestsTotal = saved.hostRequestsTotal;
    this.boundRequestsTotal = saved.boundRequestsTotal;
    this.protectedOperations = saved.protectedOperations;
    this.ambientFallbackCalls = saved.ambientFallbackCalls;
    this.boundTraceExhausted = saved.boundTraceExhausted;
    this.boundTraceTerminal = saved.boundTraceTerminal;
    Array.from(this.boundAdapters.values()).forEach((adapter, index) => {
      [adapter.scriptIndex, adapter.invocations] = saved.adapters[index];
    });
    this.pending = saved.pending;
    this.last = saved.last;
    this.lastTraceStart = saved.lastTraceStart;
    this.liveSourceGeneration = saved.liveSourceGeneration;
    this.liveGrant = saved.liveGrant;
    this.compilations = saved.compilations;
    this.parked = saved.parked;
    this.poisoned = saved.poisoned;
    this.observationWork = saved.observationWork;
    if (!Buffer.from(this.memory.buffer).equals(Buffer.from(saved.memory))
      || this.abi.globals.some((global, index) => this.shared[global.name].value !== saved.globals[index])
      || saved.table.some((entry, index) => this.table.get(index) !== entry)
      || this.signatures.length !== saved.signatures.length
      || saved.signatures.some((key, index) => this.signatures[index] !== key)
      || this.savedFiles.length !== saved.files) {
      fail('arena rollback verification failed');
    }
  }

  // A committed Calculator-v1 session state. Only the trusted host issues
  // this, after it has validated a submission's guest result and display.
  calculatorCommit() {
    if (this.profile !== 'Calculator-v1' || this.poisoned || this.pending || this.parked !== null) {
      fail('calculator commit requires an idle, unpoisoned calculator engine');
    }
    this.calculatorCommitted = this.snapshotArena();
    return { schema: 'noble-core-report/v1', profile: this.profile, stage: 'wasm',
      outcome: 'committed', arena: this.calculatorArena(),
      guest_requests: 0, protected_operations: 0, candidate_prepare_requests: 0 };
  }

  // Return to the last commit after any failed submission, including a
  // native quota/trap that poisoned the instance. This is a trusted engine
  // operation over bytes the guest itself produced and committed; no guest
  // value selects it or what it restores. A failed verification is terminal.
  calculatorRestore() {
    const saved = this.calculatorCommitted;
    if (this.profile !== 'Calculator-v1' || saved === null) fail('no committed calculator arena');
    this.calculatorCommitted = null;
    this.rollbackArena(saved);
    this.calculatorCommitted = saved;
    return { schema: 'noble-core-report/v1', profile: this.profile, stage: 'wasm',
      outcome: 'restored', arena: this.calculatorArena(),
      guest_requests: 0, protected_operations: 0, candidate_prepare_requests: 0 };
  }

  // A session-independent digest of the arena. Table entries are compared by
  // Wasm function index and arity, since references from different engines
  // are never identical.
  calculatorArena() {
    if (this.profile !== 'Calculator-v1') fail('calculator arena digest requires the calculator profile');
    const table = crypto.createHash('sha256');
    for (let index = 0; index < this.table.length; index += 1) {
      const entry = this.table.get(index);
      table.update(entry === null ? '-;' : `${entry.name}/${entry.length};`);
    }
    return { memory_bytes: this.memory.buffer.byteLength,
      memory_sha256: sha256(new Uint8Array(this.memory.buffer)),
      table_length: this.table.length, table_sha256: table.digest('hex'),
      globals: Object.fromEntries(this.abi.globals.map(global =>
        [global.name, String(this.shared[global.name].value)])),
      signatures: this.signatures.length,
      signatures_sha256: sha256(JSON.stringify(this.signatures)),
      compilations: this.compilations, files: this.savedFiles.length,
      poisoned: this.poisoned };
  }

  // After a commit, a failed submission leaves this engine awaiting restore
  // instead of terminating the worker.
  calculatorRestorable() {
    return this.profile === 'Calculator-v1' && this.calculatorCommitted !== null;
  }

  publishBinary() {
    if (!this.inProcessLive || !this.staged || this.poisoned || this.pending || this.parked
      || this.shared.generation.value !== this.staged.generation
      || this.liveSourceGeneration >= 0x7fffffffffffffffn) {
      fail('staged module no longer matches live arena');
    }
    const { module, record } = this.staged;
    const original = this.liveStackSnapshot(this.memory, this.shared);
    const saved = this.snapshotArena();
    this.staged = null;
    this.liveInstalling = true;
    try {
      const instance = new WebAssembly.Instance(module, { [this.abi.module]: this.shared });
      this.liveRoot(instance, this.memory, this.shared, original);
      if (saved.table.some((entry, index) => entry !== null && this.table.get(index) !== entry)) {
        fail('empty live root replaced existing Program code');
      }
      if (this.trace.length !== saved.trace.length || this.boundRequests.length !== saved.boundRequests.length
        || this.hostRequestsTotal !== saved.hostRequestsTotal
        || this.boundRequestsTotal !== saved.boundRequestsTotal
        || this.protectedOperations !== saved.protectedOperations
        || this.ambientFallbackCalls !== saved.ambientFallbackCalls) {
        fail('host callback during live installation');
      }
      const stack = readStack(this, instance.exports);
      const metrics = Object.fromEntries(Object.entries(this.abi.metrics)
        .map(([id, name]) => [name, Number(instance.exports.metric(Number(id)))]));
      this.last = instance;
      this.compilations += 1;
      this.lastTraceStart = this.trace.length;
      this.liveSourceGeneration += 1n;
      this.liveGrant = null;
      return { schema: 'noble-core-report/v1', profile: this.profile,
        backend: 'managed-linear-memory', stage: 'wasm', outcome: 'normal',
        status: 0, submission: record.submission, generation: this.shared.generation.value,
        source_generation: this.liveSourceGeneration.toString(),
        stack, request_trace: [], guest_requests: 0, protected_operations: 0,
        candidate_prepare_requests: 0, runtime_prover_calls: 0,
        session_state: 'retained', metrics, module: record };
    } catch (error) {
      // Even when byte-exact restoration succeeds, post-install failure is
      // terminal; callers must never mistake a corrupt arena for a refusal.
      try { this.rollbackArena(saved); }
      catch (rollbackError) { this.poisoned = true; fail(`live rollback failed: ${rollbackError}`); }
      this.poisoned = true;
      fail(`live publication failed after rollback: ${error}`);
    } finally {
      this.liveInstalling = false;
    }
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
    if (this.inProcessLive || this.pending || this.compilations || this.poisoned) {
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

  install(module, record = {}, capability, generation = null) {
    if (capability !== installationCapability) fail('raw module installation is not artifact admission');
    if (this.pending || this.poisoned) fail('session is pending or poisoned');
    if ((this.profile === 'Calculator-v1') !== (generation !== null)) fail('invalid submit generation');
    const imports = WebAssembly.Module.imports(module);
    const globals = new Set(this.abi.globals.map(global => global.name));
    // The calculator's immutable per-instance generation binding.
    if (generation !== null) globals.add('submit_generation');
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
    const bound = generation === null ? this.shared : Object.assign(Object.create(null), this.shared,
      { submit_generation: new WebAssembly.Global({ value: 'i32', mutable: false }, generation) });
    const instance = new WebAssembly.Instance(module, { [this.abi.module]: bound });
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
      if (status === 0) {
        this.liveProposalDenied = false;
        this.liveExecuting = this.inProcessLive;
        try { status = runtime.submit(); }
        finally { this.liveExecuting = false; }
        if (status === 0 && this.liveProposalDenied) status = 5;
      }
      if (status === 0 && this.profile === 'Calculator-v1') {
        compiledProgramsReachable = runtime.compiled_programs_reachable() !== 0;
        programsReachable = compiledProgramsReachable || runtime.programs_reachable() !== 0;
      }
    } catch (error) {
      nativeTrap = `${error.name}: ${error.message}`;
      status = 6;
    }
    if (status !== 0) this.liveProposal = null;
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
      ...(this.inProcessLive ? { generation: this.shared.generation.value,
        source_generation: this.liveSourceGeneration.toString(),
        proposal_queued: status === 0 && this.liveProposal !== null } : {}),
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
      session_state: !this.poisoned ? 'retained'
        : this.calculatorRestorable() ? 'restore-required' : 'terminated-after-runtime-failure',
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
    if (this.poisoned) fail('cannot push into a poisoned session');
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
    if (this.poisoned) fail('cannot observe a poisoned session');
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

