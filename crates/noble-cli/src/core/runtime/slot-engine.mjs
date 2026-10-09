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

function canonicalSlotJSON(value) {
  const normalized = item => {
    if (Array.isArray(item)) return item.map(normalized);
    if (item !== null && typeof item === 'object') {
      return Object.fromEntries(Object.keys(item).sort()
        .map(key => [key, normalized(item[key])]));
    }
    return item;
  };
  return JSON.stringify(normalized(value));
}

function sameSlotJSON(left, right) {
  return canonicalSlotJSON(left) === canonicalSlotJSON(right);
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
    this.nativeCoreOwner = null;
    this.handles = new Map();
    this.versions = new Map();
    this.compositeVersions = new Map();
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

  compile(bytes, expectedDigest) {
    if (!Buffer.isBuffer(bytes) || bytes.length < 8 || bytes.length > MAX_FRAME
      || sha256(bytes) !== expectedDigest) throw Error('installed binary differs from host-verified bytes');
    const policy = checkSlotBytes(bytes, this.abi);
    const module = new WebAssembly.Module(bytes);
    for (const [actual, expected] of [[WebAssembly.Module.imports(module), policy.imports],
      [WebAssembly.Module.exports(module), policy.exports]]) {
      if (actual.length !== expected.length || actual.some((entry, index) =>
        Object.keys(expected[index]).some(key => entry[key] !== expected[index][key]))) {
        throw Error('V8 disagrees with checked live-slot binary ABI');
      }
    }
    return { module, digest: expectedDigest };
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
      if (this.compositeVersions.has(decision.handle >>> 0)
        && !this.compositeCodeIsInstalled(this.compositeVersions.get(decision.handle >>> 0))) {
        throw Error('selected returned target code donor changed before dispatch');
      }
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

  install(request, binary) {
    if (!this.registry || this.modules.has(request.id) || !Buffer.isBuffer(binary)
      || binary.length < 8 || binary.length > MAX_FRAME
      || typeof request.binary_sha256 !== 'string'
      || this.sources.get(request.id) !== request.source_sha256
      || request.source_artifact_sha256 !== request.source_sha256
      || this.modules.size >= 128 || !Number.isInteger(request.code_span?.length)
      || request.code_span.length < 1
      || [...this.modules.values()].reduce((size, module) =>
        size + module.code_span.length, request.code_span.length) > this.maxResidentCode) {
      return slotReply('install-refused');
    }
    let compiled;
    try { compiled = this.compile(binary, request.binary_sha256); }
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
    const origin = request.selected_origin;
    if (origin != null && (!selected || origin.definition_index !== selected.definition_index
      || origin.definition_identity !== selected.definition_identity
      || origin.source_generation !== selected.source_generation
      || origin.selected_source_sha256 !== selected.selected_source_sha256
      || origin.root_program_index !== checked.root_program_index
      || origin.named_program_index !== selected.named_program_index
      || !Array.isArray(origin.programs) || origin.programs.length !== request.program_metadata.length
      || !Array.isArray(origin.outputs))) {
      return slotReply('install-refused', { diagnostic: 'host-selected origin differs from installed selected target' });
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
    if (origin) {
      for (const [index, program] of origin.programs.entries()) {
        if (program?.program_index !== index
          || !Array.isArray(program.operations) || !program.operations.length) {
          throw Error('selected installed Program lacks complete ordered recipe operations');
        }
        this.verifySelectedRecipe(instance.exports,
          instance.exports.cell_c(pinned[index]), program.operations, pinned,
          origin.definition_identity);
      }
    }
    const checkedEntries = new Map();
    if (origin) for (const program of origin.programs) for (const operation of program.operations) {
      const entry = operation.table_entry;
      if (!Number.isInteger(entry) || entry < span.start || entry >= span.start + span.length
        || typeof this.table.get(entry) !== 'function') {
        throw Error('selected code donor has no live checked table entry');
      }
      checkedEntries.set(entry, this.table.get(entry));
    }
    const token = this.registry.registerModule(compiled.digest, request.live_sites,
      checked.stack_in, sourceCatalog, request.source_artifact_sha256);
    for (const program of pinned) this.handles.set(program, { instance, moduleId: request.id });
    this.modules.set(request.id, { ...request, token, handle, instance, pinned,
      artifactSha256: compiled.digest, installed: true, checkedEntries,
      published: false, versionOwned: new Set(), compositeOwned: new Set(),
      replayOwned: new Set(),
      rootsReleased: false, discardRequested: false });
    if (span.generation === 1 && origin) {
      if (this.nativeCoreOwner
        || typeof this.table.get(1) !== 'function'
        || typeof this.table.get(2) !== 'function') {
        throw Error('selected first-generation native code donor is missing');
      }
      this.nativeCoreOwner = { id: request.id, entries: [this.table.get(1), this.table.get(2)] };
    }
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

  compositeCodeIsInstalled({ coreId, callerId, addEntries }) {
    const core = this.modules.get(coreId), caller = this.modules.get(callerId);
    if (!core || !caller || core.discardRequested || caller.discardRequested
      || core.rootsReleased || caller.rootsReleased
      || this.nativeCoreOwner?.id !== coreId
      || core.code_span.generation !== 1 || !core.selected_origin
      || core.binary_sha256 !== core.artifactSha256
      || caller.binary_sha256 !== caller.artifactSha256
      || this.table.get(1) !== this.nativeCoreOwner.entries[0]
      || this.table.get(2) !== this.nativeCoreOwner.entries[1]
      || !addEntries.every(entry => caller.checkedEntries.get(entry) !== undefined
        && caller.checkedEntries.get(entry) === this.table.get(entry))) return false;
    return true;
  }

  returnedManifest(owner, saved, claimed) {
    const selected = saved?.selected;
    const caller = this.modules.get(selected?.callerId);
    const core = this.modules.get(this.nativeCoreOwner?.id);
    if (!caller?.selected_origin || !core || !selected
      || selected.inputs.length !== 2
      || selected.inputs.some(input => input.kind !== 'i64'
        || !/^-?(0|[1-9][0-9]*)$/.test(input.value))
      || caller.target_metadata.stack_out[selected.position] !== 'Program([I64,I64]->[I64]!{})'
      || saved.sourceId !== null || saved.programIndex !== null) {
      throw Error('selected returned target lacks a checked two-root code donor');
    }
    const outputs = caller.selected_origin.outputs;
    if (outputs.length !== 1 || outputs[0].stack_position !== selected.position) {
      throw Error('selected returned target source occurrence is ambiguous');
    }
    const result = this.evaluateSelectedOrigin(caller, selected.inputs).get(selected.position);
    const inner = result?.left, left = inner?.left, right = inner?.right;
    const staticResult = result?.right;
    if (result?.kind !== 'compose' || inner?.kind !== 'compose-dynamic'
      || left?.kind !== 'quote-i64' || right?.kind !== 'quote-i64'
      || staticResult?.kind !== 'static-program'
      || left.operand?.kind !== 'root-i64' || right.operand?.kind !== 'root-i64'
      || ![left.operand.position, right.operand.position]
        .every(position => Number.isInteger(position) && position >= 0 && position < 2)) {
      throw Error('selected returned target has unsupported or unverified source bindings');
    }
    const named = caller.selected_origin.programs[caller.selected_origin.named_program_index];
    const staticIndex = staticResult.program_index;
    const staticProgram = caller.selected_origin.programs[staticIndex];
    const additions = staticProgram?.operations;
    if (named?.source_artifact !== 'definition' || named.effect_mask !== 0
      || !named.operations.every(operation =>
        ['dup', 'drop', 'swap', 'quote-i64', 'compose', 'static-program'].includes(operation.kind)
        && operation.effect_mask === 0)
      || staticProgram?.source_artifact !== 'definition'
      || !Number.isInteger(staticIndex) || staticIndex < 0
      || !Array.isArray(additions) || additions.length !== 3
      || additions.some((operation, ordinal) =>
        operation.kind !== 'add' || operation.effect_mask !== 0
        || operation.table_entry !== staticProgram.entry + ordinal)
      || caller.program_metadata[staticIndex]?.entry !== staticProgram.entry
      || !sameSlotJSON(caller.program_metadata[staticIndex]?.stack_in,
        ['I64', 'I64', 'I64', 'I64'])
      || !sameSlotJSON(caller.program_metadata[staticIndex]?.stack_out, ['I64'])
      || caller.program_metadata[staticIndex]?.effect_mask !== 0) {
      throw Error('selected returned target static add dependency lacks checked code');
    }
    const site = (node, kind) => {
      const matching = named.operations.filter(operation =>
        operation.kind === kind && operation.node === node.node
          && operation.table_entry === node.table_entry
          && operation.effect_mask === 0);
      if (matching.length !== 1) throw Error('selected returned target source site is ambiguous');
      return { node: matching[0].node, table_entry: matching[0].table_entry };
    };
    const quoteSites = [site(left, 'quote-i64'), site(right, 'quote-i64')];
    const composeSites = [site(inner, 'compose'), site(result, 'compose')];
    const staticSiteMatches = named.operations.filter(operation =>
      operation.kind === 'static-program' && operation.node === staticResult.node
        && operation.static_program_index === staticIndex
        && operation.effect_mask === 0);
    if (staticSiteMatches.length !== 1) {
      throw Error('selected returned target static dependency site is ambiguous');
    }
    const staticSite = { node: staticSiteMatches[0].node,
      table_entry: staticSiteMatches[0].table_entry };
    if (quoteSites[0].table_entry === quoteSites[1].table_entry
      || composeSites[0].node === composeSites[1].node
      || composeSites[0].table_entry === composeSites[1].table_entry) {
      throw Error('selected returned target reuses an unverified source occurrence');
    }
    const addEntries = additions.map(operation => operation.table_entry);
    const donor = { coreId: core.id, callerId: caller.id, addEntries };
    if (!this.compositeCodeIsInstalled(donor)
      || !addEntries.every(entry => entry >= caller.code_span.start
        && entry < caller.code_span.start + caller.code_span.length)
      || saved.moduleIds.size !== new Set([core.id, caller.id]).size
      || ![core.id, caller.id].every(id => saved.moduleIds.has(id))) {
      throw Error('selected returned target code donor changed, retired or has extra owner');
    }
    const retained = this.savedCells.get(saved.handle);
    if (!retained || retained.count < 1 || retained.runtime.cell_kind(saved.handle) !== 4) {
      throw Error('selected returned target owner was released');
    }
    this.verifySelectedGraph(retained.runtime, saved.handle, result, caller, selected.inputs);
    const graph = JSON.parse(this.captureValue(retained.runtime, saved.handle, 32, 4096));
    const recipe = JSON.parse(this.captureValue(retained.runtime,
      retained.runtime.cell_c(saved.handle), 32, 4096));
    const observed = this.observeProgramGraph(retained.runtime, saved.handle);
    if (observed.moduleIds.size !== 1 || !observed.moduleIds.has(caller.id)) {
      throw Error('selected returned target graph contains unchecked dynamic code owner');
    }
    const capturePositions = [left.operand.position, right.operand.position];
    const captures = [capturePositions[0], ...capturePositions.slice(1)
      .filter(position => position !== capturePositions[0])]
      .map(position => ({ type: 'I64', value: selected.inputs[position].value }));
    const supplied = claimed?.descriptor?.identity;
    if (!supplied || typeof supplied.definition_id !== 'string'
      || !/^anonymous-template-experimental-v2-blake3:[0-9a-f]{64}$/.test(supplied.definition_id)
      || typeof supplied.program_value_id !== 'string'
      || !/^anonymous-program-experimental-v2-blake3:[0-9a-f]{64}$/.test(supplied.program_value_id)
      || !sameSlotJSON(supplied.captures, captures)
      || !sameSlotJSON(supplied.interface, { input: ['I64', 'I64'],
        output: ['I64'], effects: [] })) {
      throw Error('selected returned target identity/captures differ from checked source and graph');
    }
    const descriptor = {
      schema: 'noble-live-slot-composite-code-donors/v1',
      owner: { token: owner, handle: saved.handle },
      identity: { definition_id: supplied.definition_id,
        program_value_id: supplied.program_value_id,
        captures, interface: { input: ['I64', 'I64'], output: ['I64'], effects: [] },
        lexical_recursive_owner: null },
      source: { caller_id: caller.id, source_sha256: caller.source_sha256,
        selected_source_sha256: caller.selected_origin.selected_source_sha256,
        definition_identity: caller.selected_origin.definition_identity,
        source_generation: caller.selected_origin.source_generation,
        quote_sites: quoteSites, compose_sites: composeSites,
        static_program_index: staticIndex, static_site: staticSite },
      donors: {
        native: { module_id: core.id, artifact_sha256: core.artifactSha256,
          source_sha256: core.source_sha256, span: { ...core.code_span }, entries: [1, 2] },
        static: { module_id: caller.id, artifact_sha256: caller.artifactSha256,
          source_sha256: caller.source_sha256, span: { ...caller.code_span },
          entries: addEntries },
      },
      graph_sha256: sha256(Buffer.from(canonicalSlotJSON(graph))),
      recipe_sha256: sha256(Buffer.from(canonicalSlotJSON(recipe))),
    };
    const manifestSha256 = sha256(Buffer.concat([
      Buffer.from('noble live-slot composite code donors v1\0'),
      Buffer.from(canonicalSlotJSON(descriptor)),
    ]));
    if (!sameSlotJSON(claimed, { descriptor, manifest_sha256: manifestSha256 })) {
      throw Error('host and worker independently checked composite code donors disagree');
    }
    return { core, caller, donor, manifestSha256, captures,
      graphSha256: descriptor.graph_sha256,
      recipeSha256: descriptor.recipe_sha256 };
  }

  returnedCandidate(saved, checked, manifest) {
    const { core, caller, donor, manifestSha256, captures, graphSha256,
      recipeSha256 } = checked;
    const handle = saved.handle;
    const runtime = this.savedCells.get(handle)?.runtime;
    const catalog = this.registry.admitResourceCatalog(caller.host_owner,
      caller.source_artifact_sha256, caller.resource_catalog, caller.source_schemas);
    if (!runtime || caller.versionOwned.has(handle)
      || runtime.retain_target(handle) !== 0) {
      throw Error('cannot retain exact returned target Program owner');
    }
    const identity = manifest.descriptor.identity;
    let version;
    try { version = this.registry.admitCandidate({
      programValueId: identity.program_value_id,
      definitionId: identity.definition_id,
      captures: captures.map(value => `${value.type}:${value.value}`),
      interface: { input: identity.interface.input, output: identity.interface.output,
        effectCeiling: identity.interface.effects },
      effects: [],
      semanticContext: `returned-target:${manifestSha256}:graph:${graphSha256}`,
      recipeSha256,
      claim: 'unverified', assumptions: 'unverified', artifactSha256: manifestSha256,
      resourceCatalog: caller.resource_catalog,
      sourceArtifactSha256: caller.source_artifact_sha256,
    }, handle, manifestSha256, catalog); }
    catch (error) {
      if (runtime.release_target(handle) !== 0) this.poisoned = true;
      throw error;
    }
    caller.versionOwned.add(handle);
    core.compositeOwned.add(handle);
    this.handles.set(handle, { instance:caller.instance, moduleId:caller.id });
    this.compositeVersions.set(handle, donor);
    return version;
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

  publishReturned(request, saved) {
    if (this.active) return slotReply('candidate-not-staged');
    if (request.program_index !== undefined || !request.target_manifest) {
      return slotReply('unadmitted-target');
    }
    let checked;
    try { checked = this.returnedManifest(request.owner, saved, request.target_manifest); }
    catch (error) {
      return slotReply('unadmitted-target', { diagnostic: String(error) });
    }
    const { caller, manifestSha256 } = checked;
    const handle = saved.handle;
    let version = this.versions.get(handle);
    if (version && (version.artifactSha256 !== manifestSha256
      || version.programValueId !== request.target_manifest.descriptor.identity.program_value_id)) {
      return slotReply('unadmitted-target',
        { diagnostic: 'retained returned Program differs from previously staged code donors' });
    }
    if (!version) {
      version = this.returnedCandidate(saved, checked, request.target_manifest);
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
    if (record.outcome === 'published') caller.published = true;
    return slotReply(record.outcome, { epoch: record.epoch.toString(),
      incarnation: record.incarnation?.toString() ?? null,
      generation: record.generation?.toString() ?? null });
  }

  publish(request) {
    if (request.owner !== undefined && request.id !== undefined) return slotReply('publication-refused');
    const saved = request.owner === undefined ? null : this.savedOwners.get(request.owner);
    if (request.proof_required === true || request.interface?.proofRequired === true) {
      return slotReply('selected-target-evidence-refused', {
        diagnostic: 'a pinned independent Lean checker receipt bound to exact selected target and installed code donors is unavailable',
      });
    }
    if (saved && saved.programIndex === null) return this.publishReturned(request, saved);
    const module = this.modules.get(saved ? saved.sourceId : request.id);
    if (!module || module.discardRequested || request.target_manifest !== undefined) {
      return slotReply('unadmitted-target');
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

  bootstrap(request) {
    if (this.active || !Array.isArray(request.members)
      || (request.members.length !== 2 && request.members.length !== 3)) {
      return slotReply('bootstrap-refused', { epoch: this.registry.epoch.toString() });
    }
    if (this.registry.epoch !== 0n) {
      return slotReply('bootstrap-unavailable', { epoch: this.registry.epoch.toString() });
    }
    // A refused batch may leave independently admitted candidates staged, but
    // no registry member is visible until the registry validates the whole map.
    const members = [];
    const modules = [];
    for (const member of request.members) {
      if (!member || typeof member.slot !== 'string' || typeof member.id !== 'string'
        || member.owner !== undefined || member.expected_incarnation !== undefined
        || member.expected_generation !== undefined
        || member.proof_required === true || member.interface?.proofRequired === true) {
        return slotReply('bootstrap-refused', { epoch: this.registry.epoch.toString() });
      }
      const module = this.modules.get(member.id);
      const index = member.program_index ?? module?.target_metadata.root_program_index;
      if (!module || module.discardRequested || !Number.isInteger(index)
        || index < 0 || index >= module.pinned.length) {
        return slotReply('unadmitted-target', { epoch: this.registry.epoch.toString() });
      }
      const handle = module.pinned[index];
      let version = this.versions.get(handle);
      if (!version) {
        version = this.candidate(module, handle);
        this.versions.set(handle, version);
      }
      members.push({ slotId: member.slot, interface: member.interface, version });
      modules.push(module);
    }
    const record = this.registry.bootstrap({
      expectedEpoch: slotDecimal(request.expected_epoch, 'expected epoch'), members });
    if (record.outcome === 'bootstrapped') for (const module of modules) module.published = true;
    return slotReply(record.outcome, { epoch: record.epoch.toString(),
      ...(record.slots === undefined ? {} : { slots: record.slots.map(slot =>
        ({ slot: slot.slot, incarnation: slot.incarnation.toString(),
          generation: slot.generation.toString() })) }) });
  }

  retireTarget(handle) {
    const owner = this.handles.get(handle >>> 0);
    const module = this.modules.get(owner?.moduleId);
    if (!module || !module.versionOwned.has(handle >>> 0)
      || owner.instance.exports.release_target(handle >>> 0) !== 0) return false;
    module.versionOwned.delete(handle >>> 0);
    const composite = this.compositeVersions.get(handle >>> 0);
    if (composite) {
      this.modules.get(composite.coreId)?.compositeOwned.delete(handle >>> 0);
      this.compositeVersions.delete(handle >>> 0);
      this.handles.delete(handle >>> 0);
    }
    this.versions.delete(handle >>> 0);
    this.sweepRetirement(module);
    if (composite) {
      const core = this.modules.get(composite.coreId);
      if (core && core !== module) this.sweepRetirement(core);
    }
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
      || module.versionOwned.size || module.compositeOwned.size
      || module.replayOwned.size) return;
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
      || module.replayOwned.size || module.compositeOwned.size || this.active
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
      || module.compositeOwned.size
      || [...this.savedOwners.values()].some(owner => owner.moduleIds.has(module.id))
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

  evaluateSelectedOrigin(module, inputs) {
    const origin = module.selected_origin;
    if (!origin) return null;
    if (inputs.length !== module.target_metadata.stack_in.length
      || inputs.some(input => input.kind !== 'i64' || !/^-?(0|[1-9][0-9]*)$/.test(input.value))) {
      throw Error('selected origin requires exact I64 root operands');
    }
    const program = origin.programs[origin.named_program_index];
    if (program?.program_index !== origin.named_program_index) {
      throw Error('selected origin has no exact named program');
    }
    const stack = inputs.map((input, position) =>
      ({ kind: 'scalar', value: BigInt(input.value), operand: { kind: 'root-i64', position } }));
    const pop = kind => {
      const item = stack.pop();
      if (item?.kind !== kind) throw Error('selected origin operand kind/order differs');
      return item;
    };
    for (const op of program.operations) {
      const fixed = value => ({ kind: 'scalar', value: BigInt.asIntN(64, value),
        operand: { kind: 'fixed-i64', value: BigInt.asIntN(64, value).toString() } });
      switch (op.kind) {
        case 'i64': stack.push(fixed(BigInt(op.value))); break;
        case 'dup': {
          const top = stack.at(-1);
          if (!top || top.kind !== 'scalar' && top.result?.kind !== 'static-program') {
            throw Error('selected dup lacks copyable scalar/static operand');
          }
          stack.push(top); break;
        }
        case 'drop': {
          if (!stack.length) throw Error('selected drop lacks operand');
          stack.pop(); break;
        }
        case 'swap': {
          if (stack.length < 2) throw Error('selected swap lacks operands');
          const right = stack.pop(), left = stack.pop();
          stack.push(right, left); break;
        }
        case 'add': case 'sub': case 'mul': {
          const right = pop('scalar'), left = pop('scalar');
          if (left.operand.kind !== 'fixed-i64' || right.operand.kind !== 'fixed-i64') {
            throw Error('unclassified selected root arithmetic cannot acquire origin');
          }
          stack.push(fixed(op.kind === 'add' ? left.value + right.value
            : op.kind === 'sub' ? left.value - right.value : left.value * right.value));
          break;
        }
        case 'quote-i64': {
          const input = pop('scalar');
          stack.push({ kind: 'program', result: { kind: 'quote-i64', node: op.node,
            table_entry: op.table_entry, operand: input.operand }, op });
          break;
        }
        case 'static-program':
          stack.push({ kind: 'program', result: { kind: 'static-program',
            node: op.node, program_index: op.static_program_index }, op });
          break;
        case 'compose': {
          const right = pop('program'), left = pop('program');
          const dynamic = left.result.kind === 'quote-i64'
            && right.result.kind === 'quote-i64'
            && left.result.operand.kind === 'root-i64'
            && right.result.operand.kind === 'root-i64';
          if (!dynamic && (right.result.kind !== 'static-program'
            || !['quote-i64', 'compose', 'compose-dynamic'].includes(left.result.kind))) {
            throw Error('selected compose has unsupported operand lineage');
          }
          stack.push({ kind: 'program', result: { kind: dynamic ? 'compose-dynamic' : 'compose',
            node: op.node, table_entry: op.table_entry, left: left.result,
            right: dynamic ? right.result : { kind: 'static-program', node: right.result.node,
              program_index: right.result.program_index } }, op });
          break;
        }
        default: throw Error('unclassified selected operation cannot acquire origin');
      }
    }
    const expected = new Map(origin.outputs.map(row => [row.stack_position, row.result]));
    if (expected.size !== origin.outputs.length || stack.length !== module.target_metadata.stack_out.length
      || stack.some((cell, position) => (cell.kind === 'program') !== expected.has(position)
        || cell.kind === 'program' && !sameSlotJSON(cell.result, expected.get(position)))) {
      throw Error('selected output lineage differs from independently checked source actions');
    }
    return expected;
  }

  verifySelectedRecipe(runtime, root, operations, pinned, definitionIdentity) {
    const check = (id, kind) => {
      if (!Number.isInteger(id) || id < 4096 || runtime.cell_kind(id) !== kind) {
        throw Error('selected recipe contains stale or differently typed epoch handle');
      }
    };
    const word = { add: 4, sub: 5, mul: 6, dup: 0, drop: 1, swap: 2, compose: 9 };
    const checkRange = (id, end, lastIndex = 0) => {
      if (end === 1) {
        const op = operations[lastIndex];
        check(id, 8);
        const tag = op.kind === 'i64' ? 1 : op.kind === 'static-program' ? 3
          : op.kind === 'call-selected' ? 15 : 2;
        const value = op.kind === 'i64' ? BigInt(op.value)
          : op.kind === 'call-selected' ? BigInt.asIntN(64, BigInt(definitionIdentity))
          : tag === 3 ? 0n : BigInt(op.kind === 'quote-i64' ? 8 : word[op.kind]);
        if (runtime.cell_x(id) !== tag || runtime.cell_payload(id) !== value
          || runtime.cell_y(id) !== (tag === 2 || tag === 15 ? op.input_signature : 0)
          || runtime.cell_z(id) !== (tag === 2 || tag === 15 ? op.output_signature : 0)
          || runtime.cell_w(id) !== (tag === 2 || tag === 15 ? op.effect_mask : 0)
          || runtime.cell_n(id) !== 0 || runtime.cell_b(id) || runtime.cell_c(id)
          || runtime.cell_a(id) !== (tag === 3 ? pinned[op.static_program_index] : 0)) {
          throw Error('selected installed recipe atom differs from checked source operation');
        }
        return;
      }
      check(id, 9);
      if (runtime.cell_payload(id) !== 0n || runtime.cell_x(id) || runtime.cell_y(id)
        || runtime.cell_z(id) || runtime.cell_w(id) || runtime.cell_n(id) || runtime.cell_c(id)) {
        throw Error('selected recipe concatenation has extra state');
      }
      checkRange(runtime.cell_a(id), end - 1);
      const last = runtime.cell_b(id);
      check(last, 8);
      // Reuse the one-atom validation without slicing or copying the checked
      // operation list for every node of a long left-associated recipe.
      checkRange(last, 1, end - 1);
    };
    checkRange(root, operations.length);
  }

  verifySelectedGraph(runtime, handle, result, module, inputs) {
    const cell = (id, kind) => {
      if (!Number.isInteger(id) || id < 4096 || runtime.cell_kind(id) !== kind) {
        throw Error('selected origin graph contains stale or differently typed epoch handle');
      }
    };
    const walk = (id, description) => {
      if (description.kind === 'quote-i64') {
        cell(id, 4);
        const op = module.selected_origin.programs[module.selected_origin.named_program_index]
          .operations.find(row => row.table_entry === description.table_entry && row.node === description.node);
        if (op?.kind !== 'quote-i64' || runtime.cell_x(id) !== 1
          || runtime.cell_y(id) !== op.quote_input_signature
          || runtime.cell_z(id) !== op.quote_output_signature
          || runtime.cell_payload(id) !== 0n || runtime.cell_b(id)
          || runtime.cell_w(id) !== 1 || runtime.cell_n(id) !== 1) {
          throw Error('selected quote differs from exact source site/signatures');
        }
        const captured = runtime.cell_a(id), atom = runtime.cell_c(id);
        cell(captured, 1); cell(atom, 8);
        const operand = description.operand;
        const value = operand.kind === 'root-i64'
          ? inputs[operand.position]?.value : operand.value;
        if (value === undefined || runtime.cell_payload(captured) !== BigInt(value)
          || runtime.cell_a(captured) || runtime.cell_b(captured) || runtime.cell_c(captured)
          || runtime.cell_x(captured) || runtime.cell_y(captured) || runtime.cell_z(captured)
          || runtime.cell_w(captured) || runtime.cell_n(captured)
          || runtime.cell_x(atom) !== 1 || runtime.cell_payload(atom) !== BigInt(value)
          || runtime.cell_a(atom) || runtime.cell_b(atom) || runtime.cell_c(atom)
          || runtime.cell_y(atom) || runtime.cell_z(atom) || runtime.cell_w(atom) || runtime.cell_n(atom)) {
          throw Error('selected quote captures wrong ordered root/fixed I64 operand');
        }
        return;
      }
      if (description.kind === 'static-program') {
        const selected = module.pinned[description.program_index];
        cell(id, 4);
        if (id !== selected || this.handles.get(id)?.moduleId !== module.id) {
          throw Error('selected static child is not the exact pinned installed Program');
        }
        const source = module.selected_origin.programs[description.program_index];
        if (source?.program_index !== description.program_index || source.source_artifact !== 'definition') {
          throw Error('selected static child lacks exact checked source body');
        }
        this.verifySelectedRecipe(runtime, runtime.cell_c(id), source.operations, module.pinned,
          module.selected_origin.definition_identity);
        return;
      }
      if (!['compose', 'compose-dynamic'].includes(description.kind)) {
        throw Error('unsupported selected output graph');
      }
      cell(id, 4);
      const composeOp = module.selected_origin.programs[module.selected_origin.named_program_index]
        .operations.find(row => row.table_entry === description.table_entry && row.node === description.node);
      if (composeOp?.kind !== 'compose') {
        throw Error('selected compose differs from checked source site');
      }
      const left = runtime.cell_a(id), right = runtime.cell_b(id);
      walk(left, description.left); walk(right, description.right);
      const lRecipe = runtime.cell_c(left), rRecipe = runtime.cell_c(right);
      const combined = runtime.cell_c(id);
      cell(combined, 9);
      if (runtime.cell_x(id) !== 2 || runtime.cell_payload(id) !== 0n
        || runtime.cell_y(id) !== runtime.cell_y(left)
        || runtime.cell_z(id) !== runtime.cell_z(right)
        || runtime.cell_w(id) !== Math.max(runtime.cell_w(left), runtime.cell_w(right)) + 1
        || runtime.cell_n(id) !== runtime.cell_n(left) + runtime.cell_n(right)
        || runtime.cell_a(combined) !== lRecipe || runtime.cell_b(combined) !== rRecipe
        || runtime.cell_c(combined) || runtime.cell_payload(combined) !== 0n
        || runtime.cell_x(combined) || runtime.cell_y(combined) || runtime.cell_z(combined)
        || runtime.cell_w(combined) || runtime.cell_n(combined)) {
        throw Error('selected compose child or recipe graph differs from checked operand lineage');
      }
    };
    walk(handle, result);
  }

  saveOutput(runtime, index, selectedResult = null, rootInputs = null) {
    const handle = Number(runtime.stack_value(index)) >>> 0;
    if (!handle || runtime.cell_kind(handle) !== 4) {
      throw Error('backend returned no live Program stack cell');
    }
    if (selectedResult !== null) {
      this.verifySelectedGraph(runtime, handle, selectedResult, this.active.module, rootInputs);
    }
    if (this.nextSavedOwner === Number.MAX_SAFE_INTEGER) throw Error('saved Program owner exhausted');
    const graph = this.observeProgramGraph(runtime, handle);
    const aliasResult = selectedResult?.kind === 'compose'
      && selectedResult.left?.kind === 'compose-dynamic';
    if (aliasResult && this.nativeCoreOwner) graph.moduleIds.add(this.nativeCoreOwner.id);
    const prior = this.savedCells.get(handle);
    if (prior && prior.runtime !== runtime) {
      this.poisoned = true;
      throw Error('saved backend Program owner crossed an untrusted runtime boundary');
    }
    const installed = this.handles.get(handle);
    const module = this.modules.get(installed?.moduleId);
    const programIndex = module?.pinned.indexOf(handle) ?? -1;
    let observedCaptures = null;
    if (programIndex < 0) {
      const entry = runtime.cell_x(handle);
      if (entry === 1) {
        const recipe = runtime.cell_c(handle) >>> 0;
        const captured = runtime.cell_a(handle) >>> 0;
        if (!captured || runtime.cell_kind(captured) !== 1
          || runtime.cell_a(captured) || runtime.cell_b(captured) || runtime.cell_c(captured)
          || runtime.cell_b(handle) || runtime.cell_payload(handle) !== 0n
          || !recipe
          || runtime.cell_kind(recipe) !== 8 || runtime.cell_x(recipe) !== 1
          || runtime.cell_a(recipe) || runtime.cell_b(recipe) || runtime.cell_c(recipe)
          || runtime.cell_payload(recipe) !== runtime.cell_payload(captured)
          || runtime.cell_w(handle) !== 1 || runtime.cell_n(handle) !== 1) {
          throw Error('dynamic I64 quote differs from checked backend capture layout');
        }
        observedCaptures = [{ type: 'I64', value: runtime.cell_payload(captured).toString() }];
      } else {
        observedCaptures = ['cell_a', 'cell_b'].flatMap(field => {
          const child = runtime[field](handle) >>> 0;
          return child ? [{ field, value: JSON.parse(this.captureValue(runtime, child,
            64, aliasResult ? 4096 : 1000)) }] : [];
        });
      }
    }
    // All graph and capture observations must finish before saveProgramGraph
    // acquires a backend root and charges the registry's retained versions.
    this.saving = { runtime, index, expectedHandle: handle };
    let registryOwner;
    try { registryOwner = this.registry.saveProgramGraph(handle, graph.versions); }
    finally { this.saving = null; }
    this.savedCells.set(handle, { runtime, count: (prior?.count ?? 0) + 1 });
    const token = `program-${++this.nextSavedOwner}`;
    this.savedOwners.set(token, { handle, moduleIds: graph.moduleIds, registryOwner,
      sourceId: programIndex < 0 ? null : module.id,
      programIndex: programIndex < 0 ? null : programIndex,
      selected: aliasResult ? { callerId: this.active.module.id,
        position:index, inputs:rootInputs.map(input => ({ ...input })) } : null });
    return { kind: 4, owner: token, source_id: programIndex < 0 ? null : module.id,
      program_index: programIndex < 0 ? null : programIndex,
      ...(selectedResult === null ? {} : { verified_origin: {
        caller_id: this.active.module.id, artifact_sha256: this.active.module.artifactSha256,
        stack_position: index, definition_identity: this.active.module.selected_origin.definition_identity,
        source_generation: this.active.module.selected_origin.source_generation,
        saved_handle: handle } }),
      ...(programIndex < 0 ? { capture_status: selectedResult === null
        ? 'backend-observed-source-occurrence-unavailable' : 'checked-selected-origin',
        capture_values: observedCaptures } : {}) };
  }

  inspectSavedProgram(owner) {
    // This internal host request runs after the root has saved and cleared its
    // stack. Re-read the complete graph through the retained owner, rather
    // than treating the source plan or two child summaries as the actual
    // returned target. Rust checks every node and edge before attaching D/P.
    const saved = this.savedOwners.get(owner);
    const retained = this.savedCells.get(saved?.handle);
    if (!saved || !retained || retained.count < 1
      || retained.runtime.cell_kind(saved.handle) !== 4) {
      return slotReply('saved-program-inspection-refused');
    }
    return slotReply('saved-program-inspected', { owner, saved_handle: saved.handle,
      selected_graph: JSON.parse(this.captureValue(retained.runtime, saved.handle, 32, 4096)) });
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
    const root = this.registry.pinRoot(module.handle, module.token,
      request._frozen_snapshot ?? null, request.inputs, request.refs);
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
      const originInputs = module.selected_origin ? request.inputs.map((input, index) => {
        if (input.kind !== 'i64' || runtime.stack_kind(index) !== 1
          || runtime.stack_value(index) !== BigInt(input.value)) {
          throw Error('actual injected root operand differs from selected I64 source lineage');
        }
        return { kind: 'i64', value: runtime.stack_value(index).toString() };
      }) : null;
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
      const selectedOutputs = originInputs === null ? null : this.evaluateSelectedOrigin(module, originInputs);
      const stack = [];
      for (let index = 0; index < count; index++) {
        const kind = runtime.stack_kind(index);
        if (kind === 4) {
          if (selectedOutputs && !selectedOutputs.has(index)) {
            throw Error('selected Program output lacks checked origin lineage');
          }
          const saved = this.saveOutput(runtime, index,
            selectedOutputs?.get(index) ?? null, originInputs);
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
        try { snapshot = this.registry.freezeRoot(root, request.inputs, request.refs); }
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

  request(request, binary) {
    if (!request || typeof request !== 'object' || Array.isArray(request)
      || typeof request.operation !== 'string' || this.poisoned) {
      return slotReply('session-refused');
    }
    if (request.operation === 'configure') return this.configure(request);
    if (!this.registry) return slotReply('authority-required');
    if (request.operation !== 'install' && binary.length !== 0) return slotReply('unexpected-module-frame');
    if (request.operation === 'control-barrier') {
      return slotReply('control-barrier', { control_events: this.controlEvents.splice(0) });
    }
    if (request.operation === 'install') return this.install(request, binary);
    if (request.operation === 'candidate') return this.stageCandidate(request);
    if (request.operation === 'discard') return this.discard(request);
    if (request.operation === 'retire-code') return this.retireCode(request);
    if (request.operation === 'inspect-saved-program')
      return this.inspectSavedProgram(request.owner);
    if (request.operation === 'release-program') return this.releaseProgram(request.owner);
    if (request.operation === 'release-replay') return this.releaseReplay(request.token);
    if (request.operation === 'publish' || request.operation === 'rollback') return this.publish(request);
    if (request.operation === 'bootstrap') return this.bootstrap(request);
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
      const module = this.modules.get(recorded.id);
      if (!module || !this.registry.frozenRootMatches(recorded.snapshot,
        module.handle, module.token, request.inputs, request.refs)) {
        return slotReply('replay-diverged', {
          diagnostic: 'host-issued typed root inputs or ordered borrowed bindings differ from frozen root',
        });
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
