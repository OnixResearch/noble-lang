// Only the separately selected live-slot worker constructs this engine. The
// ordinary Core and live-reload engines retain their original ABI and arena.
const SLOT_LIMITS = Object.freeze([196608, 256, 256, 2048, 4096, 100000]);
// Leave room in the 4 MiB worker frame for the response envelope and the
// separate exact per-root request trace.
const GLOBAL_TRACE_BYTES = 2 * 1024 * 1024;
const ROOT_TRACE_RESERVATION_BYTES = 1024 * 1024;
const REPLAY_TRACE_BYTES = Buffer.byteLength(',"replay":"scripted"');
const TRACE_ROWS = 4096;

function slotDecimal(value, label) {
  if (typeof value !== 'string' || !/^(0|[1-9][0-9]{0,19})$/.test(value)
    || BigInt(value) > 0xffffffffffffffffn) throw Error(`invalid ${label}`);
  return BigInt(value);
}

function slotReply(outcome, fields = {}) {
  return { schema: 'noble-live-slot-report/v1', stage: 'slot', outcome,
    guest_requests: 0, protected_operations: 0, ...fields };
}

function sameSlotJSON(left, right) {
  const normalized = value => {
    if (Array.isArray(value)) return value.map(normalized);
    if (value !== null && typeof value === 'object') {
      return Object.fromEntries(Object.keys(value).sort()
        .map(key => [key, normalized(value[key])]));
    }
    return value;
  };
  return JSON.stringify(normalized(left)) === JSON.stringify(normalized(right));
}

class SlotEngine {
  constructor(selection, baseAbi, config) {
    if (config.slot_mode !== true || config.in_process_live === true
      || process.versions.node !== selection.tools.node.version
      || process.versions.v8 !== selection.tools.node.v8
      || fs.realpathSync(process.execPath) !== fs.realpathSync(selection.tools.node.path)) {
      fail('unselected live-slot worker or incompatible Node/V8');
    }
    const assembler = selection.tools.wasm_tools;
    if (!path.isAbsolute(assembler.path)) fail('selected assembler path is not absolute');
    const version = spawnSync(assembler.path, ['--version'], {
      encoding: 'utf8', env: {}, timeout: 10000, maxBuffer: 65536 });
    if (version.error || version.status !== 0
      || !version.stdout.trim().startsWith(`wasm-tools ${assembler.version}`)) {
      fail('selected Wasm assembler is unavailable');
    }
    this.tools = { node: { ...selection.tools.node, sha256: sha256(fs.readFileSync(process.execPath)) },
      wasm_tools: { ...assembler, sha256: sha256(fs.readFileSync(assembler.path)) } };
    this.assembler = assembler.path;
    this.memory = new WebAssembly.Memory({ ...baseAbi.memory, initial: 20, maximum: 20 });
    this.table = new WebAssembly.Table(baseAbi.table);
    this.abi = { ...baseAbi,
      memory: { ...baseAbi.memory, initial: 20, maximum: 20 },
      globals: [...baseAbi.globals,
        { name: 'live_target', type: 'i32', value: 0 },
        { name: 'live_frame', type: 'i32', value: 0 }],
      exports: { ...baseAbi.exports,
        ...Object.fromEntries(Object.keys(slotExports).map(name => [name, 'checked live-slot export'])) } };
    this.shared = { memory: this.memory, table: this.table };
    for (const { name, type, value } of this.abi.globals) {
      this.shared[name] = new WebAssembly.Global({ value: type, mutable: true },
        type === 'i64' ? BigInt(value) : value);
    }
    this.registry = null;
    this.authority = null;
    this.grants = new Map();
    this.sources = new Map();
    this.modules = new Map();
    this.handles = new Map();
    this.versions = new Map();
    this.savedOwners = new Map();
    this.savedCells = new Map();
    this.nextSavedOwner = 0;
    this.saving = null;
    this.replays = new Map();
    this.nextReplay = 0;
    this.retiring = new Map();
    this.maxResidentCode = 4096;
    this.replaying = false;
    this.controlFd = process.env.NOBLE_SLOT_CONTROL_FD === '2' ? 2 : -1;
    this.controlEvents = [];
    this.checkpointFailure = null;
    this.checkpointIndex = 0;
    this.rootSequence = 0;
    this.active = null;
    this.trace = [];
    this.traceBytes = 0;
    this.traceFailure = null;
    this.poisoned = false;
    this.proposalInspectionFailed = false;
  }

  close() { this.active = null; }

  applyHostControl(request, controlId) {
    if (!request || typeof request !== 'object' || Array.isArray(request)
      || typeof request.operation !== 'string' || !/^[0-9]{1,20}$/.test(controlId)) {
      throw Error('malformed selected host control');
    }
    // Allocate the receipt before mutation: a successful CAS must never
    // become unreportable because the bounded ledger is full.
    if (this.controlEvents.length >= 256)
      throw Error('control receipts exceed bounded root capacity before mutation');
    if (this.controlEvents.some(event => event.control_id === controlId))
      throw Error('repeated root control ID before mutation');
    const event = { operation: request.operation, scope: undefined,
      outcome: 'control-failed', epoch: undefined, control_id: controlId };
    this.controlEvents.push(event);
    let result;
    let scope;
    if (request.operation === 'policy') {
      const grant = request.grant;
      const id = grant?.slotId ?? grant?.nominalId;
      scope = `${grant?.operation}:${id}`;
      if (grant?.allowed !== false || typeof grant.operation !== 'string'
        || typeof id !== 'string' || (grant.slotId !== undefined && grant.nominalId !== undefined)
        || !this.grants.has(scope)) throw Error('unapproved control mutation');
      this.grants.set(scope, false);
      result = slotReply('policy-updated');
    } else if (request.operation === 'publish' || request.operation === 'rollback') {
      if (typeof request.slot !== 'string' || typeof request.id !== 'string'
        || !this.modules.has(request.id)
        || !this.versions.has(this.modules.get(request.id)?.pinned[request.program_index
          ?? this.modules.get(request.id)?.target_metadata.root_program_index])) {
        result = slotReply('candidate-not-staged');
      } else result = this.publish(request);
      scope = request.slot;
    } else if (request.operation === 'delete') {
      if (typeof request.slot !== 'string') throw Error('unscoped slot deletion');
      const deletion = this.registry.delete({ slotId: request.slot,
        expectedEpoch: slotDecimal(request.expected_epoch, 'expected epoch'),
        expectedIncarnation: slotDecimal(request.expected_incarnation, 'expected incarnation'),
        expectedGeneration: slotDecimal(request.expected_generation, 'expected generation') });
      result = slotReply(deletion.outcome, { epoch: deletion.epoch.toString() });
      scope = request.slot;
    } else throw Error('unapproved control operation');
    event.scope = scope;
    event.outcome = result.outcome;
    event.epoch = result.epoch;
    return event;
  }

  checkpoint(importName, site = null, ordinal = null) {
    if (this.controlFd !== 2 || !this.active || this.checkpointIndex >= 4096) {
      this.poisoned = true;
      throw Error('selected host import lacks authenticated checkpoint channel');
    }
    const root = String(this.rootSequence);
    const index = ++this.checkpointIndex;
    const checkpointDeadline = performance.now() + 8000;
    const send = payload => {
      const bytes = Buffer.from(JSON.stringify(payload));
      if (bytes.length > 4096 || fs.writeSync(this.controlFd, bytes) !== bytes.length) {
        throw Error('host checkpoint write failed');
      }
    };
    const receive = () => {
      const packet = Buffer.allocUnsafe(4097);
      let size;
      for (;;) {
        try {
          size = fs.readSync(this.controlFd, packet, 0, packet.length, null);
          break;
        } catch (error) {
          // No packet is consumed on EINTR. Retrying the same bounded read is
          // safe; every other channel failure still poisons this root.
          if (error.code !== 'EINTR' || performance.now() >= checkpointDeadline) throw error;
        }
      }
      if (!size || size > 4096) throw Error('host checkpoint timed out or was malformed');
      return JSON.parse(packet.toString('utf8', 0, size));
    };
    try {
      send({ kind: 'checkpoint', root, index,
        operation: importName, ...(site === null ? {} : { site }),
        ...(ordinal === null ? {} : { ordinal }) });
      for (let count = 0; count <= 8; count++) {
        const message = receive();
        if (message?.root !== root || message?.index !== index) {
          throw Error('host checkpoint identity mismatch');
        }
        if (message.kind === 'resume') return;
        if (message.kind !== 'update' || count === 8) {
          throw Error('host checkpoint exceeded bounded control count');
        }
        const controlId = message.request?.control_id;
        const event = this.applyHostControl(message.request, controlId);
        try {
          send({ kind: 'commit', root, index,
            control_id: controlId, outcome: event.outcome,
            ...(event.epoch === undefined ? {} : { epoch: event.epoch }) });
        } catch (error) {
          const ambiguous = Error(`host control ${controlId} already committed outcome `
            + `${event.outcome} epoch=${event.epoch ?? 'unchanged'}; checkpoint ACK failed`,
          { cause: error });
          ambiguous.committed = true;
          throw ambiguous;
        }
      }
      throw Error('host checkpoint did not resume');
    } catch (error) {
      this.poisoned = true;
      this.checkpointFailure = error.message;
      throw Error(`host checkpoint failed closed; session poisoned: ${error.message}`,
        { cause: error });
    }
  }

  configure(request) {
    if (this.registry || this.modules.size) return slotReply('authority-refused');
    const authority = request.authority;
    if (!authority || !Array.isArray(authority.effects)
      || !Array.isArray(authority.grants) || !Array.isArray(authority.resources)
      || !Array.isArray(authority.sources) || authority.sources.length > 256) {
      return slotReply('authority-refused');
    }
    const sources = new Map();
    const grants = new Map();
    for (const source of authority.sources) {
      if (typeof source?.id !== 'string' || !source.id || sources.has(source.id)
        || typeof source.sha256 !== 'string' || !/^[0-9a-f]{64}$/.test(source.sha256)) {
        return slotReply('authority-refused');
      }
      sources.set(source.id, source.sha256);
    }
    for (const grant of authority.grants) {
      if (typeof grant?.operation !== 'string' || typeof grant.allowed !== 'boolean'
        || typeof (grant.slotId ?? grant.nominalId) !== 'string') {
        return slotReply('authority-refused');
      }
      grants.set(`${grant.operation}:${grant.slotId ?? grant.nominalId}`, grant.allowed);
    }
    this.sources = sources;
    this.grants = grants;
    this.registry = new SlotRegistry({ quota: authority.quota, knownEffects: authority.effects,
      authorize: event => this.grants.get(`${event.operation}:${event.slotId ?? event.nominalId}`) === true,
      traceAdmission: (bytes, rows) => this.traceBytes + bytes
        + (this.replaying ? rows * REPLAY_TRACE_BYTES : 0) <= GLOBAL_TRACE_BYTES,
      performEffect: (effect, text) => {
        if (effect !== 'test.emit') throw Error('unavailable protected host effect');
        return `emitted:${text}`;
      },
      previewEffect: (effect, text) => {
        if (effect === 'test.emit') return `emitted:${text}`;
        if (effect === 'test.abort') return '';
        throw Error('unavailable protected host effect');
      },
      retireTarget: handle => this.retireTarget(handle),
      retainProgram: (handle, version) => this.retainSaved(handle, version),
      releaseProgramBackend: (handle, version) => this.releaseSaved(handle, version),
      verifyEvidence: null });
    try {
      for (const resource of authority.resources) this.registry.registerNominal(resource);
    } catch (error) {
      this.poisoned = true;
      throw error;
    }
    this.authority = Object.freeze(authority);
    return slotReply('configured', { epoch: '0' });
  }

  compile(wat) {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-slot-'));
    const output = path.join(directory, 'checked.wasm');
    const result = spawnSync(this.assembler, ['parse', '-', '-o', output], {
      input: wat, env: {}, timeout: 30000, maxBuffer: 65536 });
    try {
      if (result.error || result.status !== 0) throw Error(`selected WAT parse refused: ${result.stderr ?? result.error}`);
      const checked = spawnSync(this.assembler, ['validate', output], {
        env: {}, timeout: 30000, maxBuffer: 65536 });
      if (checked.error || checked.status !== 0) throw Error(`selected Wasm validate refused: ${checked.stderr ?? checked.error}`);
      const bytes = fs.readFileSync(output);
      if (bytes.length > MAX_FRAME) throw Error('slot binary exceeds bounded worker frame');
      const policy = checkSlotBytes(bytes, this.abi);
      const module = new WebAssembly.Module(bytes);
      for (const [actual, expected] of [[WebAssembly.Module.imports(module), policy.imports],
        [WebAssembly.Module.exports(module), policy.exports]]) {
        if (actual.length !== expected.length || actual.some((entry, index) =>
          Object.keys(expected[index]).some(key => entry[key] !== expected[index][key]))) {
          throw Error('V8 disagrees with checked live-slot binary ABI');
        }
      }
      return { module, digest: sha256(bytes) };
    } finally { fs.rmSync(directory, { recursive: true, force: true }); }
  }

  imports(moduleId) {
    const checkedModule = () => this.modules.get(moduleId)?.token;
    const active = () => {
      if (!this.active) throw Error('guest host import without an active pinned root');
      return this.active.root;
    };
    const guarded = (fn, denied = 0) => (...args) => {
      try { return fn(...args); }
      catch (error) {
        if (error instanceof TraceCapacityError) this.traceFailure = error.message;
        const record = { operation: 'host-import-refused', moduleId, reason: String(error) };
        const bytes = Buffer.byteLength(JSON.stringify(record)) + 1;
        const pending = this.active ? this.registry.traceSize(this.active.root) : { bytes: 0, rows: 0 };
        if (this.traceBytes + pending.bytes + bytes
          + (this.replaying ? pending.rows * REPLAY_TRACE_BYTES : 0) > GLOBAL_TRACE_BYTES) {
          const failure = new TraceCapacityError(
            'live-slot trace capacity refused before host-import-refused record');
          this.traceFailure = failure.message;
          throw failure;
        }
        this.trace.push(record);
        this.traceBytes += bytes;
        return denied;
      }
    };
    const selected = guarded((ordinal, site) => {
      this.checkpoint('live_select', site >>> 0, ordinal >>> 0);
      const decision = this.registry.dispatch(active(), ordinal >>> 0, checkedModule(), site >>> 0);
      return decision.handle >>> 0;
    });
    const frameEnter = guarded((site, ptr, handle) =>
      this.registry.enterFrame(active(), checkedModule(), site, ptr, handle, this.memory));
    const frameLeave = guarded(token => this.registry.leaveFrame(active(), token), 1);
    const emit = guarded((offset, length) => {
      this.checkpoint('test_emit');
      if (!Number.isInteger(offset) || !Number.isInteger(length) || offset < 0
        || length < 0 || length > 65536 || offset + length > this.memory.buffer.byteLength) {
        throw Error('invalid protected request memory range');
      }
      const text = utf8.decode(new Uint8Array(this.memory.buffer, offset, length));
      let result;
      if (this.replaying) {
        const script = this.replaying;
        const expected = script.effects[script.index++];
        result = this.registry.replayEffect(active(), this.shared.live_frame.value,
          this.shared.live_target.value, 'test.emit', text, expected);
      } else {
        result = this.registry.requestEffect(active(), this.shared.live_frame.value,
          this.shared.live_target.value, 'test.emit', text);
      }
      return result.outcome === 'performed' ? 0 : 5;
    }, 5);
    return { ...this.shared,
      live_select: selected,
      live_bind_validate: guarded((position, ordinal) =>
        this.registry.validateBorrowBinding(active(), checkedModule(), position >>> 0, ordinal >>> 0)),
      live_frame_enter: frameEnter, live_frame_leave: frameLeave,
      resource_validate: guarded((ordinal, kind) =>
        this.registry.validateResource(active(), ordinal >>> 0, kind >>> 0)),
      resource_bind_validate: guarded((...args) =>
        this.registry.validateResourceBinding(active(), checkedModule(), ...args)),
      test_emit: emit,
      test_abort: guarded(() => {
        this.checkpoint('test_abort');
        if (this.replaying) throw Error('protected effects are unavailable during frozen replay');
        const result = this.registry.requestEffect(active(), this.shared.live_frame.value,
          this.shared.live_target.value, 'test.abort', '');
        return result.outcome === 'performed' ? 0 : 5;
      }, 5),
    };
  }

  install(request, wat) {
    if (!this.registry || this.modules.has(request.id) || !Buffer.isBuffer(wat)
      || wat.length < 8 || wat.length > 1048576 || sha256(wat) !== request.wat_sha256
      || this.sources.get(request.id) !== request.source_sha256
      || request.source_artifact_sha256 !== request.source_sha256
      || this.modules.size >= 128 || !Number.isInteger(request.code_span?.length)
      || request.code_span.length < 1
      || [...this.modules.values()].reduce((size, module) =>
        size + module.code_span.length, request.code_span.length) > this.maxResidentCode) {
      return slotReply('install-refused');
    }
    let compiled;
    try { compiled = this.compile(wat); }
    catch (error) { return slotReply('install-refused', { diagnostic: String(error) }); }
    const checked = request.target_metadata;
    if (!checked || !Array.isArray(checked.stack_in) || !Array.isArray(checked.stack_out)
      || !Array.isArray(checked.effects) || !Array.isArray(request.live_sites)
      || !Array.isArray(request.resource_catalog) || !Array.isArray(request.source_schemas)
      || !Array.isArray(request.program_metadata)) {
      return slotReply('install-refused');
    }
    const selected = request.selected_target;
    if (selected !== undefined && selected !== null
      && (!Number.isInteger(selected.definition_index) || selected.definition_index < 0
        || !Number.isInteger(selected.named_program_index) || selected.named_program_index < 0
        || typeof selected.definition_identity !== 'string'
        || !/^(0|[1-9][0-9]{0,19})$/.test(selected.definition_identity)
        || typeof selected.source_generation !== 'string'
        || !/^(0|[1-9][0-9]{0,19})$/.test(selected.source_generation)
        || selected.source_sha256 !== request.source_sha256
        || selected.wat_sha256 !== request.wat_sha256
        || ![...this.sources.values()].includes(selected.selected_source_sha256))) {
      return slotReply('install-refused', {
        diagnostic: 'checked selected definition/source/WAT identity does not match admitted authority',
      });
    }
    const sourceCatalog = this.registry.admitResourceCatalog(request.host_owner,
      request.source_artifact_sha256, request.resource_catalog, request.source_schemas);
    const instance = new WebAssembly.Instance(compiled.module,
      { [this.abi.module]: this.imports(request.id) });
    const handle = instance.exports.install_target() >>> 0;
    if (handle === 0) throw Error(`checked inert install failed: failure=${this.shared.failure.value}, `
      + `quota_reason=${this.shared.quota_reason.value}, phase=${this.shared.phase.value}, `
      + `generation=${this.shared.generation.value}`);
    const count = instance.exports.module_program_count();
    if (!Number.isInteger(count) || count < 1 || count > 256
      || checked.root_program_index >= count || request.program_metadata.length !== count
      || selected && selected.named_program_index >= count) {
      throw Error('invalid checked installed Program count');
    }
    const span = request.code_span;
    if (!span || !Number.isInteger(span.start) || !Number.isInteger(span.length)
      || !Number.isInteger(span.generation) || span.start < 4 || span.length < 1
      || span.start + span.length > this.table.length) {
      throw Error('invalid checked installed code span');
    }
    const pinned = [];
    for (let index = 0; index < count; index++) {
      const program = instance.exports.module_program_handle(index) >>> 0;
      const metadata = request.program_metadata[index];
      const runtime = instance.exports;
      if (!program || this.handles.has(program) || runtime.cell_kind(program) !== 4
        || metadata?.program_index !== index
        || metadata.entry < span.start || metadata.entry >= span.start + span.length
        || runtime.cell_x(program) !== metadata.entry
        || runtime.cell_y(program) !== metadata.input_signature
        || runtime.cell_z(program) !== metadata.output_signature
        || runtime.cell_payload(program) !== BigInt(metadata.effect_mask)
        || runtime.cell_a(program) !== metadata.capture_left
        || runtime.cell_b(program) !== metadata.capture_right
        || runtime.cell_w(program) !== metadata.recipe_depth
        || runtime.cell_n(program) !== metadata.recipe_leaves) {
        throw Error('actual installed Program cell differs from independently checked metadata');
      }
      pinned.push(program);
    }
    if (pinned[checked.root_program_index] !== handle) throw Error('installed root differs from checked metadata');
    if (instance.exports.cell_y(handle) !== checked.input_signature
      || instance.exports.cell_z(handle) !== checked.output_signature
      || instance.exports.cell_payload(handle) !== BigInt(checked.effect_mask)) {
      throw Error('installed Program is inconsistent with checked root metadata/code span');
    }
    const token = this.registry.registerModule(compiled.digest, request.live_sites,
      checked.stack_in, sourceCatalog, request.source_artifact_sha256);
    for (const program of pinned) this.handles.set(program, { instance, moduleId: request.id });
    this.modules.set(request.id, { ...request, token, handle, instance, pinned,
      artifactSha256: compiled.digest, installed: true,
      published: false, versionOwned: new Set(), replayOwned: new Set(),
      rootsReleased: false, discardRequested: false });
    return slotReply('installed', { id: request.id, handle,
      artifact_sha256: compiled.digest, generation: String(this.shared.generation.value) });
  }

  candidate(module, handle) {
    const index = module.pinned.indexOf(handle);
    const metadata = module.program_metadata[index];
    const artifactSha256 = module.artifactSha256;
    if (!metadata || module.versionOwned.has(handle)
      || module.instance.exports.retain_target(handle) !== 0) {
      throw Error('cannot acquire exact installed candidate Program owner');
    }
    module.versionOwned.add(handle);
    const captures = [];
    for (const field of ['cell_a', 'cell_b']) {
      const child = module.instance.exports[field](handle) >>> 0;
      if (child) captures.push(`${field}:${this.captureValue(module.instance.exports, child)}`);
    }
    const recipeCell = module.instance.exports.cell_c(handle) >>> 0;
    const recipeSha256 = sha256(Buffer.from(recipeCell
      ? this.captureValue(module.instance.exports, recipeCell, 4096, 1048576) : '[]'));
    const selected = module.selected_target;
    const checkedNamed = selected?.named_program_index === index;
    const definitionId = checkedNamed
      ? `checked:${selected.definition_index}:${selected.definition_identity}:${selected.source_generation}`
      : null;
    const catalog = this.registry.admitResourceCatalog(module.host_owner,
      module.source_artifact_sha256, module.resource_catalog, module.source_schemas);
    return this.registry.admitCandidate({
      programValueId: `artifact:${artifactSha256}:cell:${handle}`,
      definitionId,
      captures, interface: { input: metadata.stack_in, output: metadata.stack_out,
        effectCeiling: metadata.effects },
      effects: metadata.effects,
      semanticContext: `definition-source:${checkedNamed
        ? selected.selected_source_sha256 : 'unavailable'}:selection-source:${module.source_sha256}`
        + `:artifact:${artifactSha256}:occurrence:${index}:recipe:${recipeSha256}`,
      recipeSha256,
      claim: 'unverified', assumptions: 'unverified', artifactSha256,
      resourceCatalog: module.resource_catalog,
      sourceArtifactSha256: module.source_artifact_sha256,
    }, handle, artifactSha256, catalog);
  }

  captureValue(runtime, root, maxNodes = 64, maxBytes = 1000) {
    const nodes = [];
    const indices = new Map([[root, 0]]);
    const pending = [root];
    for (let index = 0; index < pending.length; index++) {
      const handle = pending[index] >>> 0;
      if (!handle || pending.length > maxNodes) throw Error('checked captured value exceeds host observation bound');
      const kind = runtime.cell_kind(handle);
      if (!kind) throw Error('checked candidate captures a stale backend cell');
      const [a, b, c] = ['cell_a', 'cell_b', 'cell_c']
        .map(field => runtime[field](handle) >>> 0);
      const childIndex = child => {
        if (!child) return null;
        if (!indices.has(child)) {
          indices.set(child, pending.length);
          pending.push(child);
        }
        return indices.get(child);
      };
      const x = runtime.cell_x(handle), y = runtime.cell_y(handle);
      let textBytes = null;
      if (kind === 11) {
        if (x < 0 || y < 0 || y > 65536 || x + y > this.memory.buffer.byteLength) {
          throw Error('checked captured text exceeds bounded guest memory');
        }
        textBytes = Buffer.from(this.memory.buffer, x, y).toString('hex');
      }
      nodes.push({ kind, payload: runtime.cell_payload(handle).toString(),
        x: kind === 11 ? null : x, y, textBytes, z: runtime.cell_z(handle),
        w: runtime.cell_w(handle), n: runtime.cell_n(handle),
        a: childIndex(a), b: childIndex(b), c: childIndex(c) });
    }
    const encoded = JSON.stringify(nodes);
    if (Buffer.byteLength(encoded) > maxBytes) {
      throw Error('checked captured values exceed bounded exact trace representation');
    }
    return encoded;
  }

  publish(request) {
    if (request.owner !== undefined && request.id !== undefined) return slotReply('publication-refused');
    const saved = request.owner === undefined ? null : this.savedOwners.get(request.owner);
    const module = this.modules.get(saved ? saved.sourceId : request.id);
    if (!module || module.discardRequested || saved && saved.programIndex === null) {
      return slotReply('unadmitted-target');
    }
    if (request.proof_required === true || request.interface?.proofRequired === true) {
      return slotReply('selected-target-evidence-refused', {
        diagnostic: 'a pinned independent Lean checker receipt bound to checked named identity and exact installed Wasm bytes is unavailable',
      });
    }
    const index = request.program_index ?? module.target_metadata.root_program_index;
    if (!saved && (!Number.isInteger(index) || index < 0 || index >= module.pinned.length)) {
      return slotReply('unadmitted-target');
    }
    if (saved && request.program_index !== undefined && request.program_index !== saved.programIndex) {
      return slotReply('unadmitted-target');
    }
    const handle = saved?.handle ?? module.pinned[index];
    let version = this.versions.get(handle);
    if (!version) {
      if (this.active) return slotReply('candidate-not-staged');
      version = this.candidate(module, handle);
      this.versions.set(handle, version);
    }
    const record = this.registry.publish({ slotId: request.slot,
      operation: request.operation,
      expectedEpoch: slotDecimal(request.expected_epoch, 'expected epoch'),
      expectedIncarnation: request.expected_incarnation == null ? null
        : slotDecimal(request.expected_incarnation, 'expected incarnation'),
      expectedGeneration: request.expected_generation == null ? null
        : slotDecimal(request.expected_generation, 'expected generation'),
      interface: request.interface, version });
    if (record.outcome === 'published') module.published = true;
    return slotReply(record.outcome, { epoch: record.epoch.toString(),
      incarnation: record.incarnation?.toString() ?? null,
      generation: record.generation?.toString() ?? null });
  }

  retireTarget(handle) {
    const owner = this.handles.get(handle >>> 0);
    const module = this.modules.get(owner?.moduleId);
    if (!module || !module.versionOwned.has(handle >>> 0)
      || owner.instance.exports.release_target(handle >>> 0) !== 0) return false;
    module.versionOwned.delete(handle >>> 0);
    this.versions.delete(handle >>> 0);
    this.sweepRetirement(module);
    return true;
  }

  stageCandidate(request) {
    const module = this.modules.get(request.id);
    const index = request.program_index ?? module?.target_metadata.root_program_index;
    if (!module || module.rootsReleased || module.discardRequested || this.active
      || !Number.isInteger(index) || index < 0 || index >= module.pinned.length) {
      return slotReply('candidate-refused');
    }
    const handle = module.pinned[index];
    if (!this.versions.has(handle)) this.versions.set(handle, this.candidate(module, handle));
    return slotReply('candidate-staged', { id: module.id, program_index: index });
  }

  retainSaved(handle, version) {
    const pending = this.saving;
    return pending?.expectedHandle === (handle >>> 0)
      && pending.runtime.save_stack_program(pending.index) === (handle >>> 0);
  }

  releaseSaved(handle) {
    const saved = this.savedCells.get(handle >>> 0);
    return saved?.runtime.release_saved_program(handle >>> 0) === 0;
  }

  sweepRetirement(module) {
    if (this.active || !(module.published || module.discardRequested)
      || module.versionOwned.size || module.replayOwned.size) return;
    if (!module.rootsReleased) {
      if (module.instance.exports.release_module_roots() !== 0) {
        throw Error('backend refused module-root retirement');
      }
      module.rootsReleased = true;
    }
    const { start, length, generation } = module.code_span;
    if (length === 0) return;
    const owners = module.instance.exports.code_owners(start, length);
    if (owners < 0) throw Error('code ownership queried during active dispatch');
    if (owners === 0) {
      this.retiring.set(`${start}:${length}:${generation}`, module);
    }
  }

  discard(request) {
    const module = this.modules.get(request.id);
    if (!module || module.host_owner !== request.host_owner
      || this.grants.get(`discard:${request.id}`) !== true
      || module.published
      || module.replayOwned.size || this.active
      || [...this.savedOwners.values()].some(owner => owner.moduleIds.has(module.id))) {
      return slotReply('discard-refused');
    }
    module.discardRequested = true;
    for (const handle of [...module.versionOwned]) {
      this.registry.discardCandidate(this.versions.get(handle));
    }
    this.sweepRetirement(module);
    return slotReply('discarded', { id: module.id, code_span: { ...module.code_span } });
  }

  retireCodeSpans() {
    return [...this.retiring.values()].map(module => ({ ...module.code_span }));
  }

  retireCode(request) {
    const span = request.code_span;
    if (this.active || !span || !Number.isInteger(span.start)
      || !Number.isInteger(span.length) || !Number.isInteger(span.generation)) {
      return slotReply('code-retirement-refused');
    }
    const key = `${span.start}:${span.length}:${span.generation}`;
    const module = this.retiring.get(key);
    if (!module || !module.rootsReleased || module.versionOwned.size
      || module.code_span.start !== span.start || module.code_span.length !== span.length
      || module.code_span.generation !== span.generation) {
      return slotReply('code-retirement-refused');
    }
    const owners = module.instance.exports.code_owners(span.start, span.length);
    if (owners !== 0) return slotReply('code-retirement-refused', { code_owners: owners });
    try {
      for (let index = span.start; index < span.start + span.length; index++) {
        this.table.set(index, null);
      }
    } catch (error) {
      this.poisoned = true;
      const partial = Error('code retirement failed after table clear began; session poisoned',
        { cause: error });
      partial.committed = true;
      throw partial;
    }
    for (const handle of module.pinned) this.handles.delete(handle);
    this.modules.delete(module.id);
    for (const handle of module.pinned) this.versions.delete(handle);
    this.retiring.delete(key);
    return slotReply('code-retired', { code_span: { ...span },
      code_owners: 0, table_cleared: true });
  }

  inject(runtime, input) {
    if (!input || typeof input !== 'object') throw Error('invalid root injection');
    if (input.kind === 'i64') {
      if (typeof input.value !== 'string' || !/^-?(0|[1-9][0-9]*)$/.test(input.value)) {
        throw Error('invalid decimal I64 root input');
      }
      const value = BigInt(input.value);
      if (value < -(1n << 63n) || value >= (1n << 63n)) throw Error('I64 input out of range');
      return runtime.push_i64(value);
    }
    if (input.kind === 'bool' && typeof input.value === 'boolean') {
      return runtime.push_bool(Number(input.value));
    }
    if (input.kind === 'unit') return runtime.push_unit();
    if (input.kind === 'program') {
      const saved = this.savedOwners.get(input.owner);
      if (!saved) throw Error('missing saved actual Program root owner');
      return runtime.push_program(saved.handle);
    }
    if (input.kind === 'resource') {
      const module = slotDecimal(input.module, 'resource nominal module');
      this.registry.issueResource(this.active.root, input.position, input.ordinal,
        module.toString(), input.nominalOrdinal, input.resourceKind, input.owner);
      return runtime.push_nominal_resource(input.position, input.ordinal,
        input.resourceKind, Number(module & 0xffffffffn), Number(module >> 32n), input.nominalOrdinal);
    }
    throw Error('unavailable typed host input');
  }

  observeProgramGraph(runtime, rootHandle) {
    const visited = new Set();
    const moduleIds = new Set();
    const versions = new Set();
    const pending = [rootHandle];
    while (pending.length) {
      const handle = pending.pop() >>> 0;
      if (!handle || visited.has(handle)) continue;
      if (visited.size >= 4096) throw Error('saved Program graph exceeds host observation bound');
      const kind = runtime.cell_kind(handle);
      if (!kind) throw Error('saved Program graph contains a stale cell');
      visited.add(handle);
      if (kind === 4) {
        const version = this.versions.get(handle);
        if (version) versions.add(version);
        const entry = runtime.cell_x(handle);
        for (const module of this.modules.values()) {
          const { start, length } = module.code_span;
          if (entry >= start && entry < start + length) moduleIds.add(module.id);
        }
      }
      for (const field of ['cell_a', 'cell_b', 'cell_c']) {
        const child = runtime[field](handle) >>> 0;
        if (child && runtime.cell_kind(child)) pending.push(child);
      }
    }
    return { moduleIds, versions: [...versions] };
  }

  saveOutput(runtime, index) {
    const handle = Number(runtime.stack_value(index)) >>> 0;
    if (!handle || runtime.cell_kind(handle) !== 4) {
      throw Error('backend returned no live Program stack cell');
    }
    if (this.nextSavedOwner === Number.MAX_SAFE_INTEGER) throw Error('saved Program owner exhausted');
    const graph = this.observeProgramGraph(runtime, handle);
    this.saving = { runtime, index, expectedHandle: handle };
    let registryOwner;
    try { registryOwner = this.registry.saveProgramGraph(handle, graph.versions); }
    finally { this.saving = null; }
    const prior = this.savedCells.get(handle);
    if (prior && prior.runtime !== runtime) {
      this.poisoned = true;
      throw Error('saved backend Program owner crossed an untrusted runtime boundary');
    }
    this.savedCells.set(handle, { runtime, count: (prior?.count ?? 0) + 1 });
    const token = `program-${++this.nextSavedOwner}`;
    const installed = this.handles.get(handle);
    const module = this.modules.get(installed?.moduleId);
    const programIndex = module?.pinned.indexOf(handle) ?? -1;
    let observedCaptures = null;
    if (programIndex < 0) {
      const entry = runtime.cell_x(handle);
      if (entry === 1) {
        const recipe = runtime.cell_c(handle) >>> 0;
        if (runtime.cell_a(handle) || runtime.cell_b(handle)
          || runtime.cell_kind(recipe) !== 8 || runtime.cell_x(recipe) !== 1
          || runtime.cell_payload(recipe) !== runtime.cell_payload(handle)
          || runtime.cell_w(handle) !== 1 || runtime.cell_n(handle) !== 1) {
          throw Error('dynamic scalar quote differs from checked backend capture layout');
        }
        observedCaptures = [{ type: 'I64', value: runtime.cell_payload(handle).toString() }];
      } else {
        observedCaptures = ['cell_a', 'cell_b'].flatMap(field => {
          const child = runtime[field](handle) >>> 0;
          return child ? [{ field, value: JSON.parse(this.captureValue(runtime, child)) }] : [];
        });
      }
    }
    this.savedOwners.set(token, { handle, moduleIds: graph.moduleIds, registryOwner,
      sourceId: programIndex < 0 ? null : module.id,
      programIndex: programIndex < 0 ? null : programIndex });
    return { kind: 4, owner: token, source_id: programIndex < 0 ? null : module.id,
      program_index: programIndex < 0 ? null : programIndex,
      ...(programIndex < 0 ? { capture_status: 'backend-observed-source-occurrence-unavailable',
        capture_values: observedCaptures } : {}) };
  }

  releaseProgram(token) {
    const saved = this.savedOwners.get(token);
    if (!saved) return slotReply('program-owner-refused');
    this.registry.releaseProgram(saved.registryOwner);
    const cell = this.savedCells.get(saved.handle);
    if (cell.count === 1) this.savedCells.delete(saved.handle);
    else cell.count--;
    this.savedOwners.delete(token);
    for (const moduleId of saved.moduleIds) {
      const module = this.modules.get(moduleId);
      if (module) this.sweepRetirement(module);
    }
    return slotReply('program-released', { owner: token });
  }

  releaseReplay(token) {
    const recorded = this.replays.get(token);
    if (!recorded) return slotReply('replay-owner-refused');
    const module = this.modules.get(recorded.id);
    if (!module?.replayOwned.has(token) || module.instance.exports.release_target(module.handle) !== 0) {
      this.poisoned = true;
      throw Error('frozen caller backend owner release failed; session poisoned');
    }
    module.replayOwned.delete(token);
    this.registry.releaseFrozen(recorded.snapshot);
    this.replays.delete(token);
    for (const module of this.modules.values()) this.sweepRetirement(module);
    return slotReply('replay-released', { token });
  }

  appendRootTrace(trace, size) {
    const bytes = size.bytes + (this.replaying ? size.rows * REPLAY_TRACE_BYTES : 0);
    if (size.rows !== trace.length || this.traceBytes + bytes > GLOBAL_TRACE_BYTES) {
      throw Error('exact live-slot trace retention disagrees with recorded root');
    }
    if (this.replaying) this.trace.push(...trace.map(row => ({ ...row, replay: 'scripted' })));
    else this.trace.push(...trace);
    this.traceBytes += bytes;
  }

  invoke(request) {
    this.controlEvents = [];
    this.checkpointFailure = null;
    this.traceFailure = null;
    const module = this.modules.get(request.id);
    if (!module || !Array.isArray(request.inputs) || !Array.isArray(request.refs)
      || module.rootsReleased || this.active || request.record === true && this.replays.size >= 128) {
      return slotReply('invocation-refused');
    }
    if (this.traceBytes + ROOT_TRACE_RESERVATION_BYTES + TRACE_ROWS * REPLAY_TRACE_BYTES
      > GLOBAL_TRACE_BYTES) {
      return slotReply('trace-capacity-refused', {
        diagnostic: 'exact host trace retention is full; release this session before invoking' });
    }
    const root = this.registry.pinRoot(module.handle, module.token, request._frozen_snapshot ?? null);
    if (this.rootSequence === Number.MAX_SAFE_INTEGER) throw Error('host root sequence exhausted');
    this.rootSequence++;
    this.checkpointIndex = 0;
    this.active = { root, module };
    const runtime = module.instance.exports;
    const createdOwners = [];
    let replayToken = null;
    try {
      for (const ref of request.refs) this.registry.bindSelectedRef(root,
        ref.position, ref.ordinal, ref.slot);
      const begin = runtime.begin_live(...SLOT_LIMITS);
      if (begin !== 0) throw Error(`live begin returned status ${begin}`);
      for (const input of request.inputs) {
        const status = this.inject(runtime, input);
        if (status !== 0) throw Error(`host root injection returned status ${status}`);
      }
      for (const ref of request.refs) {
        const status = runtime.bind_live_ref(ref.position, ref.ordinal);
        if (status !== 0) throw Error(`checked root ref binding returned status ${status}`);
      }
      const status = runtime.invoke_live();
      const trace = this.registry.trace(root);
      if (this.poisoned || status !== 0 || this.traceFailure) {
        throw Error(this.traceFailure
          ?? `checked guest execution returned status ${status}; control lane poisoned=${this.poisoned}`);
      }
      const count = runtime.stack_length();
      if (!Number.isInteger(count) || count < 0 || count > 128) throw Error('invalid result stack length');
      const stack = [];
      for (let index = 0; index < count; index++) {
        const kind = runtime.stack_kind(index);
        if (kind === 4) {
          const saved = this.saveOutput(runtime, index);
          createdOwners.push(saved.owner);
          stack.push(saved);
        } else stack.push({ kind, value: runtime.stack_value(index).toString() });
      }
      if (runtime.clear_stack() !== 0) throw Error('backend could not release actual operand roots');
      const rootEpoch = this.registry.rootEpoch(root).toString();
      if (request.record === true) {
        if (trace.some(item => item.operation === 'effect' && item.outcome !== 'performed')
          || stack.some(item => item.kind === 4)) {
          throw Error('frozen replay requires completed effect responses and scalar outputs');
        }
        if (this.nextReplay === Number.MAX_SAFE_INTEGER
          || runtime.retain_target(module.handle) !== 0) {
          throw Error('cannot retain actual frozen caller Program owner');
        }
        let snapshot;
        try { snapshot = this.registry.freezeRoot(root); }
        catch (error) {
          if (runtime.release_target(module.handle) !== 0) this.poisoned = true;
          throw error;
        }
        const slots = this.registry.frozenIdentity(root);
        if (slots.some(row => row.definitionId === null)) {
          if (runtime.release_target(module.handle) !== 0) this.poisoned = true;
          this.registry.releaseFrozen(snapshot);
          throw Error('frozen replay requires an independently checked selected DefinitionId');
        }
        replayToken = `replay-${++this.nextReplay}`;
        module.replayOwned.add(replayToken);
        const identity = { caller: { id: module.id, handle: module.handle,
          sourceSha256: module.source_sha256, artifactSha256: module.artifactSha256 },
        slots };
        this.replays.set(replayToken, { snapshot, id: module.id, trace, stack, identity });
      }
      const traceSize = this.registry.traceSize(root);
      this.registry.finishRoot(root);
      this.active = null;
      for (const candidate of this.modules.values()) this.sweepRetirement(candidate);
      this.appendRootTrace(trace, traceSize);
      return slotReply('executed', { epoch: rootEpoch, stack,
        ...(replayToken === null ? {} : { replay_token: replayToken }),
        ...(replayToken === null ? {} : { frozen_identity: this.replays.get(replayToken).identity }),
        request_trace: trace, control_events: this.controlEvents.splice(0),
        guest_requests: trace.length,
        protected_operations: this.replaying ? 0 : trace.filter(item =>
          item.operation === 'effect' && item.outcome === 'performed').length });
    } catch (error) {
      if (replayToken !== null) {
        try { this.releaseReplay(replayToken); }
        catch (retired) { error = retired; }
      }
      for (const owner of createdOwners) {
        try { this.releaseProgram(owner); } catch (retired) { error = retired; }
      }
      let trace = [];
      try { trace = this.registry.trace(root); }
      catch (inspection) { error = inspection; }
      let traceSize = { bytes: 0, rows: 0 };
      try { traceSize = this.registry.traceSize(root); }
      catch (inspection) { error = inspection; }
      try { this.registry.abortRoot(root); } catch (retired) { error = retired; }
      this.active = null;
      this.poisoned = true;
      this.appendRootTrace(trace, traceSize);
      return slotReply('execution-failed', { diagnostic: String(error)
        + (this.checkpointFailure === null ? '' : `; checkpoint: ${this.checkpointFailure}`),
        request_trace: trace, control_events: this.controlEvents.splice(0),
        guest_requests: trace.length,
        protected_operations: this.replaying ? 0 : trace.filter(item =>
          item.operation === 'effect' && item.outcome === 'performed').length,
        session_state: 'terminated-after-guest-failure' });
    }
  }

  request(request, wat) {
    if (!request || typeof request !== 'object' || Array.isArray(request)
      || typeof request.operation !== 'string' || this.poisoned) {
      return slotReply('session-refused');
    }
    if (request.operation === 'configure') return this.configure(request);
    if (!this.registry) return slotReply('authority-required');
    if (request.operation !== 'install' && wat.length !== 0) return slotReply('unexpected-module-frame');
    if (request.operation === 'control-barrier') {
      return slotReply('control-barrier', { control_events: this.controlEvents.splice(0) });
    }
    if (request.operation === 'install') return this.install(request, wat);
    if (request.operation === 'candidate') return this.stageCandidate(request);
    if (request.operation === 'discard') return this.discard(request);
    if (request.operation === 'retire-code') return this.retireCode(request);
    if (request.operation === 'release-program') return this.releaseProgram(request.owner);
    if (request.operation === 'release-replay') return this.releaseReplay(request.token);
    if (request.operation === 'publish' || request.operation === 'rollback') return this.publish(request);
    if (request.operation === 'invoke') return this.invoke(request);
    if (request.operation === 'delete') {
      const result = this.registry.delete({ slotId: request.slot,
        expectedEpoch: slotDecimal(request.expected_epoch, 'expected epoch'),
        expectedIncarnation: slotDecimal(request.expected_incarnation, 'expected incarnation'),
        expectedGeneration: slotDecimal(request.expected_generation, 'expected generation') });
      return slotReply(result.outcome, { epoch: result.epoch.toString() });
    }
    if (request.operation === 'reflect') {
      const module = this.modules.get(request.id);
      if (!module) return slotReply('unadmitted-site');
      return slotReply('reflected', { site: this.registry.reflectSite(module.token, request.site) });
    }
    if (request.operation === 'trace') return slotReply('trace-observed', { trace: this.trace });
    if (request.operation === 'policy') {
      const grant = request.grant;
      if (!grant || typeof grant.operation !== 'string'
        || typeof (grant.slotId ?? grant.nominalId) !== 'string'
        || typeof grant.allowed !== 'boolean') return slotReply('policy-refused');
      this.grants.set(`${grant.operation}:${grant.slotId ?? grant.nominalId}`, grant.allowed);
      return slotReply('policy-updated');
    }
    if (request.operation === 'register-resource') {
      this.registry.registerNominal(request.resource);
      return slotReply('resource-registered');
    }
    if (request.operation === 'replay') {
      const recorded = this.replays.get(request.token);
      if (!recorded || !Array.isArray(request.expected_trace)
        || !Array.isArray(request.expected_stack)
        || !request.expected_identity || typeof request.expected_identity !== 'object') {
        return slotReply('replay-refused', {
          diagnostic: 'a retained frozen owner, exact source-bound identity and effect script are required',
        });
      }
      if (!sameSlotJSON(recorded.trace, request.expected_trace)
        || !sameSlotJSON(recorded.stack, request.expected_stack)
        || !sameSlotJSON(recorded.identity, request.expected_identity)) {
        return slotReply('replay-diverged', { diagnostic: 'host frozen receipt differs from supplied replay observation' });
      }
      const effects = recorded.trace.filter(row => row.operation === 'effect');
      this.replaying = { effects, index: 0 };
      let actual;
      try {
        actual = this.invoke({ ...request, id: recorded.id, record: false,
          _frozen_snapshot: recorded.snapshot });
        if (this.replaying.index !== effects.length) {
          return slotReply('replay-diverged', {
            diagnostic: 'scripted effect sequence ended before all recorded requests were consumed',
            actual,
          });
        }
      } finally { this.replaying = false; }
      if (actual.outcome !== 'executed') return slotReply('replay-refused', { actual });
      const matched = sameSlotJSON(actual.request_trace, recorded.trace)
        && sameSlotJSON(actual.stack, recorded.stack);
      return slotReply(matched ? 'replay-matched' : 'replay-diverged',
        { token: request.token, actual_trace: actual.request_trace,
          actual_stack: actual.stack, guest_requests: actual.guest_requests,
          scripted_operations: effects.length });
    }
    return slotReply('unsupported-slot-command');
  }
}
