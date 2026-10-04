// Bounded inert observations of the engine-owned memory; no guest execution.
function readText(engine, offset, count) {
    integer(offset, engine.memory.buffer.byteLength, 'text offset');
    integer(count, engine.memory.buffer.byteLength - offset, 'text length');
    return utf8.decode(new Uint8Array(engine.memory.buffer, offset, count));
  }

function readSignature(engine, runtime, identity) {
    integer(identity, 16384, 'signature identity');
    return readText(engine, runtime.signature_address(identity), runtime.signature_length(identity));
  }

function readStack(engine, runtime) {
    const count = integer(runtime.stack_length(), 128, 'stack height');
    engine.observationWork = 0;
    // Every slot also reports its live cell handle (null for immediate
    // values), so a consumer can project a Program or pass a companion cell to
    // a later push without re-deriving the handle from the value's shape.
    return Array.from({ length: count }, (_, at) => {
      const kind = runtime.stack_kind(at);
      const raw = runtime.stack_value(at);
      return {
        handle: kind < 4 ? null : Number(BigInt.asUintN(64, raw)),
        ...readValue(engine, runtime, kind, raw, 0),
      };
    });
  }

function chargeObservation(engine, depth) {
    if (depth > 512 || ++engine.observationWork > MAX_OBSERVATION_WORK) fail('observation limit exhausted');
  }

function readBoxed(engine, runtime, handle, depth) {
    integer(handle, engine.abi.limits_maximum.session_cells, 'cell handle');
    if (handle === 0 || handle > engine.shared.heap_cursor.value) fail('invalid live cell handle');
    const kind = runtime.cell_kind(handle);
    // A boxed value also reports its own live handle, so a consumer can pass the
    // same cell to a later push without re-deriving the handle from its shape.
    return { handle, ...readValue(engine, runtime, kind, kind < 4 ? runtime.cell_payload(handle) : BigInt(handle), depth) };
  }

function readValue(engine, runtime, kind, value, depth) {
    chargeObservation(engine, depth);
    if (kind === 1) return { type: 'I64', value: String(value) };
    if (kind === 2) {
      if (value !== 0n && value !== 1n) fail('invalid Bool value');
      return { type: 'Bool', value: value === 1n };
    }
    if (kind === 3) return { type: 'Unit', value: null };
    const handle = integer(Number(value), engine.shared.heap_cursor.value, 'value handle');
    if (!handle || runtime.cell_kind(handle) !== kind) fail('value tag disagrees with cell');
    if (kind === 4) return { type: 'Program', interface: {
      stack_in: readSignature(engine, runtime, runtime.cell_y(handle)),
      stack_out: readSignature(engine, runtime, runtime.cell_z(handle)),
      effects: [0, 1, 2, 3, 4].filter(bit => Number(runtime.cell_payload(handle)) & (1 << bit))
        .map(bit => ['test.emit', 'test.abort', 'test.clock', 'live.propose', 'live.observe-generation'][bit]),
    }, ...readRecipe(engine, runtime, runtime.cell_c(handle), depth + 1) };
    if (kind === 5) return { type: 'Pair', value: [readBoxed(engine, runtime, runtime.cell_a(handle), depth + 1), readBoxed(engine, runtime, runtime.cell_b(handle), depth + 1)] };
    if (kind === 6 || kind === 7) {
      const items = [];
      let tail = handle;
      while (runtime.cell_kind(tail) === 6) {
        chargeObservation(engine, depth);
        items.push(readBoxed(engine, runtime, runtime.cell_a(tail), depth + 1));
        const next = runtime.cell_b(tail);
        if (next <= 0 || next >= tail) fail('cyclic/non-backward list');
        tail = next;
      }
      if (runtime.cell_kind(tail) !== 7) fail('invalid list tail');
      return { type: 'List', value: items };
    }
    if (kind === 10) return { type: 'Syntax', ...readRecipe(engine, runtime, runtime.cell_a(handle), depth + 1) };
    if (kind === 11) return { type: 'Text', value: readText(engine, runtime.cell_x(handle), runtime.cell_y(handle)) };
    if (kind === 12 || kind === 13) {
      const marker = runtime.cell_w(handle);
      if (marker === 0) return { type: 'Sum', variant: kind === 12 ? 'left' : 'right',
        value: readBoxed(engine, runtime, runtime.cell_a(handle), depth + 1) };
      if (engine.profile !== 'Declared-Modules-v1') fail('nominal value is outside Core-Bootstrap');
      if (marker !== 1 && marker !== 2) fail('invalid nominal representation marker');
      if (marker === 1 && kind !== 12) fail('opaque wrapper uses an invalid sum tag');
      const module = (BigInt(runtime.cell_y(handle) >>> 0) << 32n) | BigInt(runtime.cell_x(handle) >>> 0);
      return { type: 'Nominal', module: String(module), ordinal: runtime.cell_z(handle),
        shape: marker === 1 ? 'opaque' : 'variant', variant: marker === 1 ? null : kind === 12 ? 'left' : 'right',
        value: readBoxed(engine, runtime, runtime.cell_a(handle), depth + 1) };
    }
    if (kind === 14) return { type: 'Contract', statement: String(BigInt.asUintN(64, runtime.cell_payload(handle))),
      index: runtime.cell_x(handle), claim_kind: runtime.cell_y(handle), revision: runtime.cell_z(handle) };
    if (kind === 15) return { type: 'Evidence', index: Number(runtime.cell_payload(handle)),
      class: runtime.cell_x(handle), ruleset: runtime.cell_y(handle),
      contract: readBoxed(engine, runtime, runtime.cell_a(handle), depth + 1) };
    if (kind === 16) return { type: 'Certified',
      identity: String(BigInt.asUintN(64, runtime.cell_payload(handle))),
      subject: readBoxed(engine, runtime, runtime.cell_a(handle), depth + 1),
      contract: readBoxed(engine, runtime, runtime.cell_b(handle), depth + 1),
      evidence: readBoxed(engine, runtime, runtime.cell_c(handle), depth + 1) };
    fail(`unsupported observable value kind: ${kind}`);
  }

  // Read the bounded inspection events one `observe` call produced. These are
  // inert descriptions: reading them never fetches a proof file or starts a
  // prover.
function readReflection(engine, runtime) {
    const count = integer(runtime.recipe_length(), 2048, 'reflection event count');
    const events = [];
    for (let index = 0; index < count; index += 1) {
      events.push({ kind: runtime.recipe_kind(index), value: String(BigInt.asUintN(64, runtime.recipe_value(index))) });
    }
    return events;
  }

function readRecipe(engine, runtime, root, depth) {
    chargeObservation(engine, depth);
    const result = [];
    const witnesses = [];
    const pending = root ? [root] : [];
    while (pending.length) {
      chargeObservation(engine, depth);
      const handle = pending.pop();
      integer(handle, engine.shared.heap_cursor.value, 'recipe handle');
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
        const id = integer(Number(value), 25, 'builtin identity');
        result.push({ invoke: `${id < 22 ? 'builtin' : id < 24 ? 'host' : 'live'}:${names[id]}` });
      } else if (atom === 15) result.push({ invoke: `definition:${BigInt.asUintN(64, value)}` });
      else if (atom === 3) {
        const program = readBoxed(engine, runtime, child, depth + 1);
        if (program.type !== 'Program') fail('quotation recipe has no program');
        result.push({ quotation: program.recipe, interface: program.interface, witnesses: program.witnesses });
      }
      else if (atom === 8) result.push({ literal: {
        ...readBoxed(engine, runtime, child, depth + 1),
        schema: readSignature(engine, runtime, runtime.cell_y(handle)),
      } });
      else if (atom >= 26 && atom <= 32 && engine.profile !== 'Declared-Modules-v1') {
        fail('nominal or bound operation recipe is outside Core-Bootstrap');
      }
      else if (atom >= 26 && atom <= 30) result.push({ invoke: {
        nominal_module: String(BigInt.asUintN(64, value)),
        nominal_ordinal: runtime.cell_n(handle),
        operation: ['new', 'into', 'left', 'right', 'match'][atom - 26],
      } });
      else if (atom === 31) result.push({ invoke: {
        operation: 'test.emit', adapter_slot: integer(Number(value), 63, 'adapter slot'),
      } });
      else if (atom === 32) result.push({ invoke: {
        operation: 'test.clock', adapter_slot: integer(Number(value), 63, 'adapter slot'),
      } });
      else fail(`unsupported recipe atom: ${atom}`);
      if (atom === 2 || atom === 15 || atom >= 26 && atom <= 32) {
        witnesses.push({
          node: result.length - 1,
          stack_in: readSignature(engine, runtime, runtime.cell_y(handle)),
          stack_out: readSignature(engine, runtime, runtime.cell_z(handle)),
          effects: [0, 1, 2, 3, 4].filter(bit => runtime.cell_w(handle) & (1 << bit))
            .map(bit => ['test.emit', 'test.abort', 'test.clock', 'live.propose', 'live.observe-generation'][bit]),
        });
      }
    }
    return { recipe: result, witnesses };
  }

  // A host effect cannot call back into Wasm: capture only already-allocated,
  // immutable cells from the linear-memory ABI and bounded captured Text bytes.
  // The bytes are checked again before post-return reflection, so a later
  // guest write cannot replace the proposal that the callback accepted.
function captureLiveProgram(engine, root) {
    const maximum = integer(engine.shared.heap_cursor.value,
      engine.abi.limits_maximum.session_cells, 'live cell count');
    integer(root, maximum, 'proposal Program handle');
    if (root === 0) fail('missing proposal Program handle');
    const memory = new Uint8Array(engine.memory.buffer);
    const view = new DataView(engine.memory.buffer);
    const rootOffset = 65536 + (root - 1) * 48;
    if (rootOffset + 48 > memory.length) fail('proposal Program exceeds arena');
    const inputId = view.getUint32(rootOffset + 32, true);
    const outputId = view.getUint32(rootOffset + 36, true);
    const pending = [root], seen = new Set(), cells = [], text = [];
    let captured = 0;
    while (pending.length) {
      const handle = pending.pop();
      if (seen.has(handle)) continue;
      seen.add(handle);
      const offset = 65536 + (handle - 1) * 48;
      if (offset + 48 > memory.length) fail('proposal cell exceeds arena');
      const kind = view.getInt32(offset, true);
      if (kind < 1 || kind > 16 || handle === root && kind !== 4) {
        fail('proposal is not an immutable Program graph');
      }
      if (handle === root && view.getBigInt64(offset + 8, true) !== 0n) {
        fail('proposal Program has latent effects');
      }
      const bytes = Uint8Array.from(memory.subarray(offset, offset + 48));
      captured += bytes.length;
      const refs = kind === 4 || kind === 16 ? [16, 20, 24]
        : kind === 5 || kind === 6 || kind === 9 ? [16, 20]
          : [8, 10, 12, 13, 15].includes(kind) ? [16] : [];
      for (const field of refs) {
        const child = view.getInt32(offset + field, true);
        if (child !== 0) {
          if (child < 0 || child >= handle) fail('proposal cell has an invalid edge');
          pending.push(child);
        }
      }
      if (kind === 11) {
        const address = view.getInt32(offset + 28, true);
        const length = view.getInt32(offset + 32, true);
        if (address < 0 || length < 0 || address > memory.length - length
          || captured + length > 262144) fail('proposal captured Text exceeds limit');
        const value = Uint8Array.from(memory.subarray(address, address + length));
        captured += length;
        text.push({ address, value });
      }
      if (captured > 262144) fail('proposal snapshot exceeds limit');
      cells.push({ address: offset, value: bytes });
    }
    // The digest names the callback-time bytes, not a later rendering of the
    // proposal. Canonical addresses, lengths and typed records make identical
    // immutable snapshots hash identically regardless of graph walk order.
    cells.sort((left, right) => left.address - right.address);
    text.sort((left, right) => left.address - right.address
      || left.value.length - right.value.length);
    const digest = crypto.createHash('sha256');
    digest.update('noble-live-proposal-snapshot/v1\0');
    const metadata = Buffer.allocUnsafe(12);
    metadata.writeUInt32LE(root, 0);
    metadata.writeUInt32LE(inputId, 4);
    metadata.writeUInt32LE(outputId, 8);
    digest.update(metadata);
    const record = Buffer.allocUnsafe(9);
    for (const [kind, entries] of [[1, cells], [2, text]]) {
      record.writeUInt8(kind, 0);
      for (const { address, value } of entries) {
        record.writeUInt32LE(address, 1);
        record.writeUInt32LE(value.length, 5);
        digest.update(record);
        digest.update(value);
      }
    }
    return { root, inputId, outputId, cells, text, sha256: digest.digest('hex') };
  }

function checkLiveProgramSnapshot(engine, snapshot) {
    const memory = new Uint8Array(engine.memory.buffer);
    for (const cells of [snapshot.cells, snapshot.text]) {
      for (const { address, value } of cells) {
        if (address > memory.length - value.length
          || !memory.subarray(address, address + value.length)
            .every((byte, index) => byte === value[index])) {
          fail('proposal changed after callback');
        }
      }
    }
  }

