// Host-owned live-slot state. This module is not loaded by Core or Live-Wasm-Draft.
// Compiler-produced metadata and installed handles enter only through the trusted
// worker; source text, slot IDs, references, and evidence are never authority.
const SLOT_LIMIT = 128;
const TRACE_LIMIT = 4096;
const ROOT_TRACE_BYTES = 1024 * 1024;
const U64_MAX = 0xffffffffffffffffn;
const FRAME_START = 1077248;
const FRAME_END = 1310720;
const ROOT_BINDING_COUNT = 1050632;

function slotInteger(value, label) {
  if (typeof value !== 'bigint' || value < 0n || value > U64_MAX) throw Error(`invalid ${label}`);
  return value;
}

function slotName(value) {
  if (typeof value !== 'string' || !value || Buffer.byteLength(value, 'utf8') > SLOT_LIMIT
    || value.includes('\0')) throw Error('invalid slot identity');
  return value;
}

function strings(value, label) {
  if (!Array.isArray(value) || value.length > 128
    || value.some(item => typeof item !== 'string' || !item || Buffer.byteLength(item) > 1024)) {
    throw Error(`invalid ${label}`);
  }
  return Object.freeze(value.slice());
}

function same(left, right) {
  return left.length === right.length && left.every((item, index) => item === right[index]);
}

function field(value, label) {
  if (typeof value !== 'string' || !value || Buffer.byteLength(value) > 4096) {
    throw Error(`invalid ${label}`);
  }
  return value;
}

// Publication copies a bounded map; root entry retains its immutable map
// reference in O(1). No method mutates a map after installing it as current.
function mapValues(map, result = new Set()) {
  for (const entry of map.values()) result.add(entry.version);
  return result;
}

function contract(value) {
  if (!value || typeof value !== 'object') throw Error('missing checked slot contract');
  if (value.proofRequired !== undefined && typeof value.proofRequired !== 'boolean') {
    throw Error('invalid proof-required policy');
  }
  const input = strings(value.input, 'ordered input interface');
  const output = strings(value.output, 'ordered output interface');
  const ceiling = strings(value.effectCeiling, 'effect ceiling');
  if (new Set(ceiling).size !== ceiling.length) throw Error('duplicate effect ceiling');
  return Object.freeze({ input, output, ceiling, proofRequired: value.proofRequired === true });
}

function exactEvidence(evidence, version) {
  return evidence && evidence.programValueId === version.programValueId
    && evidence.definitionId === version.definitionId
    && same(evidence.captures ?? [], version.captures)
    && evidence.semanticContext === version.semanticContext
    && evidence.claim === version.claim
    && evidence.assumptions === version.assumptions
    && evidence.artifactSha256 === version.artifactSha256;
}

class CommittedRetirementError extends Error {
  constructor(transition, epoch, cause) {
    super(`live-slot ${transition} committed at epoch ${epoch}; backend retirement failed: ${cause}`);
    this.committed = true;
    this.epoch = epoch;
    this.transition = transition;
  }
}

class TraceCapacityError extends Error {}

export class SlotRegistry {
  #current = Object.freeze({ epoch: 0n, map: new Map() });
  #incarnations = new Map();
  #versions = new Map();
  #roots = new Map();
  #frozen = new Map();
  #saved = new Map();
  #savedTokens = new WeakSet();
  #staged = new Set();
  #modules = new WeakMap();
  #nominals = new Map();
  #catalogs = new WeakMap();
  #nextRoot = 0n;
  #nextFrame = 0;
  #poisoned = false;
  #authorize;
  #performEffect;
  #previewEffect;
  #retireTarget;
  #retainProgram;
  #releaseProgramBackend;
  #verifyEvidence;
  #knownEffects;
  #quota;
  #maxSlots;
  #traceAdmission;

  constructor({ quota, knownEffects, authorize, performEffect, previewEffect = null, retireTarget,
    retainProgram = null, releaseProgramBackend = null, verifyEvidence = null, maxSlots = 128,
    traceAdmission = null }) {
    if (!Number.isSafeInteger(quota) || quota < 1 || quota > 16384
      || !Number.isSafeInteger(maxSlots) || maxSlots < 1 || maxSlots > 16384
      || typeof authorize !== 'function' || typeof performEffect !== 'function'
      || previewEffect !== null && typeof previewEffect !== 'function'
      || typeof retireTarget !== 'function'
      || verifyEvidence !== null && typeof verifyEvidence !== 'function'
      || traceAdmission !== null && typeof traceAdmission !== 'function') {
      throw Error('invalid live-slot host services');
    }
    this.#quota = quota;
    this.#maxSlots = maxSlots;
    this.#knownEffects = new Set(strings(knownEffects, 'known host contracts'));
    this.#authorize = authorize;
    this.#performEffect = performEffect;
    this.#previewEffect = previewEffect;
    this.#retireTarget = retireTarget;
    this.#retainProgram = retainProgram;
    this.#releaseProgramBackend = releaseProgramBackend;
    this.#verifyEvidence = verifyEvidence;
    this.#traceAdmission = traceAdmission;
  }

  get epoch() { return this.#current.epoch; }
  get retainedVersions() { return this.#reachable().size; }

  #check() { if (this.#poisoned) throw Error('live-slot backend retirement failed'); }
  #allowed(request) { return this.#authorize(Object.freeze(request)) === true; }

  // Registrations are host policy, never imported from Submission.environment.
  // A changed schema/owner gets a NEW module identity; an old identity cannot
  // be silently rebound while pinned callers still refer to it.
  registerNominal({ module, ordinal, kind, shapeFingerprint, schemaFingerprint,
    owner, sourceArtifactSha256s, interfaceDescriptor }) {
    this.#check();
    const sources = strings(sourceArtifactSha256s, 'host-selected source hashes');
    if (typeof module !== 'string' || !/^(0|[1-9][0-9]*)$/.test(module)
      || BigInt(module) > U64_MAX || !Number.isInteger(ordinal) || ordinal < 0
      || ordinal > 0xffffffff || !Number.isInteger(kind) || kind < 1 || kind > 0xffffffff
      || !/^[0-9a-f]{64}$/.test(shapeFingerprint ?? '')
      || !/^[0-9a-f]{64}$/.test(schemaFingerprint ?? '')
      || !sources.length || new Set(sources).size !== sources.length
      || sources.some(source => !/^[0-9a-f]{64}$/.test(source))) {
      throw Error('invalid host nominal registration');
    }
    const id = `${module}:${ordinal}`;
    const registration = Object.freeze({ module, ordinal, kind, shapeFingerprint,
      schemaFingerprint, owner: field(owner, 'resource owner'),
      sourceArtifactSha256s: sources,
      interfaceDescriptor: field(interfaceDescriptor, 'nominal interface') });
    if (this.#nominals.has(id) || !this.#allowed({ operation: 'register-resource',
      nominalId: id, owner: registration.owner })) {
      throw Error('duplicate or unauthorized host nominal registration');
    }
    this.#nominals.set(id, registration);
  }

  // The CLI supplies the independently checked compiler catalog plus exact
  // selected source artifact and host owner. A self-consistent guest Env alone
  // cannot mint this token or replace the host's registered schema.
  admitResourceCatalog(owner, sourceArtifactSha256, checkedRows, sourceSchemas) {
    this.#check();
    field(owner, 'resource owner');
    if (!/^[0-9a-f]{64}$/.test(sourceArtifactSha256 ?? '')
      || !Array.isArray(checkedRows) || checkedRows.length > 128
      || !Array.isArray(sourceSchemas) || sourceSchemas.length !== checkedRows.length) {
      throw Error('invalid checked resource catalog');
    }
    const seen = new Set();
    const rows = [];
    for (const [index, row] of checkedRows.entries()) {
      const id = `${row?.module}:${row?.ordinal}`;
      const host = this.#nominals.get(id);
      const source = sourceSchemas[index];
      if (!host || seen.has(id) || host.kind !== row.kind
        || host.shapeFingerprint !== row.shapeFingerprint
        || host.owner !== owner || !host.sourceArtifactSha256s.includes(sourceArtifactSha256)
        || source?.module !== host.module || source?.ordinal !== host.ordinal
        || source?.schemaFingerprint !== host.schemaFingerprint) {
        throw Error('unregistered owner/schema/resource kind');
      }
      seen.add(id);
      rows.push(Object.freeze({ module: host.module, ordinal: host.ordinal,
        kind: host.kind, shapeFingerprint: host.shapeFingerprint }));
    }
    const token = Object.freeze({});
    this.#catalogs.set(token, Object.freeze({ owner, sourceArtifactSha256,
      rows: Object.freeze(rows) }));
    return token;
  }

  // The caller MUST pass metadata from the independently checked compiler and
  // the artifact actually admitted into the same worker. This is an internal
  // host API, not a guest-facing JSON command or a proof checker.
  admitCandidate(checked, installedHandle, artifactSha256, catalog = null) {
    this.#check();
    if (!checked || typeof checked !== 'object'
      || !Number.isSafeInteger(installedHandle) || installedHandle <= 0 || installedHandle > 0xffffffff
      || typeof artifactSha256 !== 'string' || !/^[0-9a-f]{64}$/.test(artifactSha256)
      || checked.artifactSha256 !== artifactSha256) throw Error('candidate artifact mismatch');
    if (!Array.isArray(checked.resourceCatalog)
      || checked.resourceCatalog.length > 128
      || checked.resourceCatalog.length !== (this.#catalogs.get(catalog)?.rows.length ?? 0)
      || checked.resourceCatalog.some((row, index) => {
        const admitted = this.#catalogs.get(catalog)?.rows[index];
        return !admitted || row.module !== admitted.module || row.ordinal !== admitted.ordinal
          || row.kind !== admitted.kind || row.shapeFingerprint !== admitted.shapeFingerprint;
      })
      || (catalog !== null
        && checked.sourceArtifactSha256 !== this.#catalogs.get(catalog)?.sourceArtifactSha256)) {
      throw Error('candidate lacks independently admitted nominal catalog');
    }
    const interfaceContract = contract(checked.interface);
    const effects = strings(checked.effects, 'candidate effects');
    if (new Set(effects).size !== effects.length
      || effects.some(effect => !this.#knownEffects.has(effect))) {
      throw Error('unavailable host effect contract');
    }
    const captures = strings(checked.captures, 'exact captures');
    const recipeSha256 = checked.recipeSha256;
    if (typeof recipeSha256 !== 'string' || !/^[0-9a-f]{64}$/.test(recipeSha256)) {
      throw Error('missing checked recipe digest');
    }
    const version = Object.freeze({
      programValueId: field(checked.programValueId, 'ProgramValueId'),
      definitionId: checked.definitionId === null
        ? null : field(checked.definitionId, 'DefinitionId'),
      captures, input: interfaceContract.input, output: interfaceContract.output,
      effects, semanticContext: field(checked.semanticContext, 'semantic context'),
      recipeSha256,
      claim: field(checked.claim, 'claim'), assumptions: field(checked.assumptions, 'assumptions'),
      artifactSha256, handle: installedHandle,
    });
    const key = JSON.stringify([version.programValueId, version.definitionId, version.captures,
      version.input, version.output, version.effects, version.semanticContext,
      version.recipeSha256, version.artifactSha256]);
    if (this.#versions.has(key)
      || [...this.#versions.values()].some(other => other.handle === installedHandle)) {
      throw Error('duplicate installed candidate identity or handle');
    }
    this.#versions.set(key, version);
    this.#staged.add(version);
    return version;
  }

  // Refused candidates still own physical installed handles, independently of
  // the logical retained-version quota. An explicit discard releases one.
  discardCandidate(version) {
    this.#check();
    if (!this.#staged.delete(version)) throw Error('candidate is not staged');
    this.#retire('candidate-discard');
  }

  #candidate(version, required, evidence) {
    if (!version || this.#versions.get(this.#versionKey(version)) !== version) {
      return 'unadmitted-target';
    }
    if (!same(version.input, required.input) || !same(version.output, required.output)) {
      return 'ordered-interface-mismatch';
    }
    if (version.effects.some(effect => !required.ceiling.includes(effect))) return 'effect-ceiling-widened';
    if (required.proofRequired && (version.definitionId === null
      || !exactEvidence(evidence, version)
      || !this.#verifyEvidence || this.#verifyEvidence(evidence, version) !== true)) {
      return 'selected-target-evidence-refused';
    }
    return null;
  }

  #versionKey(version) {
    return JSON.stringify([version.programValueId, version.definitionId, version.captures,
      version.input, version.output, version.effects, version.semanticContext,
      version.recipeSha256, version.artifactSha256]);
  }

  #reachable(next = this.#current.map) {
    const versions = mapValues(next);
    for (const root of this.#roots.values()) mapValues(root.state.map, versions);
    for (const frozen of this.#frozen.values()) mapValues(frozen.state.map, versions);
    for (const [version, owners] of this.#saved) if (owners > 0) versions.add(version);
    return versions;
  }

  #retire(transition) {
    const reachable = this.#reachable();
    for (const staged of this.#staged) reachable.add(staged);
    for (const [key, version] of this.#versions) {
      if (reachable.has(version)) continue;
      try {
        if (this.#retireTarget(version.handle, version) !== true) throw Error('backend refused release');
      } catch (error) {
        this.#poisoned = true;
        throw new CommittedRetirementError(transition, this.epoch, error);
      }
      this.#versions.delete(key);
    }
  }

  // A new slot and a replacement both commit by the SAME global epoch CAS.
  // Recreating a deleted ID always receives a new incarnation.
  publish({ slotId, expectedEpoch, expectedIncarnation = null, expectedGeneration = null,
    version, interface: selectedContract, evidence = null, operation = 'publish' }) {
    this.#check();
    if (operation !== 'publish' && operation !== 'rollback') throw Error('invalid publication operation');
    const id = slotName(slotId);
    slotInteger(expectedEpoch, 'expected global epoch');
    if (expectedIncarnation !== null) slotInteger(expectedIncarnation, 'expected incarnation');
    if (expectedGeneration !== null) slotInteger(expectedGeneration, 'expected slot generation');
    const previous = this.#current.map.get(id);
    if (expectedEpoch !== this.#current.epoch
      || (previous
        ? expectedIncarnation !== previous.incarnation || expectedGeneration !== previous.generation
        : expectedIncarnation !== null || expectedGeneration !== null)) {
      return { outcome: 'stale-reject', epoch: this.epoch };
    }
    const required = previous?.contract ?? contract(selectedContract);
    if (previous && selectedContract !== undefined) {
      const offered = contract(selectedContract);
      if (!same(offered.input, required.input) || !same(offered.output, required.output)
        || !same(offered.ceiling, required.ceiling) || offered.proofRequired !== required.proofRequired) {
        return { outcome: 'contract-mismatch', epoch: this.epoch };
      }
    }
    const refusal = this.#candidate(version, required, evidence);
    if (refusal) return { outcome: refusal, epoch: this.epoch };
    if (!previous && this.#current.map.size >= this.#maxSlots) {
      return { outcome: 'slot-budget-refused', epoch: this.epoch };
    }
    if (!this.#allowed({ operation, slotId: id, version })) {
      return { outcome: 'policy-denied', epoch: this.epoch };
    }
    if (this.epoch === U64_MAX || previous?.generation === U64_MAX) {
      return { outcome: 'generation-exhausted', epoch: this.epoch };
    }
    const incarnation = previous?.incarnation ?? (this.#incarnations.get(id) ?? 0n) + 1n;
    if (incarnation > U64_MAX) return { outcome: 'incarnation-exhausted', epoch: this.epoch };
    const generation = previous ? previous.generation + 1n : 1n;
    const nextMap = new Map(this.#current.map);
    const admittedEvidence = evidence === null ? null
      : Object.freeze({ ...evidence, captures: Object.freeze(evidence.captures.slice()) });
    nextMap.set(id, Object.freeze({ id, incarnation, generation, version,
      contract: required, evidence: admittedEvidence }));
    if (this.#reachable(nextMap).size > this.#quota) {
      return { outcome: 'retention-budget-refused', epoch: this.epoch };
    }
    this.#current = Object.freeze({ epoch: this.epoch + 1n, map: nextMap });
    this.#incarnations.set(id, incarnation);
    this.#staged.delete(version);
    this.#retire('publication');
    return { outcome: 'published', epoch: this.epoch, incarnation, generation };
  }

  delete({ slotId, expectedEpoch, expectedIncarnation, expectedGeneration }) {
    this.#check();
    const id = slotName(slotId);
    const previous = this.#current.map.get(id);
    if (!previous || expectedEpoch !== this.epoch
      || expectedIncarnation !== previous.incarnation || expectedGeneration !== previous.generation) {
      return { outcome: 'stale-reject', epoch: this.epoch };
    }
    if (!this.#allowed({ operation: 'delete', slotId: id, version: previous.version })) {
      return { outcome: 'policy-denied', epoch: this.epoch };
    }
    if (this.epoch === U64_MAX) return { outcome: 'generation-exhausted', epoch: this.epoch };
    const nextMap = new Map(this.#current.map);
    nextMap.delete(id);
    this.#current = Object.freeze({ epoch: this.epoch + 1n, map: nextMap });
    this.#retire('deletion');
    return { outcome: 'deleted', epoch: this.epoch };
  }

  pinRoot(callerHandle, callerModule, frozenToken = null) {
    this.#check();
    if (!Number.isSafeInteger(callerHandle) || callerHandle <= 0 || callerHandle > 0xffffffff) {
      throw Error('invalid checked root caller handle');
    }
    if (!this.#modules.has(callerModule)) throw Error('unregistered checked root module');
    const frozen = frozenToken === null ? null : this.#frozen.get(frozenToken);
    if (frozenToken !== null && (!frozen || frozen.callerHandle !== callerHandle
      || frozen.callerModule !== callerModule)) {
      throw Error('invalid frozen caller root');
    }
    if (this.#nextRoot === U64_MAX) throw Error('root identity exhausted');
    const token = Object.freeze({});
    const root = { id: ++this.#nextRoot, state: frozen?.state ?? this.#current, refs: new Map(),
      callerHandle, callerModule, entered: false, boundRefs: new Set(), frames: [], pending: null,
      resources: new Map(), selectedHandles: new Map(), trace: [], traceBytes: 0,
      operations: 0 };
    this.#roots.set(token, root);
    return token;
  }

  rootEpoch(token) { return this.#root(token).state.epoch; }
  #root(token) {
    this.#check();
    const root = this.#roots.get(token);
    if (!root) throw Error('inactive live-slot root');
    return root;
  }

  bindRef(token, logicalInputPosition, ordinal, slotId, selectedContract) {
    const root = this.#root(token);
    if (!Number.isSafeInteger(logicalInputPosition) || logicalInputPosition < 0
      || logicalInputPosition >= 128 || !Number.isSafeInteger(ordinal) || ordinal < 0 || ordinal >= 128
      || root.entered || root.refs.has(ordinal)
      || [...root.refs.values()].some(ref => ref.logicalInputPosition === logicalInputPosition)) {
      throw Error('invalid or duplicate root ref binding');
    }
    const id = slotName(slotId);
    const entry = root.state.map.get(id);
    const typed = contract(selectedContract);
    if (!entry || !same(entry.contract.input, typed.input)
      || !same(entry.contract.output, typed.output)
      || !same(entry.contract.ceiling, typed.ceiling)) throw Error('root ref interface mismatch');
    root.refs.set(ordinal, Object.freeze({ slotId: id, contract: typed, logicalInputPosition }));
  }

  bindSelectedRef(token, logicalInputPosition, ordinal, slotId) {
    const root = this.#root(token);
    const entry = root.state.map.get(slotName(slotId));
    if (!entry) throw Error('root reference has no pinned published slot');
    this.bindRef(token, logicalInputPosition, ordinal, slotId,
      { input: entry.contract.input, output: entry.contract.output,
        effectCeiling: entry.contract.ceiling, proofRequired: entry.contract.proofRequired });
  }

  // Called from the guarded `bind_live_ref` Wasm export's import, not from
  // source. Each checked logical position/issued ordinal binds at most once.
  validateBorrowBinding(token, module, logicalInputPosition, ordinal) {
    const root = this.#root(token);
    const ref = root.refs.get(ordinal);
    if (module !== root.callerModule || root.entered || !ref
      || ref.logicalInputPosition !== logicalInputPosition || root.boundRefs.has(ordinal)) {
      return 0;
    }
    root.boundRefs.add(ordinal);
    return 1;
  }

  // Root-only host-issued opaque resources are matched to a separately
  // registered nominal descriptor and exact caller input position.
  issueResource(token, logicalInputPosition, issuedOrdinal, module, nominalOrdinal, kind, owner) {
    const root = this.#root(token);
    const caller = this.#modules.get(root.callerModule);
    const registration = this.#nominals.get(`${module}:${nominalOrdinal}`);
    const catalog = this.#catalogs.get(caller?.catalog);
    if (root.entered || !Number.isInteger(logicalInputPosition)
      || logicalInputPosition < 0 || logicalInputPosition >= 128
      || !Number.isInteger(issuedOrdinal) || issuedOrdinal < 0 || issuedOrdinal >= 128
      || root.resources.has(issuedOrdinal) || !registration
      || registration.kind !== kind || registration.owner !== owner
      || caller.rootInput[logicalInputPosition] !== registration.interfaceDescriptor
      || !catalog?.rows.some(row => row.module === registration.module
        && row.ordinal === registration.ordinal && row.kind === registration.kind
        && row.shapeFingerprint === registration.shapeFingerprint)
      || !this.#allowed({ operation: 'issue-resource', nominalId: `${module}:${nominalOrdinal}`,
        owner, rootEpoch: root.state.epoch })) {
      throw Error('unregistered, unowned, or incompatible root resource');
    }
    root.resources.set(issuedOrdinal, { logicalInputPosition, registration, bound: false, live: true });
  }

  // Called by the guarded push_nominal_resource export before cell allocation.
  validateResourceBinding(token, moduleToken, logicalInputPosition, issuedOrdinal,
    kind, moduleLo, moduleHi, nominalOrdinal) {
    const root = this.#root(token);
    if (![logicalInputPosition, issuedOrdinal, kind, moduleLo, moduleHi, nominalOrdinal]
      .every(value => Number.isInteger(value) && value >= -0x80000000 && value <= 0xffffffff)) {
      return 0;
    }
    const resource = root.resources.get(issuedOrdinal);
    const module = (BigInt(moduleHi >>> 0) << 32n | BigInt(moduleLo >>> 0)).toString();
    if (moduleToken !== root.callerModule || root.entered || !resource?.live || resource.bound
      || resource.logicalInputPosition !== logicalInputPosition
      || resource.registration.module !== module
      || resource.registration.ordinal !== nominalOrdinal
      || resource.registration.kind !== (kind >>> 0)) return 0;
    resource.bound = true;
    return 1;
  }

  validateResource(token, issuedOrdinal, kind) {
    const root = this.#roots.get(token);
    const resource = root?.resources.get(issuedOrdinal >>> 0);
    return !this.#poisoned && resource?.bound && resource.live
      && resource.registration.kind === (kind >>> 0) ? 1 : 0;
  }

  // Register the compiler's independently checked call-site table against
  // one installed module, not a process-global site ID. Its import closure
  // retains the returned token even if a later module reuses numeric site IDs.
  registerModule(artifactSha256, checkedSites, rootInput = [], catalog = null,
    sourceArtifactSha256 = null) {
    this.#check();
    if (typeof artifactSha256 !== 'string' || !/^[0-9a-f]{64}$/.test(artifactSha256)
      || !Array.isArray(checkedSites) || checkedSites.length > 2048
      || catalog !== null && this.#catalogs.get(catalog)?.sourceArtifactSha256 !== sourceArtifactSha256) {
      throw Error('invalid checked module site table');
    }
    const input = strings(rootInput, 'root logical input');
    const token = Object.freeze({});
    const sites = new Map();
    for (const row of checkedSites) {
      if (!Number.isSafeInteger(row?.site_id) || row.site_id < 0 || row.site_id >= 2048
        || !Number.isSafeInteger(row.selected_ref_logical_position)
        || row.selected_ref_logical_position < 0 || row.selected_ref_logical_position >= 128
        || sites.has(row.site_id)) throw Error('invalid or duplicate checked dispatch site');
      const typed = contract({ input: row.target_input, output: row.target_output,
        effectCeiling: row.effect_ceiling });
      if (!Array.isArray(row.forwarded_source_positions)
        || !Array.isArray(row.forwarded_target_positions)
        || row.forwarded_source_positions.length !== row.forwarded_target_positions.length
        || row.forwarded_source_positions.length > 128) {
        throw Error('invalid checked forwarded borrows');
      }
      const forwarded = row.forwarded_source_positions.map((sourcePosition, index) => {
        const targetPosition = row.forwarded_target_positions[index];
        if (!Number.isSafeInteger(sourcePosition) || sourcePosition < 0 || sourcePosition >= 128
          || !Number.isSafeInteger(targetPosition) || targetPosition < 0 || targetPosition >= 128) {
          throw Error('invalid checked forwarded borrow');
        }
        return Object.freeze({ sourcePosition, targetPosition });
      });
      if (new Set(forwarded.map(pair => pair.targetPosition)).size !== forwarded.length) {
        throw Error('duplicate checked forwarded target position');
      }
      sites.set(row.site_id, Object.freeze({ siteId: row.site_id,
        borrowedInputPosition: row.selected_ref_logical_position,
        input: typed.input, output: typed.output, effectCeiling: typed.ceiling,
        forwarded: Object.freeze(forwarded) }));
    }
    this.#modules.set(token, Object.freeze({ artifactSha256, sites, rootInput: input, catalog }));
    return token;
  }

  reflectSite(module, siteId) {
    const site = this.#modules.get(module)?.sites.get(siteId);
    if (!site) throw Error('unknown checked dispatch site');
    return { instruction: 'slot.invoke', siteId, input: [...site.input],
      output: [...site.output], effects: [...site.effectCeiling, 'live.dispatch'] };
  }

  #trace(root, record) {
    const bytes = Buffer.byteLength(JSON.stringify(record)) + 1;
    if (root.trace.length >= TRACE_LIMIT || root.traceBytes + bytes > ROOT_TRACE_BYTES
      || this.#traceAdmission?.(root.traceBytes + bytes, root.trace.length + 1) === false) {
      throw new TraceCapacityError('live-slot trace capacity refused before recording request');
    }
    root.traceBytes += bytes;
    root.trace.push(Object.freeze(record));
  }

  traceSize(token) {
    const root = this.#root(token);
    return { bytes: root.traceBytes, rows: root.trace.length };
  }

  #policyVersion(version) {
    if (!version) return null;
    return { programValueId: version.programValueId,
      definitionId: version.definitionId, captures: version.captures.slice(),
      semanticContext: version.semanticContext, recipeSha256: version.recipeSha256,
      artifactSha256: version.artifactSha256,
      input: version.input.slice(), output: version.output.slice(),
      effects: version.effects.slice(), claim: version.claim,
      assumptions: version.assumptions, handle: version.handle };
  }

  // The Wasm import supplies only the two integer indices. The import closure
  // captures its exact installed module token and independently checked sites.
  dispatch(token, ordinal, module, siteId) {
    const root = this.#root(token);
    const ref = root.refs.get(ordinal);
    const site = this.#modules.get(module)?.sites.get(siteId);
    const frame = root.frames.at(-1);
    if (!ref || !site || !Number.isSafeInteger(site.siteId)
      || !same(site.input ?? [], ref.contract.input)
      || !same(site.output ?? [], ref.contract.output)
      || !same(site.effectCeiling ?? [], ref.contract.ceiling)
      || !frame || frame.bindings.get(site.borrowedInputPosition) !== ordinal
      || root.pending) throw Error('invalid checked dispatch site or active borrowed frame');
    const selected = root.state.map.get(ref.slotId);
    const authorized = selected ? this.#allowed({ operation: 'dispatch', slotId: ref.slotId,
      version: selected.version, rootEpoch: root.state.epoch }) : false;
    const reason = !selected ? 'slot-absent'
      : !authorized ? 'policy-denied'
        : this.#candidate(selected.version, selected.contract, selected.evidence);
    this.#trace(root, { operation: 'dispatch', rootEpoch: root.state.epoch.toString(),
      slotId: ref.slotId, incarnation: selected?.incarnation?.toString() ?? null,
      generation: selected?.generation?.toString() ?? null,
      programValueId: selected?.version?.programValueId ?? null,
      definitionId: selected?.version?.definitionId ?? null,
      artifactSha256: selected?.version?.artifactSha256 ?? null,
      captures: selected?.version?.captures.slice() ?? null,
      semanticContext: selected?.version?.semanticContext ?? null,
      recipeSha256: selected?.version?.recipeSha256 ?? null,
      policyInput: { operation: 'dispatch', slotId: ref.slotId,
        version: this.#policyVersion(selected?.version),
        rootEpoch: root.state.epoch.toString() }, policyAllowed: authorized,
      outcome: reason ?? 'allowed' });
    if (reason) return { outcome: reason, handle: 0 };
    root.selectedHandles.set(selected.version.handle, selected);
    root.pending = { module, site, selected };
    return { outcome: 'allowed', handle: selected.version.handle };
  }

  // The runtime writes a checked frame record into its slot-only 20-page
  // sidecar. Host validates every pair against its own parent frame ledger,
  // then returns a unique token stored by the runtime in noble.live_frame.
  enterFrame(token, module, siteId, framePtr, targetHandle, memory) {
    const root = this.#root(token);
    if (!Number.isInteger(framePtr) || framePtr < FRAME_START || framePtr % 4
      || !(memory instanceof WebAssembly.Memory) || memory.buffer.byteLength < FRAME_END
      || framePtr + 16 > FRAME_END) throw Error('invalid live-slot frame boundary');
    const bytes = new DataView(memory.buffer);
    const parent = bytes.getUint32(framePtr, true);
    const recordedHandle = bytes.getUint32(framePtr + 4, true);
    const count = bytes.getUint32(framePtr + 8, true);
    const recordedToken = bytes.getUint32(framePtr + 12, true);
    const handle = targetHandle >>> 0;
    if (count > 128 || framePtr + 16 + count * 8 > FRAME_END
      || recordedHandle !== handle || recordedToken !== 0) {
      throw Error('invalid live-slot frame record');
    }
    const current = root.frames.at(-1);
    let expected;
    if (siteId === -1 && !current && !root.pending && !root.entered) {
      if (module !== root.callerModule || handle !== root.callerHandle
        || root.boundRefs.size !== root.refs.size
        || [...root.resources.values()].some(resource => !resource.bound)
        || bytes.getUint32(ROOT_BINDING_COUNT, true) !== root.refs.size) {
        throw Error('root input borrowing differs from checked root');
      }
      expected = new Map([...root.refs]
        .map(([ordinal, ref]) => [ref.logicalInputPosition, ordinal]));
    } else {
      const pending = root.pending;
      if (!current || !pending || pending.module !== module
        || pending.site.siteId !== siteId || (pending.selected.version.handle >>> 0) !== handle
        || parent !== current.ptr) throw Error('unselected or stale live-slot child frame');
      expected = new Map(pending.site.forwarded.map(pair =>
        [pair.targetPosition, current.bindings.get(pair.sourcePosition)]));
      if ([...expected.values()].some(ordinal => ordinal === undefined)) {
        throw Error('missing forwarded borrowed input');
      }
    }
    if (expected.size !== count || (!current && parent !== 0)) {
      throw Error('live-slot frame arity or parent mismatch');
    }
    const ordered = [...expected].sort(([left], [right]) => left - right);
    for (let index = 0; index < count; index++) {
      const position = bytes.getUint32(framePtr + 16 + index * 8, true);
      const ordinal = bytes.getUint32(framePtr + 20 + index * 8, true);
      if (ordered[index]?.[0] !== position || ordered[index]?.[1] !== ordinal
        || !root.refs.has(ordinal)) {
        throw Error('forged or misordered borrowed frame input');
      }
      expected.delete(position);
    }
    if (expected.size || this.#nextFrame === 0xffffffff) {
      throw Error('borrowed frame mismatch or token exhaustion');
    }
    const frameToken = ++this.#nextFrame;
    root.frames.push({ token: frameToken, ptr: framePtr, targetHandle: handle,
      bindings: current && siteId !== -1
        ? new Map(root.pending.site.forwarded.map(pair =>
          [pair.targetPosition, current.bindings.get(pair.sourcePosition)]))
        : new Map([...root.refs].map(([ordinal, ref]) => [ref.logicalInputPosition, ordinal])) });
    root.entered = true;
    root.pending = null;
    return frameToken;
  }

  leaveFrame(token, frameToken) {
    const root = this.#root(token);
    if (root.pending || root.frames.at(-1)?.token !== frameToken) {
      throw Error('non-LIFO live-slot frame retirement');
    }
    root.frames.pop();
    return 0;
  }

  requestEffect(token, frameToken, handle, effect, request) {
    const root = this.#root(token);
    const current = root.frames.at(-1);
    const selected = root.selectedHandles.get(handle >>> 0);
    if (!current || current.token !== frameToken || current.targetHandle !== (handle >>> 0)
      || !selected || !selected.version.effects.includes(effect)
      || typeof request !== 'string' || Buffer.byteLength(request) > 65536) {
      throw Error('effect is not selected by this root and checked target');
    }
    const allowed = this.#allowed({ operation: 'effect', slotId: selected.id,
      version: selected.version, effect, request, rootEpoch: root.state.epoch });
    const record = { operation: 'effect', effect, request, slotId: selected.id,
      programValueId: selected.version.programValueId, rootEpoch: root.state.epoch.toString(),
      definitionId: selected.version.definitionId,
      captures: selected.version.captures.slice(),
      semanticContext: selected.version.semanticContext,
      recipeSha256: selected.version.recipeSha256,
      artifactSha256: selected.version.artifactSha256,
      policyInput: { operation: 'effect', slotId: selected.id,
        version: this.#policyVersion(selected.version),
        effect, request, rootEpoch: root.state.epoch.toString() },
      policyAllowed: allowed };
    if (!allowed) {
      this.#trace(root, { ...record, outcome: 'denied', protectedOperations: root.operations });
      return { outcome: 'denied', protectedOperations: root.operations };
    }
    // The adapter independently previews its deterministic bounded response.
    // Reserve the full JSON row before an external operation, not after it.
    const responseBound = this.#previewEffect?.(effect, request, selected.version);
    if (typeof responseBound !== 'string' || Buffer.byteLength(responseBound) > 65544) {
      throw Error('protected effect lacks a bounded trace response preview');
    }
    const nextRecord = { ...record, outcome: 'performed', response: responseBound,
      protectedOperations: root.operations + 1 };
    const failureRecord = { ...record, outcome: 'unknown-after-attempt',
      protectedOperations: root.operations + 1 };
    const nextBytes = Math.max(Buffer.byteLength(JSON.stringify(nextRecord)),
      Buffer.byteLength(JSON.stringify(failureRecord))) + 1;
    if (root.trace.length >= TRACE_LIMIT || root.traceBytes + nextBytes > ROOT_TRACE_BYTES
      || this.#traceAdmission?.(root.traceBytes + nextBytes, root.trace.length + 1) === false) {
      throw new TraceCapacityError('live-slot trace capacity refused before protected effect');
    }
    root.operations += 1;
    let response;
    try {
      response = this.#performEffect(effect, request, selected.version);
      if (response !== responseBound) {
        throw Error('protected effect response exceeds pre-authorized trace reservation');
      }
    } catch (error) {
      this.#trace(root, { ...record, outcome: 'unknown-after-attempt',
        protectedOperations: root.operations });
      throw error;
    }
    this.#trace(root, { ...record, outcome: 'performed', response,
      protectedOperations: root.operations });
    return { outcome: 'performed', response, protectedOperations: root.operations };
  }

  // Frozen replay executes the same checked Program and frame/target identity,
  // but consumes its recorded host response rather than authorizing or
  // performing a second external effect.
  replayEffect(token, frameToken, handle, effect, request, expected) {
    const root = this.#root(token);
    const current = root.frames.at(-1);
    const selected = root.selectedHandles.get(handle >>> 0);
    if (!current || current.token !== frameToken || current.targetHandle !== (handle >>> 0)
      || !selected || !selected.version.effects.includes(effect)
      || typeof request !== 'string' || Buffer.byteLength(request) > 65536) {
      throw Error('replay effect is not selected by this frozen root and checked target');
    }
    const record = { operation: 'effect', effect, request, slotId: selected.id,
      programValueId: selected.version.programValueId, rootEpoch: root.state.epoch.toString(),
      definitionId: selected.version.definitionId,
      captures: selected.version.captures.slice(),
      semanticContext: selected.version.semanticContext,
      recipeSha256: selected.version.recipeSha256,
      artifactSha256: selected.version.artifactSha256,
      policyInput: { operation: 'effect', slotId: selected.id,
        version: this.#policyVersion(selected.version),
        effect, request, rootEpoch: root.state.epoch.toString() },
      policyAllowed: true };
    if (!expected || expected.outcome !== 'performed'
      || typeof expected.response !== 'string' || Buffer.byteLength(expected.response) > 65536
      || expected.protectedOperations !== root.operations + 1
      || JSON.stringify(record) !== JSON.stringify(Object.fromEntries(
        Object.keys(record).map(key => [key, expected[key]])))) {
      throw Error('scripted replay request, response, order or accounting diverged');
    }
    root.operations++;
    this.#trace(root, { ...record, outcome: 'performed', response: expected.response,
      protectedOperations: root.operations });
    return { outcome: 'performed', response: expected.response,
      protectedOperations: root.operations };
  }

  trace(token) { return this.#root(token).trace.map(record => ({ ...record })); }

  frozenIdentity(token) {
    const root = this.#root(token);
    return [...root.state.map.values()].map(row => ({
      slotId: row.id, incarnation: row.incarnation.toString(),
      generation: row.generation.toString(),
      programValueId: row.version.programValueId,
      definitionId: row.version.definitionId,
      captures: row.version.captures.slice(),
      semanticContext: row.version.semanticContext,
      recipeSha256: row.version.recipeSha256,
      artifactSha256: row.version.artifactSha256,
      input: row.version.input.slice(), output: row.version.output.slice(),
      effects: row.version.effects.slice(),
    })).sort((left, right) => left.slotId.localeCompare(right.slotId));
  }

  freezeRoot(token) {
    const root = this.#root(token);
    if (root.frames.length || root.pending) throw Error('cannot freeze an active borrowed frame');
    const snapshot = Object.freeze({});
    this.#frozen.set(snapshot, Object.freeze({
      state: root.state, callerHandle: root.callerHandle, callerModule: root.callerModule,
    }));
    return snapshot;
  }

  releaseFrozen(snapshot) {
    this.#check();
    if (!this.#frozen.delete(snapshot)) throw Error('invalid frozen root owner');
    this.#retire('frozen-replay-release');
  }

  finishRoot(token) {
    const root = this.#root(token);
    if (root.frames.length || root.pending) throw Error('live-slot root still has an active frame');
    this.#roots.delete(token);
    this.#retire('root-release');
  }

  // Only the trusted worker calls this after a trapped submit has unwound.
  // A failed guest is not permitted to keep refs or a pinned old map alive.
  abortRoot(token) {
    const root = this.#root(token);
    root.pending = null;
    root.frames.length = 0;
    this.#roots.delete(token);
    this.#retire('aborted-root-release');
  }

  saveProgram(version) {
    return this.saveProgramGraph(version.handle, [version]);
  }

  // One actual saved backend Program root can retain several admitted target
  // Programs transitively through checked immutable capture/aggregate edges.
  // Count each reachable logical version, but pin the real Wasm root ONCE.
  saveProgramGraph(rootHandle, versions) {
    this.#check();
    if (!Number.isInteger(rootHandle) || rootHandle <= 0 || rootHandle > 0xffffffff
      || !Array.isArray(versions) || versions.length > 128
      || new Set(versions).size !== versions.length
      || versions.some(version => this.#versions.get(this.#versionKey(version)) !== version)
      || typeof this.#retainProgram !== 'function'
      || typeof this.#releaseProgramBackend !== 'function') {
      throw Error('unadmitted saved Program');
    }
    const reachable = Object.freeze(versions.slice());
    try {
      if (this.#retainProgram(rootHandle, reachable) !== true) {
        throw Error('backend did not retain a real Program cell');
      }
    } catch (error) {
      this.#poisoned = true;
      throw Error('saved Program backend retention failed; session poisoned', { cause: error });
    }
    for (const version of reachable) {
      this.#saved.set(version, (this.#saved.get(version) ?? 0) + 1);
    }
    const owner = Object.freeze({ rootHandle, versions: reachable });
    this.#savedTokens.add(owner);
    return owner;
  }

  releaseProgram(owner) {
    this.#check();
    if (!this.#savedTokens.has(owner)
      || owner.versions.some(version => !this.#saved.get(version))) {
      throw Error('invalid saved Program owner');
    }
    try {
      if (this.#releaseProgramBackend(owner.rootHandle, owner.versions) !== true) {
        throw Error('backend did not release a real Program cell');
      }
    } catch (error) {
      this.#poisoned = true;
      throw Error('saved Program backend release failed; session poisoned', { cause: error });
    }
    this.#savedTokens.delete(owner);
    for (const version of owner.versions) {
      const count = this.#saved.get(version);
      if (count === 1) this.#saved.delete(version);
      else this.#saved.set(version, count - 1);
    }
    this.#retire('saved-program-release');
  }
}
