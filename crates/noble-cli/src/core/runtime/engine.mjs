// A raw WebAssembly.Module is not an admission capability. Only the selected
// compiler path and the byte-exact artifact path may cross into install().
const installationCapability = Symbol('compiler-validated module');

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
const liveCodeOwnerExports = {
  signature_count: '->i32',
  code_owners: 'i32,i32->i32',
  retain_code_root: 'i32->i32',
  release_code_root: 'i32->i32',
  queued_proposal_roots: '->i32',
  code_root_state: 'i32->i32',
  save_stack_code_root: 'i32->i32',
  clear_code_observation: '->i32',
  retain_adapter_lease: '->i32',
  release_adapter_lease: '->i32',
  release_module_roots: '->i32',
  invalidate_code_span: 'i32,i32->i32',
};
const liveOwnerExportTypes = { ...liveExportTypes, ...liveCodeOwnerExports };
const bridgeExportTypes = { ...liveOwnerExportTypes,
  core_empty: 'i32->', core_quote: 'i32->',
  core_compose: 'i32->', core_restore: 'i32->' };
delete bridgeExportTypes.submit;
delete bridgeExportTypes.release_module_roots;
const liveImportTypes = {
  live_propose: 'i64,i64,i32->i32',
  live_generation: '->i64',
};
const declaredLiveImportTypes = {
  test_emit_bound: 'i32,i32,i32->i32',
  test_clock_bound: 'i32->i32,i64',
  test_abort: '->i32',
};

function checkLiveBytes(bytes, abi, declaredLive, ownerAware, bridge = false) {
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
  const hostImports = bridge ? {} : declaredLive ? declaredLiveImportTypes : liveImportTypes;
  const expectedExports = bridge ? bridgeExportTypes
    : ownerAware ? liveOwnerExportTypes : liveExportTypes;
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
          if (!Object.hasOwn(hostImports, name) || type !== hostImports[name]) {
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
      if (n !== Object.keys(expectedExports).length + 1
        || Object.keys(abi.exports).length !== Object.keys(liveExportTypes).length) {
        fail('incompatible Wasm export count');
      }
      for (let i = 0; i < n; i += 1) {
        const name = text(), kind = byte(), index = uint();
        if (seenExports.has(name)) fail('duplicate Wasm export');
        seenExports.add(name);
        if (name === 'memory') {
          if (kind !== 2 || index !== 0) fail('incompatible Wasm memory export');
        } else if (!Object.hasOwn(expectedExports, name) || kind !== 0
          || functionTypes[index] !== expectedExports[name]) fail('incompatible Wasm function export');
        exports.push({ name, kind: kind === 0 ? 'function' : 'memory' });
      }
    } else if (id === 4 || id === 5) {
      fail('module must import its only table and memory');
    } else if (id === 9) {
      const n = count();
      if (n > 16384 || bridge && n !== 0) fail('unexpected Wasm element segments');
      for (let i = 0; i < n; i += 1) {
        const flags = uint();
        if (flags !== 1 && flags !== 3 || byte() !== 0) fail('active or unsupported Wasm element segment');
        const length = count();
        if (length > 16384) fail('oversized Wasm element segment');
        for (let j = 0; j < length; j += 1) uint();
      }
    } else if (id === 11) {
      const n = count();
      if (bridge && n !== 0) fail('runtime bridge must not install data');
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
    || seenExports.size !== Object.keys(expectedExports).length + 1) fail('incomplete Wasm arena ABI');
  return { imports, exports };
}

function checkedSignatureCatalogue(bytes) {
  if (!Buffer.isBuffer(bytes) || bytes.length < 4 || bytes.length > MAX_FRAME) {
    fail('invalid checked signature catalogue');
  }
  const count = bytes.readUInt32LE(0);
  if (count > 4096) fail('checked signature catalogue exceeds Core identities');
  const rows = [];
  let at = 4, end = 262144;
  for (let id = 0; id < count; id += 1) {
    if (bytes.length - at < 8) fail('truncated checked signature catalogue');
    const address = bytes.readUInt32LE(at), length = bytes.readUInt32LE(at + 4);
    at += 8;
    if (address < end || length > bytes.length - at
      || address + length > 1048576) fail('invalid checked signature descriptor');
    rows.push({ address, length, offset: at });
    at += length;
    end = address + length;
  }
  if (at !== bytes.length) fail('trailing checked signature catalogue bytes');
  return { bytes: Buffer.from(bytes), rows, sha256: sha256(bytes) };
}

// Node's selected JSON.parse reviver exposes the original numeric token.
// Preserve it before Number rounding can turn one u64 module owner into another.
function parseDeclaredLiveState(bytes) {
  return JSON.parse(utf8.decode(bytes), (key, value, context) => {
    if (key !== 'identity' && key !== 'generation' && key !== 'expected_generation') return value;
    if (typeof value !== 'number' || !/^(0|[1-9][0-9]*)$/.test(context.source)) {
      fail('invalid declared live integer');
    }
    return BigInt(context.source);
  });
}

function declaredLiveInteger(value, maximum, label) {
  if (typeof value === 'number') {
    if (!Number.isSafeInteger(value)) fail(`invalid ${label}`);
    value = BigInt(value);
  }
  if (typeof value !== 'bigint' || value < 0n || value > maximum) fail(`invalid ${label}`);
  return value;
}

function declaredLiveObject(value, fields, label) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)
    || Object.keys(value).length !== fields.length
    || fields.some(field => !Object.hasOwn(value, field))) fail(`invalid ${label}`);
}

function sourceObject(value, fields, label) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)
    || Object.keys(value).length !== fields.length
    || fields.some(field => !Object.hasOwn(value, field))) fail(`invalid ${label}`);
}

function sourceNumber(value, maximum, label) {
  if (!Number.isSafeInteger(value) || value < 0 || value > maximum) fail(`invalid ${label}`);
  return value;
}

function sourceU64(value, label) {
  if (typeof value !== 'string' || !/^(0|[1-9][0-9]*)$/.test(value)
    || value.length > 20 || BigInt(value) > 0xffffffffffffffffn) {
    fail(`invalid ${label}`);
  }
  return value;
}

function sourceI64(value, label) {
  if (typeof value !== 'string' || !/^(0|-[1-9][0-9]*|[1-9][0-9]*)$/.test(value)
    || value.length > 20 || BigInt.asIntN(64, BigInt(value)) !== BigInt(value)) {
    fail(`invalid ${label}`);
  }
  return value;
}

function sourceSpan(span, length, label, nonempty = false) {
  sourceObject(span, ['start', 'end'], label);
  sourceNumber(span.start, length, label);
  sourceNumber(span.end, length, label);
  if (span.start > span.end || nonempty && span.start === span.end) fail(`invalid ${label}`);
  return span;
}

function sourceCodeSpan(span, tableLength) {
  sourceObject(span, ['start', 'length', 'generation'], 'source code span');
  sourceNumber(span.start, tableLength, 'source code start');
  sourceNumber(span.length, tableLength - span.start, 'source code length');
  sourceNumber(span.generation, 0xffffffff, 'source code generation');
  if (span.start < 4 || !span.generation) fail('invalid source code span');
  return span;
}

function sourceBytes(base64) {
  if (typeof base64 !== 'string' || base64.length > 87384
    || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(base64)) {
    fail('invalid original signed source bytes');
  }
  const bytes = Buffer.from(base64, 'base64');
  if (bytes.length > 65536 || bytes.toString('base64') !== base64) {
    fail('noncanonical original signed source bytes');
  }
  return bytes;
}

export class CoreEngine {
  constructor(selection, abi, { optimized = false, artifacts = null, bindings = [],
    declared_modules = false, declared_extension = null, in_process_live = false,
    text_byte_cursor = false, signed_self_recursion = false } = {}) {
    this.selection = selection;
    this.abi = abi;
    this.inProcessLive = in_process_live === true;
    configureDeclaredAbi(this, abi, declared_modules, declared_extension, bindings,
      text_byte_cursor, signed_self_recursion);
    this.declaredLive = this.inProcessLive && declared_modules === true;
    this.liveCodeOwners = this.inProcessLive;
    this.externalCodeRoots = new Set();
    this.sourceEpoch = this.profile === 'Signed-Declared-Text-v1';
    if (this.sourceEpoch && this.inProcessLive) {
      fail('signed source epoch cannot own the live Core bridge');
    }
    this.sourceStoreId = this.sourceEpoch ? crypto.randomUUID() : null;
    this.sourceExternalHandles = new Map();
    this.sourceIngressHandles = new Map();
    this.sourceObservations = new Map();
    this.sourceCurrentObservation = null;
    this.sourceNextObservation = 1;
    this.sourceEpochGeneration = 0;
    this.lastSourceObservation = null;
    this.sourceModules = [];
    this.sourceCoreEntries = null;
    this.sourceTableEntries = [];
    this.sourceCodeOwners = new Map();
    this.sourcePrograms = [];
    this.sourceSignatures = new Map();
    this.sourceLastGeneration = 0n;
    this.liveCodeSpans = new Map();
    this.liveModuleOwner = null;
    this.liveModuleNamespaceGeneration = null;
    this.supersededLiveModules = [];
    this.optimized = optimized;
    initializeHostState(this, bindings);
    this.declaredLiveState = { generation: 0n, roots: [], aliases: [], activeSlots: new Set() };
    this.stagedDeclared = null;
    this.pending = null;
    this.last = null;
    this.guestInstalled = false;
    this.activeGuest = null;
    this.unregisteredGuest = null;
    this.runtimeBridge = null;
    this.coreBridge = null;
    this.nextGuestOwner = 1;
    this.guestOwners = new WeakMap();
    this.releasedModuleOwners = new Set();
    this.liveModuleOwnerId = null;
    this.parked = null;
    this.poisoned = false;
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
    this.tools = this.checkTools();
    this.owned = artifacts === null;
    this.directory = this.owned ? fs.mkdtempSync(path.join(os.tmpdir(), 'noble-core-')) : path.join(artifacts, 'engine');
    if (!this.owned) fs.mkdirSync(this.directory);
    this.memory = new WebAssembly.Memory(abi.memory);
    this.table = new WebAssembly.Table(abi.table);
    this.shared = Object.create(null);
    installSharedGlobals(this, abi);
    if (this.sourceEpoch) {
      // The source epoch is not the submission generation. Old universal
      // Program descriptors must remain valid while unrelated modules append.
      this.shared.source_epoch_generation =
        new WebAssembly.Global({ value: 'i32', mutable: true }, 0);
    }
    installTestHosts(this, declared_modules);
    if (this.inProcessLive) {
      if (!this.declaredLive) {
        this.shared.live_propose = (owner, expected, program) => this.livePropose(owner, expected, program);
        this.shared.live_generation = () => this.liveGeneration();
      }
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
    if (this.inProcessLive) fail('external Wasm tools are unavailable in live mode');
    const result = spawnSync(this.selection.tools[name].path, args, {
      env: {}, encoding: 'utf8', timeout: 60000, killSignal: 'SIGKILL', maxBuffer: MAX_FRAME,
    });
    this.save(`${label}.json`, { binary: this.selection.tools[name].path, args,
      status: result.status, signal: result.signal, stdout: result.stdout, stderr: result.stderr,
      error: result.error ? String(result.error) : null });
    if (result.error || result.status !== 0) fail(`${label} failed: ${result.error ?? result.stderr}`);
  }

  sourceCheckPinned(allowNewEntriesThrough = this.sourceTableEntries.length + 4) {
    if (!this.sourceEpoch || this.shared.source_epoch_generation.value !== this.sourceEpochGeneration
      || this.sourceCoreEntries !== null
        && this.sourceCoreEntries.some((entry, index) => this.table.get(index) !== entry)
      || this.sourceModules.length !== 0 && this.sourceCoreEntries === null) {
      fail('signed source epoch or Core table root changed');
    }
    for (let index = 0; index < this.sourceTableEntries.length; index += 1) {
      if (this.table.get(index + 4) !== this.sourceTableEntries[index]) {
        fail('accepted old source code entry changed');
      }
    }
    for (let index = allowNewEntriesThrough; index < this.table.length; index += 1) {
      if (this.table.get(index) !== null) fail('unowned source code table entry');
    }
    for (const owner of this.sourceCodeOwners.values()) {
      if (!this.sourceModules.includes(owner.instance)) fail('old source owner module was unpinned');
      for (const entry of owner.entries) {
        if (this.table.get(entry.index) !== entry.function) fail('old source owner code was replaced');
      }
    }
    const view = new DataView(this.memory.buffer);
    for (const signature of this.sourceSignatures.values()) {
      const runtime = signature.instance.exports;
      if (!this.sourceModules.includes(signature.instance)
        || runtime.signature_address(signature.id) !== signature.address
        || runtime.signature_length(signature.id) !== signature.bytes.length
        || !signature.bytes.equals(Buffer.from(this.memory.buffer,
          signature.address, signature.bytes.length))) {
        fail('old source full-signature descriptor changed');
      }
    }
    for (const installed of this.sourcePrograms) {
      if (!this.sourceModules.includes(installed.instance)
        || !this.sourceProgramMatches(installed, installed.instance.exports, view)) {
        fail('accepted old source descriptor, slot or recipe changed');
      }
    }
    const bySlot = new Map(this.sourcePrograms.map(program =>
      [program.program.slot, program]));
    for (const installed of this.sourcePrograms) {
      for (const site of installed.program.sites) {
        if (site.action === 3) continue;
        const target = bySlot.get(site.target_slot);
        if (!target || target.handle !== view.getUint32(site.target_slot, true)
          || target.program.entry !== site.target_entry
          || target.program.input_signature !== site.input_signature
          || target.program.output_signature !== site.output_signature
          || target.program.effect_mask !== site.effect_or_witness
          || target.owner.definition_identity !== site.target_identity
          || target.owner.definition !== site.target_definition) {
          fail('accepted source descriptor target changed');
        }
      }
    }
  }

  sourceProgramMatches(installed, runtime, view) {
    const { program, owner, handle, descriptor, atomHandles, siteBytes } = installed;
    if (program.slot < 262144 || program.slot > 1048576 - 4
      || program.records < 262144
      || program.records + siteBytes.length > 1048576) {
      return false;
    }
    if (runtime.module_program_handle(program.program_index) !== handle
      || runtime.cell_kind(handle) !== 4 || runtime.cell_a(handle) !== descriptor
      || runtime.cell_b(handle) !== 0
      || runtime.cell_x(handle) !== program.entry
      || runtime.cell_y(handle) !== program.input_signature
      || runtime.cell_z(handle) !== program.output_signature
      || runtime.cell_w(handle) !== program.recipe_depth
      || runtime.cell_n(handle) !== program.recipe_leaves
      || runtime.cell_payload(handle) !== 0n
      || runtime.cell_kind(descriptor) !== 9
      || runtime.cell_a(descriptor) !== 0 || runtime.cell_b(descriptor) !== 0
      || runtime.cell_c(descriptor) !== 0
      || runtime.cell_x(descriptor) !== program.records
      || runtime.cell_y(descriptor) !== program.sites.length
      || runtime.cell_z(descriptor) !== this.sourceEpochGeneration
      || runtime.cell_w(descriptor) !== program.slot
      || runtime.cell_n(descriptor) !== owner.definition
      || BigInt.asUintN(64, runtime.cell_payload(descriptor))
        !== BigInt(owner.definition_identity)
      || view.getUint32(program.slot, true) !== handle
      || !siteBytes.equals(Buffer.from(this.memory.buffer,
        program.records, siteBytes.length))) return false;
    const recipe = runtime.cell_c(handle);
    const pending = recipe ? [recipe] : [];
    let leaf = 0, traversed = 0;
    while (pending.length) {
      const cell = pending.pop();
      if (!Number.isInteger(cell) || cell < 1 || cell >= handle
        || ++traversed > program.atoms.length * 2 + 1) return false;
      const kind = runtime.cell_kind(cell);
      if (kind === 9) {
        const a = runtime.cell_a(cell), b = runtime.cell_b(cell);
        if (!a || !b || a >= cell || b >= cell) return false;
        pending.push(b, a);
        continue;
      }
      const atom = program.atoms[leaf], expected = atomHandles[leaf++];
      if (!atom || cell !== expected || kind !== 8
        || runtime.cell_x(cell) !== atom.tag
        || runtime.cell_payload(cell) !== BigInt(atom.payload)
        || runtime.cell_y(cell) !== atom.input
        || runtime.cell_z(cell) !== atom.output
        || runtime.cell_w(cell) !== atom.effects
        || runtime.cell_n(cell) !== atom.ordinal
        || runtime.cell_b(cell) !== 0 || runtime.cell_c(cell) !== 0
        || atom.child_program !== null
          && !(installed.handles.get(atom.child_program) < cell)
        || runtime.cell_a(cell) !== (atom.child_program === null ? 0
          : installed.handles.get(atom.child_program))) return false;
    }
    return leaf === program.atoms.length;
  }

  sourcePreflight(receipt) {
    sourceObject(receipt, ['schema', 'source_epoch', 'source_generation',
      'selected_use', 'wrapper_program_index', 'wrapper_entry', 'code_span',
      'owners', 'programs'], 'universal compiler receipt');
    if (receipt.schema !== 'noble-source-universal-install/v1'
      || receipt.source_epoch !== this.sourceEpochGeneration
      || !Array.isArray(receipt.owners) || !receipt.owners.length
      || !Array.isArray(receipt.programs) || !receipt.programs.length
      || receipt.programs.length >= 4096
      || receipt.owners.length > receipt.programs.length
      || !this.sourceEpoch || this.pending || this.poisoned) {
      fail('stale or incomplete source universal receipt');
    }
    sourceU64(receipt.source_generation, 'source generation');
    if (BigInt(receipt.source_generation) < this.sourceLastGeneration) {
      fail('source namespace generation moved backward');
    }
    const span = sourceCodeSpan(receipt.code_span, this.table.length);
    const previousEnd = 4 + this.sourceTableEntries.length;
    if (span.start !== previousEnd || !span.length
      || span.generation !== this.shared.generation.value + 1
      || receipt.wrapper_entry !== span.start + span.length - 1
      || receipt.source_epoch > 0x7fffffff) {
      fail('universal code span does not append to accepted source epoch');
    }
    this.sourceCheckPinned();
    if (this.sourceCoreEntries === null
      && Array.from({ length: 4 }, (_, index) => this.table.get(index))
        .some(entry => entry !== null)) {
      fail('first universal source cannot inherit an existing Core table root');
    }
    const owners = receipt.owners.map((row, ownerIndex) => {
      sourceObject(row, ['owner_index', 'definition', 'definition_slot',
        'definition_identity', 'module_identity', 'module_version',
        'definition_ordinal', 'original_source_base64', 'source_sha256',
        'nodes', 'stamp', 'code_span', 'body_entry', 'quotations',
        'newly_compiled'], 'universal owner');
      if (row.owner_index !== ownerIndex || !Array.isArray(row.nodes)
        || row.nodes.length > 4096 || !Array.isArray(row.quotations)
        || row.quotations.length > row.nodes.length
        || typeof row.newly_compiled !== 'boolean') fail('invalid universal owner graph');
      const source = sourceBytes(row.original_source_base64);
      if (!source.length || row.source_sha256 !== sha256(source)) {
        fail('universal owner source digest differs from retained whole module');
      }
      for (const field of ['definition', 'definition_slot', 'module_version',
        'definition_ordinal', 'body_entry']) sourceNumber(row[field], 0xffffffff, field);
      for (const field of ['definition_identity', 'module_identity']) {
        sourceU64(row[field], field);
      }
      sourceObject(row.stamp, ['source_tag', 'grant_epoch', 'generation'], 'source stamp');
      if (typeof row.stamp.source_tag !== 'string'
        || !/^[a-f0-9]{64}$/.test(row.stamp.source_tag)) fail('invalid signed source tag');
      sourceU64(row.stamp.grant_epoch, 'source grant epoch');
      sourceU64(row.stamp.generation, 'source stamp generation');
      if (BigInt(row.stamp.generation) > BigInt(receipt.source_generation)) {
        fail('original source owner comes from a future namespace generation');
      }
      sourceCodeSpan(row.code_span, this.table.length);
      for (const [node, value] of row.nodes.entries()) {
        sourceObject(value, ['node', 'span'], 'source node origin');
        sourceSpan(value.span, source.length, 'source node span');
        if (value.node !== node) fail('source node association differs from original module');
      }
      const quotes = new Map();
      for (const quote of row.quotations) {
        sourceObject(quote, ['node', 'entry'], 'source quotation entry');
        sourceNumber(quote.node, row.nodes.length - 1, 'quotation source node');
        sourceNumber(quote.entry, this.table.length - 1, 'quotation table entry');
        if (quotes.has(quote.node)
          || quote.entry < row.code_span.start
          || quote.entry >= row.code_span.start + row.code_span.length) {
          fail('ambiguous source quotation code identity');
        }
        quotes.set(quote.node, quote.entry);
      }
      if (row.body_entry < row.code_span.start
        || row.body_entry >= row.code_span.start + row.code_span.length) {
        fail('unbound original owner body entry');
      }
      const previous = this.sourceCodeOwners.get(row.definition);
      if (row.newly_compiled) {
        if (previous || row.code_span.generation !== span.generation
          || row.code_span.start < span.start
          || row.code_span.start + row.code_span.length > receipt.wrapper_entry) {
          fail('new universal owner replaces previously installed Definition');
        }
      } else if (!previous || previous.source_epoch !== this.sourceEpochGeneration
        || previous.definition_slot !== row.definition_slot
        || previous.definition_identity !== row.definition_identity
        || previous.module_identity !== row.module_identity
        || previous.module_version !== row.module_version
        || previous.definition_ordinal !== row.definition_ordinal
        || previous.source_sha256 !== row.source_sha256
        || !previous.source.equals(source)
        || previous.source_tag !== row.stamp.source_tag
        || previous.grant_epoch !== row.stamp.grant_epoch
        || previous.stamp_generation !== row.stamp.generation
        || previous.nodes.length !== row.nodes.length
        || previous.nodes.some((node, index) =>
          node.span.start !== row.nodes[index].span.start
          || node.span.end !== row.nodes[index].span.end)
        || previous.code_span.start !== row.code_span.start
        || previous.code_span.length !== row.code_span.length
        || previous.code_span.generation !== row.code_span.generation
        || previous.body_entry !== row.body_entry
        || previous.quotations.size !== quotes.size
        || [...quotes].some(([node, entry]) => previous.quotations.get(node) !== entry)) {
        fail('original owner source, grant, dependency or old code identity changed');
      }
      return { ...row, source, quotes, previous };
    });
    if (new Set(owners.map(owner => owner.definition)).size !== owners.length) {
      fail('duplicate source Definition owner');
    }
    const programs = receipt.programs.map((row, programIndex) => {
      sourceObject(row, ['use_index', 'owner_index', 'program_index',
        'quotation_node', 'quotation_source_span', 'entry', 'slot', 'records',
        'input_signature', 'output_signature', 'effect_mask',
        'recipe_depth', 'recipe_leaves', 'atoms', 'sites'],
      'concrete source Program');
      sourceNumber(row.owner_index, owners.length - 1, 'concrete source owner');
      const owner = owners[row.owner_index];
      if (!owner || row.program_index !== programIndex
        || !Array.isArray(row.atoms) || !Array.isArray(row.sites)
        || row.atoms.length > 4096 || row.sites.length > row.atoms.length
        || row.effect_mask !== 0) fail('incomplete concrete Program ABI');
      for (const field of ['use_index', 'entry', 'slot', 'records',
        'input_signature', 'output_signature', 'recipe_depth', 'recipe_leaves']) {
        sourceNumber(row[field], field === 'slot' || field === 'records'
          ? 1048576 : field === 'use_index' ? 4095
            : field === 'recipe_depth' || field === 'recipe_leaves' ? 4096 : 16383, field);
      }
      if (row.quotation_node === null) {
        if (row.quotation_source_span !== null || row.entry !== owner.body_entry) {
          fail('concrete body does not retain original owner entry');
        }
      } else {
        sourceNumber(row.quotation_node, owner.nodes.length - 1, 'quoted source node');
        const node = owner.nodes[row.quotation_node];
        sourceSpan(row.quotation_source_span, owner.source.length, 'original quote span', true);
        if (!node || node.span.start !== row.quotation_source_span.start
          || node.span.end !== row.quotation_source_span.end
          || row.entry !== owner.quotes.get(row.quotation_node)) {
          fail('quotation source and original code entry differ');
        }
      }
      const siteAtoms = new Map();
      const quoteAtoms = new Map();
      for (const [index, atom] of row.atoms.entries()) {
        sourceObject(atom, ['source_node', 'tag', 'payload', 'child_program',
          'text', 'input', 'output', 'effects', 'ordinal'], 'checked recipe atom');
        sourceNumber(atom.source_node, owner.nodes.length - 1, 'atom source node');
        sourceNumber(atom.tag, 34, 'atom kind');
        sourceI64(atom.payload, 'atom payload');
        for (const field of ['input', 'output', 'effects', 'ordinal']) {
          sourceNumber(atom[field], field === 'ordinal' ? 0xffffffff : 4095, field);
        }
        const word = atom.tag === 2 ? Number(atom.payload) : 0;
        if (atom.effects !== 0 || atom.ordinal !== 0
          || atom.tag === 4 && atom.payload !== '0' && atom.payload !== '1'
          || (atom.tag === 3 || atom.tag === 5) && atom.payload !== '0'
          || atom.tag === 2 && (!Number.isInteger(word) || word < 0
            || word > 21 && word !== 26 && word !== 27)) {
          fail('source recipe contains a non-pure or malformed atom');
        }
        if (atom.text !== null || atom.tag !== 1 && atom.tag !== 2
          && atom.tag !== 3 && atom.tag !== 4 && atom.tag !== 5 && atom.tag !== 15
          || atom.child_program !== null
            && (!Number.isSafeInteger(atom.child_program) || atom.child_program < 0
              || atom.child_program >= receipt.programs.length)
          || atom.tag === 3 !== (atom.child_program !== null)) {
          fail(`unsupported source recipe atom ${index}`);
        }
        if (atom.tag === 3 || atom.tag === 15) {
          const key = `${atom.source_node}:${atom.tag}`;
          if (!siteAtoms.has(key)) siteAtoms.set(key, []);
          siteAtoms.get(key).push(atom);
        }
        if (atom.tag === 2 && atom.payload === '8') {
          if (!quoteAtoms.has(atom.source_node)) quoteAtoms.set(atom.source_node, []);
          quoteAtoms.get(atom.source_node).push(atom);
        }
      }
      for (const site of row.sites) {
        sourceObject(site, ['node', 'source_span', 'action', 'target_use',
          'target_program_index', 'target_slot', 'input_signature',
          'output_signature', 'effect_or_witness', 'target_identity',
          'target_definition', 'target_entry'], 'checked lexical site');
        sourceNumber(site.node, owner.nodes.length - 1, 'site source node');
        sourceSpan(site.source_span, owner.source.length, 'lexical site span', true);
        const origin = owner.nodes[site.node];
        if (origin.span.start !== site.source_span.start
          || origin.span.end !== site.source_span.end
          || ![1, 2, 3].includes(site.action)) fail('site is not original lexical source');
        for (const field of ['target_slot', 'input_signature', 'output_signature',
          'effect_or_witness', 'target_definition', 'target_entry']) {
          sourceNumber(site[field], field === 'target_slot' ? 1048576
            : field === 'target_definition' || field === 'target_entry'
              ? 0xffffffff : 4095, field);
        }
        sourceU64(site.target_identity, 'target owner identity');
        if (site.action === 3) {
          if (site.target_use !== null || site.target_program_index !== null
            || site.target_slot !== 0 || site.target_entry !== 0
            || site.target_identity !== owner.definition_identity
            || site.target_definition !== owner.definition) {
            fail('invalid typed quotation witness or original source owner');
          }
        } else {
          sourceNumber(site.target_use, 4095, 'target concrete use');
          sourceNumber(site.target_program_index, receipt.programs.length - 1,
            'target Program index');
          const target = receipt.programs[site.target_program_index];
          const targetOwner = target && owners[target.owner_index];
          if (!target || !targetOwner || target.use_index !== site.target_use
            || target.slot !== site.target_slot || target.entry !== site.target_entry
            || target.input_signature !== site.input_signature
            || target.output_signature !== site.output_signature
            || target.effect_mask !== site.effect_or_witness
            || targetOwner.definition_identity !== site.target_identity
            || targetOwner.definition !== site.target_definition
            || site.action === 1 && (site.target_use !== row.use_index
              || target.owner_index !== row.owner_index
              || target.quotation_node !== site.node)
            || site.action === 2 && target.quotation_node !== null) {
            fail('site target differs from the checked concrete Program');
          }
        }
        const key = `${site.node}:${site.action === 1 ? 3 : 15}`;
        const atom = site.action === 3
          ? quoteAtoms.get(site.node)?.pop()
          : siteAtoms.get(key)?.pop();
        if (!atom || site.action === 1 && (atom.tag !== 3
            || atom.child_program !== site.target_program_index)
          || site.action === 2 && (atom.tag !== 15
            || BigInt.asUintN(64, BigInt(atom.payload)) !== BigInt(site.target_identity))
          || site.action === 3 && (atom.tag !== 2 || atom.payload !== '8')) {
          fail('lexical site has no matching source recipe atom');
        }
      }
      let unbound = false;
      for (const pending of siteAtoms.values()) {
        if (pending.length) unbound = true;
      }
      for (const pending of quoteAtoms.values()) {
        if (pending.length) unbound = true;
      }
      if (unbound) {
        fail('dynamic Program recipe has no checked lexical descriptor');
      }
      return row;
    });
    const roots = new Map();
    const usePrograms = new Map();
    const ownerPrograms = new Map();
    const firstOwnerRoot = new Map();
    let previousUse = -1;
    for (const program of programs) {
      if (program.use_index < previousUse || program.use_index > previousUse + 1
        || previousUse === -1 && program.use_index !== 0
        || program.use_index !== previousUse
          && program.quotation_node !== null) {
        fail('concrete source uses are not in compiler order');
      }
      previousUse = program.use_index;
      if (!usePrograms.has(program.use_index)) usePrograms.set(program.use_index, []);
      usePrograms.get(program.use_index).push(program);
      if (!ownerPrograms.has(program.owner_index)) ownerPrograms.set(program.owner_index, []);
      ownerPrograms.get(program.owner_index).push(program);
      if (program.quotation_node === null) {
        if (roots.has(program.use_index)) fail('ambiguous concrete owner use');
        roots.set(program.use_index, program);
        if (!firstOwnerRoot.has(program.owner_index)) {
          firstOwnerRoot.set(program.owner_index, program);
        }
      }
    }
    if (!roots.has(receipt.selected_use)
      || [...usePrograms].some(([index, entries]) => {
        const root = roots.get(index);
        return !root || entries[0] !== root
          || entries.slice(1).some((entry, offset) => entry.quotation_node === null
            || offset > 0 && entry.quotation_node
              <= entries[offset].quotation_node);
      })
      || new Set(programs.map(program => program.slot)).size !== programs.length
      || receipt.wrapper_program_index !== programs.length) {
      fail('incomplete concrete source-use graph');
    }
    for (const program of programs) {
      const root = roots.get(program.use_index);
      if (!root || root.owner_index !== program.owner_index) {
        fail('quotation is not owned by its concrete source use');
      }
    }
    const depths = new Map();
    const visiting = new Set();
    const countRecipe = program => {
      if (depths.has(program.program_index)) return depths.get(program.program_index);
      if (visiting.has(program.program_index)) fail('cyclic immutable Program child recipe');
      visiting.add(program.program_index);
      let depth = 1, leaves = program.atoms.length;
      for (const atom of program.atoms) {
        if (atom.child_program === null) continue;
        const child = programs[atom.child_program];
        if (!child || child.use_index !== program.use_index
          || child.owner_index !== program.owner_index
          || child.quotation_node !== atom.source_node) {
          fail('concrete quotation child is not its lexical source owner');
        }
        const counted = countRecipe(child);
        depth = Math.max(depth, counted.depth + 1);
        leaves += counted.leaves;
      }
      visiting.delete(program.program_index);
      if (depth !== program.recipe_depth || leaves !== program.recipe_leaves
        || leaves > 4096 || depth > 4096) {
        fail('source Program has an incorrect checked recipe depth or leaves');
      }
      const result = { depth, leaves };
      depths.set(program.program_index, result);
      return result;
    };
    for (const program of programs) countRecipe(program);
    for (const owner of owners) {
      const first = firstOwnerRoot.get(owner.owner_index);
      if (!first) fail('unreferenced universal owner code');
      const quotes = usePrograms.get(first.use_index).slice(1);
      const quoteByNode = new Map(quotes.map(quote => [quote.quotation_node, quote]));
      if (quotes.length !== owner.quotations.length
        || quotes.some((program, index) =>
          program.quotation_node !== owner.quotations[index].node)) {
        fail('original owner quotation tree changed');
      }
      let entry = owner.code_span.start;
      for (const program of [first, ...quotes]) {
        if (program.entry !== entry) {
          fail('original owner body and quote code entry offset changed');
        }
        entry += Math.max(program.atoms.length, 1);
      }
      if (entry !== owner.code_span.start + owner.code_span.length) {
        fail('original owner code span does not cover every lexical instruction');
      }
      for (const program of ownerPrograms.get(owner.owner_index)) {
        const original = program.quotation_node === null ? first
          : quoteByNode.get(program.quotation_node);
        if (!original || original.atoms.length !== program.atoms.length) {
          fail('concrete instance changes old symbolic code layout');
        }
      }
    }
    const ranges = [];
    const addRange = (start, length) => {
      if (start < 262144 || start % 4 || length < 0
        || start + length > 1048576) {
        fail('unbounded source descriptor address');
      }
      if (length) ranges.push([start, start + length]);
    };
    for (const old of this.sourcePrograms) {
      addRange(old.program.slot, 4);
      if (old.siteBytes.length) addRange(old.program.records, old.siteBytes.length);
    }
    for (const signature of this.sourceSignatures.values()) {
      ranges.push([signature.address, signature.address + signature.bytes.length]);
    }
    const memory = new DataView(this.memory.buffer);
    for (const program of programs) {
      addRange(program.slot, 4);
      if (memory.getUint32(program.slot, true) !== 0) {
        fail('universal descriptor slot was already initialized');
      }
      if (program.sites.length) addRange(program.records, program.sites.length * 40);
      else if (program.records < 262144 || program.records > 1048576
        || program.records % 4) fail('unbounded empty source descriptor records');
    }
    ranges.sort((left, right) => left[0] - right[0]);
    for (let index = 1; index < ranges.length; index += 1) {
      if (ranges[index][0] < ranges[index - 1][1]) {
        fail('old or pending universal descriptor ranges overlap');
      }
    }
    const newOwners = owners.filter(owner => owner.newly_compiled)
      .sort((left, right) => left.code_span.start - right.code_span.start);
    let nextEntry = span.start;
    for (const owner of newOwners) {
      if (owner.code_span.start !== nextEntry) fail('unowned gap in new source code');
      nextEntry += owner.code_span.length;
    }
    if (nextEntry !== receipt.wrapper_entry) fail('wrapper code does not follow checked owners');
    return { receipt, span, owners, programs, previousEnd, reservedRanges: ranges };
  }

  sourcePostflight(installation, instance) {
    const { receipt, owners, programs, span, previousEnd, reservedRanges } = installation;
    const runtime = instance.exports;
    if (this.poisoned || this.shared.source_epoch_generation.value !== receipt.source_epoch
      || this.shared.generation.value !== span.generation
      || runtime.module_program_count() !== programs.length + 1) {
      fail('universal source publication has an incomplete module root');
    }
    this.sourceCheckPinned(span.start + span.length);
    const core = this.sourceCoreEntries ?? Array.from({ length: 4 },
      (_, index) => this.table.get(index));
    if (core.some((entry, index) => typeof entry !== 'function'
      || this.table.get(index) !== entry)) fail('universal source replaced Core bridge');
    for (let index = previousEnd; index < span.start + span.length; index += 1) {
      if (typeof this.table.get(index) !== 'function') {
        fail('universal source code entry was not installed');
      }
    }
    for (let index = span.start + span.length; index < this.table.length; index += 1) {
      if (this.table.get(index) !== null) fail('unowned universal source code entry');
    }
    const handles = new Map();
    const seen = new Set();
    for (let index = 0; index <= programs.length; index += 1) {
      const handle = runtime.module_program_handle(index);
      if (!Number.isSafeInteger(handle) || handle <= 0
        || handle > this.shared.heap_cursor.value || seen.has(handle)
        || runtime.cell_kind(handle) !== 4) fail('missing concrete Program owner root');
      handles.set(index, handle);
      seen.add(handle);
    }
    const wrapper = handles.get(receipt.wrapper_program_index);
    const selected = programs.find(program => program.use_index === receipt.selected_use
      && program.quotation_node === null);
    const wrapperAtom = runtime.cell_c(wrapper);
    if (runtime.cell_x(wrapper) !== receipt.wrapper_entry
      || runtime.cell_w(wrapper) !== 1 || runtime.cell_n(wrapper) !== 1
      || runtime.cell_payload(wrapper) !== 0n || runtime.cell_a(wrapper) !== 0
      || runtime.cell_b(wrapper) !== 0
      || runtime.cell_y(wrapper) !== selected.input_signature
      || runtime.cell_z(wrapper) !== selected.output_signature
      || !Number.isSafeInteger(wrapperAtom) || wrapperAtom < 1 || wrapperAtom >= wrapper
      || runtime.cell_kind(wrapperAtom) !== 8
      || runtime.cell_x(wrapperAtom) !== 15
      || runtime.cell_a(wrapperAtom) !== 0 || runtime.cell_b(wrapperAtom) !== 0
      || runtime.cell_c(wrapperAtom) !== 0
      || runtime.cell_y(wrapperAtom) !== selected.input_signature
      || runtime.cell_z(wrapperAtom) !== selected.output_signature
      || runtime.cell_w(wrapperAtom) !== 0 || runtime.cell_n(wrapperAtom) !== 0
      || BigInt.asUintN(64, runtime.cell_payload(wrapperAtom))
        !== BigInt(owners[selected.owner_index].definition_identity)) {
      fail('selected wrapper does not invoke its exact source owner');
    }
    const view = new DataView(this.memory.buffer);
    const newSignatures = new Map();
    const checkSignature = id => {
      if (newSignatures.has(id)) return;
      sourceNumber(id, 4095, 'source full signature');
      const address = runtime.signature_address(id), length = runtime.signature_length(id);
      if (!Number.isSafeInteger(address) || address < 262144
        || !Number.isSafeInteger(length) || length <= 0
        || address + length > 1048576
        || length > 16384 || !readSignature(this, runtime, id)) {
        fail('missing exact concrete full-signature descriptor');
      }
      // Keep a private copy: a Buffer view over Wasm memory would mutate
      // along with the descriptor and make later identity checks tautological.
      const literal = Buffer.from(new Uint8Array(this.memory.buffer, address, length));
      const installed = this.sourceSignatures.get(id);
      if (installed && (installed.address !== address
        || !installed.bytes.equals(literal))) {
        fail('old full-signature identity silently changed');
      }
      if (!installed) {
        if (reservedRanges.some(([start, end]) =>
          address < end && start < address + length)) {
          fail('source signature overlaps an installed descriptor or slot');
        }
        reservedRanges.push([address, address + length]);
      }
      newSignatures.set(id, { id, address, bytes: literal, instance });
    };
    const installedPrograms = [];
    for (const program of programs) {
      const owner = owners[program.owner_index], handle = handles.get(program.program_index);
      const descriptor = runtime.cell_a(handle);
      if (!Number.isSafeInteger(descriptor) || descriptor < 1 || descriptor >= handle) {
        fail('source Program does not own an immutable backward descriptor');
      }
      const siteBytes = Buffer.alloc(program.sites.length * 40);
      program.sites.forEach((site, index) => {
        const at = index * 40;
        for (const [offset, value] of [
          [0, site.node], [4, site.action], [8, site.target_slot],
          [12, site.input_signature], [16, site.output_signature],
          [20, site.effect_or_witness], [32, site.target_definition],
          [36, site.target_entry],
        ]) siteBytes.writeUInt32LE(value, at + offset);
        siteBytes.writeBigUInt64LE(BigInt(site.target_identity), at + 24);
        for (const id of [site.input_signature, site.output_signature,
          ...(site.action === 3 ? [site.effect_or_witness] : [])]) checkSignature(id);
      });
      for (const id of [program.input_signature, program.output_signature]) {
        checkSignature(id);
      }
      const atomHandles = [];
      const pending = runtime.cell_c(handle) ? [runtime.cell_c(handle)] : [];
      while (pending.length) {
        const cell = pending.pop();
        if (!Number.isInteger(cell) || cell < 1 || cell >= handle
          || atomHandles.length + pending.length > program.atoms.length * 2 + 1) {
          fail('cyclic concrete Program recipe');
        }
        if (runtime.cell_kind(cell) === 9) {
          const a = runtime.cell_a(cell), b = runtime.cell_b(cell);
          if (!a || !b || a >= cell || b >= cell) fail('nonlinear source recipe');
          pending.push(b, a);
        } else {
          if (runtime.cell_kind(cell) !== 8 || atomHandles.length >= program.atoms.length) {
            fail('unexpected concrete recipe leaf');
          }
          atomHandles.push(cell);
        }
      }
      for (const atom of program.atoms) {
        if (atom.input) checkSignature(atom.input);
        if (atom.output) checkSignature(atom.output);
      }
      const entry = { instance, owner, program, handle, descriptor,
        atomHandles, siteBytes, handles };
      if (!this.sourceProgramMatches(entry, runtime, view)) {
        fail('concrete Program cell, descriptor, slot or checked recipe differs');
      }
      if (program.entry && typeof this.table.get(program.entry) !== 'function'
        || owner.previous && owner.previous.entries.length
          && !owner.previous.entries.some(row => row.index === program.entry)
          && program.entry !== 0) fail('concrete use retargeted old code entry');
      installedPrograms.push(entry);
    }
    for (const installed of installedPrograms) {
      for (const site of installed.program.sites) {
        if (site.action === 3) continue;
        const target = installedPrograms[site.target_program_index];
        if (!target || target.program.slot !== site.target_slot
          || target.handle !== view.getUint32(site.target_slot, true)) {
          fail('concrete source lexical target slot is incomplete');
        }
      }
    }
    const acceptedOwners = [];
    for (const owner of owners) {
      if (!owner.newly_compiled) continue;
      const entries = [];
      for (let index = owner.code_span.start;
        index < owner.code_span.start + owner.code_span.length; index += 1) {
        entries.push({ index, function: this.table.get(index) });
      }
      acceptedOwners.push({ instance, source_epoch: receipt.source_epoch,
        definition: owner.definition, definition_slot: owner.definition_slot,
        definition_identity: owner.definition_identity,
        module_identity: owner.module_identity, module_version: owner.module_version,
        definition_ordinal: owner.definition_ordinal, source: owner.source,
        source_sha256: owner.source_sha256, nodes: owner.nodes,
        source_tag: owner.stamp.source_tag, grant_epoch: owner.stamp.grant_epoch,
        stamp_generation: owner.stamp.generation, code_span: owner.code_span,
        body_entry: owner.body_entry, quotations: owner.quotes, entries });
    }
    return { owners: acceptedOwners, programs: installedPrograms,
      signatures: newSignatures, codeEntries: Array.from(
        { length: span.length }, (_, index) => this.table.get(span.start + index)) };
  }

  prepare(wat, source = Buffer.alloc(0), submission = this.compilations + 1,
    workerLiteralEnd = null, workerCatalog = null, universal = null) {
    if (this.inProcessLive) fail('live mode requires binary preparation');
    if (this.pending || this.poisoned) fail('session is pending or poisoned');
    if (wat.length > MAX_FRAME || source.length > 65536) fail('preparation byte limit exceeded');
    if ((workerLiteralEnd === null) !== (workerCatalog === null)) fail('incomplete worker profile');
    if (workerLiteralEnd !== null && (this.profile !== 'Declared-Modules-v1'
      || !Number.isSafeInteger(workerLiteralEnd) || workerLiteralEnd < 262144
      || workerLiteralEnd > 983040)) fail('invalid worker literal arena');
    if (workerCatalog !== null && (!workerCatalog || !Array.isArray(workerCatalog.catalog)
      || workerCatalog.catalog.length !== 4 || !Array.isArray(workerCatalog.inputs)
      || workerCatalog.inputs.length !== 2
      || typeof workerCatalog.schema_source_sha256 !== 'string'
      || !/^[a-f0-9]{64}$/.test(workerCatalog.schema_source_sha256))) {
      fail('invalid checked worker catalog');
    }
    if (universal !== null && (!this.sourceEpoch || workerCatalog !== null
      || workerLiteralEnd !== null)) fail('universal source needs its selected worker');
    const sourceInstall = universal === null ? null : this.sourcePreflight(universal);
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
    if (sourceInstall && !WebAssembly.Module.imports(module).some(entry =>
      entry.module === this.abi.module && entry.name === 'source_epoch_generation'
        && entry.kind === 'global')) {
      fail('universal Program lacks the shared source epoch import');
    }
    this.compilations += 1;
    const ready = this.install(module, { submission, wasm_sha256: sha256(bytes), source_sha256: sha256(source),
      wat_sha256: sha256(wat), optimized: this.optimized, compilations: this.compilations, stem,
      ...(workerLiteralEnd !== null ? { worker_literal_end: workerLiteralEnd,
        worker_catalog: workerCatalog } : {}) },
    installationCapability);
    if (sourceInstall) this.pending.sourceInstall = sourceInstall;
    return ready;
  }

  liveModule(bytes, source, submission, bridgeBytes, signatureBytes, span,
    watSha256, bridgeWatSha256) {
    if (!this.inProcessLive || this.pending || this.parked || this.staged || this.stagedDeclared || this.poisoned
      || this.liveProposal || this.unregisteredGuest) {
      fail('live session is pending, parked, staged, or poisoned');
    }
    if (!Buffer.isBuffer(source) || source.length > 65536) fail('invalid live source frame');
    if (typeof watSha256 !== 'string' || !/^[0-9a-f]{64}$/.test(watSha256)
      || typeof bridgeWatSha256 !== 'string' || !/^[0-9a-f]{64}$/.test(bridgeWatSha256)) {
      fail('invalid checked source WAT digests');
    }
    integer(submission, Number.MAX_SAFE_INTEGER, 'submission');
    if (!span || !Number.isInteger(span.start) || span.start < 4
      || span.start > this.table.length || !Number.isInteger(span.length)
      || span.length < 0 || span.length > this.table.length - span.start
      || !Number.isInteger(span.generation) || span.generation !==
        ((this.shared.generation.value + 1) >>> 0)) fail('unbound live code span');
    const policy = checkLiveBytes(bytes, this.abi, this.declaredLive, this.liveCodeOwners);
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
    const bridgePolicy = checkLiveBytes(bridgeBytes, this.abi, this.declaredLive, true, true);
    const bridgeModule = new WebAssembly.Module(bridgeBytes);
    for (const [actual, expected] of [
      [WebAssembly.Module.imports(bridgeModule), bridgePolicy.imports],
      [WebAssembly.Module.exports(bridgeModule), bridgePolicy.exports],
    ]) {
      if (actual.length !== expected.length || actual.some((entry, index) =>
        Object.keys(expected[index]).some(key => entry[key] !== expected[index][key]))) {
        fail('V8 disagrees with source-free Core bridge ABI');
      }
    }
    const signatures = checkedSignatureCatalogue(signatureBytes);
    const before = { memory: sha256(Buffer.from(this.memory.buffer)),
      table: Array.from({ length: this.table.length }, (_, index) => this.table.get(index)),
      globals: this.abi.globals.map(global => this.shared[global.name].value),
      trace: this.trace.length, hostRequests: this.hostRequestsTotal };
    const bridge = new WebAssembly.Instance(bridgeModule, { [this.abi.module]: this.shared });
    if (bridge.exports.memory !== this.memory
      || sha256(Buffer.from(this.memory.buffer)) !== before.memory
      || before.table.some((entry, index) => this.table.get(index) !== entry)
      || this.abi.globals.some((global, index) => this.shared[global.name].value !== before.globals[index])
      || this.trace.length !== before.trace || this.hostRequestsTotal !== before.hostRequests) {
      this.poisoned = true;
      fail('source-free Core bridge changed the checked live arena');
    }
    return { module, record: { submission, wasm_sha256: sha256(bytes),
      source_sha256: sha256(source), optimized: false,
      wat_sha256: watSha256, bridge_wat_sha256: bridgeWatSha256,
      bridge_wasm_sha256: sha256(bridgeBytes), signature_catalogue_sha256: signatures.sha256,
      compilations: this.compilations + 1, imports: policy.imports },
      bridge, signatures, span: { ...span } };
  }

  compileBinary(bytes, source, submission, bridgeBytes, signatureBytes, span,
    watSha256, bridgeWatSha256) {
    // Interactive expressions remain ordinary pending preparations against
    // the same arena, including after a staged source-file publication.
    if (!this.inProcessLive || this.pending || this.staged || this.parked || this.poisoned) {
      fail('live compilation requires an idle arena');
    }
    let candidate;
    try {
      candidate = this.liveModule(bytes, source, submission, bridgeBytes, signatureBytes,
        span, watSha256, bridgeWatSha256);
    } catch (error) {
      if (this.poisoned) throw error;
      return this.liveRefusal(`${error.name}: ${error.message}`, 'compile-refused');
    }
    const { module, record, bridge, signatures } = candidate;
    this.liveInstalling = true;
    try {
      const ready = this.install(module, record, installationCapability);
      this.pending.checkedLive = { bridge, signatures, span: candidate.span };
      this.compilations += 1;
      return ready;
    } finally {
      this.liveInstalling = false;
    }
  }

  discardPreparedLive() {
    if (!this.inProcessLive || !this.pending || this.staged || this.parked
      || this.poisoned || this.liveExecuting) {
      fail('no idle prepared live module to discard');
    }
    // Preparation checked the candidate's imports and instantiated it, but
    // execution has not installed its table entries or run its guest body.
    this.pending = null;
    return { schema: 'noble-core-report/v1', profile: this.profile,
      backend: 'managed-linear-memory', stage: 'wasm', outcome: 'discarded',
      generation: this.shared.generation.value, guest_requests: 0,
      protected_operations: 0, candidate_prepare_requests: 0 };
  }

  liveRefusal(diagnostic, outcome = 'reload-refused') {
    const previousWork = this.observationWork;
    const previousGuard = this.liveInstalling;
    this.liveInstalling = true;
    try {
      return { schema: 'noble-core-report/v1', profile: this.profile,
        backend: 'managed-linear-memory', stage: 'wasm', outcome,
        diagnostic: String(diagnostic), generation: this.shared.generation.value,
        stack: this.guestInstalled ? readStack(this, this.runtime()) : [],
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

  verifyLiveInstallation(before, span, bridge, signatures, guestExports) {
    if (this.shared.failure.value !== 0 || this.shared.cp.value !== 0
      || this.shared.rp.value !== 0
      || new DataView(this.memory.buffer).getInt32(61440, true) !== 0
      || before.length !== this.table.length) {
      fail('Core bridge handoff requires a completed idle guest');
    }
    const first = this.coreBridge === null;
    const core = ['core_empty', 'core_quote', 'core_compose', 'core_restore'];
    for (let at = 0; at < this.table.length; at += 1) {
      const entry = this.table.get(at);
      if (at < 4) {
        if (first ? before[at] !== null || typeof entry !== 'function'
          : before[at] !== this.coreBridge.exports[core[at]]
            || entry !== before[at]) fail('Core table provenance changed before handoff');
      } else if (at >= span.start && at < span.start + span.length) {
        if (before[at] !== null || typeof entry !== 'function') {
          fail('guest code did not install its exact free span');
        }
      } else if (entry !== before[at]) {
        fail('guest changed a table entry outside its checked code span');
      }
    }
    if (bridge.exports.signature_count() !== signatures.rows.length
      || guestExports.signature_count() !== signatures.rows.length) {
      fail('guest and bridge signature catalogue cardinality differs from checked source');
    }
    for (let id = 0; id < signatures.rows.length; id += 1) {
      const row = signatures.rows[id];
      if (bridge.exports.signature_address(id) !== row.address
        || bridge.exports.signature_length(id) !== row.length
        || guestExports.signature_address(id) !== row.address
        || guestExports.signature_length(id) !== row.length
        || !Buffer.from(this.memory.buffer, row.address, row.length)
          .equals(signatures.bytes.subarray(row.offset, row.offset + row.length))) {
        fail('runtime signature catalogue differs from checked shared bytes');
      }
    }
    if (!first) return;
    const old = core.map((_, index) => this.table.get(index));
    try {
      for (let index = 0; index < 4; index += 1) {
        if (this.table.get(index) !== old[index]) fail('Core entry changed during handoff');
        this.table.set(index, bridge.exports[core[index]]);
      }
      if (core.some((name, index) => this.table.get(index) !== bridge.exports[name])) {
        fail('Core bridge handoff did not install exact typed entries');
      }
    } catch (error) {
      try {
        for (let index = 0; index < 4; index += 1) this.table.set(index, old[index]);
      } catch (rollbackError) {
        this.poisoned = true;
        fail(`Core table handoff rollback failed: ${rollbackError}`);
      }
      this.poisoned = true;
      fail(`Core table handoff failed: ${error}`);
    }
    this.coreBridge = bridge;
  }

  stageBinary(bytes, source, submission, bridgeBytes, signatureBytes, span,
    watSha256, bridgeWatSha256) {
    try {
      const { module, record, bridge, signatures } =
        this.liveModule(bytes, source, submission, bridgeBytes, signatureBytes,
          span, watSha256, bridgeWatSha256);
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
        if (!this.guestInstalled) this.liveRoot(instance, arena.memory, arena.shared, original);
      } finally {
        this.liveInstalling = false;
      }
      // No Wasm bytes or staging arena need survive the preflight.
      this.staged = { module, record, bridge, signatures, span: { ...span },
        generation: this.shared.generation.value };
      return { schema: 'noble-core-report/v1', profile: this.profile,
        backend: 'managed-linear-memory', stage: 'wasm', outcome: 'ready',
        staged: true, module: record, generation: this.shared.generation.value,
        guest_requests: 0, protected_operations: 0, candidate_prepare_requests: 0 };
    } catch (error) {
      if (this.poisoned) throw error;
      return this.liveRefusal(`${error.name}: ${error.message}`);
    }
  }

  discardBinary() {
    if (!this.inProcessLive || !this.staged) fail('no staged live module to discard');
    this.staged = null;
    return { outcome: 'discarded', generation: this.shared.generation.value };
  }

  liveCodeIdle(allowQueuedProposal = false, allowMissingInstance = false,
    allowUnregistered = false) {
    return this.liveCodeOwners && !this.poisoned && !this.pending && !this.parked
      && !this.staged && (!this.liveProposal || allowQueuedProposal)
      && !this.liveExecuting && !this.liveInstalling
      && (allowMissingInstance || this.guestInstalled)
      && (allowUnregistered || this.unregisteredGuest === null)
      && this.shared.cp.value === 0 && this.shared.failure.value === 0
      && this.shared.rp.value === 0
      && new DataView(this.memory.buffer).getInt32(61440, true) === 0;
  }

  checkedLiveCodeSpan(span, allowQueuedProposal = false, allowEmpty = false,
    allowUnregistered = false) {
    if (!this.liveCodeIdle(allowQueuedProposal, false, allowUnregistered)
      || !Number.isInteger(span.start) || span.start < 4
      || !Number.isInteger(span.length) || span.length < (allowEmpty ? 0 : 1)
      || span.length > this.table.length - span.start
      || !Number.isInteger(span.generation) || span.generation < 1
      || span.generation > 0xffffffff) fail('invalid or non-idle checked live code span');
    return `${span.start}:${span.length}:${span.generation}`;
  }

  registerLiveCode(span) {
    const key = this.checkedLiveCodeSpan(span, true, true, true);
    const owner = this.unregisteredGuest;
    if (!owner || owner.span.start !== span.start || owner.span.length !== span.length
      || owner.span.generation !== span.generation
      || this.liveModuleOwner === owner.instance
      || this.liveCodeSpans.has(key)
      || [...this.liveCodeSpans.values()].some(record =>
        span.length !== 0 && record.span.length !== 0
        && span.start < record.span.start + record.span.length
        && record.span.start < span.start + span.length)) {
      this.poisoned = true;
      fail('duplicate or overlapping installed live code span');
    }
    const entries = [];
    for (let at = span.start; at < span.start + span.length; at += 1) {
      const callable = this.table.get(at);
      if (typeof callable !== 'function') {
        this.poisoned = true;
        fail('installed live code span has a missing table entry');
      }
      entries.push(callable);
    }
    const sourceGeneration = (this.declaredLive
      ? this.declaredLiveState.generation : this.liveSourceGeneration).toString();
    const record = { span: { ...span }, entries, instance: owner.instance,
      ownerId: owner.ownerId, module: owner.record, sourceGeneration };
    this.liveCodeSpans.set(key, record);
    if (this.liveModuleOwner) this.supersededLiveModules.push(this.liveModuleOwner);
    this.liveModuleOwner = owner.instance;
    this.liveModuleOwnerId = owner.ownerId;
    this.unregisteredGuest = null;
    this.liveModuleNamespaceGeneration = this.declaredLive
      ? this.declaredLiveState.generation : null;
    return { outcome: 'code-registered', code_span: record.span,
      table_installed: true,
      guest_instance_owner: owner.ownerId,
      guest_wat_sha256: owner.record.wat_sha256,
      guest_wasm_sha256: owner.record.wasm_sha256,
      guest_source_sha256: owner.record.source_sha256,
      bridge_wat_sha256: owner.record.bridge_wat_sha256,
      bridge_wasm_sha256: owner.record.bridge_wasm_sha256,
      signature_catalogue_sha256: owner.record.signature_catalogue_sha256,
      source_generation: sourceGeneration };
  }

  sweepSupersededLiveModules() {
    if (!this.liveCodeIdle()) fail('module root release requires an idle checked live worker');
    let released = 0;
    for (const instance of this.supersededLiveModules) {
      if (instance === this.liveModuleOwner) {
        this.poisoned = true;
        fail('backend refused superseded Core module root release');
      }
      if (instance.exports.release_module_roots() !== 0) {
        this.poisoned = true;
        fail('backend refused superseded Core module root release');
      }
      this.releasedModuleOwners.add(this.guestOwners.get(instance));
      released += 1;
    }
    this.supersededLiveModules = [];
    // A published declared namespace does not own its predecessor's physical
    // Wasm code: retained source can be rechecked on a later explicit import.
    // Stack/capture roots independently protect old code during this release.
    if (this.declaredLive && this.liveModuleOwner
      && this.liveModuleNamespaceGeneration < this.declaredLiveState.generation) {
      if (this.liveModuleOwner.exports.release_module_roots() !== 0) {
        this.poisoned = true;
        fail('backend refused obsolete declared module root release');
      }
      this.releasedModuleOwners.add(this.liveModuleOwnerId);
      this.liveModuleOwner = null;
      this.liveModuleOwnerId = null;
      this.liveModuleNamespaceGeneration = null;
      released += 1;
    }
    return { outcome: 'module-roots-released', released };
  }

  releaseObservedRoots() {
    if (!this.liveCodeIdle(false, true)) fail('observed root release requires an idle checked live worker');
    if (!this.guestInstalled) {
      if (this.externalCodeRoots.size !== 0) {
        this.poisoned = true;
        fail('unexecuted live worker retained observed code roots');
      }
      return { outcome: 'observed-roots-released', released: 0 };
    }
    const runtime = this.runtime();
    let released = 0;
    // A report grants the host a real root even for nested quotation handles
    // whose raw identities are not printed. Only the host can relinquish all
    // those prior observations; stack, module and active-frame owners remain.
    for (const handle of this.externalCodeRoots) {
      const before = runtime.code_root_state(handle);
      if (!Number.isInteger(before) || before < 0 || (before & 127) === 0
        || runtime.release_code_root(handle) !== 0
        || runtime.code_root_state(handle) !== before - 1) {
        this.poisoned = true;
        fail('backend refused exact observed code root release');
      }
      this.externalCodeRoots.delete(handle);
      released += 1;
    }
    return { outcome: 'observed-roots-released', released };
  }

  recordedLiveCode(span) {
    const record = this.liveCodeSpans.get(this.checkedLiveCodeSpan(span, false, true));
    if (!record || record.span.start !== span.start || record.span.length !== span.length
      || record.span.generation !== span.generation) {
      this.poisoned = true;
      fail('unknown or stale installed live code span');
    }
    return record;
  }

  verifyLiveCodeTable(record) {
    const { start, length } = record.span;
    if (record.entries.length !== length
      || record.entries.some((entry, offset) => this.table.get(start + offset) !== entry)) {
      this.poisoned = true;
      fail('installed live code table ownership snapshot changed');
    }
  }

  inspectLiveCode(span) {
    const record = this.recordedLiveCode(span);
    this.verifyLiveCodeTable(record);
    const runtime = this.runtime();
    if (runtime.clear_code_observation() !== 0) {
      this.poisoned = true;
      fail('cannot clear idle live code observation');
    }
    // A zero-code guest has no backend CodeSpan owner query at all.
    if (span.length === 0) {
      const released = this.releasedModuleOwners.has(record.ownerId);
      return { outcome: released ? 'instance-unowned' : 'instance-owned',
        code_span: record.span, no_code_interval: true,
        module_roots_released: released, guest_instance_owner: record.ownerId };
    }
    const owners = runtime.code_owners(span.start, span.length);
    if (!Number.isInteger(owners) || owners < 0) {
      this.poisoned = true;
      fail('live code ownership inventory is unavailable');
    }
    return { outcome: owners === 0 ? 'code-unowned' : 'code-owned',
      code_span: record.span, code_owners: owners };
  }

  retireLiveCode(span) {
    const record = this.recordedLiveCode(span);
    const runtime = this.runtime();
    if (runtime.clear_code_observation() !== 0
      || (span.length === 0
        ? !this.releasedModuleOwners.has(record.ownerId)
        : runtime.code_owners(span.start, span.length) !== 0)) {
      this.poisoned = true;
      fail('live code ownership changed before exact retirement');
    }
    this.verifyLiveCodeTable(record);
    // An empty span allocates no backend Program entries to invalidate; it
    // still requires module-root release and the full host/table edge scan.
    if (span.length !== 0 && runtime.invalidate_code_span(span.start, span.length) !== 0) {
      this.poisoned = true;
      fail('backend refused unowned live code tombstones');
    }
    try {
      for (let at = span.start; at < span.start + span.length; at += 1) this.table.set(at, null);
    } catch (error) {
      this.poisoned = true;
      fail(`live code table clear failed after backend invalidation: ${error.message}`);
    }
    for (let at = span.start; at < span.start + span.length; at += 1) {
      if (this.table.get(at) !== null) {
        this.poisoned = true;
        fail('live code table clear verification failed after backend invalidation');
      }
    }
    const ownerId = record.ownerId;
    const instance = record.instance;
    if (!this.releasedModuleOwners.has(ownerId)
      || this.guestOwners.get(instance) !== ownerId
      || this.liveModuleOwner === instance
      || this.supersededLiveModules.includes(instance)
      || this.pending?.instance === instance
      || this.activeGuest === instance
      || this.unregisteredGuest?.instance === instance
      || this.last === instance
      || this.liveProposal?.rootOwner === instance.exports
      || !this.coreBridge || this.coreBridge === instance
      || this.runtimeBridge === instance) {
      this.poisoned = true;
      fail('retired guest retains a live JavaScript module owner');
    }
    const core = ['core_empty', 'core_quote', 'core_compose', 'core_restore'];
    if (core.some((name, index) => this.table.get(index) !== this.coreBridge.exports[name])) {
      this.poisoned = true;
      fail('retired guest still owns a core dispatch table entry');
    }
    this.liveCodeSpans.delete(`${span.start}:${span.length}:${span.generation}`);
    record.instance = null;
    record.entries.length = 0;
    const expected = new Map();
    let remaining = false;
    for (const other of this.liveCodeSpans.values()) {
      if (other.ownerId === ownerId) remaining = true;
      for (let offset = 0; offset < other.entries.length; offset += 1) {
        const position = other.span.start + offset;
        if (expected.has(position)) remaining = true;
        expected.set(position, other.entries[offset]);
      }
    }
    // Every remaining table reference must belong to one exact registered
    // code span. The first guest's former core refs were never in that map:
    // the separate four-slot bridge equality check above covers them.
    for (let index = 4; index < this.table.length; index += 1) {
      if (this.table.get(index) !== (expected.get(index) ?? null)) remaining = true;
    }
    if (remaining) {
      this.poisoned = true;
      fail('retired guest or unregistered function remains in the shared table');
    }
    this.releasedModuleOwners.delete(ownerId);
    const identity = { code_span: record.span,
      guest_wat_sha256: record.module.wat_sha256,
      guest_wasm_sha256: record.module.wasm_sha256,
      guest_source_sha256: record.module.source_sha256,
      bridge_wat_sha256: record.module.bridge_wat_sha256,
      bridge_wasm_sha256: record.module.bridge_wasm_sha256,
      signature_catalogue_sha256: record.module.signature_catalogue_sha256,
      source_generation: record.sourceGeneration,
      guest_instance_owner: ownerId,
      guest_table_entries_remaining: 0, guest_js_owner_edges_remaining: 0,
      core_dispatch_bridge_owned: true, guest_instance_releasable: true };
    return span.length === 0
      ? { outcome: 'instance-retired', ...identity,
        no_code_interval: true, module_roots_released: true }
      : { outcome: 'code-retired', ...identity,
        code_owners: 0, table_cleared: true };
  }

  declaredIdle() {
    return this.declaredLive && !this.poisoned && !this.pending && !this.parked
      && !this.staged && !this.liveProposal && !this.liveExecuting && !this.liveInstalling
      && this.shared.cp.value === 0 && this.shared.failure.value === 0;
  }

  declaredStateReport(outcome, state, diagnostic) {
    return { schema: 'noble-core-report/v1', profile: this.profile,
      backend: 'managed-linear-memory', stage: 'wasm', outcome,
      ...(diagnostic ? { diagnostic } : {}),
      generation: Number(state.generation), source_generation: state.generation.toString(),
      retained_roots: state.roots.length,
      guest_requests: 0, protected_operations: 0, candidate_prepare_requests: 0,
      session_state: 'retained' };
  }

  stageLiveDeclared(snapshot) {
    if (!this.declaredLive || this.stagedDeclared) fail('declared live state is unavailable or already staged');
    try {
      if (!this.declaredIdle()) fail('declared live state requires an idle worker');
      declaredLiveObject(snapshot, ['expected_generation', 'generation', 'roots', 'aliases'],
        'declared live state');
      const current = this.declaredLiveState;
      const maximum = BigInt(Number.MAX_SAFE_INTEGER);
      const expected = declaredLiveInteger(snapshot.expected_generation, maximum, 'expected generation');
      const generation = declaredLiveInteger(snapshot.generation, maximum, 'declared generation');
      if (expected !== current.generation || generation !== expected + 1n) {
        fail('stale or skipped declared live generation');
      }
      if (!Array.isArray(snapshot.roots) || snapshot.roots.length > 64
        || !Array.isArray(snapshot.aliases) || snapshot.aliases.length > 64) {
        fail('declared live root or alias limit exceeded');
      }
      const roots = [], byIdentity = new Map(), byNameVersion = new Set();
      for (const entry of snapshot.roots) {
        declaredLiveObject(entry, ['identity', 'module', 'version', 'sha256'], 'declared root');
        const identity = declaredLiveInteger(entry.identity, 64n, 'declared root identity');
        if (identity === 0n || typeof entry.module !== 'string'
          || entry.module.length > 128 || !/^[A-Za-z_][A-Za-z0-9_]*$/.test(entry.module)
          || !Number.isInteger(entry.version) || entry.version < 1 || entry.version > 4294967295
          || typeof entry.sha256 !== 'string' || !/^[0-9a-f]{64}$/.test(entry.sha256)
          || byIdentity.has(identity)) fail('invalid or rebound declared root');
        const key = `${entry.module}\0${entry.version}`;
        if (byNameVersion.has(key)) fail('duplicate declared module version');
        byNameVersion.add(key);
        const root = { identity, module: entry.module, version: entry.version, sha256: entry.sha256 };
        roots.push(root);
        byIdentity.set(identity, root);
      }
      // The namespace may change, but no earlier source owner, version, or
      // source digest may disappear or silently become a different root.
      if (roots.length < current.roots.length || current.roots.some((prior, index) => {
        const root = roots[index];
        return root.identity !== prior.identity || root.module !== prior.module
          || root.version !== prior.version || root.sha256 !== prior.sha256;
      })) fail('declared live state removed or rebound an old root');
      const byBinding = new Map();
      for (const [slot, binding] of this.boundAdapters) {
        const key = `${binding.module}\0${binding.version}`;
        if (byBinding.has(key)) fail('ambiguous immutable declared adapter binding');
        byBinding.set(key, { slot, binding });
      }
      const aliases = [], spellings = new Set(), activeSlots = new Set();
      for (const entry of snapshot.aliases) {
        declaredLiveObject(entry, ['alias', 'identity', 'slot'], 'declared alias');
        const identity = declaredLiveInteger(entry.identity, 64n, 'declared alias identity');
        const root = byIdentity.get(identity);
        if (typeof entry.alias !== 'string' || entry.alias.length > 128
          || !/^[A-Za-z_][A-Za-z0-9_]*$/.test(entry.alias)
          || spellings.has(entry.alias) || !root) fail('invalid declared alias owner');
        spellings.add(entry.alias);
        const selected = byBinding.get(`${root.module}\0${root.version}`);
        if (entry.slot !== (selected?.slot ?? null)) fail('declared alias redirected an immutable adapter slot');
        aliases.push({ alias: entry.alias, identity, slot: entry.slot });
        if (selected?.binding.allowed) activeSlots.add(selected.slot);
      }
      if (aliases.length < current.aliases.length || current.aliases.some((prior, index) =>
        aliases[index].alias !== prior.alias)) {
        fail('declared live state removed or redirected an alias spelling');
      }
      const staged = { generation, roots, aliases, activeSlots,
        arenaGeneration: this.shared.generation.value,
        sourceGeneration: this.liveSourceGeneration,
        hostRequestsTotal: this.hostRequestsTotal, boundRequestsTotal: this.boundRequestsTotal };
      this.stagedDeclared = staged;
      return { ...this.declaredStateReport('declared-state-staged', staged), staged: true };
    } catch (error) {
      return this.declaredStateReport('declared-state-refused', this.declaredLiveState,
        `${error.name}: ${error.message}`);
    }
  }

  publishLiveDeclared() {
    if (!this.declaredLive || !this.stagedDeclared) fail('no staged declared live state to publish');
    const staged = this.stagedDeclared;
    if (!this.declaredIdle() || this.declaredLiveState.generation + 1n !== staged.generation
      || this.shared.generation.value !== staged.arenaGeneration
      || this.liveSourceGeneration !== staged.sourceGeneration
      || this.hostRequestsTotal !== staged.hostRequestsTotal
      || this.boundRequestsTotal !== staged.boundRequestsTotal) {
      return this.declaredStateReport('declared-state-refused', this.declaredLiveState,
        'staged declared state no longer matches the idle worker');
    }
    this.declaredLiveState = staged;
    this.stagedDeclared = null;
    return this.declaredStateReport('declared-state-published', staged);
  }

  discardLiveDeclared() {
    if (!this.declaredLive || !this.stagedDeclared) fail('no staged declared live state to discard');
    this.stagedDeclared = null;
    return this.declaredStateReport('declared-state-discarded', this.declaredLiveState);
  }

  setLiveGrant(name, owner, sourceGeneration) {
    const selected = this.inProcessLive && !this.declaredLive && !this.poisoned && !this.pending && !this.parked
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
    if (!this.inProcessLive || this.declaredLive || !this.liveExecuting || this.liveInstalling) {
      fail('live generation observation outside execution');
    }
    if (this.trace.length >= 4096) fail('live observation trace exhausted');
    this.trace.push(`live.observe-generation:${this.liveSourceGeneration}`);
    return this.liveSourceGeneration;
  }

  livePropose(owner, expected, program) {
    if (!this.inProcessLive || this.declaredLive || !this.liveExecuting || this.liveInstalling) {
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
    const rootOwner = this.activeGuest?.exports;
    if (!rootOwner) {
      this.poisoned = true;
      fail('live proposal lost its executing guest owner');
    }
    const rootStateBefore = rootOwner.code_root_state(program);
    if (rootStateBefore < 0 || rootOwner.queued_proposal_roots() !== 0) {
      return deny('invalid-proposal-owner-state');
    }
    if (rootOwner.retain_code_root(program) !== 0) return deny('unretained-program');
    if (rootOwner.code_root_state(program) !== rootStateBefore + 1
      || rootOwner.queued_proposal_roots() !== 1) {
      if (rootOwner.release_code_root(program) !== 0) {
        this.poisoned = true;
        fail('cannot undo an inconsistently retained live proposal root');
      }
      return deny('invalid-proposal-pin');
    }
    this.liveProposal = { grant, expected, program, snapshot, rootOwner, rootStateBefore };
    this.trace.push(`live.propose:${grant.name}:queued`);
    return 0;
  }

  takeLiveProposal() {
    if (!this.inProcessLive || this.declaredLive || this.poisoned || this.pending || this.staged || this.liveExecuting) {
      fail('live proposal is unavailable');
    }
    const queued = this.liveProposal;
    this.liveProposal = null;
    if (!queued) return { schema: 'noble-core-report/v1', profile: this.profile,
      stage: 'wasm', outcome: 'proposal-none',
      guest_requests: 0, protected_operations: 0, candidate_prepare_requests: 0 };
    try {
      return this.inspectQueuedLiveProposal(queued);
    } finally {
      if (!this.releaseQueuedLiveProposal(queued).released) {
        fail('cannot release inspected live proposal code root');
      }
    }
  }

  releaseQueuedLiveProposal(queued) {
    const witness = { queued_roots_before: -1, queued_roots_after: -1,
      root_state_before: -1, root_state_after: -1, release_status: -1,
      repeat_release_status: -1, unrelated_root_release_status: null,
      root_state_after_repeat: -1, unrelated_root_state_before: null,
      unrelated_root_state_after: null,
      released: false };
    try {
      witness.queued_roots_before = queued.rootOwner.queued_proposal_roots();
      witness.root_state_before = queued.rootOwner.code_root_state(queued.program);
      // Do not skip the actual backend release even if the pre-release
      // inventory is defective: an unreleased root is also a terminal fault.
      witness.release_status = queued.rootOwner.release_code_root(queued.program);
      witness.queued_roots_after = queued.rootOwner.queued_proposal_roots();
      witness.root_state_after = queued.rootOwner.code_root_state(queued.program);
      if (this.poisoned || this.shared.failure.value !== 0) {
        witness.repeat_release_status = queued.rootOwner.release_code_root(queued.program);
        witness.root_state_after_repeat = queued.rootOwner.code_root_state(queued.program);
        const unrelated = this.externalCodeRoots.values().next().value;
        if (unrelated !== undefined && unrelated !== queued.program) {
          witness.unrelated_root_state_before = queued.rootOwner.code_root_state(unrelated);
          witness.unrelated_root_release_status = queued.rootOwner.release_code_root(unrelated);
          witness.unrelated_root_state_after = queued.rootOwner.code_root_state(unrelated);
        }
      }
    } catch {
      // An owner failure is terminal, but must not erase the preceding guest
      // request trace when cancellation follows a failed invocation.
    }
    witness.released = witness.queued_roots_before === 1
      && witness.root_state_before === queued.rootStateBefore + 1
      && witness.release_status === 0
      && witness.queued_roots_after === 0
      && witness.root_state_after === queued.rootStateBefore
      && (witness.repeat_release_status === -1
        || witness.repeat_release_status === 4
          && witness.root_state_after_repeat === witness.root_state_after)
      && (witness.unrelated_root_release_status === null
        || witness.unrelated_root_release_status === 4
          && witness.unrelated_root_state_before === witness.unrelated_root_state_after);
    if (!witness.released) {
      this.poisoned = true;
      this.proposalInspectionFailed = true;
    }
    return witness;
  }

  inspectQueuedLiveProposal(queued) {
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
      saved = this.snapshotLive();
    } catch (error) {
      this.poisoned = true;
      this.proposalInspectionFailed = true;
      fail(`cannot snapshot live proposal inspection: ${error.message}`);
    }
    const restore = () => {
      try { this.rollbackLive(saved); }
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

  snapshotLive() {
    if (this.memory.buffer.byteLength !== this.abi.memory.maximum * 65536
      || this.table.length !== this.abi.table.maximum) fail('live arena exceeds fixed limits');
    return { memory: Uint8Array.from(new Uint8Array(this.memory.buffer)),
      table: Array.from({ length: this.table.length }, (_, index) => this.table.get(index)),
      globals: this.abi.globals.map(global => this.shared[global.name].value),
      trace: this.trace.slice(), boundRequests: this.boundRequests.slice(),
      hostRequestsTotal: this.hostRequestsTotal, boundRequestsTotal: this.boundRequestsTotal,
      protectedOperations: this.protectedOperations, ambientFallbackCalls: this.ambientFallbackCalls,
      boundTraceExhausted: this.boundTraceExhausted, boundTraceTerminal: this.boundTraceTerminal,
      adapters: Array.from(this.boundAdapters.values(), adapter => [adapter.scriptIndex, adapter.invocations]),
      pending: this.pending, last: this.last, lastTraceStart: this.lastTraceStart,
      guestInstalled: this.guestInstalled, activeGuest: this.activeGuest,
      unregisteredGuest: this.unregisteredGuest, runtimeBridge: this.runtimeBridge,
      coreBridge: this.coreBridge,
      nextGuestOwner: this.nextGuestOwner,
      liveSourceGeneration: this.liveSourceGeneration, liveGrant: this.liveGrant,
      compilations: this.compilations, parked: this.parked, poisoned: this.poisoned,
      externalCodeRoots: new Set(this.externalCodeRoots),
      observationWork: this.observationWork };
  }

  rollbackLive(saved) {
    if (this.memory.buffer.byteLength !== saved.memory.length || this.table.length !== saved.table.length) {
      fail('cannot restore grown live arena');
    }
    new Uint8Array(this.memory.buffer).set(saved.memory);
    for (let index = 0; index < saved.table.length; index += 1) this.table.set(index, saved.table[index]);
    this.abi.globals.forEach((global, index) => { this.shared[global.name].value = saved.globals[index]; });
    this.trace = saved.trace;
    this.boundRequests = saved.boundRequests;
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
    this.guestInstalled = saved.guestInstalled;
    this.activeGuest = saved.activeGuest;
    this.unregisteredGuest = saved.unregisteredGuest;
    this.runtimeBridge = saved.runtimeBridge;
    this.coreBridge = saved.coreBridge;
    this.nextGuestOwner = saved.nextGuestOwner;
    this.lastTraceStart = saved.lastTraceStart;
    this.liveSourceGeneration = saved.liveSourceGeneration;
    this.liveGrant = saved.liveGrant;
    this.compilations = saved.compilations;
    this.externalCodeRoots = saved.externalCodeRoots;
    this.parked = saved.parked;
    this.poisoned = saved.poisoned;
    this.observationWork = saved.observationWork;
    if (!Buffer.from(this.memory.buffer).equals(Buffer.from(saved.memory))
      || this.abi.globals.some((global, index) => this.shared[global.name].value !== saved.globals[index])
      || saved.table.some((entry, index) => this.table.get(index) !== entry)) {
      fail('live rollback verification failed');
    }
  }

  publishBinary() {
    if (!this.inProcessLive || !this.staged || this.stagedDeclared || this.poisoned || this.pending || this.parked
      || this.shared.generation.value !== this.staged.generation
      || this.liveSourceGeneration >= 0x7fffffffffffffffn) {
      fail('staged module no longer matches live arena');
    }
    const { module, record, bridge, signatures, span } = this.staged;
    const original = this.liveStackSnapshot(this.memory, this.shared);
    const saved = this.snapshotLive();
    this.staged = null;
    this.liveInstalling = true;
    try {
      const instance = new WebAssembly.Instance(module, { [this.abi.module]: this.shared });
      this.liveRoot(instance, this.memory, this.shared, original);
      this.verifyLiveInstallation(saved.table, span, bridge, signatures, instance.exports);
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
      const ownerId = this.nextGuestOwner++;
      this.guestOwners.set(instance, ownerId);
      this.unregisteredGuest = { instance, ownerId, span, record };
      this.guestInstalled = true;
      this.runtimeBridge = bridge;
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
      try { this.rollbackLive(saved); }
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
      schema: 'noble-artifact-admission/v1', profile: 'Wasm-Draft',
      stage: 'admission', outcome, diagnostic, actual_imports: actualImports,
      claimed_effects: claimedEffects, allowed_effects: allowedEffects,
      guest_requests: 0, protected_operations: 0,
    });
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

  install(module, record = {}, capability) {
    if (capability !== installationCapability) fail('raw module installation is not artifact admission');
    if (this.pending || this.poisoned) fail('session is pending or poisoned');
    const imports = WebAssembly.Module.imports(module);
    const globals = new Set(this.abi.globals.map(global => global.name));
    if (this.sourceEpoch) {
      if (this.shared.source_epoch_generation.value !== this.sourceEpochGeneration) {
        this.poisoned = true;
        fail('signed source epoch global changed before installation');
      }
      globals.add('source_epoch_generation');
    }
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
    const sourceBefore = this.sourceEpoch ? {
      memory: Buffer.from(this.memory.buffer),
      table: Array.from({ length: this.table.length }, (_, index) => this.table.get(index)),
      globals: this.abi.globals.map(global => this.shared[global.name].value),
    } : null;
    let instance;
    try { instance = new WebAssembly.Instance(module, { [this.abi.module]: this.shared }); }
    catch (error) {
      if (this.sourceEpoch) this.poisoned = true;
      throw error;
    }
    if (sourceBefore && (!Buffer.from(this.memory.buffer).equals(sourceBefore.memory)
      || sourceBefore.table.some((entry, index) => this.table.get(index) !== entry)
      || this.abi.globals.some((global, index) =>
        this.shared[global.name].value !== sourceBefore.globals[index]))) {
      this.poisoned = true;
      fail('signed source instantiation changed the pending shared arena');
    }
    if (requests !== this.trace.length || boundRequests !== this.boundRequests.length
      || hostRequestsTotal !== this.hostRequestsTotal || boundRequestsTotal !== this.boundRequestsTotal
      || ambientFallbackCalls !== this.ambientFallbackCalls
      || protectedOperations !== this.protectedOperations || sp !== this.shared.sp.value) {
      if (this.sourceEpoch) this.poisoned = true;
      fail('instantiation executed candidate body');
    }
    if (this.sourceEpoch && this.shared.source_epoch_generation.value !== this.sourceEpochGeneration) {
      this.poisoned = true;
      fail('instantiation changed signed source epoch');
    }
    // Shared table entries keep callable instances alive; do not retain inert
    // modules in an unbounded side list across otherwise empty submissions.
    this.pending = { instance, module, record: { ...record, imports }, traceStart: requests };
    return { schema: 'noble-core-report/v1', profile: this.profile, backend: 'managed-linear-memory',
      stage: 'wasm', outcome: 'ready', module: this.pending.record, guest_requests: 0, protected_operations: 0,
      ...(this.declaredModules ? { host_requests: 0, acquired_authority: false,
        ambient_fallback_calls: 0, real_host_evidence: false, real_host_fallback_calls: 0 } : {}),
      candidate_prepare_requests: 0 };
  }

  execute({ inputs = [], typedValues = null, limits = {} } = {}) {
    if (!this.pending || this.stagedDeclared || this.poisoned) fail('no prepared module or poisoned session');
    const { instance, record, traceStart, checkedLive, sourceInstall } = this.pending;
    const boundStart = this.boundRequests.length;
    const boundTotalStart = this.boundRequestsTotal;
    const hostStart = this.hostRequestsTotal;
    const traceExhaustedStart = this.boundTraceExhausted;
    const adapterStarts = this.declaredModules
      ? new Map(Array.from(this.boundAdapters, ([slot, binding]) => [slot, binding.invocations]))
      : null;
    const protectedStart = this.protectedOperations;
    const fallbackStart = this.ambientFallbackCalls;
    this.lastTraceStart = traceStart;
    this.pending = null;
    const runtime = instance.exports;
    const tableBefore = this.inProcessLive
      ? Array.from({ length: this.table.length }, (_, index) => this.table.get(index)) : null;
    if (!this.inProcessLive) this.last = instance;
    const defaults = this.abi.limits_maximum;
    const keys = ['allocation_bytes', 'recipe_leaves', 'program_depth', 'operand_bytes', 'continuation_bytes', 'steps'];
    if (Object.keys(limits).some(key => !keys.includes(key))) fail('unknown runtime limit');
    let status = runtime.begin(...keys.map(key => integer(limits[key] ?? defaults[key], defaults[key], key)));
    let nativeTrap = null;
    let inputExhausted = null;
    let sourcePublication = null;
    try {
      if (typedValues !== null) {
        const manifest = record.worker_catalog;
        if (this.profile !== 'Declared-Modules-v1' || inputs.length !== 0
          || !Array.isArray(typedValues) || typedValues.length !== 2
          || !Number.isSafeInteger(record.worker_literal_end)
          || record.worker_literal_end > 983040 || record.worker_literal_end < 262144
          || !manifest || !Array.isArray(manifest.inputs) || manifest.inputs.length !== 2) {
          fail('worker input requires admitted declared module and disjoint literal memory');
        }
        this.workerScratch(typedValues);
        const context = { cursor: 983040, nodes: 0 };
        for (let index = 0; index < 2; index += 1) {
          if (status) break;
          const expected = manifest.inputs[index];
          const row = manifest.catalog.find(entry => entry.schema.module_id === expected.module_id
            && entry.schema.ordinal === expected.ordinal
            && entry.schema.version === expected.version
            && entry.schema.shape_sha256 === expected.shape_sha256);
          if (!row) fail('worker input schema absent from checked catalog');
          const handle = this.workerValue(runtime, typedValues[index], context, 0,
            ['Nominal', expected.module_id, expected.ordinal, row.shape], manifest.catalog);
          status = runtime.host_push(handle);
        }
      } else {
        for (const input of inputs) {
          if (status) break;
          status = this.inject(runtime, input);
        }
      }
      if (status === 0) {
        this.liveProposalDenied = false;
        this.liveExecuting = this.inProcessLive;
        this.activeGuest = this.inProcessLive ? instance : null;
        try { status = runtime.submit(); }
        finally { this.liveExecuting = false; this.activeGuest = null; }
        if (status === 0 && this.sourceEpoch) {
          sourcePublication = sourceInstall
            ? this.sourcePostflight(sourceInstall, instance) : null;
          if (!sourceInstall) this.sourceCheckPinned(this.table.length);
        }
        if (status === 0 && this.liveProposalDenied) status = 5;
        if (status === 0 && this.inProcessLive) {
          if (!checkedLive) fail('executed guest has no checked Core bridge');
          this.verifyLiveInstallation(tableBefore, checkedLive.span,
            checkedLive.bridge, checkedLive.signatures, instance.exports);
          const ownerId = this.nextGuestOwner++;
          this.guestOwners.set(instance, ownerId);
          this.unregisteredGuest = { instance, ownerId, span: checkedLive.span, record };
          this.guestInstalled = true;
          this.runtimeBridge = checkedLive.bridge;
        }
      }
    } catch (error) {
      nativeTrap = `${error.name}: ${error.message}`;
      if (typedValues !== null && (error.message === 'worker text budget exhausted'
        || error.message === 'worker value budget exhausted'
        || error.message === 'worker list budget exhausted')) {
        inputExhausted = error.message === 'worker text budget exhausted'
          ? 'typed-input-scratch' : 'typed-input-nodes';
      }
      status = inputExhausted === null ? 6 : 2;
    }
    let cancelledProposal = null;
    if (status !== 0 && this.liveProposal) {
      const queued = this.liveProposal;
      this.liveProposal = null;
      cancelledProposal = this.releaseQueuedLiveProposal(queued);
      if (!cancelledProposal.released) {
        nativeTrap = `${nativeTrap ? `${nativeTrap}; ` : ''}cannot release failed live proposal code root`;
        status = 6;
      }
    }
    this.poisoned = status !== 0;
    let metrics;
    this.workerObservation = record.worker_catalog !== undefined;
    let stack = [];
    let observationExhausted = null;
    let observingStack = false;
    try {
      metrics = Object.fromEntries(Object.entries(this.abi.metrics).map(([id, name]) => [name, Number(runtime.metric(Number(id)))]));
      if (status === 0) {
        observingStack = true;
        stack = readStack(this, runtime);
      }
    } catch (error) {
      if (this.liveProposal) {
        const queued = this.liveProposal;
        this.liveProposal = null;
        if (!this.releaseQueuedLiveProposal(queued).released) {
          this.poisoned = true;
          fail(`cannot release failed live proposal after observation error: ${error}`);
        }
      }
      if (!observingStack || !this.workerObservation || !['typed observation byte limit exhausted',
        'typed observation work limit exhausted'].includes(error.message)) {
        this.poisoned = true;
        throw error;
      }
      status = 2;
      this.poisoned = true;
      observationExhausted = error.message === 'typed observation work limit exhausted'
        ? 'typed-observation-work' : 'typed-observation-bytes';
    }
    const outcome = status === 0 ? 'normal' : status === 1 || status === 2 ? 'runtime-exhausted' : 'trap';
    const isDeclared = this.declaredModules;
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
        proposal_queued: status === 0 && this.liveProposal !== null,
        ...(this.liveCodeOwners ? {
          queued_proposal_roots: runtime.queued_proposal_roots() } : {}),
        ...(cancelledProposal ? {
          cancelled_proposal_roots_before_cancel: cancelledProposal.queued_roots_before,
          cancelled_proposal_roots_remaining: cancelledProposal.queued_roots_after,
          cancelled_proposal_root_state_before: cancelledProposal.root_state_before,
          cancelled_proposal_root_state_after: cancelledProposal.root_state_after,
          cancelled_proposal_release_status: cancelledProposal.release_status,
          cancelled_proposal_repeat_release_status: cancelledProposal.repeat_release_status,
          cancelled_proposal_unrelated_root_release_status: cancelledProposal.unrelated_root_release_status,
          cancelled_proposal_root_state_after_repeat: cancelledProposal.root_state_after_repeat,
          cancelled_proposal_unrelated_root_state_before: cancelledProposal.unrelated_root_state_before,
          cancelled_proposal_unrelated_root_state_after: cancelledProposal.unrelated_root_state_after,
        } : {}) } : {}),
      quota_reason: inputExhausted ?? observationExhausted
        ?? this.abi.quota_reason[String(metrics.quota_reason)] ?? null, stack,
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
      metrics, module: record,
      ...(this.sourceEpoch && status === 0 && this.lastSourceObservation !== null
        ? { source_observation_id: this.lastSourceObservation,
          source_store_id: this.sourceStoreId } : {}) };
    if (this.sourceEpoch && status === 0) {
      if (typeof runtime.module_program_count !== 'function'
        || typeof runtime.module_program_handle !== 'function'
        || typeof runtime.release_module_programs !== 'function'
        || typeof runtime.retire_source_epoch !== 'function') {
        this.poisoned = true;
        fail('signed source module lacks owned epoch exports');
      }
      if (this.sourceCoreEntries === null) {
        if (this.sourceModules.length || this.shared.generation.value !== 1) {
          this.poisoned = true;
          fail('signed source core was not installed first');
        }
        this.sourceCoreEntries = Array.from({ length: 4 }, (_, index) => this.table.get(index));
        if (this.sourceCoreEntries.some(entry => typeof entry !== 'function')) {
          this.poisoned = true;
          fail('signed source core entries are missing');
        }
      }
      const previousEnd = this.sourceTableEntries.length + 4;
      let codeEnd = previousEnd;
      while (codeEnd < this.table.length && typeof this.table.get(codeEnd) === 'function') {
        codeEnd += 1;
      }
      let complete = !sourceInstall
        || codeEnd === sourceInstall.span.start + sourceInstall.span.length;
      for (let index = codeEnd; index < this.table.length; index += 1) {
        if (this.table.get(index) !== null) complete = false;
      }
      if (!complete) {
        this.poisoned = true;
        fail('source table publication is not a complete append-only code span');
      }
      for (let index = previousEnd; index < codeEnd; index += 1) {
        this.sourceTableEntries.push(this.table.get(index));
      }
      this.sourceModules.push(instance);
      if (sourcePublication) {
        for (const owner of sourcePublication.owners) {
          if (this.sourceCodeOwners.has(owner.definition)) {
            this.poisoned = true;
            fail('new source Definition was already installed');
          }
          this.sourceCodeOwners.set(owner.definition, owner);
        }
        this.sourcePrograms.push(...sourcePublication.programs);
        for (const [id, signature] of sourcePublication.signatures) {
          if (!this.sourceSignatures.has(id)) this.sourceSignatures.set(id, signature);
        }
        this.sourceLastGeneration = BigInt(sourceInstall.receipt.source_generation);
      }
    }
    if (record.stem) this.save(`${record.stem}-execution.json`, report);
    return report;
  }

  // Only the selected signed/Text CLI requests an epoch after receiving the
  // unmodified guest report. A reported raw handle is never reused: there is
  // no release protocol for callers holding an arbitrary nested value.
  retireSourceEpoch(generation, codeEnd) {
    if (!this.sourceEpoch || this.poisoned || this.pending || this.parked
      || this.staged || this.stagedDeclared || this.liveProposal || this.liveExecuting
      || this.sourceExternalHandles.size || this.sourceIngressHandles.size
      || this.sourceObservations.size || this.sourceCurrentObservation
      || !this.sourceModules.length
      || this.sourceEpochGeneration >= 0x7fffffff
      || this.shared.source_epoch_generation.value !== this.sourceEpochGeneration
      || generation !== this.shared.generation.value
      || codeEnd !== this.sourceTableEntries.length + 4
      || !Number.isInteger(codeEnd) || codeEnd < 4 || codeEnd > this.table.length
      || this.shared.failure.value || this.shared.sp.value || this.shared.cp.value
      || this.shared.rp.value || this.shared.observed_program.value
      || this.shared.heap_baseline.value > this.shared.heap_cursor.value
      || !this.sourceCoreEntries
      || this.sourceCoreEntries.some((entry, index) => this.table.get(index) !== entry)) {
      return { outcome: 'source-epoch-owned' };
    }
    try { this.sourceCheckPinned(); }
    catch (error) {
      this.poisoned = true;
      fail(`signed source owner pre-retirement check failed: ${error}`);
    }
    let programCount = 0;
    const seen = new Set();
    for (const instance of this.sourceModules) {
      const runtime = instance.exports;
      const count = runtime.module_program_count();
      if (!Number.isInteger(count) || count <= 0 || count > 4096) {
        return { outcome: 'source-epoch-owned' };
      }
      for (let index = 0; index < count; index++) {
        const handle = runtime.module_program_handle(index);
        if (!Number.isInteger(handle) || handle < 1
          || handle > this.shared.heap_cursor.value || seen.has(handle)
          || runtime.cell_kind(handle) !== 4) {
          return { outcome: 'source-epoch-owned' };
        }
        const entry = runtime.cell_x(handle);
        if (entry !== 0 && (entry < 4 || entry >= codeEnd)) {
          return { outcome: 'source-epoch-owned' };
        }
        seen.add(handle);
        programCount += 1;
      }
    }
    for (let index = 4; index < this.table.length; index++) {
      const entry = this.table.get(index);
      if (index < codeEnd ? typeof entry !== 'function' : entry !== null) {
        return { outcome: 'source-epoch-owned' };
      }
    }
    const cellsBefore = this.shared.heap_cursor.value;
    const generationBefore = this.shared.generation.value;
    const releasedBefore = this.runtime().metric(3);
    try {
      // Nonlive Action::Call loads a Program global from its own instance.
      // Clear every installed module global and every non-core table entry;
      // the four source-independent core callbacks hold no $sN reference.
      for (const instance of this.sourceModules) {
        const runtime = instance.exports;
        if (runtime.release_module_programs() !== 0
          || Array.from({ length: runtime.module_program_count() },
            (_, index) => runtime.module_program_handle(index)).some(Boolean)) {
          fail('signed source module roots did not clear');
        }
      }
      for (let index = 4; index < codeEnd; index++) this.table.set(index, null);
      for (let index = 4; index < codeEnd; index++) {
        if (this.table.get(index) !== null) fail('signed source code table failed to clear');
      }
      if (this.runtime().retire_source_epoch() !== 0
        || this.shared.heap_cursor.value !== 0 || this.shared.heap_baseline.value !== 0
        || this.shared.generation.value !== generationBefore
        || this.runtime().metric(3) - releasedBefore !== BigInt(cellsBefore * 48)
        || this.sourceCoreEntries.some((entry, index) => this.table.get(index) !== entry)
        || this.shared.source_epoch_generation.value !== this.sourceEpochGeneration) {
        fail('signed source heap retirement violated the checked epoch');
      }
      const releasedAfter = Number(this.runtime().metric(3));
      // Ordinary signed/Text modules can interleave with universal modules
      // without a source receipt. After ALL roots, table entries, and heap
      // cells retire, clear their entire reserved static-text region rather
      // than infer a cursor from only the universal compiler receipts.
      if (this.memory.buffer.byteLength < 1048576) fail('signed source text arena was lost');
      new Uint8Array(this.memory.buffer, 262144, 1048576 - 262144).fill(0);
      this.sourceModules = [];
      this.last = null;
      this.sourceTableEntries = [];
      this.sourceCodeOwners.clear();
      this.sourcePrograms = [];
      this.sourceSignatures.clear();
      this.sourceEpochGeneration += 1;
      this.shared.source_epoch_generation.value = this.sourceEpochGeneration;
      return { schema: 'noble-source-epoch-retirement/v1',
        outcome: 'source-epoch-retired', generation, code_end: codeEnd,
        source_store_id: this.sourceStoreId,
        source_epoch_before: this.sourceEpochGeneration - 1,
        source_epoch_after: this.sourceEpochGeneration,
        heap_cells_before: cellsBefore, released_cells: cellsBefore,
        released_bytes_before: Number(releasedBefore),
        released_bytes_after: releasedAfter,
        released_programs: programCount, retained_core_entries: 4,
        heap_cells_after: 0, cleared_code_entries: codeEnd - 4 };
    } catch (error) {
      this.poisoned = true;
      fail(`signed source epoch failed closed: ${error}`);
    }
  }

  workerScratch(values) {
    const pending = [values[1], values[0]];
    let cursor = 983040, nodes = 0;
    while (pending.length) {
      const value = pending.pop();
      if (!value || typeof value !== 'object' || ++nodes > 4096)
        fail('worker value budget exhausted');
      if (value.tag === 'text') {
        if (typeof value.value !== 'string') fail('invalid worker Text');
        const count = Buffer.byteLength(value.value, 'utf8');
        if (count > 65536 || count > this.memory.buffer.byteLength - cursor)
          fail('worker text budget exhausted');
        cursor += count;
      } else if (value.tag === 'pair') {
        pending.push(value.right, value.left);
      } else if (value.tag === 'sum' || value.tag === 'opaque' || value.tag === 'variant') {
        pending.push(value.value);
      } else if (value.tag === 'list') {
        if (!Array.isArray(value.items) || value.items.length > 4096 - nodes - pending.length)
          fail('worker list budget exhausted');
        for (let at = value.items.length - 1; at >= 0; at -= 1) pending.push(value.items[at]);
      } else if (value.tag !== 'i64' && value.tag !== 'bool' && value.tag !== 'unit') {
        fail('unsupported worker value');
      }
    }
  }

  workerValue(runtime, value, context, depth, expected, catalog) {
    if (depth >= 32 || ++context.nodes > 4096) fail('worker value budget exhausted');
    if (!value || typeof value !== 'object' || !Array.isArray(expected)) fail('invalid worker value');
    if (expected[0] === 'I64' && value.tag === 'i64') {
      if (typeof value.value !== 'string' || value.value.length > 20
        || !/^(0|-[1-9][0-9]*|[1-9][0-9]*)$/.test(value.value)) fail('invalid I64');
      const number = BigInt(value.value);
      if (BigInt.asIntN(64, number) !== number) fail('worker I64 range exceeded');
      return runtime.host_i64(number);
    }
    if (expected[0] === 'Bool' && value.tag === 'bool' && typeof value.value === 'boolean')
      return runtime.host_bool(value.value ? 1 : 0);
    if (expected[0] === 'Unit' && value.tag === 'unit' && value.value === null)
      return runtime.host_unit();
    if (expected[0] === 'Text' && value.tag === 'text' && typeof value.value === 'string') {
      const count = Buffer.byteLength(value.value, 'utf8');
      if (count > 65536 || context.cursor + count > this.memory.buffer.byteLength) fail('worker text budget exhausted');
      const bytes = Buffer.from(value.value, 'utf8');
      new Uint8Array(this.memory.buffer, context.cursor, bytes.length).set(bytes);
      const handle = runtime.host_text(context.cursor, count);
      context.cursor += count;
      return handle;
    }
    if (expected[0] === 'Pair' && value.tag === 'pair') return runtime.host_pair(
      this.workerValue(runtime, value.left, context, depth + 1, expected[1], catalog),
      this.workerValue(runtime, value.right, context, depth + 1, expected[2], catalog));
    if (expected[0] === 'Sum' && value.tag === 'sum'
      && (value.arm === 'left' || value.arm === 'right'))
      return runtime.host_sum(value.arm === 'left' ? 0 : 1,
        this.workerValue(runtime, value.value, context, depth + 1,
          expected[value.arm === 'left' ? 1 : 2], catalog));
    if (expected[0] === 'List' && value.tag === 'list' && Array.isArray(value.items)) {
      if (value.items.length > 4096) fail('worker list budget exhausted');
      let tail = runtime.host_list_nil();
      for (let at = value.items.length - 1; at >= 0; at -= 1) {
        tail = runtime.host_list_cons(
          this.workerValue(runtime, value.items[at], context, depth + 1, expected[1], catalog), tail);
      }
      return tail;
    }
    if (expected[0] === 'Nominal' && (value.tag === 'opaque' || value.tag === 'variant')) {
      const row = catalog.find(entry => entry.schema.module_id === expected[1]
        && entry.schema.ordinal === expected[2]
        && JSON.stringify(entry.shape) === JSON.stringify(expected[3]));
      if (!row || !value.schema || Object.keys(value.schema).length !== 4
        || Object.keys(row.schema).some(key => value.schema[key] !== row.schema[key])) {
        fail('worker nominal has no checked schema');
      }
      const shape = row.shape, constructors = row.constructors;
      if (!Array.isArray(shape) || !constructors) fail('worker nominal shape absent');
      if (value.tag === 'opaque' && (shape[0] !== 'opaque' || constructors.opaque !== true))
        fail('private or wrong opaque constructor');
      if (value.tag === 'variant' && (shape[0] !== 'variant'
        || !((value.arm === 'left' && constructors.left === true)
          || (value.arm === 'right' && constructors.right === true)))) {
        fail('private or wrong variant constructor');
      }
      const arm = value.tag === 'opaque' ? 0 : value.arm === 'left' ? 1 : 2;
      return runtime.host_nominal(BigInt(value.schema.module_id), value.schema.ordinal, arm,
        this.workerValue(runtime, value.value, context, depth + 1,
          shape[value.tag === 'opaque' || arm === 1 ? 1 : 2], catalog));
    }
    fail('unsupported worker value');
  }

  // Standalone typed injection outside one submission. Values stay first-class
  // session state; each push fails closed on a missing or mistyped cell.
  push(inputs) {
    const initialized = this.inProcessLive ? this.guestInstalled : this.last !== null;
    if (!initialized && (inputs.length !== 0 || this.shared.sp.value !== 0)) fail('no initialized runtime for injection');
    const runtime = initialized ? this.runtime() : null;
    let status = 0;
    for (const input of inputs) {
      if (status) break;
      status = this.inject(runtime, input);
    }
    let stack = [];
    if (!status && runtime) stack = readStack(this, runtime);
    return { schema: 'noble-core-report/v1', profile: this.profile, backend: 'managed-linear-memory',
      stage: 'wasm', outcome: status ? 'injection-refused' : 'pushed', status, stack,
      ...(this.sourceEpoch && !status && this.lastSourceObservation !== null
        ? { source_observation_id: this.lastSourceObservation,
          source_store_id: this.sourceStoreId } : {}),
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
    const saved = bytes.slice();
    const roots = [];
    if (this.liveCodeOwners && count) {
      const runtime = this.runtime();
      for (let index = 0; index < count; index += 1) {
        if (runtime.stack_kind(index) < 4) continue;
        const handle = runtime.save_stack_code_root(index);
        if (!Number.isInteger(handle) || handle <= 0) {
          this.poisoned = true;
          fail('cannot retain parked stack code root');
        }
        roots.push(handle);
      }
    }
    this.parked = { count, bytes: saved, roots };
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
    if (this.liveCodeOwners) {
      const runtime = this.runtime();
      for (const handle of this.parked.roots) {
        if (runtime.release_code_root(handle) !== 0) {
          this.poisoned = true;
          fail('cannot release restored stack code root');
        }
      }
    }
    this.parked = null;
    const stack = readStack(this, this.runtime());
    return { outcome: 'restored', stack,
      ...(this.sourceEpoch && this.lastSourceObservation !== null
        ? { source_observation_id: this.lastSourceObservation,
          source_store_id: this.sourceStoreId } : {}) };
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
    if (input.type === 'Program') {
      const handle = integer(input.handle, 4096, 'program handle');
      this.requireExternalCodeRoot(handle, input.source_epoch, input.source_store_id);
      return runtime.push_program(handle);
    }
    if (input.type === 'Contract') return runtime.push_contract(
      integer(input.index, 4294967295, 'contract index'), BigInt(input.statement),
      integer(input.claim_kind ?? 1, 1, 'claim kind'), integer(input.revision ?? 0, 4294967295, 'claim revision'));
    if (input.type === 'Evidence') {
      const contract = integer(input.contract, 4096, 'contract handle');
      this.requireExternalCodeRoot(contract, input.source_epoch, input.source_store_id);
      return runtime.push_evidence(
        integer(input.index, 4294967295, 'evidence index'),
        integer(input.class ?? 1, 255, 'evidence class'),
        integer(input.ruleset ?? 1, 4294967295, 'ruleset version'), contract);
    }
    if (input.type === 'Certified') {
      const subject = integer(input.subject, 4096, 'subject handle');
      const contract = integer(input.contract, 4096, 'contract handle');
      const evidence = integer(input.evidence, 4096, 'evidence handle');
      for (const handle of [subject, contract, evidence]) {
        this.requireExternalCodeRoot(handle, input.source_epoch, input.source_store_id);
      }
      return runtime.push_certified(subject, contract, evidence, BigInt(input.identity));
    }
    fail('unsupported runtime input type');
  }

  retainExternalCodeRoot(runtime, handle, visible = true) {
    if (this.sourceEpoch) {
      if (!Number.isInteger(handle) || handle < 1
        || handle > this.abi.limits_maximum.session_cells
        || runtime.cell_kind(handle) < 4 || this.sourceCurrentObservation === null) {
        this.poisoned = true;
        fail('invalid externally exposed signed source handle');
      }
      const group = this.sourceCurrentObservation.counts;
      const count = (this.sourceExternalHandles.get(handle) ?? 0) + 1;
      if (!Number.isSafeInteger(count)) fail('signed source observation ledger exhausted');
      this.sourceExternalHandles.set(handle, count);
      group.set(handle, (group.get(handle) ?? 0) + 1);
      if (visible) {
        this.sourceIngressHandles.set(handle, (this.sourceIngressHandles.get(handle) ?? 0) + 1);
        const ingress = this.sourceCurrentObservation.ingress;
        ingress.set(handle, (ingress.get(handle) ?? 0) + 1);
      }
      return;
    }
    if (!this.liveCodeOwners || this.externalCodeRoots.has(handle)) return;
    if (!Number.isInteger(handle) || handle < 1 || handle > this.abi.limits_maximum.session_cells
      || runtime.retain_code_root(handle) !== 0) {
      this.poisoned = true;
      fail('cannot retain externally visible code root');
    }
    this.externalCodeRoots.add(handle);
  }

  beginSourceObservation() {
    if (this.sourceCurrentObservation || this.sourceNextObservation > Number.MAX_SAFE_INTEGER
      || this.sourceObservations.size >= 4096) fail('signed source observation ledger exhausted');
    this.sourceCurrentObservation = { id: this.sourceNextObservation,
      counts: new Map(), ingress: new Map() };
  }

  finishSourceObservation() {
    const group = this.sourceCurrentObservation;
    if (!group) fail('signed source observation has no owner');
    this.lastSourceObservation = group.counts.size ? group.id : null;
    if (group.counts.size) {
      this.sourceObservations.set(group.id, group);
      this.sourceNextObservation += 1;
    }
    this.sourceCurrentObservation = null;
  }

  releaseSourceObservation(store, id) {
    const refused = () => ({ schema: 'noble-core-report/v1',
      profile: this.profile, backend: 'managed-linear-memory', stage: 'wasm',
      outcome: 'source-observation-refused', guest_requests: 0,
      protected_operations: 0, candidate_prepare_requests: 0,
      session_state: 'retained' });
    if (!this.sourceEpoch || this.poisoned || this.pending || this.parked
      || this.staged || this.stagedDeclared || this.liveExecuting
      || this.sourceCurrentObservation || this.shared.failure.value
      || this.shared.sp.value || this.shared.cp.value || this.shared.rp.value
      || this.shared.observed_program.value) {
      return refused();
    }
    if (store !== this.sourceStoreId) return refused();
    const group = this.sourceObservations.get(id);
    if (!group) return refused();
    for (const [handle, count] of group.counts) {
      if (!Number.isInteger(count) || count < 1
        || (this.sourceExternalHandles.get(handle) ?? 0) < count
        || (this.sourceIngressHandles.get(handle) ?? 0) < (group.ingress.get(handle) ?? 0)) {
        return refused();
      }
    }
    let released = 0;
    for (const [handle, count] of group.counts) {
      const remaining = this.sourceExternalHandles.get(handle) - count;
      if (remaining) this.sourceExternalHandles.set(handle, remaining);
      else this.sourceExternalHandles.delete(handle);
      const visible = group.ingress.get(handle) ?? 0;
      if (visible) {
        const ingressRemaining = this.sourceIngressHandles.get(handle) - visible;
        if (ingressRemaining) this.sourceIngressHandles.set(handle, ingressRemaining);
        else this.sourceIngressHandles.delete(handle);
      }
      released += count;
    }
    this.sourceObservations.delete(id);
    return { schema: 'noble-core-report/v1', profile: this.profile,
      backend: 'managed-linear-memory', stage: 'wasm',
      outcome: 'source-observation-released', source_observation_id: id,
      released_handles: released, remaining_handles: this.sourceExternalHandles.size,
      source_epoch: this.sourceEpochGeneration, source_store_id: this.sourceStoreId,
      guest_requests: 0, protected_operations: 0, candidate_prepare_requests: 0,
      session_state: 'retained' };
  }

  requireExternalCodeRoot(handle, epoch, store) {
    if (this.sourceEpoch && (epoch !== this.sourceEpochGeneration
      || store !== this.sourceStoreId
      || !this.sourceIngressHandles.has(handle))) {
      fail('signed source raw cell handle lacks an exposed owner');
    }
    if (this.liveCodeOwners && !this.externalCodeRoots.has(handle)) {
      fail('live raw cell handle lacks a retained owner');
    }
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
  project(handle, epoch, store) {
    const runtime = this.runtime();
    handle = integer(handle, this.abi.limits_maximum.session_cells, 'cell handle');
    this.requireExternalCodeRoot(handle, epoch, store);
    const subject = runtime.certified_subject(handle);
    if (subject && this.sourceEpoch) this.beginSourceObservation();
    if (subject) this.retainExternalCodeRoot(runtime, subject);
    if (subject && this.sourceEpoch) this.finishSourceObservation();
    return { schema: 'noble-core-report/v1', profile: this.profile, backend: 'managed-linear-memory',
      stage: 'wasm', outcome: subject ? 'projected' : 'projection-failed', handle, subject,
      ...(subject && this.sourceEpoch ? { source_epoch: this.sourceEpochGeneration,
        source_store_id: this.sourceStoreId, source_observation_id: this.lastSourceObservation } : {}),
      identity_unchanged: subject !== 0,
      guest_requests: 0, protected_operations: 0, candidate_prepare_requests: 0 };
  }

  runtime() {
    if (this.inProcessLive) {
      if (!this.guestInstalled || !this.runtimeBridge) fail('no installed Core bridge to inspect');
      return this.runtimeBridge.exports;
    }
    if (!this.last) fail('no executed instance to inspect');
    return this.last.exports;
  }

  close() {
    this.pending = null;
    if (this.owned) fs.rmSync(this.directory, { recursive: true });
  }
}

